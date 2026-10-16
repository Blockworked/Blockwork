//! Finding objects inside one window: capture the window, ask a detector
//! for boxes, map them to screen coordinates, then track them across frames
//! so a macro can follow one target. For automation where it's allowed
//! (single-player, local test servers, UI testing) - not for use against
//! other players.

pub mod aim;
pub mod http;
#[cfg(feature = "onnx")]
pub mod onnx;
#[cfg(feature = "onnx")]
pub mod models;
pub mod yolo;
pub mod track;

use crate::screen::{Image, Rect, Shot};
use crate::window::MatchBy;
use serde::{Deserialize, Serialize};

pub use aim::AimController;

/// One box. In a detector's answer it's in image pixels; everywhere else in
/// screen coordinates, the space `MoveMouse` and `Shot::to_screen` use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    /// Top-left corner.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub label: String,
    /// 0 to 1.
    pub score: f64,
    /// Stable across frames once the tracker has seen it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

impl Detection {
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// One list item: compact JSON, rounded so it reads well in the editor.
    pub fn to_item(&self) -> String {
        let round = |v: f64| (v * 10.0).round() / 10.0;
        let mut d = self.clone();
        (d.x, d.y, d.w, d.h) = (round(d.x), round(d.y), round(d.w), round(d.h));
        d.score = (d.score * 1000.0).round() / 1000.0;
        serde_json::to_string(&d).unwrap_or_default()
    }

    pub fn from_item(text: &str) -> Result<Detection, String> {
        serde_json::from_str(text.trim()).map_err(|_| format!("'{}' isn't a detection", short(text)))
    }

    /// The reporter's fields: `x`, `y`, `width`, `height`, `center x`,
    /// `center y`, `score` (0-100), `label` and `id`.
    pub fn field(&self, name: &str) -> Result<crate::input::value::Evaluated, String> {
        use crate::input::value::Evaluated::{Number, Text};
        let (cx, cy) = self.center();
        Ok(match name.trim() {
            "x" => Number(self.x),
            "y" => Number(self.y),
            "width" => Number(self.w),
            "height" => Number(self.h),
            "center x" => Number(cx),
            "center y" => Number(cy),
            "score" => Number((self.score * 1000.0).round() / 10.0),
            "label" => Text(self.label.clone()),
            "id" => Number(self.id.map_or(-1.0, |id| id as f64)),
            other => return Err(format!("detections have no '{other}'")),
        })
    }
}

fn short(text: &str) -> String {
    let mut out: String = text.chars().take(40).collect();
    if text.chars().count() > 40 {
        out.push('…');
    }
    out
}

/// Grabs a window's pixels. A trait so tests run without a display.
pub trait WindowCapture {
    fn capture_window(&self, pattern: &str, by: MatchBy) -> Result<Shot, String>;
}

/// Finds boxes in an image, in that image's pixels. A trait so tests run
/// without a model.
pub trait Detector {
    fn detect(&self, image: &Image) -> Result<Vec<Detection>, String>;
}

/// The real screen.
pub struct Screen;

impl WindowCapture for Screen {
    fn capture_window(&self, pattern: &str, by: MatchBy) -> Result<Shot, String> {
        crate::screen::capture_window(pattern, by)
    }
}

/// How the results list is ordered, so item 1 is the natural target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Most confident first.
    #[default]
    Score,
    /// Nearest the crosshair (window center) first.
    Closest,
    /// Tallest box first: usually the nearest object.
    Largest,
}

pub const SORTS: [&str; 3] = ["Score", "Closest", "Largest"];

impl std::str::FromStr for Sort {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "" | "Score" => Ok(Sort::Score),
            "Closest" => Ok(Sort::Closest),
            "Largest" => Ok(Sort::Largest),
            _ => Err(format!("unknown detection order '{s}'")),
        }
    }
}

/// Everything one detection pass needs. Kept by the tracker so the aim
/// block can repeat the pass that filled a list.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub pattern: String,
    pub by: MatchBy,
    /// A local detector URL (see [`http::HttpDetector`]), or a built-in
    /// model's id or `.onnx` path (see `models`).
    pub detector: String,
    /// Only this label (any case); empty keeps every label.
    pub label: String,
    /// Minimum score, 0 to 1.
    pub min_score: f64,
    pub sort: Sort,
    /// Only this part of the window, relative to its top-left corner.
    pub region: Option<Rect>,
}

