//! Variables, lists and globals.

use super::*;

#[test]
fn list_commands_and_reporters_share_the_same_literal_store() {
    use crate::input::value::Op;

    let text = |value: &str| Value::Text {
        value: value.to_string(),
    };
    let reporter = |op, args| Value::Op {
        op,
        args,
        saved: Box::new(Value::number(0.0)),
    };
    let variables: VariableStore = Arc::new(Mutex::new(HashMap::new()));
    let lists: ListStore = Arc::new(Mutex::new(HashMap::from([(
        "items".to_string(),
        vec![ListItem::Text("one".into()), ListItem::Number(2.0)],
    )])));

    run_instructions_with_lists(
        vec![
            Instruction::new(InstructionKind::AddToList {
                value: Value::number(3.0),
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::InsertIntoList {
                value: text("z"),
                index: Value::number(2.0),
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::ReplaceItemOfList {
                index: Value::number(3.0),
                name: "items".into(),
                value: text("y"),
            }),
            Instruction::new(InstructionKind::ShiftList {
                name: "items".into(),
                amount: Value::number(1.0),
            }),
            Instruction::new(InstructionKind::DeleteOfList {
                index: Value::number(2.0),
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::ReverseList {
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::SetVariable(
                "item".into(),
                reporter(
                    Op::from_name("ListItem"),
                    vec![Value::number(1.0), text("items")],
                ),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "position".into(),
                reporter(
                    Op::from_name("ListItemNumber"),
                    vec![text("z"), text("items")],
                ),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "amount".into(),
                reporter(Op::from_name("ListAmount"), vec![text("y"), text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "length".into(),
                reporter(Op::from_name("ListLength"), vec![text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "contains".into(),
                reporter(
                    Op::from_name("ListContains"),
                    vec![text("items"), text("z")],
                ),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "exists".into(),
                reporter(
                    Op::from_name("ListItemExists"),
                    vec![Value::number(3.0), text("items")],
                ),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "empty".into(),
                reporter(Op::from_name("ListIsEmpty"), vec![text("items")]),
            )),
            Instruction::new(InstructionKind::DeleteAllOfList {
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::SetVariable(
                "now_empty".into(),
                reporter(Op::from_name("ListIsEmpty"), vec![text("items")]),
            )),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&variables),
        Arc::clone(&lists),
    );

    assert!(lists.lock().unwrap().get("items").unwrap().is_empty());
    let variables = variables.lock().unwrap();
    assert_eq!(variables.get("item"), Some(&Evaluated::Text("y".into())));
    assert_eq!(variables.get("position"), Some(&Evaluated::Number(2.0)));
    assert_eq!(variables.get("amount"), Some(&Evaluated::Number(1.0)));
    assert_eq!(variables.get("length"), Some(&Evaluated::Number(3.0)));
    assert_eq!(variables.get("contains"), Some(&Evaluated::Bool(true)));
    assert_eq!(variables.get("exists"), Some(&Evaluated::Bool(true)));
    assert_eq!(variables.get("empty"), Some(&Evaluated::Bool(false)));
    assert_eq!(variables.get("now_empty"), Some(&Evaluated::Bool(true)));
}

#[test]
fn resolve_list_reporters_supports_canvas_preview_without_running_a_macro() {
    use crate::input::value::Op;

    let lists = HashMap::from([(
        "items".to_string(),
        vec![ListItem::Text("first".into()), ListItem::Number(7.0)],
    )]);
    let list_item = Value::Op {
        op: Op::from_name("ListItem"),
        args: vec![
            Value::number(2.0),
            Value::Text {
                value: "items".into(),
            },
        ],
        saved: Box::new(Value::number(0.0)),
    };
    let preview = resolve_list_reporters(&list_item, &lists).and_then(|value| value.eval_text());
    assert_eq!(preview, Ok("7".to_string()));

    let nested_length = Value::Op {
        op: Op::Add,
        args: vec![
            Value::Op {
                op: Op::from_name("ListLength"),
                args: vec![Value::Text {
                    value: "items".into(),
                }],
                saved: Box::new(Value::number(0.0)),
            },
            Value::number(1.0),
        ],
        saved: Box::new(Value::number(0.0)),
    };
    let preview =
        resolve_list_reporters(&nested_length, &lists).and_then(|value| value.eval_text());
    assert_eq!(preview, Ok("3".to_string()));
}

#[test]
fn set_variable_writes_evaluated_value() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::number(5.0),
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn change_variable_adds_to_existing_numeric_value() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(10.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::ChangeVariable(
            "x".to_string(),
            Value::number(5.0),
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(15.0))
    );
}

#[test]
fn change_variable_coerces_non_numeric_current_value_to_zero() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Text("hello".to_string()));
    run_instructions(
        vec![Instruction::new(InstructionKind::ChangeVariable(
            "x".to_string(),
            Value::number(5.0),
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn change_variable_with_text_delta_is_a_no_op() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(10.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::ChangeVariable(
            "x".to_string(),
            Value::Text {
                value: "abc".to_string(),
            },
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(10.0))
    );
}

#[test]
fn set_variable_value_can_read_other_variables() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("y".to_string(), Evaluated::Number(7.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::Var {
                name: "y".to_string(),
            },
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
}

#[test]
fn globals_accept_variable_and_computed_names() {
    crate::input::value::register_blockwork_operators();
    let name = "runner-dynamic-global-picker";
    crate::globals::remove(name);
    crate::globals::create(name).unwrap();
    let dynamic = Value::Var {
        name: "target".into(),
    };
    let computed = Value::op(
        Op::from_name("Join"),
        vec![Value::text("runner-dynamic-"), Value::text("global-picker")],
    );
    let mac = Macro::new(
        "Dynamic globals".into(),
        String::new(),
        vec![
            set("target", Value::text(name)),
            set(
                "initial",
                Value::op(Op::from_name("GlobalVariable"), vec![dynamic.clone()]),
            ),
            Instruction::new(InstructionKind::SetGlobal {
                name: dynamic.clone(),
                value: Value::number(4.0),
            }),
            Instruction::new(InstructionKind::ChangeGlobal {
                name: computed,
                by: Value::number(3.0),
            }),
            set(
                "result",
                Value::op(Op::from_name("GlobalVariable"), vec![dynamic]),
            ),
        ],
    );
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("initial"), Some(&Evaluated::Text(String::new())));
    assert_eq!(vars.get("result"), Some(&Evaluated::Number(7.0)));
    crate::globals::remove(name);
}

#[test]
fn for_each_loops_and_dict_blocks() {
    use crate::input::value::Op;
    use blockstitch_core::graph::{DictEntry, DictItem, DictStore};

    let text = |value: &str| Value::text(value);
    let var = |name: &str| Value::Var { name: name.into() };
    let join = |a: Value, b: Value| Value::op(Op::from_name("Join"), vec![a, b]);
    let append = |into: &str, value: Value| set(into, join(var(into), value));
    let variables: VariableStore = Arc::new(Mutex::new(HashMap::from([
        ("seen".to_string(), Evaluated::Text(String::new())),
        ("parts".to_string(), Evaluated::Text(String::new())),
        ("keys".to_string(), Evaluated::Text(String::new())),
    ])));
    let lists: ListStore = Arc::new(Mutex::new(HashMap::from([(
        "items".to_string(),
        vec![
            ListItem::Text("a".into()),
            ListItem::Number(2.0),
            ListItem::Text("stop".into()),
            ListItem::Text("never".into()),
        ],
    )])));
    let dicts: DictStore = Arc::new(Mutex::new(HashMap::from([(
        "prices".to_string(),
        vec![DictEntry::new("tea".into(), DictItem::Number(3.0))],
    )])));

    run_fired_strand(
        crate::run_log::Source::default(),
        vec![
            // Escape loop leaves a for-each like any other loop.
            Instruction::new(InstructionKind::ForEachItem {
                list: "items".into(),
                variable: "item".into(),
                body: vec![
                    Instruction::new(InstructionKind::If {
                        condition: Value::op(Op::Eq, vec![var("item"), text("stop")]),
                        body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                    }),
                    append("seen", var("item")),
                ],
            }),
            Instruction::new(InstructionKind::ForEachPart {
                text: text("x;y;z"),
                separator: text(";"),
                variable: "part".into(),
                body: vec![append("parts", var("part"))],
            }),
            Instruction::new(InstructionKind::SetDictKey {
                name: "prices".into(),
                key: text("cake"),
                value: Value::number(5.0),
            }),
            Instruction::new(InstructionKind::SetDictKey {
                name: "prices".into(),
                key: text("tea"),
                value: Value::number(4.0),
            }),
            Instruction::new(InstructionKind::ForEachKey {
                dict: "prices".into(),
                variable: "key".into(),
                body: vec![append(
                    "keys",
                    join(
                        var("key"),
                        Value::op(Op::from_name("DictValue"), vec![var("key"), text("prices")]),
                    ),
                )],
            }),
            Instruction::new(InstructionKind::DeleteDictKey {
                name: "prices".into(),
                key: text("tea"),
            }),
            set(
                "size",
                Value::op(Op::from_name("DictSize"), vec![text("prices")]),
            ),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&variables),
        lists,
        Arc::clone(&dicts),
        Arc::new(HashMap::new()),
        0,
    );
    {
        let vars = variables.lock().unwrap();
        let get = |name: &str| vars.get(name).cloned().map(|v| v.as_text());
        assert_eq!(get("seen").as_deref(), Some("a2"));
        assert_eq!(get("parts").as_deref(), Some("xyz"));
        assert_eq!(get("keys").as_deref(), Some("tea4cake5"));
        assert_eq!(get("size").as_deref(), Some("1"));
    }

    // Filled from JSON, then cleared.
    run_fired_strand(
        crate::run_log::Source::default(),
        vec![
            Instruction::new(InstructionKind::DictFromJson {
                text: text(r#"{"name": "Sam", "age": 7}"#),
                name: "prices".into(),
            }),
            set(
                "name",
                Value::op(
                    Op::from_name("DictValue"),
                    vec![text("name"), text("prices")],
                ),
            ),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&variables),
        Default::default(),
        Arc::clone(&dicts),
        Arc::new(HashMap::new()),
        0,
    );
    assert_eq!(
        variables.lock().unwrap().get("name"),
        Some(&Evaluated::Text("Sam".into()))
    );
    run_fired_strand(
        crate::run_log::Source::default(),
        vec![Instruction::new(InstructionKind::ClearDict {
            name: "prices".into(),
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&variables),
        Default::default(),
        Arc::clone(&dicts),
        Arc::new(HashMap::new()),
        0,
    );
    assert!(dicts.lock().unwrap()["prices"].is_empty());
}

#[test]
fn for_each_file_walks_a_folder_in_name_order() {
    let dir = std::env::temp_dir().join(format!("blockwork-each-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["b.txt", "a.txt"] {
        std::fs::write(dir.join(name), "").unwrap();
    }
    let variables: VariableStore = Arc::new(Mutex::new(HashMap::from([(
        "found".to_string(),
        Evaluated::Number(0.0),
    )])));
    run_instructions(
        vec![Instruction::new(InstructionKind::ForEachFile {
            folder: Value::text(dir.to_str().unwrap()),
            variable: "path".into(),
            body: vec![bump("found")],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&variables),
    );
    let vars = variables.lock().unwrap();
    assert_eq!(vars.get("found"), Some(&Evaluated::Number(2.0)));
    assert_eq!(
        vars.get("path"),
        Some(&Evaluated::Text(
            dir.join("b.txt").to_string_lossy().into_owned()
        ))
    );
    std::fs::remove_dir_all(&dir).ok();
}
