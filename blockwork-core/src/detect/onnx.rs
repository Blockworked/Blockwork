//! Built-in detection: YOLOv8-style ONNX models run in-process with tract,
//! on the GPU through CUDA when it's there (the `gpu` feature).

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

type Plan = dyn Runnable;

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
    let model = model.with_input_fact(0, f32::fact([1, 3, size, size]).into())?.into_typed()?.into_decluttered()?;
    let plan: Arc<Plan> = prepare(model)?.into();
    // The first run sets the device up (and compiles GPU kernels); do it now
    // rather than on a macro's first frame.
    plan.run(tvec!(Tensor::zero::<f32>(&[1, 3, size, size])?.into_tvalue()))?;
    Ok((plan, size, names))
}

/// The GPU runtime when CUDA works, else the CPU.
fn prepare(model: TypedModel) -> TractResult<Box<Plan>> {
    #[cfg(all(feature = "gpu", any(target_os = "linux", windows)))]
    {
        // Linked for its runtime registration.
        use tract_cuda as _;
        match runtime_for_name("cuda") {
            Ok(Some(gpu)) => {
                tracing::info!("preparing the detection model on the GPU (the very first time compiles kernels, about 30 s)");
                match gpu.prepare(model.clone()) {
                    Ok(plan) => {
                        tracing::info!("detection models run on the GPU (CUDA)");
                        return Ok(plan);
                    }
                    Err(e) => tracing::info!("detection models run on the CPU: CUDA failed: {e:#}"),
                }
            }
            Ok(None) => tracing::info!("detection models run on the CPU: no CUDA runtime"),
            Err(e) => tracing::info!("detection models run on the CPU: {e:#}"),
        }
    }
    runtime_for_name("cpu")?.ok_or_else(|| TractError::msg("no CPU runtime"))?.prepare(model)
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
/// `BLOCKWORK_TEST_MODEL` (a YOLOv8 .onnx), `BLOCKWORK_TEST_IMAGE` (a PNG)
/// and optionally `BLOCKWORK_TEST_LABEL` (what must be found; default person).
#[cfg(test)]
mod live {
    use super::*;

    /// Smoke-test a downloaded model without requiring a screenshot fixture.
    #[test]
    #[ignore]
    fn loads_and_runs_a_model() {
        let model = std::env::var("BLOCKWORK_TEST_MODEL").expect("set BLOCKWORK_TEST_MODEL");
        let detector = OnnxDetector::load(Path::new(&model)).unwrap();
        assert!(detector.size > 0);
        assert!(!detector.labels.is_empty(), "the model must include label metadata");
        let image = Image::new(detector.size as u32, detector.size as u32);
        let found = detector.detect(&image).unwrap();
        assert!(found.iter().all(|d| d.x.is_finite() && d.y.is_finite()
            && d.w.is_finite() && d.h.is_finite() && d.score.is_finite()));
        eprintln!("loaded and ran {model}: input {}, labels {:?}, {} detections on a blank frame",
            detector.size, detector.labels, found.len());
    }

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
        let want = std::env::var("BLOCKWORK_TEST_LABEL").unwrap_or("person".into());
        assert!(found.iter().any(|d| d.label == want && d.score >= 0.25), "{found:?}");
    }
}

/// Per-stage timings on a big frame; run by hand with `--ignored --nocapture`
/// and the same env vars as `live`.
#[cfg(test)]
mod speed {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn stage_timings() {
        let model = std::env::var("BLOCKWORK_TEST_MODEL").expect("set BLOCKWORK_TEST_MODEL");
        let small = Image::load(&std::env::var("BLOCKWORK_TEST_IMAGE").expect("set BLOCKWORK_TEST_IMAGE")).unwrap();
        // Upscaled to a 1440p window, like a real capture.
        let (w, h) = (2560u32, 1440u32);
        let mut big = Image::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p = small.pixel(x * small.width / w, y * small.height / h);
                let i = ((y * w + x) * 4) as usize;
                big.rgba[i..i + 3].copy_from_slice(&p);
            }
        }
        let detector = OnnxDetector::load(Path::new(&model)).unwrap();
        let time = |name: &str, f: &mut dyn FnMut()| {
            f();
            let started = Instant::now();
            for _ in 0..50 {
                f();
            }
            eprintln!("{name}: {:.1} ms", started.elapsed().as_secs_f64() * 20.0);
        };
        time("letterbox 1440p", &mut || drop(yolo::letterbox(&big, detector.size as u32)));
        time("detect 1440p", &mut || drop(detector.detect(&big).unwrap()));
    }
}