/// A block's region text: `x,y,width,height` inside the window, or empty
/// for the whole window.
pub fn parse_region(text: &str) -> Result<Option<Rect>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let bad = || format!("'{text}' isn't a region (use x,y,width,height)");
    let n: Vec<f64> = text.split(',').map(|v| v.trim().parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    match n[..] {
        [x, y, width, height] if width > 0.0 && height > 0.0 => Ok(Some(Rect { x, y, width, height })),
        _ => Err(bad()),
    }
}

/// One pass's result, in screen coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub detections: Vec<Detection>,
    /// The window center, where a first-person game draws its crosshair.
    pub crosshair: (f64, f64),
}

/// Captures the window, runs the detector and keeps the boxes that pass the
/// query's filters, mapped to screen coordinates. Not yet sorted or tracked.
pub fn detect(query: &Query, capture: &dyn WindowCapture, detector: &dyn Detector) -> Result<Frame, String> {
    let shot = capture.capture_window(&query.pattern, query.by)?;
    // The crosshair stays the window's center even when only part is searched.
    let crosshair = shot.center();
    let shot = match query.region {
        Some(r) => shot
            .crop(Rect { x: shot.origin.0 + r.x, y: shot.origin.1 + r.y, ..r })
            .map_err(|_| "the region is outside the window".to_string())?,
        None => shot,
    };
    let found = detector.detect(&shot.image)?;
    let label = query.label.trim().to_lowercase();
    let detections = found
        .into_iter()
        .filter(|d| label.is_empty() || d.label.to_lowercase() == label)
        .filter(|d| d.score >= query.min_score)
        .map(|d| to_screen(&shot, d))
        .collect();
    Ok(Frame { detections, crosshair })
}

fn to_screen(shot: &Shot, d: Detection) -> Detection {
    let (x, y) = shot.to_screen(d.x, d.y);
    Detection { x, y, w: d.w / shot.scale, h: d.h / shot.scale, ..d }
}

/// Orders detections per `sort`; ties keep the higher score first.
pub fn sort(detections: &mut [Detection], sort: Sort, crosshair: (f64, f64)) {
    let distance = |d: &Detection| {
        let (cx, cy) = d.center();
        (cx - crosshair.0).hypot(cy - crosshair.1)
    };
    let by_score = |a: &Detection, b: &Detection| b.score.total_cmp(&a.score);
    match sort {
        Sort::Score => detections.sort_by(by_score),
        Sort::Closest => detections.sort_by(|a, b| distance(a).total_cmp(&distance(b)).then_with(|| by_score(a, b))),
        Sort::Largest => detections.sort_by(|a, b| b.h.total_cmp(&a.h).then_with(|| by_score(a, b))),
    }
}

/// The detector a block's text names: a URL is a local detector service;
/// anything else is a built-in model.
pub fn detector_for(text: &str) -> Result<Box<dyn Detector>, String> {
    let text = text.trim();
    if text.contains("://") {
        return Ok(Box::new(http::HttpDetector::new(text)?));
    }
    #[cfg(feature = "onnx")]
    {
        let path = models::resolve(text, &models::ask_to_download)?;
        Ok(Box::new(onnx::OnnxDetector::load(&path)?))
    }
    #[cfg(not(feature = "onnx"))]
    Err("built-in models aren't part of this build; use a local detector URL".into())
}

