//! Reading the screen: pixel colors, image search and OCR, plus cursor and
//! monitor geometry.
//!
//! Every coordinate here is a *screen coordinate*: the same space the Move
//! mouse block's absolute mode uses (logical pixels on scaled displays,
//! except on Windows, where the daemon is DPI aware and they're physical).
//! Captures can be denser than that (physical pixels), so a [`Shot`] keeps
//! its origin and scale to map between the two.

mod find;
pub mod image;
mod ocr;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub use find::find;
pub use image::Image;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(windows)]
use windows as platform;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use super::{Rect, Shot};
    const MSG: &str = "reading the screen isn't supported on this platform";
    pub(super) const CHEAP_AREAS: bool = false;
    pub(super) fn capture() -> Result<Shot, String> {
        Err(MSG.into())
    }
    pub(super) fn capture_area(_: Rect, _native: bool) -> Option<Shot> {
        None
    }
    pub(super) fn monitors() -> Vec<Rect> {
        Vec::new()
    }
    pub(super) fn cursor() -> Result<(i32, i32), String> {
        Err(MSG.into())
    }
}

/// A rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One capture of the whole desktop.
pub struct Shot {
    pub image: Image,
    /// Screen coordinates of the image's top-left pixel.
    pub origin: (f64, f64),
    /// Image pixels per screen unit (2 on a Retina display, 1.5 at 150%).
    pub scale: f64,
}

impl Shot {
    /// The image pixel under screen point (`x`, `y`).
    pub fn to_image(&self, x: f64, y: f64) -> (i64, i64) {
        (((x - self.origin.0) * self.scale).floor() as i64, ((y - self.origin.1) * self.scale).floor() as i64)
    }

    /// The screen point at the middle of image pixel (`px`, `py`).
    pub fn to_screen(&self, px: f64, py: f64) -> (f64, f64) {
        (self.origin.0 + px / self.scale, self.origin.1 + py / self.scale)
    }

    /// The part of the image under a screen rectangle, clipped to the image.
    pub fn region(&self, x: f64, y: f64, width: f64, height: f64) -> Result<Image, String> {
        self.crop(Rect { x, y, width, height }).map(|shot| shot.image)
    }

    /// The part of this shot under a screen rectangle, clipped to the image,
    /// keeping the mapping back to screen coordinates.
    pub fn crop(&self, rect: Rect) -> Result<Shot, String> {
        let Rect { x, y, width, height } = rect;
        let (x0, y0) = self.to_image(x, y);
        let (x1, y1) = self.to_image(x + width, y + height);
        let clip = |v: i64, max: u32| v.clamp(0, max as i64) as u32;
        let (x0, x1) = (clip(x0, self.image.width), clip(x1.max(x0 + 1), self.image.width));
        let (y0, y1) = (clip(y0, self.image.height), clip(y1.max(y0 + 1), self.image.height));
        if x1 <= x0 || y1 <= y0 {
            return Err(format!("the region {x},{y} {width}x{height} is off screen"));
        }
        Ok(Shot {
            image: self.image.crop(x0, y0, x1 - x0, y1 - y0),
            origin: self.to_screen(x0 as f64, y0 as f64),
            scale: self.scale,
        })
    }

    /// The screen point at the middle of the image.
    pub fn center(&self) -> (f64, f64) {
        self.to_screen(self.image.width as f64 / 2.0, self.image.height as f64 / 2.0)
    }
}

/// How long a capture is reused, so one expression reading several pixels
/// takes one screenshot.
const REUSE_FOR: Duration = Duration::from_millis(60);

static LAST: Mutex<Option<(Instant, Arc<Shot>)>> = Mutex::new(None);

/// The current screen contents (reused for a few ms).
pub fn capture() -> Result<Arc<Shot>, String> {
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, shot)) = last.as_ref()
        && at.elapsed() < REUSE_FOR
    {
        return Ok(Arc::clone(shot));
    }
    let shot = Arc::new(platform::capture()?);
    *last = Some((Instant::now(), Arc::clone(&shot)));
    Ok(shot)
}

/// A fresh capture, ignoring the reuse window (the screen picker wants
/// what's there now).
pub fn capture_fresh() -> Result<Arc<Shot>, String> {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
    capture()
}

/// What's under a screen rectangle: captured on its own where that's cheap,
/// otherwise cut from the shared desktop capture.
fn area(rect: Rect) -> Result<Shot, String> {
    if platform::CHEAP_AREAS
        && let Some(shot) = platform::capture_area(rect, true)
    {
        return Ok(shot);
    }
    capture()?.crop(rect)
}

