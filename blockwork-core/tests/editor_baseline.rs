//! A saved macro that exercises the shared editor model: nested bodies, a
//! custom reporter, comments, a list and a two-output block. It pins down the
//! on-disk shape and what a variable rename reaches today.
//! `BLOCKWORK_BLESS=1` rewrites the fixture in the current format.

use blockwork_core::input::value::Value;
use blockwork_core::macros::{Instruction, InstructionKind, Macro, migrate};
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-baseline/macro.json")
}

fn load() -> Macro {
    blockwork_core::init();
    let (mac, upgrade) = migrate::parse(&std::fs::read_to_string(fixture()).unwrap()).unwrap();
    assert!(
        !upgrade.changed(),
        "the fixture needed an upgrade: {upgrade:?}"
    );
    mac
}

fn find<'a>(list: &'a [Instruction], id: &str) -> Option<&'a Instruction> {
    list.iter().find_map(|ins| {
        if ins.id == id {
            return Some(ins);
        }
        match &ins.kind {
            InstructionKind::Repeat { body, .. } | InstructionKind::If { body, .. } => {
                find(body, id)
            }
            InstructionKind::IfElse {
                then_body,
                else_body,
                ..
            } => find(then_body, id).or_else(|| find(else_body, id)),
            InstructionKind::TryCatch { body, catch_body } => {
                find(body, id).or_else(|| find(catch_body, id))
            }
            _ => None,
        }
    })
}

fn kind<'a>(mac: &'a Macro, id: &str) -> &'a InstructionKind {
    let found = mac
        .graph
        .strands
        .iter()
        .find_map(|s| find(&s.instructions, id));
    &found.unwrap_or_else(|| panic!("no instruction {id}")).kind
}

fn reads(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Var { name } => out.push(name.clone()),
        Value::Op { args, .. } | Value::Call { args, .. } => {
            args.iter().for_each(|a| reads(a, out))
        }
        _ => {}
    }
}

#[test]
fn saved_macro_loads_without_upgrade_and_saves_back_unchanged() {
    blockwork_core::init();
    let (mac, upgrade) = migrate::parse(&std::fs::read_to_string(fixture()).unwrap()).unwrap();
    let saved = migrate::to_value(&mac).unwrap();
    if std::env::var_os("BLOCKWORK_BLESS").is_some() {
        std::fs::write(
            fixture(),
            serde_json::to_string_pretty(&saved).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    assert!(
        !upgrade.changed(),
        "the fixture needed an upgrade: {upgrade:?}"
    );
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture()).unwrap()).unwrap();
    assert_eq!(saved, original);
}

#[test]
fn renaming_a_variable_reaches_nested_reads_and_writes() {
    let mut mac = load();
    assert_eq!(
        mac.graph.rename_variable("score", "points").unwrap(),
        "points"
    );
    assert!(matches!(kind(&mac, "bump"), InstructionKind::ChangeVariable(n, _) if n == "points"));
    assert!(matches!(kind(&mac, "reset"), InstructionKind::SetVariable(n, _) if n == "points"));
    let InstructionKind::SetVariable(name, call) = kind(&mac, "doubled") else {
        panic!()
    };
    let mut names = vec![name.clone()];
    reads(call, &mut names);
    let InstructionKind::If { condition, .. } = kind(&mac, "check") else {
        panic!()
    };
    reads(condition, &mut names);
    let InstructionKind::Log(text) = kind(&mac, "say") else {
        panic!()
    };
    reads(text, &mut names);
    assert_eq!(names, ["points"; 4]);
    // Unrelated names stay as they were.
    let mut loose = Vec::new();
    reads(&mac.graph.floating_values[0].value, &mut loose);
    assert_eq!(loose, ["hit x", "hit y"]);
}

#[test]
fn renaming_reaches_both_find_image_outputs() {
    let mut mac = load();
    mac.graph.rename_variable("hit x", "px").unwrap();
    mac.graph.rename_variable("hit y", "py").unwrap();
    let InstructionKind::FindImage {
        x_variable,
        y_variable,
        ..
    } = kind(&mac, "find")
    else {
        panic!()
    };
    assert_eq!((x_variable.as_str(), y_variable.as_str()), ("px", "py"));
}
