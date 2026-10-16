//! macOS screen access through CoreGraphics. Capturing other apps' windows
//! needs the Screen Recording permission.

use super::{Image, Rect, Shot};
use core_graphics::display::{CGDisplay, CGPoint, CGRect, CGSize};
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::window::{create_image, kCGNullWindowID, kCGWindowImageDefault, kCGWindowListOptionOnScreenOnly};

pub(super) fn capture_area(_: Rect, _native: bool) -> Option<Shot> {
    None
}

pub(super) fn capture() -> Result<Shot, String> {
    let desktop = super::desktop()?;
    let bounds = CGRect::new(&CGPoint::new(desktop.x, desktop.y), &CGSize::new(desktop.width, desktop.height));
    let image = create_image(bounds, kCGWindowListOptionOnScreenOnly, kCGNullWindowID, kCGWindowImageDefault)
        .ok_or("couldn't capture the screen; allow Blockwork under Privacy & Security > Screen Recording")?;
    let (width, height, stride) = (image.width(), image.height(), image.bytes_per_row());
    if image.bits_per_pixel() != 32 {
        return Err("unsupported screen pixel format".into());
    }
    let data = image.data();
    let bytes = data.bytes();
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        // 32-bit little-endian BGRA.
        for px in bytes[row * stride..row * stride + width * 4].chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    Ok(Shot {
        image: Image { width: width as u32, height: height as u32, rgba },
        origin: (desktop.x, desktop.y),
        scale: width as f64 / desktop.width,
    })
}

pub(super) fn monitors() -> Vec<Rect> {
    CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            let b = CGDisplay::new(id).bounds();
            Rect { x: b.origin.x, y: b.origin.y, width: b.size.width, height: b.size.height }
        })
        .collect()
}

pub(super) fn cursor() -> Result<(i32, i32), String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).map_err(|_| "couldn't read the cursor position")?;
    let event = CGEvent::new(source).map_err(|_| "couldn't read the cursor position")?;
    let at = event.location();
    Ok((at.x.round() as i32, at.y.round() as i32))
}
