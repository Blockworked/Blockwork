//! One runtime per application process, loaded independently of Qt. The
//! Application starts it; the accessibility service and editor share it.
//! All C entry points catch panics and return owned UTF-8 JSON envelopes.

mod automation;
#[cfg(target_os = "android")]
mod jni;
mod journal;
mod prompts;

use blockwork_app::{
    Event,
    embedded::{EditorSession, EmbeddedRuntime},
    host::HostConfig,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

struct Process {
    root: PathBuf,
    runtime: EmbeddedRuntime,
}

static PROCESS: OnceLock<Result<Process, String>> = OnceLock::new();
static EDITORS: Mutex<Option<HashMap<u64, EditorSession>>> = Mutex::new(None);
static NEXT_EDITOR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn process() -> Result<&'static Process, String> {
    PROCESS
        .get()
        .ok_or("Android runtime has not started")?
        .as_ref()
        .map_err(Clone::clone)
}

fn start(root: PathBuf) -> Result<Value, String> {
    if !root.is_absolute() {
        return Err("Android storage directory must be absolute".into());
    }
    let result = PROCESS
        .get_or_init(|| {
            let mut host = HostConfig::android();
            host.prompt = Some(std::sync::Arc::new(prompts::ask));
            host.input = Some(std::sync::Arc::new(Mutex::new(automation::AndroidInput)));
            let runtime = EmbeddedRuntime::start(root.clone(), host)?;
            journal::install(&root)?;
            runtime
                .backend()
                .configure_android_alarms(root.join("android-alarms.json"))?;
            automation::install_reporters();
            blockwork_core::screen::install_backend(std::sync::Arc::new(
                automation::AndroidScreen,
            ))?;
            Ok(Process {
                root: root.clone(),
                runtime,
            })
        })
        .as_ref()
        .map_err(Clone::clone)?;
    if result.root != root {
        return Err("runtime already started with another storage directory".into());
    }
    Ok(json!({"started": true}))
}

fn call(command: &str, args: &str) -> Result<Value, String> {
    process()?.runtime.call(
        command,
        serde_json::from_str(args).map_err(|e| format!("invalid arguments: {e}"))?,
    )
}

fn attach() -> Result<Value, String> {
    let session = process()?.runtime.attach()?;
    let id = NEXT_EDITOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    EDITORS
        .lock()
        .map_err(|e| e.to_string())?
        .get_or_insert_with(HashMap::new)
        .insert(id, session);
    Ok(json!(id))
}

fn poll(id: u64) -> Result<Value, String> {
    let mut editors = EDITORS.lock().map_err(|e| e.to_string())?;
    let session = editors
        .as_mut()
        .and_then(|sessions| sessions.get_mut(&id))
        .ok_or("editor is detached")?;
    let mut events = Vec::new();
    // Bound each GUI tick; leave the rest queued for the next poll.
    for _ in 0..64 {
        let Some(event) = session.poll()? else { break };
        events.push(match event {
            Event::State(state) => json!({"kind": "state", "data": serde_json::from_str::<Value>(&state).map_err(|e| e.to_string())?}),
            Event::Trace(trace) => json!({"kind": "trace", "data": trace}),
            Event::Quit => json!({"kind": "quit"}),
            Event::CloseToTray(_) => continue,
        });
    }
    Ok(Value::Array(events))
}

fn detach(id: u64) -> Result<Value, String> {
    if let Some(editors) = EDITORS.lock().map_err(|e| e.to_string())?.as_mut() {
        editors.remove(&id);
    }
    Ok(Value::Null)
}

fn envelope(action: impl FnOnce() -> Result<Value, String>) -> String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(Ok(result)) => json!({"ok": true, "result": result}).to_string(),
        Ok(Err(error)) => json!({"ok": false, "error": error}).to_string(),
        Err(_) => json!({"ok": false, "error": "Android runtime panic"}).to_string(),
    }
}

