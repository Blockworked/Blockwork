//! The website's editor (blockwork-web) against this app: one script of edits
//! through both, and the open macro, undo state and text buffers compared
//! after every step with ids renamed in order of appearance.

use crate::{AppHandle, Backend, initial_state};
use blockstitch_ffi::Host;
use blockwork_web::Demo;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

fn backend() -> Backend {
    let dir = std::env::temp_dir().join(format!("blockwork-web-parity-{}", std::process::id()));
    blockwork_core::config::set_config_dir_override(dir);
    blockwork_core::init();
    let settings = blockwork_core::config::AppSettings::default();
    let state = initial_state(&settings, None, true, false);
    let (events, _) = broadcast::channel(16);
    Backend { state: Arc::new(Mutex::new(state)), app: AppHandle { events }, foreground: Arc::new(Mutex::new(Default::default())), alarms: Arc::new(Mutex::new(Default::default())) }
}

fn is_id(text: &str) -> bool {
    let hex = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit());
    (text.len() == 32 && hex(text))
        || (text.len() == 36 && text.split('-').map(str::len).eq([8, 4, 4, 4, 12]) && text.split('-').all(hex))
}

/// Renames every id to `#n` by first appearance (serde_json orders keys).
fn normalize(value: &Value, ids: &mut HashMap<String, usize>) -> Value {
    match value {
        Value::String(s) if is_id(s) => {
            let next = ids.len();
            Value::String(format!("#{}", ids.entry(s.clone()).or_insert(next)))
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| normalize(v, ids)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), normalize(v, ids))).collect()),
        other => other.clone(),
    }
}

/// What both sides must agree on.
fn view(state: &Value) -> Value {
    let mut ids = HashMap::new();
    let buffers = |v: &Value| {
        let mut list: Vec<Value> = v.as_array().cloned().unwrap_or_default();
        list.sort_by_key(|b| b.to_string());
        Value::Array(list)
    };
    json!({
        "macro": normalize(&state["current_macro"], &mut ids),
        "can_undo": state["can_undo"],
        "can_redo": state["can_redo"],
        "buffers": normalize(&buffers(&state["invalid_field_buffers"]), &mut ids),
        "standalone_key": state["standalone_key"],
    })
}

/// Swaps `"$n"` (the answer to step n) and `"$n.field"` into a step's args.
fn bind(args: &Value, answers: &[(Value, Value)]) -> Value {
    match args {
        Value::String(s) if s.starts_with('$') => {
            let (step, field) = s[1..].split_once('.').unwrap_or((&s[1..], ""));
            let (app, _) = &answers[step.parse::<usize>().unwrap()];
            if field.is_empty() { app.clone() } else { app[field].clone() }
        }
        Value::Array(items) => Value::Array(items.iter().map(|v| bind(v, answers)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), bind(v, answers))).collect()),
        other => other.clone(),
    }
}

fn num(n: f64) -> Value {
    json!({ "kind": "Number", "value": n })
}

fn at(index: usize) -> Value {
    json!([{ "index": index, "slot": null }])
}

fn wait_field(strand: &str, index: usize) -> Value {
    json!({ "kind": "Field", "strand_id": strand, "index": at(index), "field_id": "WaitDuration", "path": [] })
}

