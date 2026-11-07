use super::*;
use crate::input::types::MacroButton;

#[test]
fn typed_values_keep_numbers_as_numbers() {
    use crate::input::value::Evaluated;
    assert_eq!(typed_value(" 42 "), Evaluated::Number(42.0));
    assert_eq!(typed_value("-1.5"), Evaluated::Number(-1.5));
    assert_eq!(typed_value("hello"), Evaluated::Text("hello".into()));
    assert_eq!(typed_value("inf"), Evaluated::Text("inf".into()));
    assert_eq!(typed_value(""), Evaluated::Text(String::new()));
}
use crate::input::types::Coordinate;
use crate::input::value::Evaluated;

#[test]
fn signal_names_migrate_and_unused_names_survive_round_trip() {
    let mut mac: Macro = serde_json::from_value(serde_json::json!({
        "name": "Old macro", "description": "", "signals": ["Unused"],
        "strands": [{"id": "s", "x": 0, "y": 0, "instructions": [
            {"id": "send", "kind": {"SendSignal": {"name": {"kind": "Text", "value": " Go "}, "wait": true}}},
            {"id": "receive", "kind": {"WhenSignal": {"kind": "Text", "value": "go"}}}
        ]}]
    })).unwrap();
    assert_eq!(mac.signals, ["Unused", "Go"]);
    mac.strands.clear();
    let restored: Macro = serde_json::from_str(&serde_json::to_string(&mac).unwrap()).unwrap();
    assert_eq!(restored.signals, ["Unused", "Go"]);
}

#[test]
fn signal_rename_updates_nested_named_references_but_preserves_expressions() {
    let mut mac = Macro::new(
        "Signals".into(),
        "".into(),
        vec![
            Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
            Instruction::new(InstructionKind::Repeat {
                count: Value::number(1.0),
                body: vec![
                    Instruction::new(InstructionKind::SendSignal {
                        name: Value::text(" GO ").into(),
                        wait: true,
                    }),
                    Instruction::new(InstructionKind::SendSignal {
                        name: SignalName::new(Value::text("go"), true),
                        wait: false,
                    }),
                ],
            }),
        ],
    );
    mac.collect_named_signals();
    assert!(mac.validate_signal_name(" GO ", None).is_err());
    assert!(mac.validate_signal_name(" ", None).is_err());
    mac.rename_signal("go", "finished").unwrap();
    assert_eq!(mac.signals, ["finished"]);
    let mut names = Vec::new();
    mac.graph.walk_instructions(&mut |ins| {
        if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
            &ins.kind
        {
            names.push((name.value.clone(), name.expression));
        }
    });
    assert_eq!(
        names,
        vec![
            (Value::text("finished"), false),
            (Value::text("finished"), false),
            (Value::text("go"), true)
        ]
    );
    let restored: Macro = serde_json::from_str(&serde_json::to_string(&mac).unwrap()).unwrap();
    assert_eq!(restored.signals, ["finished"]);
    assert_eq!(restored.graph, mac.graph);
}

#[test]
fn legacy_dynamic_signals_remain_expressions() {
    let value = Value::Op {
        op: Op::Join,
        args: vec![Value::text("task:"), Value::text("42")],
        saved: Box::new(Value::text("")),
    };
    let old = serde_json::json!({"WhenSignal": value});
    let instruction: InstructionKind = serde_json::from_value(old).unwrap();
    let InstructionKind::WhenSignal(name) = instruction else {
        panic!("expected signal receiver");
    };
    assert!(name.expression);
    assert_eq!(name.value, value);
}

