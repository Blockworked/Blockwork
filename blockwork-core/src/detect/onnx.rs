//! Built-in detection: YOLOv8-style ONNX models run in-process with tract.

use super::yolo::{self, Letterbox};
use super::{Detection, Detector};
use crate::screen::Image;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tract_onnx::prelude::*;

/// Input size for models that don't fix one.
const DEFAULT_SIZE: usize = 640;
/// Boxes below this never leave the model; blocks filter further.
const FLOOR_SCORE: f32 = 0.05;

type Plan = TypedRunnableModel;

pub struct OnnxDetector {
    plan: Arc<Plan>,
    size: usize,
    labels: Arc<Vec<String>>,
}

/// Loaded models by path; loading and optimizing takes a while.
static LOADED: Mutex<Option<HashMap<PathBuf, (Arc<Plan>, usize, Arc<Vec<String>>)>>> = Mutex::new(None);

impl OnnxDetector {
    pub fn load(path: &Path) -> Result<OnnxDetector, String> {
        let mut cache = LOADED.lock().unwrap_or_else(|e| e.into_inner());
        let cache = cache.get_or_insert_with(HashMap::new);
        if let Some((plan, size, labels)) = cache.get(path) {
            return Ok(OnnxDetector { plan: Arc::clone(plan), size: *size, labels: Arc::clone(labels) });
        }
        let (plan, size, labels) = build(path).map_err(|e| format!("couldn't load the model {}: {e}", path.display()))?;
        let labels = Arc::new(labels);
        cache.insert(path.to_path_buf(), (Arc::clone(&plan), size, Arc::clone(&labels)));
        Ok(OnnxDetector { plan, size, labels })
    }
}

fn build(path: &Path) -> TractResult<(Arc<Plan>, usize, Vec<String>)> {
    let model = tract_onnx::onnx().model_for_path(path)?;
    let names = model
        .properties
        .get("onnx.metadata_props.names")
        .and_then(|t| t.try_as_plain_ram().ok()?.to_scalar::<String>().ok().cloned())
        .map(|text| yolo::parse_names(&text))
        .unwrap_or_default();
    // Use the model's own square input size when it has one.
    let fixed = model.input_fact(0)?.shape.as_concrete_finite()?.and_then(|dims| dims.get(3).copied());
    let size = fixed.unwrap_or(DEFAULT_SIZE);
    let plan = model
        .with_input_fact(0, f32::fact([1, 3, size, size]).into())?
        .into_optimized()?
        .into_runnable()?;
    Ok((plan, size, names))
}

impl Detector for OnnxDetector {
    fn detect(&self, image: &Image) -> Result<Vec<Detection>, String> {
        let (input, lb): (Vec<f32>, Letterbox) = yolo::letterbox(image, self.size as u32);
        let tensor = Tensor::from_shape(&[1, 3, self.size, self.size], &input).map_err(|e| e.to_string())?;
        let outputs = self.plan.run(tvec!(tensor.into_tvalue())).map_err(|e| format!("the model failed: {e}"))?;
        let output = outputs.first().ok_or("the model gave no output")?;
        let view = output.to_plain_array_view::<f32>().map_err(|e| e.to_string())?;
        let shape = view.shape().to_vec();
        let data: Vec<f32> = view.iter().copied().collect();
        yolo::decode(&data, &shape, &self.labels, FLOOR_SCORE, lb)
    }
}

/// A real model on a real picture; run by hand with `--ignored`, with
/// `BLOCKWORK_TEST_MODEL` (a YOLOv8 .onnx) and `BLOCKWORK_TEST_IMAGE` (a PNG).
#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore]
    fn runs_a_yolo_model() {
        let model = std::env::var("BLOCKWORK_TEST_MODEL").expect("set BLOCKWORK_TEST_MODEL");
        let image = Image::load(&std::env::var("BLOCKWORK_TEST_IMAGE").expect("set BLOCKWORK_TEST_IMAGE")).unwrap();
        let started = std::time::Instant::now();
        let detector = OnnxDetector::load(Path::new(&model)).unwrap();
        eprintln!("loaded in {:?}, input {}, {} labels", started.elapsed(), detector.size, detector.labels.len());
        let mut found = Vec::new();
        for _ in 0..3 {
            let started = std::time::Instant::now();
            found = detector.detect(&image).unwrap();
            eprintln!("detected in {:?}", started.elapsed());
        }
        for d in found.iter().filter(|d| d.score >= 0.25) {
            eprintln!("{} {:.2} at {:.0},{:.0} {:.0}x{:.0}", d.label, d.score, d.x, d.y, d.w, d.h);
        }
        assert!(found.iter().any(|d| d.label == "person" && d.score >= 0.25), "{found:?}");
    }
}
