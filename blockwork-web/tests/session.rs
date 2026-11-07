//! The web session on its own: built without `system`, as the page links it.

use std::ffi::{CStr, CString, c_char};

use blockstitch_ffi::Host;
use blockwork_web::Demo;
use serde_json::{Value, json};

const SAVED: &str = include_str!("../../blockwork-core/tests/fixtures/editor-baseline/macro.json");

fn loaded() -> Demo {
    let mut demo = Demo::default();
    let saved: Value = serde_json::from_str(SAVED).unwrap();
    demo.dispatch("load", &json!({ "macros": [saved.clone(), saved] }))
        .unwrap();
    demo
}

#[test]
fn loads_saved_macros_through_the_apps_migrations() {
    let demo = loaded();
    let state = demo.state();
    assert_eq!(state["macro_selected"], 0);
    assert_eq!(state["macro_names"].as_array().unwrap().len(), 2);
    assert!(
        !state["current_macro"]["strands"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(state["can_undo"], false);
}

#[test]
fn a_bad_file_fails_the_load_and_keeps_what_was_open() {
    let mut demo = loaded();
    assert!(
        demo.dispatch("load", &json!({ "macros": ["not a macro"] }))
            .is_err()
    );
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
    for command in blockwork_web::commands() {
        let answer = demo.dispatch(command, &json!({}));
        assert!(
            !matches!(&answer, Err(e) if e.starts_with("unsupported command")),
            "{command}"
        );
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
        assert_eq!(
            created,
            json!({ "ok": true, "result": null, "error": null })
        );
        let strand = call(handle, r#"{"command":"add_strand","args":{"x":1,"y":2}}"#);
        assert!(strand["result"].is_string());
        let state = call(handle, r#"{"command":"state"}"#);
        assert_eq!(state["result"]["can_undo"], true);
        let refused = call(handle, r#"{"command":"start_recording"}"#);
        assert_eq!(refused["ok"], false);
        assert_eq!(call(handle, "not json")["ok"], false);
        bs_session_free(handle);
        assert_eq!(
            call(handle, r#"{"command":"state"}"#)["error"],
            "no such session"
        );
    }
}

#[test]
fn blocks_can_be_dropped_into_a_reporters_branch() {
    use blockwork_core::input::value::Value as BlockValue;
    let mut demo = loaded();
    let call = BlockValue::Call {
        block_id: "custom".into(),
        args: vec![],
        branches: vec![],
        saved: Box::new(BlockValue::number(0.0)),
    };
    demo.dispatch(
        "create_floating_value",
        &json!({ "x": 0, "y": 0, "value": call }),
    )
    .unwrap();
    let floating = |demo: &Demo| {
        demo.state()["current_macro"]["floating_values"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone()
    };
    let id = floating(&demo)["id"].as_str().unwrap().to_string();
    let location = json!({ "kind": "Floating", "floating_id": id, "path": [] });
    let strand = format!(
        "@reporter-branch:{}",
        json!({ "location": location, "slot": 0 })
    );
    let wait =
        json!({ "id": "nested", "type": "Wait", "duration": { "kind": "Number", "value": 10 } });
    demo.dispatch(
        "add_instruction",
        &json!({ "strandId": strand, "path": [{ "index": 0 }], "instruction": wait }),
    )
    .unwrap();
    assert_eq!(floating(&demo)["value"]["branches"][0][0]["id"], "nested");
    demo.dispatch("undo", &json!({})).unwrap();
    assert!(
        floating(&demo)["value"]["branches"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// A palette drop can ask for its variable to be made, and a block dropped on
/// a strand's top keeps the strand where it landed, as in the app.
#[test]
fn a_drop_makes_its_variable_and_keeps_the_strands_top() {
    let mut demo = loaded();
    let wait = |id: &str| json!({ "id": id, "type": "Wait", "duration": { "kind": "Number", "value": 10 } });
    let strand = demo
        .dispatch(
            "add_strand",
            &json!({ "x": 0, "y": 0, "instruction": wait("first"), "variableToCreate": "made" }),
        )
        .unwrap();
    let strand = strand.as_str().unwrap().to_string();
    demo.dispatch(
        "add_instruction",
        &json!({ "strandId": strand, "path": [{ "index": 0 }], "instruction": wait("top"), "targetY": 432, "variableToCreate": "dropped" }),
    )
    .unwrap();
    let state = demo.state();
    let strands = state["current_macro"]["strands"].as_array().unwrap();
    let moved = strands.iter().find(|s| s["id"] == strand.as_str()).unwrap();
    assert_eq!(moved["y"], 432, "{moved}");
    let variables = state["current_macro"]["variables"].to_string();
    assert!(
        variables.contains("\"made\"") && variables.contains("\"dropped\""),
        "{variables}"
    );
}

/// The other direction: every command `Demo::run` answers is in `commands()`,
/// so the parity script and the page's contract check see it too.
#[test]
fn every_answered_command_is_listed() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/session.rs")).unwrap();
    let body = &source[source.find("fn run(&mut self").unwrap()..];
    let (mut seen, mut missing) = (0, Vec::new());
    for line in body.lines() {
        let trimmed = line.trim_start();
        if line.len() - trimmed.len() != 12 || !trimmed.starts_with('"') || !trimmed.contains("=>")
        {
            continue;
        }
        let pattern = trimmed.split("=>").next().unwrap();
        for name in pattern.split('"').skip(1).step_by(2) {
            seen += 1;
            if !blockwork_web::commands().contains(&name) {
                missing.push(name.to_owned());
            }
        }
    }
    assert!(
        seen > 20,
        "found only {seen} commands; is the scan still reading run()?"
    );
    assert!(missing.is_empty(), "add to PAGE_COMMANDS: {missing:?}");
}