#[test]
fn block_def_migrates_legacy_returns_value_true_to_returns_value_shape() {
    let def: BlockDef =
        serde_json::from_str(r#"{"id":"b1","pieces":[],"returns_value":true}"#).unwrap();
    assert_eq!(def.shape, BlockShape::ReturnsValue);
}

#[test]
fn block_def_migrates_legacy_returns_value_false_to_normal_shape() {
    let def: BlockDef =
        serde_json::from_str(r#"{"id":"b1","pieces":[],"returns_value":false}"#).unwrap();
    assert_eq!(def.shape, BlockShape::Normal);
}

#[test]
fn block_def_reads_current_shape_field() {
    let def: BlockDef =
        serde_json::from_str(r#"{"id":"b1","pieces":[],"shape":"ReturnsBool"}"#).unwrap();
    assert_eq!(def.shape, BlockShape::ReturnsBool);
}

#[test]
fn open_app_launch_options_are_optional_and_round_trip() {
    let old = r#"{"OpenApp":{"command":"brave","name":"Brave","icon":null}}"#;
    let kind: InstructionKind = serde_json::from_str(old).unwrap();
    let InstructionKind::OpenApp { launch, .. } = &kind else {
        panic!()
    };
    assert!(launch.is_empty());
    // Untouched blocks save the same shape as before.
    assert_eq!(serde_json::to_string(&kind).unwrap(), old);

    let legacy = r#"{"OpenApp":{"command":"brave","name":"Brave","icon":null,"launch":{"new_instance":true}}}"#;
    let migrated: InstructionKind = serde_json::from_str(legacy).unwrap();
    assert_eq!(migrated, kind);
    assert_eq!(serde_json::to_string(&migrated).unwrap(), old);

    let launch = LaunchOptions {
        args: "--incognito".into(),
        working_dir: "~/Downloads".into(),
        env: vec![EnvVar {
            name: "LANG".into(),
            value: "C".into(),
        }],
    };
    let kind = InstructionKind::OpenApp {
        command: "brave".into(),
        name: "Brave".into(),
        icon: None,
        launch,
    };
    let back: InstructionKind =
        serde_json::from_str(&serde_json::to_string(&kind).unwrap()).unwrap();
    assert_eq!(back, kind);
}

#[test]
fn block_def_round_trips_shape_through_serialize() {
    let def = BlockDef {
        id: "b1".into(),
        pieces: vec![],
        shape: BlockShape::Ending,
        color: default_block_color(),
        icon: String::new(),
        identifier: String::new(),
        description: String::new(),
    };
    let json = serde_json::to_string(&def).unwrap();
    assert!(
        json.contains(r#""shape":"Ending""#),
        "expected serialized shape field, got: {json}"
    );
    let round_tripped: BlockDef = serde_json::from_str(&json).unwrap();
    assert_eq!(round_tripped.shape, BlockShape::Ending);
}

#[test]
fn block_def_defaults_color_for_older_macro_files() {
    let def: BlockDef =
        serde_json::from_str(r#"{"id":"b1","pieces":[],"shape":"Normal"}"#).unwrap();
    assert_eq!(def.color, default_block_color());
}

#[test]
fn image_blocks_from_older_files_pick_the_best_match_anywhere() {
    let kind: InstructionKind = serde_json::from_str(
        r#"{"FindImage":{"image":"","tolerance":{"kind":"Number","value":10.0},"x_variable":"a","y_variable":"b"}}"#,
    )
    .unwrap();
    let InstructionKind::FindImage {
        pick,
        region,
        count_variable,
        ..
    } = kind
    else {
        panic!("{kind:?}")
    };
    assert_eq!((pick, region, count_variable), (None, None, None));
    let kind: InstructionKind = serde_json::from_str(
        r#"{"WaitForImage":{"image":"","tolerance":{"kind":"Number","value":10.0},"appear":true,"timeout":{"kind":"Number","value":0.0}}}"#,
    )
    .unwrap();
    assert!(matches!(
        kind,
        InstructionKind::WaitForImage { region: None, .. }
    ));
}

#[test]
fn browser_match_is_saved_only_once_added() {
    let plain = InstructionKind::BrowserClick {
        selector: Value::text("button"),
        nth: None,
    };
    let text = serde_json::to_string(&plain).unwrap();
    assert!(!text.contains("nth"), "{text}");
    let added = InstructionKind::BrowserType {
        text: Value::text("x"),
        selector: Value::text("focused"),
        nth: Some(Value::number(-1.0)),
    };
    let back: InstructionKind =
        serde_json::from_str(&serde_json::to_string(&added).unwrap()).unwrap();
    assert_eq!(back, added);
    let dto = crate::frontend::instruction_to_dto(&Instruction::new(added.clone()));
    assert_eq!(
        crate::frontend::dto_to_instruction(&dto).unwrap().kind,
        added
    );
}

#[test]
fn mouse_modifier_component_is_optional_and_preserves_saved_combos() {
    for modifiers in [
        None,
        Some(Value::text("")),
        Some(Value::text("Control+Shift")),
    ] {
        let ins = Instruction::new(InstructionKind::WhenMouseButton {
            button: MacroButton::Back,
            pressed: true,
            modifiers: modifiers.clone(),
        });
        let json = serde_json::to_value(&ins.kind).unwrap();
        assert_eq!(
            json["WhenMouseButton"].get("modifiers").is_some(),
            modifiers.is_some()
        );
        let loaded: InstructionKind = serde_json::from_value(json).unwrap();
        assert_eq!(loaded, ins.kind);
        let dto = crate::frontend::instruction_to_dto(&ins);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json.get("modifiers").is_some(), modifiers.is_some());
        let dto = serde_json::from_value(json).unwrap();
        let mut loaded = crate::frontend::dto_to_instruction(&dto).unwrap();
        assert_eq!(loaded.kind, ins.kind);
        assert_eq!(
            fields::value_slot_mut(&mut loaded.kind, fields::FieldId::MouseModifiers).is_some(),
            modifiers.is_some()
        );
    }
}

#[test]
fn optional_wait_and_tolerance_components_round_trip() {
    use crate::frontend::{InstructionDto, dto_to_instruction, instruction_to_dto};
    for json in [
        r#"{"type":"WaitUntil","id":"wait","condition":{"kind":"Bool"}}"#,
        r#"{"type":"WaitForImage","id":"image","image":"","appear":true}"#,
        r#"{"type":"FindImage","id":"find","image":"","x_variable":"x","y_variable":"y"}"#,
        r#"{"type":"ClickImage","id":"click","image":"","button":"Left"}"#,
    ] {
        let dto: InstructionDto = serde_json::from_str(json).unwrap();
        let mut ins = dto_to_instruction(&dto).unwrap();
        let plain = serde_json::to_value(instruction_to_dto(&ins)).unwrap();
        for key in ["timeout", "error_timeout", "tolerance"] {
            assert!(plain.get(key).is_none());
        }
        if let InstructionKind::WaitUntil {
            timeout,
            error_timeout,
            ..
        }
        | InstructionKind::WaitForImage {
            timeout,
            error_timeout,
            ..
        } = &mut ins.kind
        {
            *timeout = Some(Value::number(10.0));
            *error_timeout = Some(Value::number(20.0));
        }
        if let InstructionKind::FindImage { tolerance, .. }
        | InstructionKind::ClickImage { tolerance, .. }
        | InstructionKind::WaitForImage { tolerance, .. } = &mut ins.kind
        {
            *tolerance = Some(Value::number(0.0));
        }
        let loaded: InstructionKind =
            serde_json::from_str(&serde_json::to_string(&ins.kind).unwrap()).unwrap();
        assert_eq!(loaded, ins.kind);
        let dto = instruction_to_dto(&ins);
        let loaded: InstructionDto =
            serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
        assert_eq!(dto_to_instruction(&loaded).unwrap().kind, ins.kind);
    }
}

#[test]
fn image_block_components_are_saved_only_once_added() {
    let plain = InstructionKind::ClickImage {
        image: String::new(),
        tolerance: Some(Value::number(10.0)),
        button: MacroButton::Left,
        pick: None,
        region: None,
    };
    let text = serde_json::to_string(&plain).unwrap();
    assert!(!text.contains("pick") && !text.contains("region"), "{text}");
    let added = InstructionKind::ClickImage {
        image: String::new(),
        tolerance: Some(Value::number(10.0)),
        button: MacroButton::Left,
        pick: Some(ImageMatch {
            mode: ImagePick::NearPoint,
            index: Value::number(1.0),
            near_x: Value::number(5.0),
            near_y: Value::number(6.0),
        }),
        region: Some("0,0,10,10".into()),
    };
    let back: InstructionKind =
        serde_json::from_str(&serde_json::to_string(&added).unwrap()).unwrap();
    assert_eq!(back, added);
    let dto = crate::frontend::instruction_to_dto(&Instruction::new(added.clone()));
    assert_eq!(
        crate::frontend::dto_to_instruction(&dto).unwrap().kind,
        added
    );
}

#[test]
fn block_def_color_is_normalized_when_loading_a_macro() {
    let mac: Macro = serde_json::from_str(r##"{"id":"m1","name":"Test","description":"","strands":[],"block_defs":[{"id":"b1","pieces":[],"shape":"Normal","color":"#beef00"}]}"##).unwrap();
    assert_eq!(mac.block_defs[0].color, "#BEEF00");
}

#[test]
fn block_def_color_falls_back_when_loading_an_invalid_color() {
    let mac: Macro = serde_json::from_str(r#"{"id":"m1","name":"Test","description":"","strands":[],"block_defs":[{"id":"b1","pieces":[],"shape":"Normal","color":"not a color"}]}"#).unwrap();
    assert_eq!(mac.block_defs[0].color, default_block_color());
}

#[test]
fn macro_lists_live_in_the_graph_but_stay_flat_on_the_wire() {
    // Lists moved to blockstitch's `BlockGraph`, reached via `Deref`;
    // the save shape keeps `lists` next to `strands` either way.
    let mac: Macro = serde_json::from_str(
        r#"{"id":"m1","name":"Test","description":"","strands":[],"lists":[{"name":"queue","items":[{"kind":"Number","value":1.0}]}]}"#,
    )
    .unwrap();
    assert_eq!(mac.graph.lists.len(), 1);
    assert_eq!(mac.graph.lists[0].name, "queue");
    assert_eq!(mac.lists[0].name, "queue");
    let json = serde_json::to_string(&mac).unwrap();
    assert!(json.contains(r#""lists":[{"name":"queue""#), "got: {json}");
    let back: Macro = serde_json::from_str(&json).unwrap();
    assert_eq!(back, mac);
}

#[test]
fn new_macro_defaults_to_one_when_ran_strand() {
    let mac = Macro::new("Test".into(), "".into(), vec![]);
    assert_eq!(mac.strands.len(), 1);
    assert!(mac.strands[0].starts_with_header());
}

#[test]
fn detects_absolute_mouse_moves_in_nested_bodies() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    mac.strands[0]
        .instructions
        .push(Instruction::new(InstructionKind::If {
            condition: Value::Bool,
            body: vec![Instruction::new(InstructionKind::Token(
                InputToken::MoveMouse(Value::number(10.0), Value::number(20.0), Coordinate::Abs),
            ))],
        }));

    assert!(mac.has_absolute_mouse_move());
}

#[test]
fn legacy_flat_code_migrates_to_when_ran_strand() {
    let json = r#"{"name":"Old","description":"","code":[{"Comment":"hi"}]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    assert_eq!(mac.strands.len(), 1);
    // The legacy inline `Comment` instruction is pulled out into a
    // freestanding `Comment` note, not left in the instruction stream.
    assert_eq!(
        mac.strands[0].instructions,
        vec![Instruction::new(InstructionKind::WhenRan)]
    );
    assert_eq!(mac.comments.len(), 1);
    assert_eq!(mac.comments[0].text, "hi");
    assert_eq!(mac.comments[0].attached_to, None);
}

#[test]
fn legacy_root_strand_gains_when_ran_on_load() {
    let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"Comment":"hi"}]},
            {"id":"stray","x":10,"y":10,"instructions":[]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    let root = mac.strand("root").unwrap();
    assert!(root.starts_with_header());
    assert_eq!(
        root.instructions,
        vec![Instruction::new(InstructionKind::WhenRan)]
    );
    assert_eq!(mac.comments.len(), 1);
    assert_eq!(mac.comments[0].text, "hi");
    // Untouched, non-entry strand should survive as-is.
    assert!(mac.strand("stray").unwrap().instructions.is_empty());
}

#[test]
fn already_migrated_root_strand_is_not_double_prepended() {
    let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Comment":"hi"}]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    assert_eq!(
        mac.strand("root").unwrap().instructions,
        vec![Instruction::new(InstructionKind::WhenRan)]
    );
    assert_eq!(mac.comments.len(), 1);
    assert_eq!(mac.comments[0].text, "hi");
}

#[test]
fn migrate_legacy_comments_reaches_into_nested_if_body() {
    let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{"kind":"Bool"},"body":[{"Comment":"nested"}]}}
            ]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    let root = mac.strand("root").unwrap();
    match &root.instructions[1].kind {
        InstructionKind::If { body, .. } => assert!(
            body.is_empty(),
            "nested Comment should be extracted, not left in the If body"
        ),
        other => panic!("expected If, got {other:?}"),
    }
    assert_eq!(mac.comments.len(), 1);
    assert_eq!(mac.comments[0].text, "nested");
    assert_eq!(mac.comments[0].attached_to, None);
}

#[test]
fn prune_orphaned_comments_drops_comment_attached_to_removed_instruction() {
    let wait = Instruction::new(InstructionKind::Wait(Value::number(1000.0)));
    let wait_id = wait.id.clone();
    let mut mac = Macro::new("Test".into(), "".into(), vec![wait]);
    mac.comments.push(Comment {
        id: "c1".into(),
        x: 0,
        y: 0,
        text: "hi".into(),
        collapsed: false,
        attached_to: Some(wait_id),
    });
    mac.comments.push(Comment {
        id: "c2".into(),
        x: 0,
        y: 0,
        text: "freestanding".into(),
        collapsed: false,
        attached_to: None,
    });

    // Remove the Wait instruction (index 1 - index 0 is the WhenRan header).
    mac.strands[0].instructions.remove(1);
    mac.prune_orphaned_comments();

    assert_eq!(mac.comments.len(), 1);
    assert_eq!(mac.comments[0].id, "c2");
}

#[test]
fn prune_orphaned_comments_cascades_into_nested_wrap_body() {
    let inner = Instruction::new(InstructionKind::Wait(Value::number(1.0)));
    let inner_id = inner.id.clone();
    let if_ins = Instruction::new(InstructionKind::If {
        condition: Value::Bool,
        body: vec![inner],
    });
    let mut mac = Macro::new("Test".into(), "".into(), vec![if_ins]);
    mac.comments.push(Comment {
        id: "c1".into(),
        x: 0,
        y: 0,
        text: "nested".into(),
        collapsed: false,
        attached_to: Some(inner_id),
    });

    // Deleting the whole If block (index 1) takes its nested body with it.
    mac.strands[0].instructions.remove(1);
    mac.prune_orphaned_comments();

    assert!(mac.comments.is_empty());
}

#[test]
fn prune_orphaned_comments_keeps_comment_attached_to_surviving_instruction() {
    let wait = Instruction::new(InstructionKind::Wait(Value::number(1000.0)));
    let wait_id = wait.id.clone();
    let mut mac = Macro::new("Test".into(), "".into(), vec![wait]);
    mac.comments.push(Comment {
        id: "c1".into(),
        x: 0,
        y: 0,
        text: "hi".into(),
        collapsed: false,
        attached_to: Some(wait_id),
    });

    mac.prune_orphaned_comments();

    assert_eq!(mac.comments.len(), 1);
}

#[test]
fn migrate_bool_slots_repairs_poisoned_if_condition_on_load() {
    // Pre-`Value::Bool` save: dragging the default boolean block out of
    // an `If`'s condition once left a bare `Number` behind.
    let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{"kind":"Number","value":0.0},"body":[]}}
            ]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    match &mac.strand("root").unwrap().instructions[1].kind {
        InstructionKind::If { condition, .. } => assert_eq!(condition, &Value::Bool),
        other => panic!("expected If, got {other:?}"),
    }
}

