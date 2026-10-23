//! A plain RGBA image, with PNG and `data:` URI conversions for the images
//! blocks carry.

use base64::Engine;

#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA, 4 bytes per pixel.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}x{})", self.width, self.height)
    }
}

const DATA_URI_PREFIX: &str = "data:image/png;base64,";

impl Image {
    /// An all-black image.
    pub fn new(width: u32, height: u32) -> Image {
        let mut rgba = vec![0; width as usize * height as usize * 4];
        rgba.chunks_exact_mut(4).for_each(|px| px[3] = 255);
        Image { width, height, rgba }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Image {
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in y..y + height {
            let start = (row as usize * self.width as usize + x as usize) * 4;
            rgba.extend_from_slice(&self.rgba[start..start + width as usize * 4]);
        }
        Image { width, height, rgba }
    }

    /// Repeats each pixel `factor` times both ways.
    pub fn scale_up(&self, factor: u32) -> Image {
        if factor <= 1 {
            return self.clone();
        }
        let (width, height) = (self.width * factor, self.height * factor);
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let i = ((y / factor) as usize * self.width as usize + (x / factor) as usize) * 4;
                rgba.extend_from_slice(&self.rgba[i..i + 4]);
            }
        }
        Image { width, height, rgba }
    }

    pub fn from_png(bytes: &[u8]) -> Result<Image, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(|e| format!("not a PNG image: {e}"))?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG image is too large")?];
        let info = reader.next_frame(&mut buf).map_err(|e| format!("couldn't read PNG image: {e}"))?;
        buf.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
            png::ColorType::Grayscale => buf.iter().flat_map(|&v| [v, v, v, 255]).collect(),
            png::ColorType::Indexed => return Err("unsupported PNG color type".into()),
        };
        Ok(Image { width: info.width, height: info.height, rgba })
    }

    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&self.rgba).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(out)
    }

    pub fn to_data_uri(&self) -> Result<String, String> {
        Ok(format!("{DATA_URI_PREFIX}{}", base64::engine::general_purpose::STANDARD.encode(self.to_png()?)))
    }

    /// An image block's image: a PNG `data:` URI, or a path to a PNG file.
    pub fn load(source: &str) -> Result<Image, String> {
        let source = source.trim();
        if source.is_empty() {
            return Err("no image chosen; capture one from the screen first".into());
        }
        if let Some(encoded) = source.strip_prefix(DATA_URI_PREFIX) {
            let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|e| format!("broken image data: {e}"))?;
            return Image::from_png(&bytes);
        }
        let bytes = crate::files::read_bytes(source)?;
        Image::from_png(&bytes)
    }
}

/// [`Image::load`], remembering recent images so a block in a loop doesn't
/// decode its picture every time.
pub fn load_cached(source: &str) -> Result<std::sync::Arc<Image>, String> {
    use std::hash::{Hash, Hasher};
    use std::sync::{Arc, Mutex};
    static CACHE: Mutex<Vec<(u64, Arc<Image>)>> = Mutex::new(Vec::new());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    let key = hasher.finish();
    if let Some((_, image)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return Ok(Arc::clone(image));
    }
    let image = Arc::new(Image::load(source)?);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.insert(0, (key, Arc::clone(&image)));
    cache.truncate(16);
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_data_uris_round_trip() {
        let mut image = Image::new(3, 2);
        image.rgba[4..8].copy_from_slice(&[10, 20, 30, 255]);
        let uri = image.to_data_uri().unwrap();
        assert!(uri.starts_with(DATA_URI_PREFIX));
        assert_eq!(Image::load(&uri).unwrap(), image);
        assert_eq!(image.crop(1, 0, 1, 1).pixel(0, 0), [10, 20, 30]);
        assert_eq!(image.scale_up(2).pixel(3, 1), [10, 20, 30]);
        assert!(Image::load("").is_err());
    }
}