fn owned(action: impl FnOnce() -> Result<Value, String>) -> *mut c_char {
    CString::new(envelope(action))
        .expect("JSON escapes NUL")
        .into_raw()
}

unsafe fn text<'a>(ptr: *const c_char) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err("null string".into());
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|e| e.to_string())
}

/// # Safety
/// `root` must point to a NUL-terminated UTF-8 string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blockwork_android_start(root: *const c_char) -> *mut c_char {
    owned(|| start(PathBuf::from(unsafe { text(root)? })))
}

/// # Safety
/// Both strings must be valid NUL-terminated UTF-8 for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blockwork_android_call(
    command: *const c_char,
    args: *const c_char,
) -> *mut c_char {
    owned(|| call(unsafe { text(command)? }, unsafe { text(args)? }))
}

#[unsafe(no_mangle)]
pub extern "C" fn blockwork_android_attach() -> *mut c_char {
    owned(attach)
}

#[unsafe(no_mangle)]
pub extern "C" fn blockwork_android_poll(id: u64) -> *mut c_char {
    owned(|| poll(id))
}

#[unsafe(no_mangle)]
pub extern "C" fn blockwork_android_detach(id: u64) -> *mut c_char {
    owned(|| detach(id))
}

/// # Safety
/// `ptr` must be null or an unreleased string returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blockwork_android_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn process_runtime_survives_editor_detach_and_routes_cancellable_prompts() {
        let root = std::env::temp_dir().join(format!("blockwork-android-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // Application and service can race at startup; they must get the same
        // backend and storage, never duplicate watchers or macro catalogs.
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let root = &root;
                scope.spawn(move || {
                    start(root.clone()).unwrap();
                });
            }
        });
        assert!(start(root.join("other")).is_err());
        assert!(
            call("start_recording", "{}")
                .unwrap_err()
                .contains("unavailable")
        );
        let state = call("get_state", "{}").unwrap();
        assert_eq!(state["capabilities"]["platform"], "android");
        assert_eq!(
            state["capabilities"]["services"]["recording"]["supported"],
            false
        );
        assert_eq!(state["grab_available"], false);

        call("new_macro", "{}").unwrap();
        call("set_title", r#"{"title":"Embedded workflow"}"#).unwrap();
        call("create_variable", r#"{"name":"answer"}"#).unwrap();
        let strand = call(
            "add_strand",
            &json!({"x": 10, "y": 20,
            "instruction": {"id": "entry", "type": "WhenRan"}})
            .to_string(),
        )
        .unwrap();
        for (index, instruction) in [
            json!({"id": "wait", "type": "Wait", "duration": {"kind": "Number", "value": 300.0}}),
            json!({"id": "set", "type": "SetVariable", "name": "answer", "value": {"kind": "Number", "value": 42.0}}),
        ].into_iter().enumerate() {
            call("add_instruction", &json!({"strandId": strand, "path": [{"index": index + 1, "slot": null}], "instruction": instruction}).to_string()).unwrap();
        }
        let editor = attach().unwrap().as_u64().unwrap();
        let first = poll(editor).unwrap();
        assert_eq!(first[0]["kind"], "state");
        call("run_macro", "{}").unwrap();
        detach(editor).unwrap();
        assert!(poll(editor).is_err());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let preview = call(
                "preview_value",
                r#"{"value":{"kind":"Var","name":"answer"}}"#,
            )
            .unwrap();
            if preview == "42" && blockwork_core::macros::run_registry::count() == 0 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "workflow did not finish after editor detached: {preview}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let reattached = attach().unwrap().as_u64().unwrap();
        let state = poll(reattached).unwrap()[0]["data"].clone();
        assert_eq!(state["current_macro"]["name"], "Embedded workflow");
        // Run deregistration precedes the final disk write. Wait for that
        // observable result rather than racing the persistence thread.
        loop {
            let saved = blockwork_core::config::read_macro_by_id(
                state["current_macro"]["id"].as_str().unwrap(),
            );
            if saved.is_some_and(|mac| {
                mac.variables
                    .iter()
                    .any(|v| v.name == "answer" && v.value.as_text() == "42")
            }) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "workflow result was not persisted"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        detach(reattached).unwrap();

        let request = blockwork_core::prompt::Request::new(
            blockwork_core::prompt::Kind::Text,
            "Question",
            "Answer",
        );
        let stopped = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopped);
        let waiter =
            std::thread::spawn(move || prompts::ask(&request, &|| flag.load(Ordering::Relaxed)));
        let deadline = Instant::now() + Duration::from_secs(2);
        let prompt = loop {
            let pending = prompts::pending().unwrap();
            if !pending.is_null() {
                break pending;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        stopped.store(true, Ordering::Relaxed);
        assert_eq!(waiter.join().unwrap().unwrap_err(), "stopped");
        assert!(prompts::pending().unwrap().is_null());
        assert!(prompts::answer(prompt["id"].as_u64().unwrap(), Default::default()).is_err());

        // A gesture is not complete until Android's callback arrives; a
        // concurrent strand cannot submit another gesture in that interval.
        use blockwork_core::input::touch::{Gesture, TouchAction};
        use blockwork_core::macros::backend::InputBackend;
        automation::ready(true).unwrap();
        let input = Arc::new(Mutex::new(automation::AndroidInput));
        let gesture = Gesture {
            action: TouchAction::Tap,
            x: 10.0,
            y: 20.0,
            end_x: 10.0,
            end_y: 20.0,
            duration_ms: 50,
        };
        let wait_action = || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let actions = automation::take().unwrap();
                if !actions.as_array().unwrap().is_empty() {
                    return actions[0].clone();
                }
                assert!(Instant::now() < deadline, "no Android action arrived");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        // Screen callers share one host capture, preserve physical pixels,
        // and never reuse a frame after revocation/rotation.
        automation::display_changed(2, 2).unwrap();
        assert_eq!(blockwork_core::screen::desktop().unwrap().width, 2.0);
        std::thread::scope(|scope| {
            let color = scope.spawn(|| blockwork_core::screen::pixel_color(1.0, 0.0));
            let average = scope.spawn(|| blockwork_core::screen::average_color(0.0, 0.0, 2.0, 2.0));
            let request = wait_action();
            assert_eq!(request["operation"]["kind"], "screenshot");
            automation::screenshot_reply(
                request["id"].as_u64().unwrap(),
                2,
                2,
                &[
                    0xffff0000u32 as i32,
                    0xff00ff00u32 as i32,
                    0xff0000ffu32 as i32,
                    0xffffffffu32 as i32,
                ],
            )
            .unwrap();
            assert_eq!(color.join().unwrap().unwrap(), "#00ff00");
            assert_eq!(average.join().unwrap().unwrap(), "#7f7f7f");
            assert!(automation::take().unwrap().as_array().unwrap().is_empty());
        });
        blockwork_core::screen::invalidate();
        std::thread::scope(|scope| {
            let capture = scope.spawn(blockwork_core::screen::capture);
            let request = wait_action();
            automation::ready(false).unwrap();
            assert!(capture.join().unwrap().is_err());
            // A late callback cannot populate the cache after service loss.
            automation::screenshot_reply(
                request["id"].as_u64().unwrap(),
                1,
                1,
                &[0xffffffffu32 as i32],
            )
            .unwrap();
            assert!(blockwork_core::screen::capture().is_err());
        });
        automation::ready(true).unwrap();
        std::thread::scope(|scope| {
            let capture = scope.spawn(blockwork_core::screen::capture);
            let request = wait_action();
            automation::display_changed(4, 2).unwrap();
            automation::screenshot_reply(request["id"].as_u64().unwrap(), 4, 2, &[0; 8]).unwrap();
            assert!(
                capture
                    .join()
                    .unwrap()
                    .err()
                    .unwrap()
                    .contains("geometry changed")
            );
        });
        std::thread::scope(|scope| {
            let capture = scope.spawn(blockwork_core::screen::capture);
            let request = wait_action();
            automation::screenshot_reply(request["id"].as_u64().unwrap(), 2, 2, &[0; 4]).unwrap();
            assert!(
                capture
                    .join()
                    .unwrap()
                    .err()
                    .unwrap()
                    .contains("size differs")
            );
        });
        std::thread::scope(|scope| {
            let color = scope.spawn(|| blockwork_core::screen::pixel_color(0.0, 0.0));
            let request = wait_action();
            automation::reply(
                request["id"].as_u64().unwrap(),
                json!({"ok": false, "error": "Secure content blocked"}),
            )
            .unwrap();
            assert_eq!(color.join().unwrap().unwrap_err(), "Secure content blocked");
        });
        std::thread::scope(|scope| {
            let first = scope.spawn(|| input.lock().unwrap().gesture(&gesture, &|| false));
            let a = wait_action();
            let second = scope.spawn(|| input.lock().unwrap().gesture(&gesture, &|| false));
            assert!(automation::take().unwrap().as_array().unwrap().is_empty());
            assert!(!first.is_finished());
            automation::reply(a["id"].as_u64().unwrap(), json!({"ok": true})).unwrap();
            first.join().unwrap().unwrap();
            let b = wait_action();
            assert_ne!(a["id"], b["id"]);
            automation::reply(b["id"].as_u64().unwrap(), json!({"ok": true})).unwrap();
            second.join().unwrap().unwrap();
        });
        use blockwork_core::input::element::{ElementAction, ElementSelector};
        let selector = ElementSelector {
            package: "fixture.app".into(),
            resource_id: "fixture.app:id/name".into(),
            ..Default::default()
        };
        for (action, value) in [
            (ElementAction::Exists, json!(false)),
            (ElementAction::ReadText, json!("fresh text")),
        ] {
            std::thread::scope(|scope| {
                let reading = scope.spawn(|| {
                    input
                        .lock()
                        .unwrap()
                        .element(&selector, action, "", &|| false)
                });
                let request = wait_action();
                assert_eq!(request["operation"]["selector"]["package"], "fixture.app");
                automation::reply(
                    request["id"].as_u64().unwrap(),
                    json!({"ok": true, "result": value}),
                )
                .unwrap();
                assert_eq!(reading.join().unwrap().unwrap(), value);
            });
        }
        for action in [
            ElementAction::Click,
            ElementAction::LongClick,
            ElementAction::ScrollForward,
            ElementAction::ScrollBackward,
            ElementAction::SetText,
            ElementAction::Exists,
            ElementAction::ReadText,
            ElementAction::Wait,
        ] {
            let instruction = blockwork_core::macros::Instruction::new(
                blockwork_core::macros::InstructionKind::Element {
                    action,
                    selector: selector.clone(),
                    text: blockwork_core::input::value::Value::text("replacement"),
                    timeout: blockwork_core::input::value::Value::number(1000.0),
                    variable: "answer".into(),
                },
            );
            let dto = blockwork_core::frontend::instruction_to_dto(&instruction);
            assert_eq!(
                blockwork_core::frontend::dto_to_instruction(&dto)
                    .unwrap()
                    .kind,
                instruction.kind
            );
            assert_eq!(
                serde_json::from_value::<blockwork_core::macros::Instruction>(
                    serde_json::to_value(&instruction).unwrap()
                )
                .unwrap()
                .kind,
                instruction.kind
            );
        }
        // Exercise the actual runner's wait loop: absence must retry with a
        // fresh request, then the following text read must publish its result.
        call("new_macro", "{}").unwrap();
        call("create_variable", r#"{"name":"element-result"}"#).unwrap();
        let element_strand = call(
            "add_strand",
            &json!({"x": 0, "y": 0, "instruction": {"id": "element-entry", "type": "WhenRan"}})
                .to_string(),
        )
        .unwrap();
        for (index, action) in ["Wait", "ReadText"].into_iter().enumerate() {
            call("add_instruction", &json!({"strandId": element_strand, "path": [{"index": index+1, "slot": null}], "instruction": {
                "id": format!("element-{index}"), "type": "Element", "selector": selector, "action": action,
                "text": {"kind": "Text", "value": ""}, "timeout": {"kind": "Number", "value": 1000}, "variable": if action == "ReadText" {"element-result"} else {""}
            }}).to_string()).unwrap();
        }
        call("run_macro", "{}").unwrap();
        for value in [json!(false), json!(true), json!("resolved text")] {
            let request = wait_action();
            automation::reply(
                request["id"].as_u64().unwrap(),
                json!({"ok": true, "result": value}),
            )
            .unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if call(
                "preview_value",
                r#"{"value":{"kind":"Var","name":"element-result"}}"#,
            )
            .unwrap()
                == "resolved text"
                && blockwork_core::macros::run_registry::count() == 0
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "element result was not published"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::scope(|scope| {
            let reading = scope.spawn(|| call("preview_value", &json!({"value": {"kind": "Op", "op": "AndroidElementExists", "args": [
                {"kind":"Text","value":"fixture.app"}, {"kind":"Text","value":"fixture.app:id/name"},
                {"kind":"Text","value":""}, {"kind":"Text","value":""}, {"kind":"Text","value":""}
            ], "saved": {"kind":"Bool"}}}).to_string()));
            let request = wait_action();
            automation::reply(
                request["id"].as_u64().unwrap(),
                json!({"ok": true, "result": true}),
            )
            .unwrap();
            assert_eq!(reading.join().unwrap().unwrap(), "true");
        });
        // A stalled native reply must obey the block's timeout rather than
        // waiting for the broker's longer transport timeout.
        call("new_macro", "{}").unwrap();
        call("create_variable", r#"{"name":"wait-error"}"#).unwrap();
        let timed = call(
            "add_strand",
            &json!({"x":0,"y":0,"instruction":{"id":"timed-entry","type":"WhenRan"}}).to_string(),
        )
        .unwrap();
        call("add_instruction", &json!({"strandId": timed, "path":[{"index":1,"slot":null}], "instruction":{
            "id":"catch-timeout","type":"TryCatch","body":[{"id":"timed-wait","type":"Element","action":"Wait","selector":selector,
                "text":{"kind":"Text","value":""},"timeout":{"kind":"Number","value":50},"variable":""}],
            "catch_body":[{"id":"caught","type":"SetVariable","name":"wait-error","value":{"kind":"Text","value":"caught"}}]
        }}).to_string()).unwrap();
        call("run_macro", "{}").unwrap();
        let unanswered = wait_action();
        let cancel = wait_action();
        assert_eq!(cancel["id"], unanswered["id"]);
        assert_eq!(cancel["operation"]["kind"], "cancel");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if call(
                "preview_value",
                r#"{"value":{"kind":"Var","name":"wait-error"}}"#,
            )
            .unwrap()
                == "caught"
                && blockwork_core::macros::run_registry::count() == 0
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "element timeout was not catchable"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        stopped.store(false, Ordering::Relaxed);
        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| {
                input
                    .lock()
                    .unwrap()
                    .gesture(&gesture, &|| stopped.load(Ordering::Relaxed))
            });
            let request = wait_action();
            stopped.store(true, Ordering::Relaxed);
            assert!(waiting.join().unwrap().unwrap_err().contains("cancelled"));
            let cancel = wait_action();
            assert_eq!(cancel["id"], request["id"]);
            assert_eq!(cancel["operation"]["kind"], "cancel");
        });
        std::thread::scope(|scope| {
            let waiting = scope.spawn(|| input.lock().unwrap().gesture(&gesture, &|| false));
            let _ = wait_action();
            automation::ready(false).unwrap();
            assert!(
                waiting
                    .join()
                    .unwrap()
                    .unwrap_err()
                    .contains("disconnected")
            );
        });

        // Foreground headers are distinct from process events, persist in
        // desktop builds, and cannot run until a real transition is observed.
        use blockwork_core::input::value::Value as BlockValue;
        use blockwork_core::macros::{Instruction, InstructionKind};
        for kind in [
            InstructionKind::WhenForegroundApp {
                package: "fixture.target".into(),
                entered: false,
            },
            InstructionKind::LaunchAndroidApp {
                package: BlockValue::text("fixture.target"),
            },
        ] {
            let instruction = Instruction::new(kind);
            let dto = blockwork_core::frontend::instruction_to_dto(&instruction);
            assert_eq!(
                blockwork_core::frontend::dto_to_instruction(&dto)
                    .unwrap()
                    .kind,
                instruction.kind
            );
            let saved = serde_json::to_value(&instruction).unwrap();
            assert_eq!(
                serde_json::from_value::<Instruction>(saved).unwrap().kind,
                instruction.kind
            );
        }
        automation::ready(true).unwrap();
        call("new_macro", "{}").unwrap();
        for name in ["entries", "exits"] {
            call("create_variable", &json!({"name":name}).to_string()).unwrap();
        }
        for (entered, name) in [(true, "entries"), (false, "exits")] {
            let strand = call("add_strand", &json!({"x":0,"y":0,"instruction":{
                "id":format!("foreground-{name}"),"type":"WhenForegroundApp","package":"fixture.target","entered":entered}}).to_string()).unwrap();
            call("add_instruction", &json!({"strandId":strand,"path":[{"index":1,"slot":null}],"instruction":{
                "id":format!("count-{name}"),"type":"ChangeVariable","name":name,"value":{"kind":"Number","value":1}}}).to_string()).unwrap();
        }
        let backend = process().unwrap().runtime.backend();
        let wait_variable = |name: &str, expected: &str| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let value = call(
                    "preview_value",
                    &json!({"value":{"kind":"Var","name":name}}).to_string(),
                )
                .unwrap();
                if value == expected && blockwork_core::macros::run_registry::count() == 0 {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "{name}: expected {expected}, got {value}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.before"))
                .unwrap(),
            0
        );
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.target"))
                .unwrap(),
            1
        );
        wait_variable("entries", "1");
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.target"))
                .unwrap(),
            0
        );
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.other"))
                .unwrap(),
            1
        );
        wait_variable("exits", "1");
        backend.observe_foreground_app(None).unwrap();
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.target"))
                .unwrap(),
            0
        );
        automation::ready(false).unwrap();
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.other"))
                .unwrap(),
            0
        );
        automation::ready(true).unwrap();
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.other"))
                .unwrap(),
            0
        );
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.target"))
                .unwrap(),
            1
        );
        wait_variable("entries", "2");
        call("emergency_stop", "{}").unwrap();
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.other"))
                .unwrap(),
            0
        );
        call("resume_triggers", "{}").unwrap();
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.other"))
                .unwrap(),
            0
        );
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.target"))
                .unwrap(),
            1
        );
        wait_variable("entries", "3");
        // Imported desktop constraints cannot silently use Android's
        // unsupported battery/window fallbacks to start UI automation.
        for (power, window) in [
            ("OnBattery", ""),
            ("PluggedIn", ""),
            ("Any", "desktop window"),
        ] {
            call("set_macro_conditions", &json!({"onlyInWindow":window,"power":power,"activeFrom":"","activeUntil":"","activeDays":0}).to_string()).unwrap();
            assert_eq!(
                backend
                    .observe_foreground_app(Some("fixture.other"))
                    .unwrap(),
                0
            );
            assert_eq!(
                backend
                    .observe_foreground_app(Some("fixture.target"))
                    .unwrap(),
                0
            );
        }
        assert!(blockwork_core::run_log::snapshot().iter().any(|entry| {
            entry
                .message
                .contains("desktop power/window conditions are unavailable")
        }));
        call("set_macro_conditions", &json!({"onlyInWindow":"","power":"Any","activeFrom":"","activeUntil":"","activeDays":0}).to_string()).unwrap();
        let id = call("get_state", "{}").unwrap()["current_macro"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        blockwork_core::permissions::restrict(&id, vec![]);
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.other"))
                .unwrap(),
            1
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        let question = loop {
            let question = prompts::pending().unwrap();
            if !question.is_null() {
                break question;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(question["request"]["ability"], "screen");
        prompts::answer(
            question["id"].as_u64().unwrap(),
            blockwork_core::prompt::Reply {
                answer: "deny".into(),
                text: String::new(),
            },
        )
        .unwrap();
        wait_variable("exits", "1");
        // Service loss while permission is pending cancels the whole trigger.
        assert_eq!(
            backend
                .observe_foreground_app(Some("fixture.target"))
                .unwrap(),
            1
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if !prompts::pending().unwrap().is_null() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        automation::ready(false).unwrap();
        wait_variable("entries", "3");
        assert!(prompts::pending().unwrap().is_null());
        blockwork_core::permissions::trust(&id);
        automation::ready(true).unwrap();
        // Launches use only the Android package operation, including failures
        // caught by the real runner; no desktop command path is involved.
        std::thread::scope(|scope| {
            let launch = scope.spawn(|| {
                input
                    .lock()
                    .unwrap()
                    .launch_android_app("fixture.target", &|| false)
            });
            let request = wait_action();
            assert_eq!(
                request["operation"],
                json!({"kind":"launch_app","package":"fixture.target"})
            );
            automation::reply(
                request["id"].as_u64().unwrap(),
                json!({"ok":false,"error":"Package has no launcher"}),
            )
            .unwrap();
            assert!(launch.join().unwrap().unwrap_err().contains("no launcher"));
        });
        assert!(
            input
                .lock()
                .unwrap()
                .launch_android_app("intent://bad", &|| false)
                .is_err()
        );

        call("new_macro", "{}").unwrap();
        call("create_variable", r#"{"name":"launch-error"}"#).unwrap();
        let strand = call(
            "add_strand",
            &json!({"x":0,"y":0,"instruction":{"id":"launch-entry","type":"WhenRan"}}).to_string(),
        )
        .unwrap();
        call("add_instruction", &json!({"strandId":strand,"path":[{"index":1,"slot":null}],"instruction":{
            "id":"launch-catch","type":"TryCatch","body":[{"id":"launch","type":"LaunchAndroidApp","package":{"kind":"Text","value":"fixture.target"}}],
            "catch_body":[{"id":"launch-caught","type":"SetVariable","name":"launch-error","value":{"kind":"Text","value":"caught"}}]
        }}).to_string()).unwrap();
        call("run_macro", "{}").unwrap();
        let request = wait_action();
        assert_eq!(request["operation"]["kind"], "launch_app");
        automation::reply(
            request["id"].as_u64().unwrap(),
            json!({"ok":false,"error":"Package has no launcher"}),
        )
        .unwrap();
        wait_variable("launch-error", "caught");
        // Alarm delivery claims private history before the real runner starts.
        // Duplicate broadcasts never repeat a completed or skipped occurrence.
        let clock = 1_893_456_000_000i64; // 2030-01-01T00:00:00Z
        for ui in [false, true] {
            call("new_macro", "{}").unwrap();
            call("create_variable", r#"{"name":"alarm-result"}"#).unwrap();
            let strand = call("add_strand", &json!({"x":0,"y":0,"instruction":{"id":"alarm-header","type":"WhenAt","at":{"kind":"Text","value":"2030-01-01T00:01:00Z"}}}).to_string()).unwrap();
            if ui {
                call("add_instruction", &json!({"strandId":strand,"path":[{"index":1,"slot":null}],"instruction":{
                    "id":"scheduled-tap","type":"Touch","action":"Tap","x":{"kind":"Number","value":1},"y":{"kind":"Number","value":1},
                    "end_x":{"kind":"Number","value":1},"end_y":{"kind":"Number","value":1},"duration":{"kind":"Number","value":50}
                }}).to_string()).unwrap();
                call("set_macro_android_exact_time", r#"{"enabled":true}"#).unwrap();
                let blocked = backend.android_alarm_plan(clock, "UTC", false).unwrap();
                assert!(blocked.alarms.is_empty());
                assert!(!blocked.errors.is_empty());
            }
            call("add_instruction", &json!({"strandId":strand,"path":[{"index":if ui {2} else {1},"slot":null}],"instruction":{
                "id":"alarm-done","type":"SetVariable","name":"alarm-result","value":{"kind":"Text","value":"fired"}
            }}).to_string()).unwrap();
            let plan = backend.android_alarm_plan(clock, "UTC", true).unwrap();
            assert_eq!(plan.alarms.len(), 1);
            let alarm = &plan.alarms[0];
            assert_eq!(alarm.at_ms, clock + 60000);
            assert_eq!(alarm.exact, ui);
            if ui {
                assert!(
                    !backend
                        .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| false))
                        .unwrap()
                );
                assert!(
                    !backend
                        .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                        .unwrap()
                );
                assert!(automation::take().unwrap().as_array().unwrap().is_empty());
            } else {
                assert!(
                    backend
                        .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                        .unwrap()
                );
                wait_variable("alarm-result", "fired");
                assert!(
                    !backend
                        .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                        .unwrap()
                );
            }
            assert!(
                backend
                    .android_alarm_plan(alarm.at_ms + 1, "UTC", true)
                    .unwrap()
                    .alarms
                    .is_empty()
            );
            // Accessibility readiness changes must preserve exact-access state.
            automation::ready(false).unwrap();
            assert_eq!(
                call("get_state", "{}").unwrap()["capabilities"]["services"]["exact_alarms"]["ready"],
                true
            );
            automation::ready(true).unwrap();
        }

        // Updated desktop builds must preserve Android blocks, including
        // fields with expressions, without converting them into notes.
        let mut mac = blockwork_core::macros::Macro::new("Touch".into(), "".into(), vec![]);
        let touch = blockwork_core::macros::Instruction::new(
            blockwork_core::macros::InstructionKind::Touch {
                action: TouchAction::Swipe,
                x: blockwork_core::input::value::Value::number(1.0),
                y: blockwork_core::input::value::Value::number(2.0),
                end_x: blockwork_core::input::value::Value::number(3.0),
                end_y: blockwork_core::input::value::Value::number(4.0),
                duration: blockwork_core::input::value::Value::number(500.0),
            },
        );
        let dto = blockwork_core::frontend::instruction_to_dto(&touch);
        assert_eq!(
            blockwork_core::frontend::dto_to_instruction(&dto)
                .unwrap()
                .kind,
            touch.kind
        );
        mac.strands.push(blockwork_core::macros::Strand {
            id: "touch".into(),
            x: 0,
            y: 0,
            instructions: vec![touch.clone()],
        });
        let saved = blockwork_core::macros::migrate::to_value(&mac).unwrap();
        let (restored, upgrade) = blockwork_core::macros::migrate::from_value(saved).unwrap();
        assert!(upgrade.notes.is_empty());
        assert!(
            restored
                .strands
                .iter()
                .flat_map(|s| &s.instructions)
                .any(|i| i.kind == touch.kind)
        );
    }
}