#[test]
fn migrate_bool_slots_repairs_poisoned_operand_nested_inside_condition() {
    // The poisoned `Number` can be arbitrarily deep - here inside an
    // `And` that itself is the `If`'s condition. Its sibling (a real
    // comparison) must survive untouched.
    let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{
                    "kind":"Op","op":"And",
                    "args":[
                        {"kind":"Number","value":0.0},
                        {"kind":"Op","op":"Eq","args":[{"kind":"Number","value":1.0},{"kind":"Number","value":1.0}],"saved":{"kind":"Number","value":0.0}}
                    ],
                    "saved":{"kind":"Number","value":0.0}
                },"body":[]}}
            ]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    match &mac.strand("root").unwrap().instructions[1].kind {
        InstructionKind::If {
            condition: Value::Op {
                op: Op::And, args, ..
            },
            ..
        } => {
            assert_eq!(args[0], Value::Bool);
            assert_eq!(
                args[1],
                Value::Op {
                    op: Op::Eq,
                    args: vec![Value::number(1.0), Value::number(1.0)],
                    saved: Box::new(Value::Bool)
                }
            );
        }
        other => panic!("expected If(And(..)), got {other:?}"),
    }
}

#[test]
fn migrate_bool_slots_repairs_custom_block_boolean_arguments() {
    // The Boolean type of a custom-block argument lives in its
    // definition, not at the call site. A prior drag-out bug saved a zero
    // here, so loading must recover the blank hexagon from that type.
    let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"CallBlock":{
                "block_id":"b1","args":[{"kind":"Number","value":0.0}]
            }}]}
        ],"block_defs":[{"id":"b1","pieces":[
            {"kind":"Input","id":"i1","name":"flag","value_type":"Bool"}
        ],"shape":"Normal"}]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    let args = mac.strands[0]
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::CallBlock { args, .. } => Some(args),
            _ => None,
        })
        .expect("expected CallBlock");
    assert_eq!(args, &vec![Value::Bool]);
}

