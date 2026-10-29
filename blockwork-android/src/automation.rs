//! Requests are completed asynchronously by Android's main looper. Rust
//! strands wait here, never on the UI thread or with the app-state lock held.
use blockwork_core::input::{
    touch::{Gesture, TouchAction},
    types::{Axis, Direction, MacroButton, MacroKey},
};
use blockwork_core::macros::backend::InputBackend;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

enum NativeReply {
    Json(Value),
    Screenshot(blockwork_core::screen::Shot),
}

#[derive(Default)]
struct Broker {
    ready: bool,
    next: u64,
    queued: VecDeque<Value>,
    replies: HashMap<u64, Option<Result<NativeReply, String>>>,
}
static BROKER: Mutex<Option<Broker>> = Mutex::new(None);
static CHANGED: Condvar = Condvar::new();

static SCHEDULE_UI_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn schedule_ui_ready(ready: bool) {
    SCHEDULE_UI_READY.store(ready, std::sync::atomic::Ordering::SeqCst);
}
pub fn schedule_ui_is_ready() -> bool {
    SCHEDULE_UI_READY.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn ready(ready: bool) -> Result<Value, String> {
    schedule_ui_ready(ready);
    let mut lock = BROKER.lock().map_err(|e| e.to_string())?;
    let broker = lock.get_or_insert_with(Broker::default);
    broker.ready = ready;
    if !ready {
        broker.queued.clear();
        for reply in broker.replies.values_mut() {
            *reply = Some(Err(
                "accessibility service disconnected or device locked".into()
            ));
        }
        blockwork_core::macros::run_registry::stop_all();
    }
    CHANGED.notify_all();
    drop(lock);
    blockwork_core::screen::invalidate();
    if let Ok(process) = super::process() {
        let mut services = Vec::new();
        for name in [
            "touch",
            "elements",
            "screenshots",
            "picker",
            "foreground_events",
            "app_launch",
        ] {
            services.push((
                name.into(),
                if ready {
                    blockwork_app::host::Capability::ready()
                } else {
                    blockwork_app::host::Capability::unavailable(
                        true,
                        "Enable accessibility and unlock the device",
                    )
                },
            ));
        }
        process.runtime.backend().update_host_services(services)?;
    }
    Ok(Value::Null)
}

pub fn take() -> Result<Value, String> {
    let mut lock = BROKER.lock().map_err(|e| e.to_string())?;
    Ok(Value::Array(
        lock.get_or_insert_with(Broker::default)
            .queued
            .drain(..)
            .collect(),
    ))
}

pub fn reply(id: u64, answer: Value) -> Result<Value, String> {
    finish(
        id,
        if answer["ok"].as_bool() == Some(true) {
            Ok(NativeReply::Json(answer["result"].clone()))
        } else {
            Err(answer["error"]
                .as_str()
                .unwrap_or("Android action failed")
                .into())
        },
    )
}

pub fn screenshot_reply(id: u64, width: u32, height: u32, pixels: &[i32]) -> Result<Value, String> {
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > 32_000_000
        || u64::from(width) * u64::from(height) != pixels.len() as u64
    {
        return finish(id, Err("Invalid Android screenshot dimensions".into()));
    }
    if *DISPLAY.lock().map_err(|e| e.to_string())? != (width, height) {
        return finish(
            id,
            Err("Screenshot size differs from the default display; capture again".into()),
        );
    }
    // Bitmap.getPixels exposes ARGB integers, independent of byte order/stride.
    let rgba = pixels
        .iter()
        .flat_map(|pixel| {
            let p = *pixel as u32;
            [(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]
        })
        .collect();
    finish(
        id,
        Ok(NativeReply::Screenshot(blockwork_core::screen::Shot {
            image: blockwork_core::screen::Image {
                width,
                height,
                rgba,
            },
            origin: (0.0, 0.0),
            scale: 1.0,
        })),
    )
}

fn finish(id: u64, answer: Result<NativeReply, String>) -> Result<Value, String> {
    let mut lock = BROKER.lock().map_err(|e| e.to_string())?;
    if let Some(slot) = lock
        .get_or_insert_with(Broker::default)
        .replies
        .get_mut(&id)
    {
        *slot = Some(answer);
        CHANGED.notify_all();
    }
    Ok(Value::Null)
}

fn request_native(
    operation: Value,
    timeout: Duration,
    stopped: &dyn Fn() -> bool,
) -> Result<NativeReply, String> {
    let mut lock = BROKER.lock().map_err(|e| e.to_string())?;
    let broker = lock.get_or_insert_with(Broker::default);
    if !broker.ready {
        return Err("enable the Blockwork accessibility service and unlock the device".into());
    }
    broker.next += 1;
    let id = broker.next;
    broker.replies.insert(id, None);
    broker
        .queued
        .push_back(json!({"id": id, "operation": operation}));
    let deadline = Instant::now() + timeout;
    loop {
        let broker = lock.as_mut().unwrap();
        if stopped() || Instant::now() >= deadline {
            broker.replies.remove(&id);
            broker.queued.retain(|queued| queued["id"] != id);
            broker
                .queued
                .push_back(json!({"id": id, "operation": {"kind": "cancel"}}));
            return Err(if stopped() {
                "Android operation cancelled"
            } else {
                "Android action timed out"
            }
            .into());
        }
        if let Some(Some(result)) = broker.replies.remove(&id) {
            return result;
        }
        // Keep the pending slot until Android replies.
        broker.replies.entry(id).or_insert(None);
        lock = CHANGED
            .wait_timeout(lock, Duration::from_millis(25))
            .map_err(|e| e.to_string())?
            .0;
    }
}

fn request(
    operation: Value,
    timeout: Duration,
    stopped: &dyn Fn() -> bool,
) -> Result<Value, String> {
    match request_native(operation, timeout, stopped)? {
        NativeReply::Json(value) => Ok(value),
        NativeReply::Screenshot(_) => Err("Unexpected screenshot reply".into()),
    }
}

static DISPLAY: Mutex<(u32, u32)> = Mutex::new((0, 0));
pub fn display_changed(width: u32, height: u32) -> Result<Value, String> {
    *DISPLAY.lock().map_err(|e| e.to_string())? = (width, height);
    blockwork_core::screen::invalidate();
    Ok(Value::Null)
}

pub struct AndroidScreen;
impl blockwork_core::screen::ScreenBackend for AndroidScreen {
    fn capture(&self) -> Result<blockwork_core::screen::Shot, String> {
        match request_native(
            json!({"kind": "screenshot"}),
            Duration::from_secs(8),
            &|| false,
        )? {
            NativeReply::Screenshot(shot) => Ok(shot),
            NativeReply::Json(_) => Err("Android did not return screenshot pixels".into()),
        }
    }
    fn monitors(&self) -> Vec<blockwork_core::screen::Rect> {
        let (width, height) = *DISPLAY.lock().unwrap_or_else(|e| e.into_inner());
        if width == 0 || height == 0 {
            return vec![];
        }
        vec![blockwork_core::screen::Rect {
            x: 0.0,
            y: 0.0,
            width: width as f64,
            height: height as f64,
        }]
    }
}

pub struct AndroidInput;
const DESKTOP: &str =
    "desktop keyboard and mouse operations are unavailable on Android; use touch or element blocks";
impl InputBackend for AndroidInput {
    fn launch_android_app(
        &mut self,
        package: &str,
        stopped: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        blockwork_core::input::android_app::validate_package(package)?;
        request(
            json!({"kind": "launch_app", "package": package}),
            Duration::from_secs(5),
            stopped,
        )
        .map(|_| ())
    }

    fn element(
        &mut self,
        selector: &blockwork_core::input::element::ElementSelector,
        action: blockwork_core::input::element::ElementAction,
        text: &str,
        stopped: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        selector.validate()?;
        request(
            json!({"kind": "element", "selector": selector, "action": action, "text": text}),
            Duration::from_secs(5),
            stopped,
        )
    }

    fn gesture(&mut self, gesture: &Gesture, stopped: &dyn Fn() -> bool) -> Result<(), String> {
        request(
            json!({"kind": "gesture", "gesture": gesture}),
            Duration::from_millis(gesture.duration_ms + 5000),
            stopped,
        )
        .map(|_| ())
    }
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: MacroButton,
        stopped: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        if button != MacroButton::Left {
            return Err(
                "Android image clicks support a tap; choose the left/default button".into(),
            );
        }
        self.gesture(
            &Gesture {
                action: TouchAction::Tap,
                x: x as f64,
                y: y as f64,
                end_x: x as f64,
                end_y: y as f64,
                duration_ms: 50,
            },
            stopped,
        )
    }
    fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> {
        Err(DESKTOP.into())
    }
    fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> {
        Err(DESKTOP.into())
    }
    fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> {
        Err(DESKTOP.into())
    }
    fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
        Err(DESKTOP.into())
    }
    fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> {
        Err(DESKTOP.into())
    }
    fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> {
        Err(DESKTOP.into())
    }
    fn text(&mut self, text: &str) -> Result<(), String> {
        request(
            json!({"kind": "text", "text": text}),
            Duration::from_secs(5),
            &|| false,
        )
        .map(|_| ())
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
}

pub fn install_reporters() {
    blockwork_core::input::element::set_query_handler(std::sync::Arc::new(|selector, action| {
        request(
            json!({"kind": "element", "selector": selector, "action": action, "text": ""}),
            Duration::from_secs(5),
            &|| false,
        )
    }));
}
