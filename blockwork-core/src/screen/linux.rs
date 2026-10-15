//! Linux screen access. X11 is read directly. Wayland has no common capture
//! API, so `grim` is tried first (wlroots compositors), then the desktop
//! portal's Screenshot (KDE, GNOME; may ask for permission once).

use super::{Image, Rect, Shot};
use crate::flatpak::host_command;
use crate::macros::backend::{cursor_track, wayland_display, x11_cursor};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn wayland() -> bool {
    wayland_display::is_wayland_session()
}

pub(super) fn capture() -> Result<Shot, String> {
    if !wayland() {
        return x11::capture();
    }
    let image = wayland_capture()?;
    // Wayland captures are in physical pixels; screen coordinates are logical.
    let (origin, scale) = match wayland_display::logical_desktop_bounds() {
        Some(b) if b.width > 0 => ((b.x as f64, b.y as f64), image.width as f64 / b.width as f64),
        _ => ((0.0, 0.0), 1.0),
    };
    Ok(Shot { image, origin, scale })
}

/// Set once `grim` has failed, so later captures go straight to the portal.
static NO_GRIM: AtomicBool = AtomicBool::new(false);

fn wayland_capture() -> Result<Image, String> {
    if !NO_GRIM.load(Ordering::Relaxed) {
        match grim() {
            Ok(image) => return Ok(image),
            Err(_) => NO_GRIM.store(true, Ordering::Relaxed),
        }
    }
    portal::screenshot()
}

fn grim() -> Result<Image, String> {
    let output = host_command("grim").args(["-t", "ppm", "-"]).output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    parse_ppm(&output.stdout)
}

/// Binary PPM (P6, 8-bit), as `grim -t ppm` writes it.
fn parse_ppm(data: &[u8]) -> Result<Image, String> {
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 && i < data.len() {
        if data[i] == b'#' {
            while i < data.len() && data[i] != b'\n' {
                i += 1;
            }
        } else if data[i].is_ascii_whitespace() {
            i += 1;
        } else {
            let start = i;
            while i < data.len() && !data[i].is_ascii_whitespace() {
                i += 1;
            }
            fields.push(String::from_utf8_lossy(&data[start..i]).into_owned());
        }
    }
    let bad = || "unexpected screenshot format".to_string();
    if fields.len() < 4 || fields[0] != "P6" || fields[3] != "255" {
        return Err(bad());
    }
    let width: u32 = fields[1].parse().map_err(|_| bad())?;
    let height: u32 = fields[2].parse().map_err(|_| bad())?;
    let pixels = data.get(i + 1..).ok_or_else(bad)?;
    if pixels.len() < width as usize * height as usize * 3 {
        return Err(bad());
    }
    let rgba = pixels.chunks_exact(3).take(width as usize * height as usize).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    Ok(Image { width, height, rgba })
}

mod portal {
    use super::*;
    use std::collections::HashMap;
    use zbus::zvariant::{OwnedValue, Value};

    /// One portal request at a time; each can show a permission prompt.
    static BUSY: Mutex<()> = Mutex::new(());

    pub(super) fn screenshot() -> Result<Image, String> {
        let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        let unavailable = |e: zbus::Error| format!("couldn't take a screenshot through the desktop portal: {e}");
        let connection = zbus::blocking::Connection::session().map_err(unavailable)?;
        let token = format!("blockwork{}", uuid::Uuid::new_v4().simple());
        let sender = connection.unique_name().map(|n| n.trim_start_matches(':').replace('.', "_")).unwrap_or_default();
        let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        // Subscribe before asking, so a fast answer isn't missed.
        let request =
            zbus::blocking::Proxy::new(&connection, "org.freedesktop.portal.Desktop", path.as_str(), "org.freedesktop.portal.Request")
                .map_err(unavailable)?;
        let responses = request.receive_signal("Response").map_err(unavailable)?;
        let mut options: HashMap<&str, Value> = HashMap::new();
        options.insert("handle_token", token.as_str().into());
        options.insert("interactive", false.into());
        options.insert("modal", false.into());
        connection
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                Some("org.freedesktop.portal.Screenshot"),
                "Screenshot",
                &("", options),
            )
            .map_err(unavailable)?;