#[test]
fn migrate_bool_slots_leaves_legitimately_numeric_fields_alone() {
    // A `Wait` duration is never boolean-typed - a `Number` there is
    // always legitimate and must not be touched.
    let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Wait":{"kind":"Number","value":0.0}}]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    assert_eq!(
        mac.strand("root").unwrap().instructions[1],
        Instruction::new(InstructionKind::Wait(Value::number(0.0)))
    );
}

#[test]
fn legacy_wait_with_randomness_migrates_to_random_op() {
    let json = r#"{"Wait":[1000.0,50.0]}"#;
    let ins: Instruction = serde_json::from_str(json).unwrap();
    assert_eq!(
        ins,
        Instruction::new(InstructionKind::Wait(Value::Op {
            op: Op::Random,
            args: vec![
                Value::Op {
                    op: Op::Sub,
                    args: vec![Value::number(1000.0), Value::number(50.0)],
                    saved: Box::new(Value::number(0.0)),
                },
                Value::Op {
                    op: Op::Add,
                    args: vec![Value::number(1000.0), Value::number(50.0)],
                    saved: Box::new(Value::number(0.0)),
                },
            ],
            saved: Box::new(Value::number(0.0)),
        }))
    );
}

#[test]
fn legacy_wait_with_zero_randomness_migrates_to_plain_duration() {
    let json = r#"{"Wait":[1000.0,0.0]}"#;
    let ins: Instruction = serde_json::from_str(json).unwrap();
    assert_eq!(
        ins,
        Instruction::new(InstructionKind::Wait(Value::number(1000.0)))
    );
}