/// The color at screen point (`x`, `y`) as `#rrggbb`.
pub fn pixel_color(x: f64, y: f64) -> Result<String, String> {
    let shot = area(Rect { x, y, width: 1.0, height: 1.0 }).map_err(|_| format!("{x},{y} is off screen"))?;
    let (px, py) = shot.to_image(x, y);
    if px < 0 || py < 0 || px >= shot.image.width as i64 || py >= shot.image.height as i64 {
        return Err(format!("{x},{y} is off screen"));
    }
    Ok(hex(shot.image.pixel(px as u32, py as u32)))
}

/// The average color of a screen rectangle as `#rrggbb`.
pub fn average_color(x: f64, y: f64, width: f64, height: f64) -> Result<String, String> {
    let region = area(Rect { x, y, width: width.max(1.0), height: height.max(1.0) })?.image;
    let mut sum = [0u64; 3];
    for px in region.rgba.chunks_exact(4) {
        for c in 0..3 {
            sum[c] += px[c] as u64;
        }
    }
    let n = (region.width as u64 * region.height as u64).max(1);
    Ok(hex([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]))
}

/// Text read from a screen rectangle; an empty rectangle reads the whole screen.
pub fn read_text(x: f64, y: f64, width: f64, height: f64) -> Result<String, String> {
    let image = if width <= 0.0 || height <= 0.0 { capture()?.image.clone() } else { area(Rect { x, y, width, height })?.image };
    ocr::read(&image)
}

/// Where `needle` is on screen (the screen point at its middle), if it is.
/// `tolerance` is 0-100: how different the pixels may be, in percent.
pub fn find_image(needle: &Image, tolerance: f64) -> Result<Option<(f64, f64)>, String> {
    let shot = capture()?;
    Ok(find(&shot.image, needle, tolerance).map(|(px, py)| {
        shot.to_screen(px as f64 + needle.width as f64 / 2.0, py as f64 + needle.height as f64 / 2.0)
    }))
}

/// What's inside a matching window right now (the active window when
/// `pattern` is empty). Anything covering the window is captured too.
pub fn capture_window(pattern: &str, by: crate::window::MatchBy) -> Result<Shot, String> {
    window_shot(pattern, by, true)
}

/// [`capture_window`], but where it's cheaper the image may be at the
/// window's logical size instead of every physical pixel: plenty for a
/// detector, and a quarter of the work on a 2x screen.
pub fn capture_window_small(pattern: &str, by: crate::window::MatchBy) -> Result<Shot, String> {
    window_shot(pattern, by, false)
}

fn window_shot(pattern: &str, by: crate::window::MatchBy, native: bool) -> Result<Shot, String> {
    let bounds = window_bounds(pattern, by)?;
    if let Some(shot) = platform::capture_area(bounds, native) {
        return Ok(shot);
    }
    capture()?.crop(bounds).map_err(|_| "the window is off screen".to_string())
}

/// How long a window lookup is reused: aiming captures the same window many
/// times a second, and each lookup is a compositor round trip.
const BOUNDS_TTL: std::time::Duration = std::time::Duration::from_millis(500);

fn window_bounds(pattern: &str, by: crate::window::MatchBy) -> Result<Rect, String> {
    use std::time::Instant;
    static LAST: Mutex<Option<(String, crate::window::MatchBy, Rect, Instant)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, b, rect, at)) = &*last
        && p == pattern
        && *b == by
        && at.elapsed() < BOUNDS_TTL
    {
        return Ok(*rect);
    }
    let rect = crate::window::bounds(pattern, by)?;
    *last = Some((pattern.to_string(), by, rect, Instant::now()));
    Ok(rect)
}

/// Every monitor, in screen coordinates.
pub fn monitors() -> Vec<Rect> {
    platform::monitors()
}

/// The box around every monitor.
pub fn desktop() -> Result<Rect, String> {
    let monitors = monitors();
    let first = monitors.first().ok_or("couldn't find any monitors")?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x + first.width, first.y + first.height);
    for m in &monitors[1..] {
        x0 = x0.min(m.x);
        y0 = y0.min(m.y);
        x1 = x1.max(m.x + m.width);
        y1 = y1.max(m.y + m.height);
    }
    Ok(Rect { x: x0, y: y0, width: x1 - x0, height: y1 - y0 })
}

/// The mouse cursor's screen position.
pub fn cursor() -> Result<(i32, i32), String> {
    platform::cursor()
}

pub fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Parses `#rgb`, `#rrggbb` (the `#` is optional) or `r,g,b`.
pub fn parse_color(text: &str) -> Result<[u8; 3], String> {
    let text = text.trim();
    let bad = || format!("'{text}' isn't a color (use #rrggbb)");
    if text.contains(',') {
        let parts: Vec<u8> = text.split(',').map(|p| p.trim().parse::<u8>()).collect::<Result<_, _>>().map_err(|_| bad())?;
        return <[u8; 3]>::try_from(parts).map_err(|_| bad());
    }
    let digits = text.trim_start_matches('#');
    let channel = |s: &str| u8::from_str_radix(s, 16).map_err(|_| bad());
    match digits.len() {
        6 => Ok([channel(&digits[0..2])?, channel(&digits[2..4])?, channel(&digits[4..6])?]),
        3 => {
            let one = |i: usize| channel(&digits[i..i + 1]).map(|v| v * 17);
            Ok([one(0)?, one(1)?, one(2)?])
        }
        _ => Err(bad()),
    }
}