fn script() -> Vec<(&'static str, Value)> {
    vec![
        /* 0 */ ("new_macro", json!({})),
        /* 1 */ ("add_strand", json!({ "x": 10, "y": 20, "instruction": { "id": "i1", "type": "WhenRan" } })),
        /* 2 */ ("add_instruction", json!({ "strandId": "$1", "path": at(1), "instruction": { "id": "i2", "type": "Wait", "duration": num(100.0) } })),
        /* 3 */ ("add_instruction", json!({ "strandId": "$1", "path": at(2), "instruction": { "id": "i3", "type": "Repeat", "count": num(3.0), "body": [] } })),
        /* 4 */ ("add_instruction", json!({ "strandId": "$1", "path": [{ "index": 2, "slot": 0 }, { "index": 0, "slot": null }],
                   "instruction": { "id": "i4", "type": "Text", "text": { "kind": "Text", "value": "hi" } } })),
        /* 5 */ ("edit_value_field", json!({ "location": wait_field("$1", 1), "text": "2" })),
        /* 6 */ ("edit_value_field", json!({ "location": wait_field("$1", 1), "text": "25" })),
        /* 7 */ ("edit_value_field", json!({ "location": wait_field("$1", 1), "text": "2x" })),
        /* 8 */ ("edit_value_field", json!({ "location": wait_field("$1", 1), "text": "09" })),
        /* 9 */ ("set_value_kind", json!({ "location": wait_field("$1", 1), "kind": "Add" })),
        /* 10 */ ("edit_value_field", json!({ "location": { "kind": "Field", "strand_id": "$1", "index": at(1), "field_id": "WaitDuration", "path": [1] }, "text": "07" })),
        /* 11 */ ("create_variable", json!({ "name": "n" })),
        /* 12 */ ("rename_variable", json!({ "oldName": "n", "newName": "count" })),
        /* 13 */ ("create_signal", json!({ "name": "go" })),
        /* 14 */ ("rename_signal", json!({ "oldName": "go", "newName": "start" })),
        /* 15 */ ("create_floating_value", json!({ "x": 300, "y": 40, "value": { "kind": "Var", "name": "count" }, "originBlockId": null })),
        /* 16 */ ("move_floating_value", json!({ "floatingId": "$15", "x": 320, "y": 60 })),
        /* 17 */ ("move_value", json!({ "from": wait_field("$1", 1), "x": 500, "y": 90, "originBlockId": null })),
        /* 18 */ ("move_value", json!({ "from": { "kind": "Floating", "floating_id": "$15", "path": [] }, "to": wait_field("$1", 1) })),
        /* 19 */ ("create_comment", json!({ "x": 10, "y": 300, "text": "note" })),
        /* 20 */ ("edit_comment_text", json!({ "commentId": "$19", "text": "note 1" })),
        /* 21 */ ("edit_comment_text", json!({ "commentId": "$19", "text": "note 12" })),
        /* 22 */ ("set_comment_collapsed", json!({ "commentId": "$19", "collapsed": true })),
        /* 23 */ ("copy_instructions", json!({ "strandId": "$1", "path": at(1), "count": 2 })),
        /* 24 */ ("paste_fragment", json!({ "x": 400, "y": 400, "fragment": "$23" })),
        /* 25 */ ("duplicate_instruction", json!({ "strandId": "$1", "path": at(1) })),
        /* 26 */ ("reorder_instruction", json!({ "strandId": "$1", "path": at(2), "direction": 1 })),
        /* 27 */ ("remove_instruction", json!({ "strandId": "$1", "path": at(3) })),
        /* 28 */ ("split_strand", json!({ "strandId": "$1", "path": at(2), "x": 50, "y": 600 })),
        /* 29 */ ("merge_strand", json!({ "draggedId": "$28", "targetId": "$1", "path": at(2) })),
        /* 30 */ ("move_strand", json!({ "strandId": "$24", "x": 700, "y": 10 })),
        /* 31 */ ("create_list", json!({ "name": "items" })),
        /* 32 */ ("set_list_items", json!({ "name": "items", "items": [{ "kind": "Number", "value": 1.0 }, { "kind": "Text", "value": "b" }] })),
        /* 33 */ ("rename_list", json!({ "oldName": "items", "newName": "things" })),
        /* 34 */ ("create_block", json!({ "pieces": [{ "kind": "Label", "id": "l", "text": "jump" },
                     { "kind": "Input", "id": "i", "name": "height", "value_type": "Any" }],
                   "shape": "Normal", "color": "#ff8800", "icon": "star", "identifier": "jump", "description": "Jumps" })),
        /* 35 */ ("undo", json!({})),
        /* 36 */ ("undo", json!({})),
        /* 37 */ ("redo", json!({})),
        /* 38 */ ("remove_comment", json!({ "commentId": "$19" })),
        /* 39 */ ("delete_instruction", json!({ "strandId": "$1", "path": at(1), "x": 80, "y": 500 })),
        /* 40 */ ("remove_strand", json!({ "strandId": "$24" })),
        /* 41 */ ("set_title", json!({ "title": "Parity" })),
        /* 42 */ ("set_macro_speed_multiplier", json!({ "multiplier": 99.0 })),
        /* 43 */ ("set_macro_conditions", json!({ "onlyInWindow": " Firefox ", "power": "OnBattery",
                   "activeFrom": "9:00", "activeUntil": "17:30", "activeDays": 255 })),
        /* 44 */ ("start_standalone_key_capture", json!({})),
        /* 45 */ ("key_capture_event", json!({ "keys": [["ControlLeft", "Control"], ["KeyA", "a"]] })),
        /* 46 */ ("add_instruction", json!({ "strandId": "$1", "path": at(1), "instruction": { "id": "i5", "type": "Key", "key": { "kind": "Text", "value": "a" }, "direction": "Click" } })),
        /* 47 */ ("start_key_capture", json!({ "strandId": "$1", "path": at(1) })),
        /* 48 */ ("key_capture_event", json!({ "keys": [["ShiftLeft", "Shift"], ["F5", "F5"]] })),
        /* 49 */ ("delete_variable", json!({ "name": "count" })),
        /* 50 */ ("undo", json!({})),
        /* 51 */ ("redo", json!({})),
        // Refusals: both sides must refuse, and change nothing.
        /* 52 */ ("create_signal", json!({ "name": "START" })),
        /* 53 */ ("add_instruction", json!({ "strandId": "$1", "path": at(1), "instruction": { "id": "h", "type": "WhenRan" } })),
        /* 54 */ ("rename_list", json!({ "oldName": "nope", "newName": "x" })),
        /* 55 */ ("undo", json!({})),
    ]
}