#[test]
fn legacy_when_time_moves_its_clock_into_a_time_value() {
    use crate::input::schedule::Weekday;
    let ins: Instruction = serde_json::from_str(
        r#"{"WhenTime":{"kind":"Weekly","weekday":"Monday","hour":7,"minute":5}}"#,
    )
    .unwrap();
    let expected = InstructionKind::WhenTime(
        TimeSchedule::Weekly {
            weekday: Weekday::Monday,
        },
        Value::text("07:05"),
    );
    assert_eq!(ins, Instruction::new(expected.clone()));
    // And the new shape reads back as itself.
    let round: Instruction = serde_json::from_str(&serde_json::to_string(&ins).unwrap()).unwrap();
    assert_eq!(round, Instruction::new(expected));
}

#[test]
fn legacy_single_arg_wait_migrates_to_value() {
    let json = r#"{"Wait":1000}"#;
    let ins: Instruction = serde_json::from_str(json).unwrap();
    assert_eq!(
        ins,
        Instruction::new(InstructionKind::Wait(Value::number(1000.0)))
    );
}

#[test]
fn fields_added_later_load_with_their_defaults() {
    let pattern = serde_json::to_string(&Value::text("Firefox")).unwrap();
    let json = format!(r#"{{"WindowAction":{{"action":"Focus","pattern":{pattern}}}}}"#);
    let ins: Instruction = serde_json::from_str(&json).unwrap();
    assert!(matches!(ins.kind, InstructionKind::WindowAction { ref by, .. } if by == "Title"));
}

#[test]
fn legacy_bare_number_move_mouse_fields_migrate_to_value() {
    let json = r#"{"Token":{"MoveMouse":[5,10,"Rel"]}}"#;
    let ins: Instruction = serde_json::from_str(json).unwrap();
    assert_eq!(
        ins,
        Instruction::new(InstructionKind::Token(InputToken::MoveMouse(
            Value::number(5.0),
            Value::number(10.0),
            Coordinate::Rel
        ))),
    );
}

#[test]
fn rename_variable_renames_declaration_and_every_reference() {
    let mut mac = Macro::new(
        "Test".into(),
        "".into(),
        vec![
            Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            )),
            Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::Var {
                    name: "x".to_string(),
                },
            )),
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "x".to_string(),
            }))),
        ],
    );
    mac.variables.push(VariableDef {
        name: "x".to_string(),
        value: Evaluated::Number(0.0),
    });
    mac.floating_values.push(FloatingValue {
        id: "f1".into(),
        x: 0,
        y: 0,
        value: Value::Var {
            name: "x".to_string(),
        },
        origin_block_id: None,
    });

    mac.rename_variable("x", "y").unwrap();

    assert_eq!(mac.variables[0].name, "y");
    let strand = &mac.strands[0];
    assert_eq!(
        strand.instructions[1],
        Instruction::new(InstructionKind::SetVariable(
            "y".to_string(),
            Value::number(1.0)
        ))
    );
    assert_eq!(
        strand.instructions[2],
        Instruction::new(InstructionKind::ChangeVariable(
            "y".to_string(),
            Value::Var {
                name: "y".to_string()
            }
        ))
    );
    assert_eq!(
        strand.instructions[3],
        Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
            name: "y".to_string()
        })))
    );
    assert_eq!(
        mac.floating_values[0].value,
        Value::Var {
            name: "y".to_string()
        }
    );
}

