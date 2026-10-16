//! YOLOv8-style models: letterboxing a frame into the model's square input,
//! and turning its `[1, 4 + classes, anchors]` output back into boxes.
//! Model-free, so it's tested without a runtime.

use super::Detection;
use crate::screen::Image;

/// How a frame was fitted into the model input, to map boxes back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Letterbox {
    pub scale: f64,
    pub pad_x: f64,
    pub pad_y: f64,
}

/// Scales `image` to fit `size`x`size`, keeping its aspect, padded with gray.
/// Returns CHW RGB floats in 0-1, the layout YOLO exports expect.
pub fn letterbox(image: &Image, size: u32) -> (Vec<f32>, Letterbox) {
    let s = size as usize;
    let scale = (size as f64 / image.width.max(1) as f64).min(size as f64 / image.height.max(1) as f64);
    let (w, h) = (((image.width as f64 * scale).round() as usize).max(1), ((image.height as f64 * scale).round() as usize).max(1));
    let (pad_x, pad_y) = ((s - w.min(s)) / 2, (s - h.min(s)) / 2);
    let mut out = vec![114.0 / 255.0; 3 * s * s];
    for y in 0..h.min(s) {
        // Nearest neighbour is plenty for a detector and keeps this cheap.
        let sy = ((y as f64 + 0.5) / scale) as u32;
        let sy = sy.min(image.height - 1);
        for x in 0..w.min(s) {
            let sx = (((x as f64 + 0.5) / scale) as u32).min(image.width - 1);
            let px = image.pixel(sx, sy);
            let at = (y + pad_y) * s + x + pad_x;
            for c in 0..3 {
                out[c * s * s + at] = px[c] as f32 / 255.0;
            }
        }
    }
    (out, Letterbox { scale, pad_x: pad_x as f64, pad_y: pad_y as f64 })
}

/// Boxes scoring at least `min_score` from a raw output of `shape`, in the
/// original frame's pixels, after non-max suppression. Accepts both
/// `[1, 4 + classes, anchors]` (the YOLOv8 export) and its transpose.
pub fn decode(output: &[f32], shape: &[usize], labels: &[String], min_score: f32, lb: Letterbox) -> Result<Vec<Detection>, String> {
    let (rows, cols) = match shape {
        [1, a, b] | [a, b] => (*a, *b),
        _ => return Err(format!("unexpected model output shape {shape:?}")),
    };
    if output.len() != rows * cols {
        return Err("model output doesn't match its shape".into());
    }
    // Channels are 4 box values plus one score per class: the side matching
    // the label count, else the side that can hold them, else the short side.
    let classes = labels.len() + 4;
    let channels_first = if !labels.is_empty() && (rows == classes) != (cols == classes) {
        rows == classes
    } else if (rows < 5) != (cols < 5) {
        cols < 5
    } else {
        rows <= cols
    };
    let (channels, anchors) = if channels_first { (rows, cols) } else { (cols, rows) };
    if channels < 5 {
        return Err(format!("unexpected model output shape {shape:?}"));
    }
    let at = |c: usize, a: usize| if channels_first { output[c * anchors + a] } else { output[a * channels + c] };
    let mut found = Vec::new();
    for a in 0..anchors {
        let (class, score) = (4..channels).map(|c| (c - 4, at(c, a))).fold((0, f32::MIN), |best, x| if x.1 > best.1 { x } else { best });
        if score < min_score {
            continue;
        }
        let (cx, cy, w, h) = (at(0, a) as f64, at(1, a) as f64, at(2, a) as f64, at(3, a) as f64);
        found.push(Detection {
            x: (cx - w / 2.0 - lb.pad_x) / lb.scale,
            y: (cy - h / 2.0 - lb.pad_y) / lb.scale,
            w: w / lb.scale,
            h: h / lb.scale,
            label: labels.get(class).cloned().unwrap_or_else(|| class.to_string()),
            score: score as f64,
            id: None,
        });
    }
    Ok(nms(found, 0.45))
}