/// How far apart two colors are, 0 (same) to 100 (black vs white).
pub fn color_distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    let sq: f64 = (0..3).map(|i| (a[i] as f64 - b[i] as f64).powi(2)).sum();
    sq.sqrt() / (3.0f64 * 255.0 * 255.0).sqrt() * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_and_compare() {
        assert_eq!(parse_color("#ff8000"), Ok([255, 128, 0]));
        assert_eq!(parse_color("f80"), Ok([255, 136, 0]));
        assert_eq!(parse_color(" 1, 2, 3 "), Ok([1, 2, 3]));
        assert!(parse_color("red").is_err());
        assert_eq!(hex([255, 128, 0]), "#ff8000");
        assert_eq!(color_distance([0, 0, 0], [0, 0, 0]), 0.0);
        assert!((color_distance([0, 0, 0], [255, 255, 255]) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn shots_map_between_screen_and_image_pixels() {
        let shot = Shot { image: Image::new(300, 150), origin: (-100.0, 0.0), scale: 1.5 };
        assert_eq!(shot.to_image(-100.0, 0.0), (0, 0));
        assert_eq!(shot.to_image(0.0, 10.0), (150, 15));
        assert_eq!(shot.to_screen(150.0, 15.0), (0.0, 10.0));
        let region = shot.region(0.0, 0.0, 20.0, 10.0).unwrap();
        assert_eq!((region.width, region.height), (30, 15));
        assert!(shot.region(500.0, 500.0, 10.0, 10.0).is_err());
        // A crop hanging off the left edge starts at the image edge.
        let crop = shot.crop(Rect { x: -120.0, y: 10.0, width: 40.0, height: 20.0 }).unwrap();
        assert_eq!(crop.origin, (-100.0, 10.0));
        assert_eq!((crop.image.width, crop.image.height), (30, 30));
        assert_eq!(crop.center(), (-90.0, 20.0));
    }
}

/// Real-desktop checks; run by hand with `--ignored` on a desktop session.
#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn reads_the_real_screen() {
        let started = std::time::Instant::now();
        let shot = super::capture().unwrap();
        eprintln!("capture {}x{} scale {} origin {:?} in {:?}", shot.image.width, shot.image.height, shot.scale, shot.origin, started.elapsed());
        eprintln!("monitors {:?}", super::monitors());
        eprintln!("cursor {:?}", super::cursor());
        eprintln!("pixel {:?}", super::pixel_color(10.0, 10.0));
        let needle = shot.region(20.0, 4.0, 90.0, 24.0).unwrap();
        let started = std::time::Instant::now();
        eprintln!("find {:?} in {:?}", super::find_image(&needle, 5.0), started.elapsed());
        let started = std::time::Instant::now();
        eprintln!("text {:?} in {:?}", super::read_text(0.0, 0.0, 600.0, 120.0), started.elapsed());
    }
}

/// Window checks against a test window titled `$BW_LIVE_WINDOW`; run by hand
/// with `--ignored`.
#[cfg(test)]
mod live_window {
    use crate::window::MatchBy;

    fn title() -> String {
        std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW to a test window's title")
    }

    #[test]
    #[ignore]
    fn window_bounds() {
        let started = std::time::Instant::now();
        eprintln!("bounds {:?} in {:?}", crate::window::bounds(&title(), MatchBy::Title), started.elapsed());
    }

    #[test]
    #[ignore]
    fn window_capture_rate() {
        let title = title();
        for _ in 0..10 {
            let started = std::time::Instant::now();
            let shot = super::capture_window(&title, MatchBy::Title).unwrap();
            eprintln!("capture_window {}x{} origin {:?} scale {} in {:?}", shot.image.width, shot.image.height, shot.origin, shot.scale, started.elapsed());
        }
        // Where pure red sits, to check the crop against a centered red marker.
        let shot = super::capture_window(&title, MatchBy::Title).unwrap();
        let (w, h) = (shot.image.width, shot.image.height);
        let red: Vec<(u32, u32)> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| shot.image.pixel(x, y) == [255, 0, 0]).collect();
        if let (Some(x0), Some(x1), Some(y0), Some(y1)) =
            (red.iter().map(|p| p.0).min(), red.iter().map(|p| p.0).max(), red.iter().map(|p| p.1).min(), red.iter().map(|p| p.1).max())
        {
            let marker = shot.to_screen((x0 + x1 + 1) as f64 / 2.0, (y0 + y1 + 1) as f64 / 2.0);
            eprintln!("red marker center {marker:?}, window center {:?}", shot.center());
        }
    }
}