#[test]
fn rename_variable_rejects_an_undeclared_name_and_changes_nothing() {
    let mut mac = Macro::new(
        "Test".into(),
        "".into(),
        vec![Instruction::new(InstructionKind::Token(InputToken::Text(
            Value::Var {
                name: "x".to_string(),
            },
        )))],
    );
    assert!(mac.rename_variable("x", "y").is_err());
    assert_eq!(
        mac.strands[0].instructions[1],
        Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
            name: "x".to_string()
        })))
    );
}

#[test]
fn rename_var_reaches_into_if_body_and_condition() {
    let mut ins = Instruction::new(InstructionKind::If {
        condition: Value::Var {
            name: "x".to_string(),
        },
        body: vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::Var {
                name: "x".to_string(),
            },
        ))],
    });
    ins.rename_var("x", "y");
    match &ins.kind {
        InstructionKind::If { condition, body } => {
            assert_eq!(
                *condition,
                Value::Var {
                    name: "y".to_string()
                }
            );
            assert_eq!(
                body[0],
                Instruction::new(InstructionKind::SetVariable(
                    "y".to_string(),
                    Value::Var {
                        name: "y".to_string()
                    }
                ))
            );
        }
        _ => panic!("expected If"),
    }
}

#[test]
fn rename_var_reaches_into_if_else_both_branches() {
    let mut ins = Instruction::new(InstructionKind::IfElse {
        condition: Value::Var {
            name: "x".to_string(),
        },
        then_body: vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::number(1.0),
        ))],
        else_body: vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::number(2.0),
        ))],
    });
    ins.rename_var("x", "y");
    match &ins.kind {
        InstructionKind::IfElse {
            then_body,
            else_body,
            ..
        } => {
            assert_eq!(
                then_body[0],
                Instruction::new(InstructionKind::SetVariable(
                    "y".to_string(),
                    Value::number(1.0)
                ))
            );
            assert_eq!(
                else_body[0],
                Instruction::new(InstructionKind::SetVariable(
                    "y".to_string(),
                    Value::number(2.0)
                ))
            );
        }
        _ => panic!("expected IfElse"),
    }
}

