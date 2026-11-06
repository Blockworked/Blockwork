//! Every block's wire shape survives the trip editor -> backend -> editor
//! unchanged. `fixtures/wire_blocks.json` holds one of each `InstructionDto`
//! variant; a new block adds one there.

use blockwork_core::frontend::{InstructionDto, dto_to_instruction, instruction_to_dto};
use serde_json::Value;

#[test]
fn every_block_round_trips_through_the_wire() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wire_blocks.json"
    );
    let blocks: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for block in &blocks {
        let dto: InstructionDto =
            serde_json::from_value(block.clone()).unwrap_or_else(|e| panic!("{block}: {e}"));
        let ins = dto_to_instruction(&dto).unwrap_or_else(|| panic!("{block} did not convert"));
        // Compared as serialized DTOs, so `1` and `1.0` agree.
        assert_eq!(
            serde_json::to_value(instruction_to_dto(&ins)).unwrap(),
            serde_json::to_value(&dto).unwrap()
        );
    }
}

#[test]
fn the_fixture_has_every_block() {
    let source = include_str!("../src/frontend/instructions.rs");
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wire_blocks.json"
    );
    let fixture = std::fs::read_to_string(path).unwrap();
    let table =
        &source[source.find("wire_blocks! {").unwrap()..source.find("to_wire(ins, id)").unwrap()];
    for line in table.lines().map(str::trim) {
        let name: String = line
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        assert!(
            fixture.contains(&format!("\"type\": \"{name}\"")),
            "{name} has no case in wire_blocks.json"
        );
    }
}

/// `BlockCatalog.qml`'s `registerVariableFields` (the dropdowns the editor
/// renames with a variable) must match `for_each_ref` in `block_kind.rs`.
#[test]
fn variable_dropdowns_match_the_backend_refs() {
    use blockwork_core::macros::{BlockKind, RefKind};
    let qml = include_str!("../../blockwork-qt/qml/BlockCatalog.qml");
    let start = qml.find("registerVariableFields({").unwrap();
    let table = &qml[start..start + qml[start..].find("});").unwrap()];
    let mut listed = std::collections::HashMap::new();
    for entry in table.split(']').filter(|e| e.contains('[')) {
        let (kind, fields) = entry.split_once('[').unwrap();
        let kind = kind
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .rsplit(|c: char| !c.is_ascii_alphanumeric())
            .next()
            .unwrap();
        let fields: Vec<String> = fields
            .split(',')
            .map(|f| f.trim().trim_matches('"').to_string())
            .collect();
        listed.insert(kind.to_string(), fields);
    }
    assert!(listed.len() > 5, "parsed {listed:?}");

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wire_blocks.json"
    );
    let blocks: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for block in &blocks {
        let ty = block["type"].as_str().unwrap();
        let kind = dto_to_instruction(&serde_json::from_value(block.clone()).unwrap())
            .unwrap()
            .kind;
        let mut refs = Vec::new();
        kind.visit_refs(&mut |k, name| {
            if k == RefKind::Variable {
                refs.push(name.to_string())
            }
        });
        // Blockstitch's own variable blocks register theirs.
        if refs.is_empty() || matches!(ty, "SetVariable" | "ChangeVariable") {
            assert!(
                !listed.contains_key(ty),
                "{ty} is listed but holds no variable"
            );
            continue;
        }
        let fields = listed.get(ty).unwrap_or_else(|| {
            panic!("{ty} holds a variable but has no registerVariableFields entry")
        });
        for field in fields {
            if let Some(name) = block[field].as_str() {
                assert!(
                    refs.contains(&name.to_string()),
                    "{ty}.{field} is not a variable the backend renames"
                );
            }
        }
    }
}