/// [`detect`] with the real screen and the query's detector.
pub fn detect_on_screen(query: &Query) -> Result<Frame, String> {
    detect(query, &Screen, &*detector_for(&query.detector)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) struct FakeCapture(pub Shot);

    impl WindowCapture for FakeCapture {
        fn capture_window(&self, _: &str, _: MatchBy) -> Result<Shot, String> {
            Ok(Shot { image: self.0.image.clone(), origin: self.0.origin, scale: self.0.scale })
        }
    }

    pub(crate) struct FakeDetector(pub Vec<Detection>);

    impl Detector for FakeDetector {
        fn detect(&self, _: &Image) -> Result<Vec<Detection>, String> {
            Ok(self.0.clone())
        }
    }

    pub(crate) fn boxed(x: f64, y: f64, w: f64, h: f64, label: &str, score: f64) -> Detection {
        Detection { x, y, w, h, label: label.into(), score, id: None }
    }

    fn query(label: &str, min_score: f64) -> Query {
        Query { pattern: String::new(), by: MatchBy::Any, detector: String::new(), label: label.into(), min_score, sort: Sort::Score, region: None }
    }

    #[test]
    fn boxes_map_from_window_pixels_to_screen() {
        // A window at (100, 50) on a 2x display: 400x200 image pixels.
        let capture = FakeCapture(Shot { image: Image::new(400, 200), origin: (100.0, 50.0), scale: 2.0 });
        let detector = FakeDetector(vec![boxed(40.0, 20.0, 64.0, 128.0, "player", 0.9)]);
        let frame = detect(&query("", 0.0), &capture, &detector).unwrap();
        assert_eq!(frame.detections, vec![boxed(120.0, 60.0, 32.0, 64.0, "player", 0.9)]);
        assert_eq!(frame.crosshair, (200.0, 100.0));
        assert_eq!(frame.detections[0].center(), (136.0, 92.0));
    }

    #[test]
    fn a_region_searches_part_of_the_window_but_keeps_its_crosshair() {
        let capture = FakeCapture(Shot { image: Image::new(400, 200), origin: (100.0, 50.0), scale: 2.0 });
        let detector = FakeDetector(vec![boxed(10.0, 4.0, 20.0, 40.0, "player", 0.9)]);
        let part = Query { region: Some(Rect { x: 50.0, y: 20.0, width: 60.0, height: 40.0 }), ..query("", 0.0) };
        let frame = detect(&part, &capture, &detector).unwrap();
        // Box pixels are relative to the region, which starts at (150, 70) on screen.
        assert_eq!(frame.detections, vec![boxed(155.0, 72.0, 10.0, 20.0, "player", 0.9)]);
        assert_eq!(frame.crosshair, (200.0, 100.0));
        let outside = Query { region: Some(Rect { x: 500.0, y: 0.0, width: 10.0, height: 10.0 }), ..query("", 0.0) };
        assert!(detect(&outside, &capture, &detector).is_err());
    }

    #[test]
    fn regions_parse() {
        assert_eq!(parse_region(" "), Ok(None));
        assert_eq!(parse_region("10, 20,300,200"), Ok(Some(Rect { x: 10.0, y: 20.0, width: 300.0, height: 200.0 })));
        for bad in ["10,20,300", "a,b,c,d", "0,0,0,10", "1,2,3,4,5"] {
            assert!(parse_region(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn label_and_score_filters_apply() {
        let capture = FakeCapture(Shot { image: Image::new(10, 10), origin: (0.0, 0.0), scale: 1.0 });
        let detector = FakeDetector(vec![
            boxed(0.0, 0.0, 1.0, 1.0, "Player", 0.9),
            boxed(0.0, 0.0, 1.0, 1.0, "player", 0.3),
            boxed(0.0, 0.0, 1.0, 1.0, "nametag", 0.95),
        ]);
        let kept = detect(&query("player", 0.5), &capture, &detector).unwrap().detections;
        assert_eq!(kept, vec![boxed(0.0, 0.0, 1.0, 1.0, "Player", 0.9)]);
        assert_eq!(detect(&query("", 0.0), &capture, &detector).unwrap().detections.len(), 3);
    }

    #[test]
    fn sorting_picks_the_natural_target_first() {
        let far = boxed(0.0, 0.0, 10.0, 20.0, "p", 0.9);
        let near = boxed(90.0, 90.0, 20.0, 40.0, "p", 0.5);
        let mut list = vec![far.clone(), near.clone()];
        sort(&mut list, Sort::Closest, (100.0, 100.0));
        assert_eq!(list, vec![near.clone(), far.clone()]);
        sort(&mut list, Sort::Score, (100.0, 100.0));
        assert_eq!(list, vec![far.clone(), near.clone()]);
        sort(&mut list, Sort::Largest, (0.0, 0.0));
        assert_eq!(list, vec![near, far]);
    }

    #[test]
    fn items_round_trip_and_report_fields() {
        use crate::input::value::Evaluated;
        let d = Detection { id: Some(3), ..boxed(812.04, 401.0, 64.0, 128.0, "player", 0.8712) };
        let item = d.to_item();
        assert_eq!(item, r#"{"x":812.0,"y":401.0,"w":64.0,"h":128.0,"label":"player","score":0.871,"id":3}"#);
        let back = Detection::from_item(&item).unwrap();
        assert_eq!(back.field("center x"), Ok(Evaluated::Number(844.0)));
        assert_eq!(back.field("center y"), Ok(Evaluated::Number(465.0)));
        assert_eq!(back.field("score"), Ok(Evaluated::Number(87.1)));
        assert_eq!(back.field("label"), Ok(Evaluated::Text("player".into())));
        assert_eq!(back.field("id"), Ok(Evaluated::Number(3.0)));
        assert!(back.field("speed").is_err());
        assert!(Detection::from_item("").is_err());
        assert!(Detection::from_item("hello").is_err());
    }
}
