//! Saved-file versions. Each step upgrades a macro's JSON by one version, so
//! a file from any older Blockwork still loads. Blocks that can't be read at
//! all become notes on the canvas instead of failing the whole file.

use crate::macros::{Instruction, Macro};
use serde_json::{json, Map, Value};

/// The version this build writes; 1 is the format 0.6.0 first had, 2 split
/// run command into one that waits and one that doesn't. Android versions 3–6
/// added touch, elements, foreground apps and exact-time settings. Master also
/// introduced pack state at version 3; its ownership migration runs in Macro's
/// deserializer. Version 7 combines those formats and preserves pack metadata,
/// so Android builds that only understand version 6 reject these newer files.
pub const FORMAT_VERSION: u64 = 7;
const VERSION_KEY: &str = "format_version";

/// What loading had to change.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Upgrade {
    /// The file's version; files from before versions count as 0.
    pub from: u64,
    /// Things worth telling the user, like blocks that became notes.
    pub notes: Vec<String>,
}

impl Upgrade {
    /// Whether the file should be rewritten in the current format.
    pub fn changed(&self) -> bool {
        self.from < FORMAT_VERSION || !self.notes.is_empty()
    }
}

type Step = fn(&mut Value, &mut Vec<String>);

/// `STEPS[n]` upgrades version `n` to `n + 1`.
const STEPS: [Step; FORMAT_VERSION as usize] = [v0_old_window_blocks, v1_run_command_waits, v2_android_touch, v3_android_elements, v4_android_apps, v5_android_alarms, v6_pack_state];

// Pack ownership is migrated by Macro's deserializer, including master v3 files.
fn v6_pack_state(_: &mut Value, _: &mut Vec<String>) {}
fn v5_android_alarms(_: &mut Value, _: &mut Vec<String>) {}
fn v4_android_apps(_: &mut Value, _: &mut Vec<String>) {}
fn v3_android_elements(_: &mut Value, _: &mut Vec<String>) {}
fn v2_android_touch(_: &mut Value, _: &mut Vec<String>) {}

pub fn parse(text: &str) -> Result<(Macro, Upgrade), String> {
    from_value(serde_json::from_str(text).map_err(|e| format!("not a macro file: {e}"))?)
}

pub fn from_value(mut value: Value) -> Result<(Macro, Upgrade), String> {
    if !value.is_object() {
        return Err("not a macro file".into());
    }
    let from = value.get(VERSION_KEY).and_then(Value::as_u64).unwrap_or(0);
    if from > FORMAT_VERSION {
        return Err("it was saved by a newer version of Blockwork. Update Blockwork to open it".into());
    }
    let mut upgrade = Upgrade { from, notes: Vec::new() };
    for step in &STEPS[from as usize..] {
        step(&mut value, &mut upgrade.notes);
    }
    let mac = match serde_json::from_value::<Macro>(value.clone()) {
        Ok(mac) => mac,
        Err(first) => {
            let mut removed = Vec::new();
            salvage(&mut value, &mut removed);
            let mut names: Vec<&String> = removed.iter().collect();
            names.sort();
            names.dedup();
            for name in names {
                let count = removed.iter().filter(|n| *n == name).count();
                upgrade.notes.push(if count == 1 {
                    format!("A \"{name}\" block this version can't run became a note")
                } else {
                    format!("{count} \"{name}\" blocks this version can't run became notes")
                });
            }
            serde_json::from_value(value).map_err(|_| first.to_string())?
        }
    };
    Ok((mac, upgrade))
}

/// `mac` as saved JSON, stamped with [`FORMAT_VERSION`].
pub fn to_value(mac: &Macro) -> Result<Value, String> {
    let mut value = serde_json::to_value(mac).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(VERSION_KEY.into(), FORMAT_VERSION.into());
    }
    Ok(value)
}

/// The block name of an instruction in either saved shape (`{id, kind}` or bare).
fn kind_name(ins: &Value) -> String {
    let kind = ins.get("kind").unwrap_or(ins);
    match kind {
        Value::String(name) => name.clone(),
        Value::Object(obj) if obj.len() == 1 => obj.keys().next().cloned().unwrap_or_default(),
        _ => "unknown".into(),
    }
}