        // The first request may wait on a permission prompt, so allow a while.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut responses = responses;
            let _ = tx.send(responses.next());
        });
        let message = rx
            .recv_timeout(Duration::from_secs(60))
            .map_err(|_| "the screenshot request timed out".to_string())?
            .ok_or("the desktop portal closed the screenshot request")?;
        let (code, results): (u32, HashMap<String, OwnedValue>) =
            message.body().deserialize().map_err(|e| format!("unexpected portal answer: {e}"))?;
        if code != 0 {
            return Err("screen capture was denied; allow Blockwork to take screenshots in your desktop's settings".into());
        }
        let uri: String = results
            .get("uri")
            .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
            .ok_or("the portal didn't return a screenshot")?;
        let file = uri.strip_prefix("file://").ok_or("the portal returned an unexpected screenshot location")?;
        let file = percent_decode(file);
        let bytes = std::fs::read(&file).map_err(|e| format!("couldn't read the screenshot: {e}"));
        // The portal saves into the user's pictures; don't leave copies behind.
        let _ = std::fs::remove_file(&file);
        Image::from_png(&bytes?)
    }

    fn percent_decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%'
                && i + 2 < bytes.len()
                && let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    #[cfg(test)]
    #[test]
    fn decodes_file_uris() {
        assert_eq!(percent_decode("/home/a%20b/Screenshot.png"), "/home/a b/Screenshot.png");
    }
}

mod x11 {
    use super::*;
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;
    use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

    pub(super) fn capture() -> Result<Shot, String> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| format!("couldn't connect to the X server: {e}"))?;
        let root = &conn.setup().roots[screen];
        let (width, height) = (root.width_in_pixels, root.height_in_pixels);
        let reply = conn
            .get_image(ImageFormat::Z_PIXMAP, root.root, 0, 0, width, height, !0)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| format!("couldn't capture the screen: {e}"))?;
        if reply.data.len() < width as usize * height as usize * 4 {
            return Err(format!("unsupported X11 screen depth {}", reply.depth));
        }
        // 24/32-bit ZPixmap is BGRX in memory.
        let rgba = reply.data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        Ok(Shot { image: Image { width: width as u32, height: height as u32, rgba }, origin: (0.0, 0.0), scale: 1.0 })
    }

    pub(super) fn monitors() -> Vec<Rect> {
        let Ok((conn, screen)) = x11rb::connect(None) else { return Vec::new() };
        let root = &conn.setup().roots[screen];
        let monitors = conn.randr_get_monitors(root.root, true).ok().and_then(|c| c.reply().ok());
        match monitors {
            Some(reply) if !reply.monitors.is_empty() => reply
                .monitors
                .iter()
                .map(|m| Rect { x: m.x as f64, y: m.y as f64, width: m.width as f64, height: m.height as f64 })
                .collect(),
            _ => vec![Rect { x: 0.0, y: 0.0, width: root.width_in_pixels as f64, height: root.height_in_pixels as f64 }],
        }
    }
}

pub(super) fn monitors() -> Vec<Rect> {
    if !wayland() {
        return x11::monitors();
    }
    wayland_display::logical_outputs()
        .map(|outputs| {
            outputs
                .iter()
                .map(|o| Rect { x: o.bounds.x as f64, y: o.bounds.y as f64, width: o.bounds.width as f64, height: o.bounds.height as f64 })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn cursor() -> Result<(i32, i32), String> {
    if !wayland() {
        return x11_cursor::query_cursor_pos().ok_or_else(|| "couldn't read the cursor position".to_string());
    }
    let env = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if env("HYPRLAND_INSTANCE_SIGNATURE") {
        // "123, 456"
        let out = run("hyprctl", &["cursorpos"])?;
        let mut parts = out.split(',').map(|p| p.trim().parse::<f64>());
        if let (Some(Ok(x)), Some(Ok(y))) = (parts.next(), parts.next()) {
            return Ok((x.round() as i32, y.round() as i32));
        }
    }
    if std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.to_uppercase().contains("KDE"))
        && let Ok(out) = run("kdotool", &["getmouselocation"])
    {
        // "x:123 y:456 screen:0 window:{...}"
        let field = |name: &str| out.split_whitespace().find_map(|w| w.strip_prefix(name)?.parse::<f64>().ok());
        if let (Some(x), Some(y)) = (field("x:"), field("y:")) {
            return Ok((x.round() as i32, y.round() as i32));
        }
    }
    // Elsewhere only Blockwork's own estimate exists (exact after a move).
    cursor_track::current().ok_or_else(|| "this desktop doesn't share the cursor position; move the mouse with a block first".to_string())
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = host_command(program).args(args).output().map_err(|e| format!("couldn't run {program}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{program} failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_binary_ppm() {
        let mut data = b"P6\n# grim\n2 1\n255\n".to_vec();
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let image = parse_ppm(&data).unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.pixel(1, 0), [4, 5, 6]);
        assert!(parse_ppm(b"P3\n1 1\n255\n").is_err());
    }
}