/// Steps the script expects to be refused.
const REFUSED: &[usize] = &[52, 53, 54];

#[test]
fn web_editor_replays_like_the_app() {
    let app = backend();
    let mut web = Demo::default();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let mut answers: Vec<(Value, Value)> = Vec::new();
    for (step, (command, args)) in script().into_iter().enumerate() {
        let app_args = bind(&args, &answers);
        // The web side gets its own ids for the same steps.
        let web_answers: Vec<(Value, Value)> = answers.iter().map(|(_, w)| (w.clone(), Value::Null)).collect();
        let web_args = bind(&args, &web_answers);
        let from_app = runtime.block_on(app.dispatch(command, app_args));
        let from_web = web.dispatch(command, &web_args);
        assert_eq!(from_app.is_ok(), from_web.is_ok(), "step {step} {command}: app {from_app:?} web {from_web:?}");
        let app_state = serde_json::to_value(crate::commands::get_state(&app.state).unwrap()).unwrap();
        let (a, w) = (view(&app_state), view(&web.state()));
        assert_eq!(a, w, "step {step} {command} differs\napp {a:#}\nweb {w:#}");
        assert_eq!(from_app.is_err(), REFUSED.contains(&step), "step {step} {command}: {from_app:?}");
        answers.push((from_app.unwrap_or(Value::Null), from_web.unwrap_or(Value::Null)));
    }
}

fn op(name: &str, args: Vec<Value>) -> Value {
    json!({ "kind": "Op", "op": name, "args": args, "saved": num(0.0) })
}

fn text(t: &str) -> Value {
    json!({ "kind": "Text", "value": t })
}

/// Click-to-preview of pure reporters answers the same on both sides.
#[test]
fn web_preview_matches_the_app_for_pure_reporters() {
    let app = backend();
    let mut web = Demo::default();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let setup = [
        ("new_macro", json!({})),
        ("create_variable", json!({ "name": "count" })),
        ("create_list", json!({ "name": "items" })),
        ("set_list_items", json!({ "name": "items", "items": [num(4.0), text("b")] })),
    ];
    for (command, args) in setup {
        runtime.block_on(app.dispatch(command, args.clone())).unwrap();
        web.dispatch(command, &args).unwrap();
    }
    let values = [
        op("Add", vec![num(2.0), num(3.5)]),
        op("Div", vec![num(1.0), num(0.0)]),
        op("Mod", vec![num(-7.0), num(3.0)]),
        op("Round", vec![num(2.5)]),
        op("Join", vec![text("clicked "), json!({ "kind": "Var", "name": "count" })]),
        op("Gt", vec![text("10"), text("9")]),
        op("Eq", vec![text("Hello"), text("hello")]),
        op("Length", vec![text("blocks")]),
        op("LetterOf", vec![num(2.0), text("blocks")]),
        op("Case", vec![text("upper"), text("abc")]),
        op("Math", vec![text("sqrt"), num(16.0)]),
        op("TextCheck", vec![text("order 1234"), text("matches regex"), text(r"\d{4}$")]),
        op("SplitPart", vec![text("a,b,c"), text(","), num(2.0)]),
        op("FormatNumber", vec![num(12.3456), num(2.0)]),
        op("FormatDate", vec![num(0.0), text("%Y")]),
        op("Not", vec![json!({ "kind": "Bool" })]),
        op("Add", vec![text("x"), num(1.0)]),
    ];
    for value in values {
        let from_app = runtime.block_on(app.dispatch("preview_value", json!({ "value": value.clone() })));
        let from_web = web.dispatch("preview_value", &json!({ "value": value.clone() }));
        assert_eq!(from_app, from_web, "{value}");
    }
    // Side effects are refused by both. (Host reporters are covered in
    // blockwork-web's own tests: this process registers the app's.)
    let shell = json!({ "value": op("ShellOutput", vec![text("ls")]) });
    assert_eq!(
        runtime.block_on(app.dispatch("preview_value", shell.clone())),
        web.dispatch("preview_value", &shell)
    );
}