/// Replaces every instruction that won't parse with a note, keeping the
/// rest of the macro. Nested blocks are fixed first so one bad block inside
/// a loop doesn't take the loop with it.
fn salvage(value: &mut Value, removed: &mut Vec<String>) {
    let Some(strands) = value.get_mut("strands").and_then(Value::as_array_mut) else { return };
    for strand in strands {
        if let Some(list) = strand.get_mut("instructions").and_then(Value::as_array_mut) {
            salvage_list(list, removed);
        }
    }
}

fn salvage_list(list: &mut [Value], removed: &mut Vec<String>) {
    for ins in list {
        if serde_json::from_value::<Instruction>(ins.clone()).is_ok() {
            continue;
        }
        if let Some(payload) = ins.get_mut("kind").and_then(Value::as_object_mut) {
            payload.values_mut().for_each(|p| salvage_nested(p, removed));
            if serde_json::from_value::<Instruction>(ins.clone()).is_ok() {
                continue;
            }
        }
        let name = kind_name(ins);
        let mut detail = ins.get("kind").unwrap_or(ins).to_string();
        if detail.len() > 300 {
            let cut = (0..=300).rev().find(|i| detail.is_char_boundary(*i)).unwrap_or(0);
            detail.truncate(cut);
            detail.push('…');
        }
        removed.push(name.clone());
        let id = ins.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(blockstitch_core::graph::new_id);
        *ins = json!({ "id": id, "kind": { "Comment": format!("Removed a \"{name}\" block Blockwork can't run: {detail}") } });
    }
}

/// Block lists inside a block's fields, including call branches (lists of lists).
fn salvage_nested(value: &mut Value, removed: &mut Vec<String>) {
    match value {
        Value::Object(fields) => fields.values_mut().for_each(|v| salvage_nested(v, removed)),
        Value::Array(items) if items.iter().all(Value::is_array) => items.iter_mut().for_each(|v| salvage_nested(v, removed)),
        Value::Array(items) if items.iter().all(|i| i.get("id").is_some() && i.get("kind").is_some()) => salvage_list(items, removed),
        _ => {}
    }
}

/// Calls `f` on every object in the tree, parents first.
fn walk_objects(value: &mut Value, f: &mut dyn FnMut(&mut Map<String, Value>)) {
    match value {
        Value::Object(obj) => {
            f(obj);
            obj.values_mut().for_each(|v| walk_objects(v, f));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| walk_objects(v, f)),
        _ => {}
    }
}

fn is_empty_text(value: Option<&Value>) -> bool {
    value.is_none_or(|v| v.get("kind").and_then(Value::as_str) == Some("Text") && v.get("value").and_then(Value::as_str) == Some(""))
}

/// The window blocks from before they were merged into `WindowAction` and
/// `MoveWindow` matched an app and a title at once; keep whichever was set,
/// preferring the title. `WaitUntil` was a bare condition.
fn v0_old_window_blocks(value: &mut Value, notes: &mut Vec<String>) {
    let mut converted = 0;
    walk_objects(value, &mut |obj| {
        if obj.len() != 1 {
            return;
        }
        let Some((name, payload)) = obj.iter_mut().next() else { return };
        let Some(fields) = payload.as_object_mut() else { return };
        let action = match name.as_str() {
            "FocusWindow" => "Focus",
            "MinimizeWindow" => "Minimize",
            "MaximizeWindow" => "Maximize",
            "RestoreWindow" => "Restore",
            "CloseWindow" => "Close",
            "MoveWindow" if !fields.contains_key("by") => "",
            "WaitUntil" if !fields.contains_key("condition") => {
                let condition = std::mem::take(payload);
                *payload = json!({ "condition": condition, "timeout": { "kind": "Number", "value": 0.0 } });
                converted += 1;
                return;
            }
            _ => return,
        };
        let (pattern, by) = if !is_empty_text(fields.get("title_contains")) || is_empty_text(fields.get("app")) {
            (fields.remove("title_contains"), "Title")
        } else {
            (fields.remove("app"), "App")
        };
        let pattern = pattern.unwrap_or_else(|| json!({ "kind": "Text", "value": "" }));
        let new = if action.is_empty() {
            let (x, y) = (fields.remove("x"), fields.remove("y"));
            json!({ "MoveWindow": { "pattern": pattern, "by": by, "resize": false, "x": x, "y": y } })
        } else {
            json!({ "WindowAction": { "action": action, "pattern": pattern, "by": by } })
        };
        if let Value::Object(new) = new {
            *obj = new;
        }
        converted += 1;
    });
    if converted > 0 {
        notes.push(format!("Updated {converted} block{} from an older version", if converted == 1 { "" } else { "s" }));
    }
}

