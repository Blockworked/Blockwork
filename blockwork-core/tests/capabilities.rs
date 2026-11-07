//! The capability table against everything this build can run: every
//! reporter (pure ones in `blockwork-model`, machine ones here) and every
//! block's run-time check.

use blockwork_core::capabilities::{
    CAPABILITIES, UNGUARDED_OPERATORS, block_capabilities, checked_when_run, operator_capability,
};
use blockwork_core::macros::{InstructionKind, Role};
use std::collections::HashSet;

/// Every Blockwork reporter's operator name, read from both `input/value.rs` files.
fn reporter_names() -> Vec<String> {
    let dir = env!("CARGO_MANIFEST_DIR");
    let mut names = Vec::new();
    for path in [
        format!("{dir}/src/input/value.rs"),
        format!("{dir}/../blockwork-model/src/input/value.rs"),
    ] {
        let source = std::fs::read_to_string(&path).unwrap();
        names.extend(source.lines().filter_map(|line| {
            Some(
                line.trim()
                    .strip_prefix("op: \"")?
                    .split('"')
                    .next()?
                    .to_string(),
            )
        }));
    }
    assert!(
        names.len() > 40,
        "parsed only {} reporters from input/value.rs",
        names.len()
    );
    names
}

/// Every block from the wire round-trip test's fixture.
fn every_block() -> Vec<InstructionKind> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../blockwork-model/tests/fixtures/wire_blocks.json"
    );
    let fixtures: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    fixtures
        .as_array()
        .unwrap()
        .iter()
        .map(|block| {
            let dto: blockwork_core::frontend::InstructionDto =
                serde_json::from_value(block.clone()).unwrap();
            blockwork_core::frontend::dto_to_instruction(&dto)
                .unwrap()
                .kind
        })
        .collect()
}

#[test]
fn every_checked_capability_can_ask() {
    for kind in every_block() {
        for cap in checked_when_run(&kind) {
            assert!(
                blockwork_core::permissions::GUARDED
                    .iter()
                    .any(|(k, _)| *k == cap),
                "{cap} can't be asked about"
            );
        }
    }
}

/// `capabilities.json` is this table for code outside Rust (the community
/// site's pack review). `BLESS=1 cargo test -p blockwork-core --test capabilities` rewrites it.
#[test]
fn the_exported_table_is_current() {
    use serde_json::{Map, json};
    let mut blocks = Map::new();
    for kind in every_block() {
        let mut caps: Vec<&str> = block_capabilities(&kind).to_vec();
        if kind.info().role == Role::Trigger {
            caps.push("triggers");
        }
        // The variant name, as a saved document spells it.
        let name = match serde_json::to_value(&kind).unwrap() {
            serde_json::Value::String(name) => name,
            serde_json::Value::Object(map) => map.keys().next().unwrap().clone(),
            other => panic!("unexpected kind shape {other}"),
        };
        if !caps.is_empty() {
            blocks.insert(name, json!(caps));
        }
    }
    let mut operators = Map::new();
    for name in reporter_names() {
        if let Some(cap) = operator_capability(&name) {
            operators.insert(name, json!(cap));
        }
    }
    let table = json!({
        "capabilities": CAPABILITIES.iter().map(|(k, label, risky)| json!([k, label, risky])).collect::<Vec<_>>(),
        "blocks": blocks,
        "operators": operators,
    });
    let text = serde_json::to_string_pretty(&table).unwrap() + "\n";
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/capabilities.json");
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(path, &text).unwrap();
    }
    // A Windows checkout may have turned the file's line ends into CRLF.
    let saved = std::fs::read_to_string(path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        saved == text,
        "capabilities.json is out of date: rerun with BLESS=1"
    );
}

#[test]
fn every_reporter_is_decided() {
    let kinds: HashSet<&str> = CAPABILITIES.iter().map(|(k, ..)| *k).collect();
    for name in reporter_names() {
        match operator_capability(&name) {
            Some(cap) => {
                assert!(kinds.contains(cap), "{cap} has no label");
                assert!(
                    !UNGUARDED_OPERATORS.contains(&name.as_str()),
                    "{name} is both guarded and unguarded"
                );
            }
            None => assert!(
                UNGUARDED_OPERATORS.contains(&name.as_str()),
                "{name} needs an entry in operator_capability or UNGUARDED_OPERATORS"
            ),
        }
    }
}