#[test]
fn scrub_block_calls_reaches_into_nested_if_body() {
    let mut ins = Instruction::new(InstructionKind::If {
        condition: Value::number(1.0),
        body: vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::Call {
                block_id: "gone".to_string(),
                args: vec![],
                branches: vec![],
                saved: Box::new(Value::number(0.0)),
            },
        ))],
    });
    ins.scrub_block_calls("gone");
    match &ins.kind {
        InstructionKind::If { body, .. } => {
            assert_eq!(
                body[0],
                Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(0.0)
                ))
            );
        }
        _ => panic!("expected If"),
    }
}

#[test]
fn body_mut_addresses_if_and_if_else_slots() {
    let mut if_ins = InstructionKind::If {
        condition: Value::number(1.0),
        body: vec![Instruction::new(InstructionKind::Comment("a".into()))],
    };
    assert_eq!(
        if_ins.body_mut(0),
        Some(&mut vec![Instruction::new(InstructionKind::Comment(
            "a".into()
        ))])
    );
    assert_eq!(if_ins.body_mut(1), None);

    let mut if_else = InstructionKind::IfElse {
        condition: Value::number(1.0),
        then_body: vec![Instruction::new(InstructionKind::Comment("then".into()))],
        else_body: vec![Instruction::new(InstructionKind::Comment("else".into()))],
    };
    assert_eq!(
        if_else.body_mut(0),
        Some(&mut vec![Instruction::new(InstructionKind::Comment(
            "then".into()
        ))])
    );
    assert_eq!(
        if_else.body_mut(1),
        Some(&mut vec![Instruction::new(InstructionKind::Comment(
            "else".into()
        ))])
    );
    assert_eq!(if_else.body_mut(2), None);
}

