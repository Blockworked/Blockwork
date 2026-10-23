//! The web session on its own: built without `system`, as the page links it.

use std::ffi::{CStr, CString, c_char};

use blockstitch_ffi::Host;
use blockwork_web::Demo;
use serde_json::{Value, json};

const SAVED: &str = include_str!("../../blockwork-core/tests/fixtures/editor-baseline/macro.json");

fn loaded() -> Demo {
    let mut demo = Demo::default();
    let saved: Value = serde_json::from_str(SAVED).unwrap();
    demo.dispatch("load", &json!({ "macros": [saved.clone(), saved] })).unwrap();
    demo
}

#[test]
fn loads_saved_macros_through_the_apps_migrations() {
    let demo = loaded();
    let state = demo.state();
    assert_eq!(state["macro_selected"], 0);
    assert_eq!(state["macro_names"].as_array().unwrap().len(), 2);
    assert!(!state["current_macro"]["strands"].as_array().unwrap().is_empty());
    assert_eq!(state["can_undo"], false);
}

#[test]
fn a_bad_file_fails_the_load_and_keeps_what_was_open() {
    let mut demo = loaded();
    assert!(demo.dispatch("load", &json!({ "macros": ["not a macro"] })).is_err());
    assert_eq!(demo.state()["macro_names"].as_array().unwrap().len(), 2);
}

#[test]
fn host_reporters_say_they_need_the_app() {
    let demo = loaded();
    let op = |name: &str| json!({ "value": { "kind": "Op", "op": name, "args": [], "saved": { "kind": "Number", "value": 0 } } });
    for name in ["BatteryPercentage", "ClipboardText", "ActiveWindowTitle"] {
        let error = Demo::dispatch(&mut loaded(), "preview_value", &op(name)).unwrap_err();
        assert!(error.contains("Blockwork app"), "{name}: {error}");
    }
    let mut demo = demo;
    assert!(demo.dispatch("preview_value", &op("Timestamp")).is_ok());
}

#[test]
fn unknown_commands_are_refused_by_name() {
    let mut demo = loaded();
    let error = demo.dispatch("run_macro", &json!({})).unwrap_err();
    assert!(error.contains("run_macro"));
    for command in blockwork_web::COMMANDS {
        let answer = demo.dispatch(command, &json!({}));
        assert!(!matches!(&answer, Err(e) if e.starts_with("unsupported command")), "{command}");
    }
}

unsafe extern "C" {
    fn bs_abi_version() -> u32;
    fn bs_session_new() -> u32;
    fn bs_session_free(handle: u32);
    fn bs_dispatch(handle: u32, command: *const c_char) -> *mut c_char;
    fn bs_string_free(text: *mut c_char);
}

fn call(handle: u32, line: &str) -> Value {
    let line = CString::new(line).unwrap();
    unsafe {
        let raw = bs_dispatch(handle, line.as_ptr());
        let text = CStr::from_ptr(raw).to_string_lossy().into_owned();
        bs_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

#[test]
fn the_c_abi_answers_in_the_shared_envelope() {
    unsafe {
        assert_eq!(bs_abi_version(), blockstitch_ffi::ABI_VERSION);
        let handle = bs_session_new();
        let created = call(handle, r#"{"command":"new_macro"}"#);
        assert_eq!(created, json!({ "ok": true, "result": null, "error": null }));
        let strand = call(handle, r#"{"command":"add_strand","args":{"x":1,"y":2}}"#);
        assert!(strand["result"].is_string());
        let state = call(handle, r#"{"command":"state"}"#);
        assert_eq!(state["result"]["can_undo"], true);
        let refused = call(handle, r#"{"command":"start_recording"}"#);
        assert_eq!(refused["ok"], false);
        assert_eq!(call(handle, "not json")["ok"], false);
        bs_session_free(handle);
        assert_eq!(call(handle, r#"{"command":"state"}"#)["error"], "no such session");
    }
}