/// Every block has a palette entry and help text, and every value piece in
/// `BlockCatalog.qml` names a real `FieldId`.
#[test]
fn the_catalog_covers_every_block() {
    let catalog = include_str!("../../blockwork-qt/qml/BlockCatalog.qml");
    let help = include_str!("../../blockwork-qt/qml/BlockHelp.qml");
    let start = catalog.find("paletteSections: [").unwrap();
    let sections = &catalog[start..start + catalog[start..].find("\n    ]").unwrap()];
    // Not in the palette: placed by the editor itself, or old shapes.
    const UNLISTED: [&str; 5] = [
        "Comment",
        "BlockHeader",
        "CallBlock",
        "BranchCallBlock",
        "RunBranch",
    ];
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wire_blocks.json"
    );
    let blocks: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut missing = Vec::new();
    for block in &blocks {
        let ty = block["type"].as_str().unwrap();
        if UNLISTED.contains(&ty) {
            continue;
        }
        if !sections.contains(&format!("\"{ty}\"")) {
            missing.push(format!("{ty}: no paletteSections entry"));
        }
        if !help.contains(&format!("{ty}: qsTr(")) {
            missing.push(format!("{ty}: no BlockHelp explainer"));
        }
    }
    // Which blocks have rows is checked in QML, against blockstitch's own
    // lists (tst_host_vocabulary.qml, test_every_block_has_exactly_one_row).
    let pieces = catalog
        .split("value(\"")
        .skip(1)
        .chain(catalog.split("box(\"").skip(1));
    for piece in pieces {
        let field = piece.split('"').next().unwrap();
        if field
            .parse::<blockwork_core::macros::fields::FieldId>()
            .is_err()
        {
            missing.push(format!("{field} (a value or box piece) is not a FieldId"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// Every `When...` trigger is a header, has a run-log label, and is named by
/// a watcher in `blockwork-app` (adding a block, step 7).
#[test]
fn every_trigger_has_a_label_and_a_watcher() {
    use blockstitch_core::graph::BlockKind;
    let app = concat!(env!("CARGO_MANIFEST_DIR"), "/../blockwork-app/src");
    let mut sources = String::new();
    let mut dirs = vec![std::path::PathBuf::from(app)];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources += &std::fs::read_to_string(path).unwrap();
            }
        }
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wire_blocks.json"
    );
    let blocks: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut missing = Vec::new();
    for block in &blocks {
        let ty = block["type"].as_str().unwrap();
        if !ty.starts_with("When") || ty == "WhenRan" {
            continue;
        }
        let ins = dto_to_instruction(&serde_json::from_value(block.clone()).unwrap()).unwrap();
        if !ins.kind.is_header() {
            missing.push(format!("{ty}: not in is_header"));
        }
        if ins.kind.trigger_label().is_none() {
            missing.push(format!("{ty}: no trigger_label in macros/trigger.rs"));
        }
        if !sources.contains(&format!("InstructionKind::{ty}")) {
            missing.push(format!("{ty}: no watcher in blockwork-app names it"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// `block_kind.rs` matches with `_` arms, so the compiler won't flag a new
/// block whose body or variable it forgets. This holds every fixture block's
/// instruction lists to `body` and its variable and list names to `visit_refs`.
#[test]
fn block_kind_walks_every_body_and_name() {
    use blockstitch_core::graph::BlockKind;
    // Name-shaped fields that aren't a variable or list.
    const NOT_REFS: [&str; 3] = ["OpenApp.name", "CloseApp.name", "RunBranch.name"];
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/wire_blocks.json"
    );
    let blocks: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut missing = Vec::new();
    for block in &blocks {
        let ty = block["type"].as_str().unwrap();
        let kind = dto_to_instruction(&serde_json::from_value(block.clone()).unwrap())
            .unwrap()
            .kind;
        let fields = block.as_object().unwrap();
        // An instruction list, or a call's list of branch lists.
        let is_body = |v: &Value| {
            v.as_array()
                .is_some_and(|a| a.iter().any(|i| i.get("type").is_some()))
        };
        let bodies: usize = fields
            .values()
            .map(|v| match v.as_array() {
                _ if is_body(v) => 1,
                Some(a) if a.iter().any(is_body) && a.iter().all(Value::is_array) => a.len(),
                _ => 0,
            })
            .sum();
        for slot in 0..bodies {
            if kind.body(slot as u8).is_none() {
                missing.push(format!("{ty}: body slot {slot} not in BlockKind::body"));
            }
        }
        if kind.body(bodies as u8).is_some() {
            missing.push(format!("{ty}: body has a slot past its {bodies} lists"));
        }
        let mut refs = Vec::new();
        kind.visit_refs(&mut |_, name| refs.push(name.to_string()));
        for (key, value) in fields {
            let named = key == "name" || key.ends_with("variable") || key.ends_with("list");
            let qualified = format!("{ty}.{key}");
            if let Some(name) = value.as_str().filter(|_| named)
                && !NOT_REFS.contains(&qualified.as_str())
                && !refs.iter().any(|r| r == name)
            {
                missing.push(format!("{qualified}: not visited by for_each_ref"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