#[test]
fn snapping_above_keeps_target_position_and_undoes_atomically() {
    for command in ["add_instruction", "merge_strand", "merge_tail"] {
        let app = backend();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let send = |cmd, args| runtime.block_on(app.dispatch(cmd, args)).unwrap();
        send("new_macro", json!({}));
        let wait = |id| json!({"id": id, "type": "Wait", "duration": num(1.0)});
        let target = send(
            "add_strand",
            json!({"x": 40, "y": 240, "instruction": wait("lower")}),
        );
        let source = send(
            "add_strand",
            json!({"x": 400, "y": 240, "instruction": wait("upper")}),
        );
        if command == "merge_tail" {
            send(
                "add_instruction",
                json!({"strandId": source, "path": at(1), "instruction": wait("tail")}),
            );
        }
        let before = app
            .state
            .lock()
            .unwrap()
            .current_macro
            .as_ref()
            .unwrap()
            .graph
            .clone();
        let args = match command {
            "add_instruction" => {
                json!({"strandId": target, "path": at(0), "instruction": wait("palette"), "targetY": 190})
            }
            "merge_strand" => {
                json!({"draggedId": source, "targetId": target, "path": at(0), "targetY": 190})
            }
            _ => {
                json!({"strandId": source, "path": at(1), "targetId": target, "targetPath": at(0), "targetY": 190})
            }
        };
        send(command, args);
        let after = app
            .state
            .lock()
            .unwrap()
            .current_macro
            .as_ref()
            .unwrap()
            .graph
            .clone();
        let strand = after.strand(target.as_str().unwrap()).unwrap();
        assert_eq!(strand.x, 40);
        assert_eq!(
            strand.y + 50,
            240,
            "{command} must preserve the lower block's y"
        );
        assert_eq!(strand.instructions.last().unwrap().id, "lower");
        send("undo", json!({}));
        assert_eq!(
            app.state
                .lock()
                .unwrap()
                .current_macro
                .as_ref()
                .unwrap()
                .graph,
            before
        );
        send("redo", json!({}));
        assert_eq!(
            app.state
                .lock()
                .unwrap()
                .current_macro
                .as_ref()
                .unwrap()
                .graph,
            after
        );
    }
}

#[test]
fn custom_block_position_is_preserved_and_undoable() {
    let app = backend();
    let mut web = Demo::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(app.dispatch("new_macro", json!({})))
        .unwrap();
    web.dispatch("new_macro", &json!({})).unwrap();
    let args = json!({"pieces": [{"kind": "Label", "id": "label", "text": "custom"}],
        "shape": "Normal", "color": "#51b84b", "x": -1375, "y": 812});
    runtime
        .block_on(app.dispatch("create_block", args.clone()))
        .unwrap();
    web.dispatch("create_block", &args).unwrap();
    let state = || serde_json::to_value(crate::commands::get_state(&app.state).unwrap()).unwrap();
    let created = state();
    for snapshot in [created.clone(), web.state()] {
        let header = snapshot["current_macro"]["strands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|strand| strand["instructions"][0]["type"] == "BlockHeader")
            .unwrap();
        assert_eq!(header["x"], -1375);
        assert_eq!(header["y"], 812);
    }
    runtime.block_on(app.dispatch("undo", json!({}))).unwrap();
    web.dispatch("undo", &json!({})).unwrap();
    assert!(
        state()["current_macro"]["block_defs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(view(&state()), view(&web.state()));
    runtime.block_on(app.dispatch("redo", json!({}))).unwrap();
    web.dispatch("redo", &json!({})).unwrap();
    assert_eq!(view(&state()), view(&created));
    assert_eq!(view(&state()), view(&web.state()));
}