/// Keeps the best box among same-label boxes overlapping more than `iou`.
pub fn nms(mut boxes: Vec<Detection>, iou: f64) -> Vec<Detection> {
    boxes.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Detection> = Vec::new();
    for b in boxes {
        if !kept.iter().any(|k| k.label == b.label && overlap(k, &b) > iou) {
            kept.push(b);
        }
    }
    kept
}

fn overlap(a: &Detection, b: &Detection) -> f64 {
    let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    w * h / (a.w * a.h + b.w * b.h - w * h)
}

/// Class names from an Ultralytics export's `names` metadata, a Python dict
/// like `{0: 'person', 1: 'bicycle'}`.
pub fn parse_names(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"(\d+)\s*:\s*['"]([^'"]*)['"]"#).expect("valid regex");
    let mut pairs: Vec<(usize, String)> =
        re.captures_iter(text).filter_map(|c| Some((c[1].parse().ok()?, c[2].to_string()))).collect();
    pairs.sort();
    let mut names = Vec::new();
    for (i, name) in pairs {
        names.resize(i, String::new());
        names.push(name);
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterbox_centers_a_wide_frame() {
        // A 2x2 red block, since halving keeps every other pixel.
        let mut image = Image::new(200, 100);
        for i in [0, 1, 200, 201] {
            image.rgba[i * 4..i * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
        let (input, lb) = letterbox(&image, 100);
        assert_eq!(lb, Letterbox { scale: 0.5, pad_x: 0.0, pad_y: 25.0 });
        assert_eq!(input.len(), 3 * 100 * 100);
        // Padding is gray; the frame's top-left red pixel lands below it.
        assert!((input[0] - 114.0 / 255.0).abs() < 1e-6);
        assert_eq!(input[25 * 100], 1.0);
        assert_eq!(input[100 * 100 + 25 * 100], 0.0);
    }

    #[test]
    fn decode_maps_boxes_back_and_suppresses_duplicates() {
        // Channels-first: 4 box rows + 2 classes, 3 anchors.
        let lb = Letterbox { scale: 0.5, pad_x: 0.0, pad_y: 25.0 };
        #[rustfmt::skip]
        let out = [
            50.0, 52.0, 10.0, // cx
            50.0, 50.0, 10.0, // cy
            20.0, 20.0, 4.0,  // w
            40.0, 40.0, 4.0,  // h
            0.9,  0.8,  0.1,  // person
            0.1,  0.2,  0.3,  // dog
        ];
        let labels = vec!["person".to_string(), "dog".to_string()];
        let found = decode(&out, &[1, 6, 3], &labels, 0.35, lb).unwrap();
        // The second box overlaps the first too much and loses; the third is too weak.
        assert_eq!(found.len(), 1);
        let d = &found[0];
        assert_eq!((d.x, d.y, d.w, d.h, d.label.as_str()), (80.0, 10.0, 40.0, 80.0, "person"));
        assert!((d.score - 0.9).abs() < 1e-6);
        // The transposed layout reads the same.
        let mut t = vec![0.0; 18];
        for c in 0..6 {
            for a in 0..3 {
                t[a * 6 + c] = out[c * 3 + a];
            }
        }
        assert_eq!(decode(&t, &[1, 3, 6], &labels, 0.35, lb).unwrap(), found);
        assert!(decode(&out, &[1, 6, 4], &labels, 0.25, lb).is_err());
    }

    #[test]
    fn unknown_classes_fall_back_to_their_index() {
        let lb = Letterbox { scale: 1.0, pad_x: 0.0, pad_y: 0.0 };
        let found = decode(&[5.0, 5.0, 2.0, 2.0, 0.0, 0.7], &[1, 6, 1], &[], 0.5, lb).unwrap();
        assert_eq!(found[0].label, "1");
    }

    #[test]
    fn ultralytics_names_parse() {
        assert_eq!(parse_names("{0: 'person', 1: 'bicycle', 3: \"car\"}"), vec!["person", "bicycle", "", "car"]);
        assert!(parse_names("").is_empty());
    }
}
