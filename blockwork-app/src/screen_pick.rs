//! The editor's screen picker: the UI lets the user choose a point or region
//! on a live overlay, then this captures it as a block image, coordinates
//! and a color.

use blockwork_core::screen::{self, Image, Shot};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct PickDto {
    /// The region as a PNG `data:` URI; empty for a point.
    image: String,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    /// Color at the point, or the region's middle.
    color: String,
}

/// Captures the screen and returns the region at (`x`, `y`) of
/// `width` x `height` screen units; a zero size picks a single point.
pub(crate) fn grab(x: f64, y: f64, width: f64, height: f64) -> Result<PickDto, String> {
    pick_from(&*screen::capture_fresh()?, x, y, width, height)
}

fn pick_from(shot: &Shot, x: f64, y: f64, width: f64, height: f64) -> Result<PickDto, String> {
    let (width, height) = (width.max(0.0), height.max(0.0));
    let region = width > 0.0 && height > 0.0;
    let image = if region { shot.region(x, y, width, height)?.to_data_uri()? } else { String::new() };
    let (px, py) = shot.to_image(x + width / 2.0, y + height / 2.0);
    if px < 0 || py < 0 || px >= shot.image.width as i64 || py >= shot.image.height as i64 {
        return Err(format!("{x},{y} is off screen"));
    }
    Ok(PickDto {
        image,
        x: x.round() as i64,
        y: y.round() as i64,
        width: width.round() as i64,
        height: height.round() as i64,
        color: screen::hex(shot.image.pixel(px as u32, py as u32)),
    })
}

/// A PNG file as a block image.
pub(crate) fn image_from_file(path: &str) -> Result<String, String> {
    let path = path.strip_prefix("file://").unwrap_or(path);
    Image::load(path)?.to_data_uri()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_map_screen_coordinates_onto_the_capture() {
        // A 2x desktop whose left edge sits at screen x = -100.
        let mut image = Image::new(400, 200);
        let i = (21 * 400 + 30) * 4;
        image.rgba[i..i + 3].copy_from_slice(&[255, 0, 0]);
        let shot = Shot { image, origin: (-100.0, 0.0), scale: 2.0 };

        let point = pick_from(&shot, -85.0, 10.5, 0.0, 0.0).unwrap();
        assert_eq!((point.x, point.y, point.width, point.height), (-85, 11, 0, 0));
        assert_eq!(point.color, "#ff0000");
        assert!(point.image.is_empty());

        let region = pick_from(&shot, -90.0, 5.0, 20.0, 15.0).unwrap();
        assert_eq!((region.width, region.height), (20, 15));
        let cropped = Image::load(&region.image).unwrap();
        assert_eq!((cropped.width, cropped.height), (40, 30));
        assert_eq!(cropped.pixel(10, 11), [255, 0, 0]);

        assert!(pick_from(&shot, 500.0, 500.0, 0.0, 0.0).is_err());
    }
}