/// Run command always waited for its command; now it starts it and moves on,
/// and "run command and wait" waits. Old blocks keep waiting.
fn v1_run_command_waits(value: &mut Value, notes: &mut Vec<String>) {
    let mut converted = 0;
    walk_objects(value, &mut |obj| {
        if obj.len() == 1 && obj.get("Command").is_some_and(Value::is_string) {
            if let Some(command) = obj.remove("Command") {
                obj.insert("CommandWait".into(), command);
                converted += 1;
            }
        }
    });
    if converted > 0 {
        let blocks = if converted == 1 { "run command block is now a \"run command and wait\" block" } else { "run command blocks are now \"run command and wait\" blocks" };
        notes.push(format!("{converted} {blocks}, so they still wait for their command to finish"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::InstructionKind;

    fn first_strand(mac: &Macro) -> Vec<InstructionKind> {
        mac.strands[0].instructions.iter().map(|i| i.kind.clone()).collect()
    }

    #[test]
    fn current_files_round_trip_with_their_version() {
        let mac = Macro::new("Test".into(), String::new(), Vec::new());
        let value = to_value(&mac).unwrap();
        assert_eq!(value[VERSION_KEY], FORMAT_VERSION);
        let (back, upgrade) = from_value(value).unwrap();
        assert_eq!(back, mac);
        assert!(!upgrade.changed());
    }

    #[test]
    fn master_v3_and_android_v6_preserve_pack_metadata_and_touch() {
        use crate::block_packs::{PackExport, PackInfo};
        use crate::input::touch::TouchAction;
        use crate::input::value::Value as InputValue;
        let touch = Instruction::new(InstructionKind::Touch {
            action: TouchAction::Tap,
            x: InputValue::Number { value: 10.0 }, y: InputValue::Number { value: 20.0 },
            end_x: InputValue::Number { value: 10.0 }, end_y: InputValue::Number { value: 20.0 },
            duration: InputValue::Number { value: 80.0 },
        });
        let mut mac = Macro::new("Rebase compatibility".into(), String::new(), vec![touch]);
        mac.settings.android_exact_time = true;
        mac.settings.pack_exports.push(PackExport {
            info: PackInfo { id: "fixture-pack".into(), name: "Fixture pack".into(), ..Default::default() },
            block_ids: Vec::new(), variable_roles: Vec::new(),
        });
        for version in [3, 6] {
            let mut value = to_value(&mac).unwrap();
            value[VERSION_KEY] = json!(version);
            let (back, upgrade) = from_value(value).unwrap();
            assert_eq!(upgrade.from, version);
            assert!(upgrade.changed());
            assert!(upgrade.notes.is_empty());
            assert_eq!(back, mac);
            assert_eq!(to_value(&back).unwrap()[VERSION_KEY], 7);
        }
    }

    #[test]
    fn newer_files_are_refused() {
        let err = parse(r#"{"format_version":999,"name":"x","description":"","strands":[]}"#).unwrap_err();
        assert!(err.contains("newer version"));
    }

    #[test]
    fn old_window_blocks_are_upgraded() {
        let (mac, upgrade) = parse(
            r#"{"id":"m","name":"Old","description":"","strands":[{"id":"s","x":0,"y":0,"instructions":[
                {"id":"a","kind":"WhenRan"},
                {"id":"b","kind":{"FocusWindow":{"app":{"kind":"Text","value":"firefox"},"title_contains":{"kind":"Text","value":""}}}},
                {"id":"c","kind":{"MoveWindow":{"scope":"App","app":{"kind":"Text","value":""},"title_contains":{"kind":"Text","value":"Notes"},"x":{"kind":"Number","value":5.0},"y":{"kind":"Number","value":6.0}}}},
                {"id":"d","kind":{"WaitUntil":{"kind":"Bool"}}}
            ]}]}"#,
        )
        .unwrap();
        assert_eq!(upgrade.from, 0);
        assert!(upgrade.changed());
        let kinds = first_strand(&mac);
        assert!(matches!(&kinds[1], InstructionKind::WindowAction { action, by, pattern }
            if action == "Focus" && by == "App" && pattern.eval_text().unwrap() == "firefox"));
        assert!(matches!(&kinds[2], InstructionKind::MoveWindow { by, resize: false, pattern, .. }
            if by == "Title" && pattern.eval_text().unwrap() == "Notes"));
        assert!(matches!(&kinds[3], InstructionKind::WaitUntil { .. }));
    }

    #[test]
    fn old_run_commands_keep_waiting() {
        let (mac, upgrade) = parse(
            r#"{"format_version":1,"id":"m","name":"Old","description":"","strands":[{"id":"s","x":0,"y":0,"instructions":[
                {"id":"a","kind":"WhenRan"},
                {"id":"b","kind":{"Command":"make"}},
                {"id":"c","kind":{"Repeat":{"count":{"kind":"Number","value":2.0},"body":[{"id":"d","kind":{"Command":"sleep 1"}}]}}}
            ]}]}"#,
        )
        .unwrap();
        assert_eq!(upgrade.notes.len(), 1, "{:?}", upgrade.notes);
        assert!(upgrade.notes[0].starts_with("2 run command blocks"));
        let kinds = first_strand(&mac);
        assert_eq!(kinds[1], InstructionKind::CommandWait("make".into()));
        let InstructionKind::Repeat { body, .. } = &kinds[2] else { panic!("{kinds:?}") };
        assert_eq!(body[0].kind, InstructionKind::CommandWait("sleep 1".into()));
        // Current files keep both kinds as they are.
        let (back, upgrade) = from_value(to_value(&mac).unwrap()).unwrap();
        assert_eq!(back, mac);
        assert!(!upgrade.changed());
        let mac = Macro::new("New".into(), String::new(), vec![Instruction::new(InstructionKind::Command("make".into()))]);
        let (back, _) = from_value(to_value(&mac).unwrap()).unwrap();
        assert_eq!(back, mac);
    }

    #[test]
    fn unknown_blocks_become_notes_and_the_rest_loads() {
        let (mac, upgrade) = parse(
            r#"{"format_version":1,"id":"m","name":"Odd","description":"","strands":[{"id":"s","x":0,"y":0,"instructions":[
                {"id":"a","kind":"WhenRan"},
                {"id":"b","kind":{"Repeat":{"count":{"kind":"Number","value":2.0},"body":[
                    {"id":"c","kind":{"ClickElement":{"name":"Six"}}},
                    {"id":"d","kind":"StopMacro"}
                ]}}},
                {"id":"e","kind":{"Teleport":{}}}
            ]}]}"#,
        )
        .unwrap();
        assert_eq!(upgrade.notes.len(), 2, "{:?}", upgrade.notes);
        let kinds = first_strand(&mac);
        // The bad blocks left the strand as notes; the loop kept its good block.
        assert_eq!(kinds.len(), 2);
        let InstructionKind::Repeat { body, .. } = &kinds[1] else { panic!("{kinds:?}") };
        assert_eq!(body.len(), 1);
        assert!(mac.comments.iter().any(|c| c.text.contains("ClickElement")));
        assert!(mac.comments.iter().any(|c| c.text.contains("Teleport")));
    }
}
