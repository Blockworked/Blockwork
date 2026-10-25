//! macOS screen access through CoreGraphics. Capturing other apps' windows
//! needs the Screen Recording permission.

use super::{Image, Rect, Shot};
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::display::{CGDisplay, CGPoint, CGRect, CGSize};
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::image::{CGImage, CGImageAlphaInfo, CGImageByteOrderInfo};
use core_graphics::window::{
    create_image, kCGNullWindowID, kCGWindowImageBestResolution, kCGWindowImageNominalResolution,
    kCGWindowListOptionOnScreenOnly,
};

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
}

pub(super) const CHEAP_AREAS: bool = true;

pub(super) fn capture_area(rect: Rect, native: bool) -> Option<Shot> {
    capture_rect(rect, native).ok()
}

pub(super) fn capture() -> Result<Shot, String> {
    capture_rect(super::desktop()?, true)
}

fn capture_rect(rect: Rect, native: bool) -> Result<Shot, String> {
    // Without this grant CoreGraphics can succeed while silently excluding
    // other applications, producing plausible but incorrect colors/images.
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err("Blockwork needs Screen Recording access to capture other apps; enable this Blockwork app in System Settings → Privacy & Security → Screen & System Audio Recording, then restart Blockwork".into());
    }
    let desktop = super::desktop()?;
    let x = rect.x.max(desktop.x).floor();
    let y = rect.y.max(desktop.y).floor();
    let right = (rect.x + rect.width).min(desktop.x + desktop.width).ceil();
    let bottom = (rect.y + rect.height)
        .min(desktop.y + desktop.height)
        .ceil();
    if ![rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|n| n.is_finite())
        || rect.width <= 0.0
        || rect.height <= 0.0
        || right <= x
        || bottom <= y
    {
        return Err("the capture region is off screen or has invalid dimensions".into());
    }
    let bounds = CGRect::new(&CGPoint::new(x, y), &CGSize::new(right - x, bottom - y));
    let resolution = if native {
        kCGWindowImageBestResolution
    } else {
        kCGWindowImageNominalResolution
    };
    let image = create_image(
        bounds,
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        resolution,
    )
    .ok_or(
        "couldn't capture the screen; allow Blockwork under Privacy & Security > Screen Recording",
    )?;
    Ok(Shot {
        image: normalize_image(&image),
        origin: (x, y),
        scale: image.width() as f64 / bounds.size.width,
    })
}

// CoreGraphics may return different byte orders and color spaces (including
// wide-gamut and HDR). Render into an explicit 8-bit sRGB RGBA buffer instead
// of treating every source image as little-endian BGRA.
fn normalize_image(image: &CGImage) -> Image {
    let (width, height) = (image.width(), image.height());
    let space =
        CGColorSpace::create_with_name(unsafe { core_graphics::color_space::kCGColorSpaceSRGB })
            .unwrap_or_else(CGColorSpace::create_device_rgb);
    let mut context = CGContext::create_bitmap_context(
        None,
        width,
        height,
        8,
        width * 4,
        &space,
        CGImageAlphaInfo::CGImageAlphaNoneSkipLast as u32
            | CGImageByteOrderInfo::CGImageByteOrder32Big as u32,
    );
    context.draw_image(
        CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(width as f64, height as f64),
        ),
        image,
    );
    let mut rgba = context.data().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    Image {
        width: width as u32,
        height: height as u32,
        rgba,
    }
}

pub(super) fn monitors() -> Vec<Rect> {
    CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            let b = CGDisplay::new(id).bounds();
            Rect {
                x: b.origin.x,
                y: b.origin.y,
                width: b.size.width,
                height: b.size.height,
            }
        })
        .collect()
}

pub(super) fn cursor() -> Result<(i32, i32), String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "couldn't read the cursor position")?;
    let event = CGEvent::new(source).map_err(|_| "couldn't read the cursor position")?;
    let at = event.location();
    Ok((at.x.round() as i32, at.y.round() as i32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::data_provider::CGDataProvider;
    use std::sync::Arc;

    #[test]
    fn normalizes_rgb_and_bgra_without_swapping_rows_or_channels() {
        let space = CGColorSpace::create_with_name(unsafe {
            core_graphics::color_space::kCGColorSpaceSRGB
        })
        .unwrap();
        let cases = [
            (
                24,
                6,
                0,
                vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
            ),
            (
                32,
                8,
                CGImageAlphaInfo::CGImageAlphaNoneSkipFirst as u32
                    | CGImageByteOrderInfo::CGImageByteOrder32Little as u32,
                vec![0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
            ),
        ];
        for (bits, stride, info, data) in cases {
            let provider = CGDataProvider::from_buffer(Arc::new(data));
            let source = CGImage::new(2, 2, 8, bits, stride, &space, info, &provider, false, 0);
            let image = normalize_image(&source);
            assert_eq!(image.pixel(0, 0), [255, 0, 0]);
            assert_eq!(image.pixel(1, 0), [0, 255, 0]);
            assert_eq!(image.pixel(0, 1), [0, 0, 255]);
            assert_eq!(image.pixel(1, 1), [255, 255, 255]);
            assert!(image.rgba.chunks_exact(4).all(|p| p[3] == 255));
        }
    }

    #[test]
    #[ignore = "captures the real macOS desktop"]
    fn live_region_capture_preserves_screen_coordinates_and_resolution() {
        let desktop = super::super::desktop().unwrap();
        let rect = Rect {
            x: desktop.x + 32.0,
            y: desktop.y + 64.0,
            width: 80.0,
            height: 40.0,
        };
        for native in [true, false] {
            let shot = capture_rect(rect, native).unwrap();
            assert_eq!(shot.origin, (rect.x, rect.y));
            assert_eq!(shot.to_image(rect.x, rect.y), (0, 0));
            assert_eq!(shot.center(), (rect.x + 40.0, rect.y + 20.0));
            assert_eq!(shot.image.width as f64, 80.0 * shot.scale);
            assert_eq!(shot.image.height as f64, 40.0 * shot.scale);
            if !native {
                assert_eq!(shot.scale, 1.0);
            }
        }
        assert!(capture_rect(Rect { width: 0.0, ..rect }, true).is_err());
        assert!(
            capture_rect(
                Rect {
                    x: f64::NAN,
                    ..rect
                },
                true
            )
            .is_err()
        );
    }
}