#[test]
fn body_mut_addresses_loop_slots() {
    let mut repeat = InstructionKind::Repeat {
        count: Value::number(3.0),
        body: vec![Instruction::new(InstructionKind::Comment("a".into()))],
    };
    assert_eq!(
        repeat.body_mut(0),
        Some(&mut vec![Instruction::new(InstructionKind::Comment(
            "a".into()
        ))])
    );
    assert_eq!(repeat.body_mut(1), None);

    let mut forever = InstructionKind::Forever {
        body: vec![Instruction::new(InstructionKind::Comment("b".into()))],
    };
    assert_eq!(
        forever.body_mut(0),
        Some(&mut vec![Instruction::new(InstructionKind::Comment(
            "b".into()
        ))])
    );

    let mut while_ins = InstructionKind::While {
        condition: Value::Bool,
        body: vec![Instruction::new(InstructionKind::Comment("c".into()))],
    };
    assert_eq!(
        while_ins.body_mut(0),
        Some(&mut vec![Instruction::new(InstructionKind::Comment(
            "c".into()
        ))])
    );

    assert_eq!(InstructionKind::EscapeLoop.body_mut(0), None);
    assert_eq!(InstructionKind::ContinueLoop.body_mut(0), None);
}

#[test]
fn stop_scopes_round_trip_and_legacy_stop_defaults_to_all() {
    use crate::frontend::{InstructionDto, dto_to_instruction, instruction_to_dto};
    let legacy: super::InstructionKind = serde_json::from_str("\"StopMacro\"").unwrap();
    assert_eq!(
        legacy,
        super::InstructionKind::Stop {
            scope: super::StopScope::All
        }
    );
    let legacy_dto: InstructionDto =
        serde_json::from_value(serde_json::json!({"type":"StopMacro", "id":"old"})).unwrap();
    assert_eq!(dto_to_instruction(&legacy_dto).unwrap().kind, legacy);
    for scope in [
        super::StopScope::All,
        super::StopScope::ThisStrand,
        super::StopScope::OtherStrands,
    ] {
        let ins = super::Instruction::new(super::InstructionKind::Stop { scope });
        let json = serde_json::to_value(&ins).unwrap();
        assert_eq!(
            serde_json::from_value::<super::Instruction>(json).unwrap(),
            ins
        );
        let dto = instruction_to_dto(&ins);
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["type"], "Stop");
        let dto = serde_json::from_value::<InstructionDto>(json).unwrap();
        assert_eq!(dto_to_instruction(&dto).unwrap(), ins);
    }
}
