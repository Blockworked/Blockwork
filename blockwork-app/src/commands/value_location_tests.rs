use super::*;
use blockstitch_core::editor::apply_value_kind;
use blockstitch_core::editor::{EditCommand, ValueDestination, ValueLocation, prune_value_buffers};
use blockwork_core::frontend::{FragmentDto, InstructionDto, dto_to_fragment, fragment_to_dto};
use blockwork_core::input::types::Coordinate;
use blockwork_core::input::value::Op;
use blockwork_core::macros::BlockDef;
use blockwork_core::macros::EDIT_CONTEXT;
use blockwork_core::macros::{
    BlockShape, FieldId, FloatingValue, InputValueType, MacroGraph, VariableDef,
};

/// A flat, non-nested `InstrPath` - the shape every location was
/// addressed by before nested `If`/`IfElse` bodies existed.
fn top(index: usize) -> Vec<PathStep> {
    vec![PathStep { index, slot: None }]
}

fn test_macro() -> Macro {
    Macro {
        id: "m".into(),
        name: "Test".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![Strand {
                id: "s1".into(),
                x: 0,
                y: 0,
                instructions: vec![
                    Instruction::new(InstructionKind::WhenRan),
                    Instruction::new(InstructionKind::Wait(Value::number(1000.0))),
                ],
            }],
            floating_values: vec![FloatingValue {
                id: "f1".into(),
                x: 10,
                y: 20,
                value: Value::number(5.0),
                origin_block_id: None,
            }],
            ..MacroGraph::new()
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: blockwork_core::macros::MacroSettings::default(),
    }
}

#[test]
fn bundle_dependencies_are_transitive_and_handle_cycles() {
    let mut a = test_macro();
    a.id = "a".into();
    a.name = "First".into();
    a.strands[0].instructions = vec![Instruction::new(InstructionKind::RunMacro {
        target: Value::text(" second "),
        wait: false,
    })];
    let mut b = test_macro();
    b.id = "b".into();
    b.name = "Second".into();
    b.strands[0].instructions = vec![Instruction::new(InstructionKind::StopMacroNamed(
        Value::text("Third"),
    ))];
    let mut c = test_macro();
    c.id = "c".into();
    c.name = "Third".into();
    c.strands[0].instructions = vec![Instruction::new(InstructionKind::StopMacroNamed(
        Value::text("First"),
    ))];
    let all = vec![a, b, c];
    let ids = bundle_macros(&all, &["a".into()])
        .into_iter()
        .map(|m| m.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, ["a", "b", "c"]);
    assert!(bundle_macros(&all, &[]).is_empty());
    assert!(bundle_macros(&all, &["missing".into()]).is_empty());
}

#[test]
fn resolves_field_location() {
    let mut mac = test_macro();
    let loc = ValueLocation::Field {
        strand_id: "s1".into(),
        index: top(1),
        field_id: FieldId::WaitDuration.to_string(),
        path: vec![],
    };
    assert_eq!(mac.value_at_mut(&loc), Some(&mut Value::number(1000.0)));
}

#[test]
fn resolves_floating_location() {
    let mut mac = test_macro();
    let loc = ValueLocation::Floating {
        floating_id: "f1".into(),
        path: vec![],
    };
    assert_eq!(mac.value_at_mut(&loc), Some(&mut Value::number(5.0)));
}

#[test]
fn missing_location_resolves_to_none() {
    let mut mac = test_macro();
    let bad_field = ValueLocation::Field {
        strand_id: "nope".into(),
        index: top(0),
        field_id: FieldId::WaitDuration.to_string(),
        path: vec![],
    };
    assert_eq!(mac.value_at_mut(&bad_field), None);
    let bad_floating = ValueLocation::Floating {
        floating_id: "nope".into(),
        path: vec![],
    };
    assert_eq!(mac.value_at_mut(&bad_floating), None);
}

#[test]
fn apply_value_kind_tucks_leaf_away_as_saved() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Add", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Add,
            args: vec![Value::number(0.0), Value::number(0.0)],
            saved: Box::new(Value::number(5.0))
        }
    );
}

#[test]
fn apply_value_kind_collapses_operator_to_number_best_effort() {
    let mut node = Value::Op {
        op: Op::Add,
        args: vec![Value::number(2.0), Value::number(3.0)],
        saved: Box::new(Value::number(0.0)),
    };
    apply_value_kind(&mut node, "Number", &HashMap::new()).unwrap();
    assert_eq!(node, Value::number(5.0));
}

#[test]
fn apply_value_kind_random_tucks_leaf_away_as_saved() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Random", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Random,
            args: vec![Value::number(0.0), Value::number(0.0)],
            saved: Box::new(Value::number(5.0))
        }
    );
}

#[test]
fn apply_value_kind_rejects_unknown_kind() {
    let mut node = Value::number(0.0);
    assert!(apply_value_kind(&mut node, "Bogus", &HashMap::new()).is_err());
}

#[test]
fn apply_value_kind_var_becomes_a_plain_leaf() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Var:x", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Var {
            name: "x".to_string()
        }
    );
}

#[test]
fn apply_value_kind_number_best_effort_carries_over_variable_value() {
    let mut node = Value::Var {
        name: "x".to_string(),
    };
    let env = HashMap::from([("x".to_string(), Evaluated::Number(7.0))]);
    apply_value_kind(&mut node, "Number", &env).unwrap();
    assert_eq!(node, Value::number(7.0));
}

#[test]
fn apply_value_kind_join_tucks_leaf_away_as_saved() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Join", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text {
                    value: String::new()
                },
                Value::Text {
                    value: String::new()
                }
            ],
            saved: Box::new(Value::number(5.0)),
        }
    );
}

#[test]
fn apply_value_kind_join3_grows_existing_join_args() {
    let mut node = Value::Op {
        op: Op::Join,
        args: vec![
            Value::Text { value: "a".into() },
            Value::Text { value: "b".into() },
        ],
        saved: Box::new(Value::number(9.0)),
    };
    apply_value_kind(&mut node, "Join3", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text { value: "a".into() },
                Value::Text { value: "b".into() },
                Value::Text {
                    value: String::new()
                },
            ],
            saved: Box::new(Value::number(9.0)),
        }
    );
}

#[test]
fn apply_value_kind_join_shrinks_existing_join3_args() {
    let mut node = Value::Op {
        op: Op::Join,
        args: vec![
            Value::Text { value: "a".into() },
            Value::Text { value: "b".into() },
            Value::Text { value: "c".into() },
        ],
        saved: Box::new(Value::number(0.0)),
    };
    apply_value_kind(&mut node, "Join", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Join,
            args: vec![
                Value::Text { value: "a".into() },
                Value::Text { value: "b".into() }
            ],
            saved: Box::new(Value::number(0.0)),
        }
    );
}

#[test]
fn apply_value_kind_new_line_tucks_leaf_away_as_saved_with_no_args() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "NewLine", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::NewLine,
            args: vec![],
            saved: Box::new(Value::number(5.0))
        }
    );
}

#[test]
fn apply_value_kind_tab_tucks_leaf_away_as_saved_with_no_args() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Tab", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Tab,
            args: vec![],
            saved: Box::new(Value::number(5.0))
        }
    );
}

#[test]
fn apply_value_kind_swapping_join_to_new_line_drops_args() {
    let mut node = Value::Op {
        op: Op::Join,
        args: vec![
            Value::Text { value: "a".into() },
            Value::Text { value: "b".into() },
        ],
        saved: Box::new(Value::number(9.0)),
    };
    apply_value_kind(&mut node, "NewLine", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::NewLine,
            args: vec![],
            saved: Box::new(Value::number(9.0))
        }
    );
}

#[test]
fn apply_value_kind_round_tucks_leaf_away_as_saved_with_one_arg() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Round", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Round,
            args: vec![Value::number(0.0)],
            saved: Box::new(Value::number(5.0))
        }
    );
}

#[test]
fn apply_value_kind_math_tucks_leaf_away_with_abs_selector() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Math", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Math,
            args: vec![
                Value::Text {
                    value: "Abs".into()
                },
                Value::number(0.0)
            ],
            saved: Box::new(Value::number(5.0)),
        }
    );
}

#[test]
fn apply_value_kind_case_defaults_second_arg_to_upper() {
    let mut node = Value::number(5.0);
    apply_value_kind(&mut node, "Case", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::Case,
            args: vec![
                Value::Text {
                    value: String::new()
                },
                Value::Text {
                    value: "Upper".into()
                }
            ],
            saved: Box::new(Value::number(5.0)),
        }
    );
}

#[test]
fn apply_value_kind_letter_of_grows_from_round_with_mixed_type_default() {
    // Round (arity 1) -> LetterOf (arity 2): the surviving first arg
    // keeps its value untouched, and the newly-added second slot gets
    // LetterOf's own default type (text), not Round's (number).
    let mut node = Value::Op {
        op: Op::Round,
        args: vec![Value::number(7.0)],
        saved: Box::new(Value::number(0.0)),
    };
    apply_value_kind(&mut node, "LetterOf", &HashMap::new()).unwrap();
    assert_eq!(
        node,
        Value::Op {
            op: Op::LetterOf,
            args: vec![
                Value::number(7.0),
                Value::Text {
                    value: String::new()
                }
            ],
            saved: Box::new(Value::number(0.0)),
        }
    );
}

#[test]
fn prune_value_buffers_drops_only_descendants_of_changed_path() {
    let mut buffers: HashMap<ValueLocation, String> = HashMap::new();
    let field = |path: Vec<u8>| ValueLocation::Field {
        strand_id: "s1".into(),
        index: top(1),
        field_id: FieldId::WaitDuration.to_string(),
        path,
    };
    buffers.insert(field(vec![0]), "kept-sibling-subtree-root".into());
    buffers.insert(field(vec![1]), "dropped-descendant".into());
    buffers.insert(field(vec![1, 0]), "dropped-nested-descendant".into());
    buffers.insert(
        ValueLocation::Field {
            strand_id: "s2".into(),
            index: top(1),
            field_id: FieldId::WaitDuration.to_string(),
            path: vec![1],
        },
        "kept-different-strand".into(),
    );

    prune_value_buffers(&mut buffers, &field(vec![1]));

    assert_eq!(buffers.len(), 2);
    assert!(buffers.contains_key(&field(vec![0])));
    assert!(buffers.contains_key(&ValueLocation::Field {
        strand_id: "s2".into(),
        index: top(1),
        field_id: FieldId::WaitDuration.to_string(),
        path: vec![1]
    }));
}

#[test]
fn location_requires_integer_only_for_pixel_fields() {
    assert!(FieldId::MoveMouseX.requires_integer());
    assert!(FieldId::MoveMouseY.requires_integer());
    assert!(FieldId::ScrollAmount.requires_integer());
    assert!(!FieldId::WaitDuration.requires_integer());
    // And the graph looks it up through the instruction a location addresses.
    let mac = test_macro();
    assert!(!mac.location_requires_integer(&ValueLocation::Field {
        strand_id: "s1".into(),
        index: top(1),
        field_id: FieldId::WaitDuration.to_string(),
        path: vec![],
    }));
    assert!(!mac.location_requires_integer(&ValueLocation::Floating {
        floating_id: "f1".into(),
        path: vec![]
    }));
}

#[test]
fn floating_values_round_trip_through_json() {
    let mac = test_macro();
    let json = serde_json::to_string(&mac).unwrap();
    let back: Macro = serde_json::from_str(&json).unwrap();
    assert_eq!(back.floating_values, mac.floating_values);
}

#[test]
fn macro_without_floating_values_key_loads_with_empty_vec() {
    let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"s1","x":0,"y":0,"instructions":["WhenRan"]}
        ]}"#;
    let mac: Macro = serde_json::from_str(json).unwrap();
    assert!(mac.floating_values.is_empty());
}

#[test]
fn legacy_bare_number_floating_value_migrates() {
    let json = r#"{"id":"f1","x":10,"y":20,"value":5.0}"#;
    let fv: FloatingValue = serde_json::from_str(json).unwrap();
    assert_eq!(fv.value, Value::number(5.0));
}

#[test]
fn move_mouse_field_still_resolves_after_rework() {
    let mut mac = Macro {
        id: "m".into(),
        name: "T".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![Strand {
                id: "s1".into(),
                x: 0,
                y: 0,
                instructions: vec![Instruction::new(InstructionKind::Token(
                    blockwork_core::input::types::InputToken::MoveMouse(
                        Value::number(1.0),
                        Value::number(2.0),
                        Coordinate::Rel,
                    ),
                ))],
            }],
            ..MacroGraph::new()
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: blockwork_core::macros::MacroSettings::default(),
    };
    let loc = ValueLocation::Field {
        strand_id: "s1".into(),
        index: top(0),
        field_id: FieldId::MoveMouseY.to_string(),
        path: vec![],
    };
    assert_eq!(mac.value_at_mut(&loc), Some(&mut Value::number(2.0)));
}

#[test]
fn preview_value_stringifies_numeric_result() {
    let dto = Value::Op {
        op: Op::Add,
        args: vec![Value::Number { value: 2.0 }, Value::Number { value: 3.0 }],
        saved: Box::new(Value::Number { value: 0.0 }),
    };
    assert_eq!(
        preview_value_with_env(&dto, &HashMap::new()),
        Ok("5".to_string())
    );
}

#[test]
fn preview_value_joins_text_args() {
    let dto = Value::Op {
        op: Op::Join,
        args: vec![
            Value::Text {
                value: "foo".into(),
            },
            Value::Text {
                value: "bar".into(),
            },
        ],
        saved: Box::new(Value::Number { value: 0.0 }),
    };
    assert_eq!(
        preview_value_with_env(&dto, &HashMap::new()),
        Ok("foobar".to_string())
    );
}

#[test]
fn preview_value_surfaces_eval_errors() {
    let dto = Value::Op {
        op: Op::Div,
        args: vec![Value::Number { value: 1.0 }, Value::Number { value: 0.0 }],
        saved: Box::new(Value::Number { value: 0.0 }),
    };
    assert!(preview_value_with_env(&dto, &HashMap::new()).is_err());
}

#[tokio::test]
async fn palette_variable_creation_is_atomic_with_insertion_and_undo() {
    config::set_config_dir_override(std::env::temp_dir().join(format!(
        "blockwork-palette-variables-{}",
        std::process::id()
    )));
    blockwork_core::init();
    let mut initial = crate::initial_state(&config::AppSettings::default(), None, true, false);
    initial.current_macro = Some(Macro::new("Palette variables".into(), "".into(), vec![]));
    let initial_strands = initial.current_macro.as_ref().unwrap().strands.len();
    let state = Arc::new(Mutex::new(initial));
    let (events, _) = tokio::sync::broadcast::channel(16);
    let app = AppHandle { events };
    let variable_block = || {
        serde_json::from_value::<InstructionDto>(serde_json::json!({
            "id": "set", "type": "SetVariable", "name": "my variable",
            "value": {"kind": "Number", "value": 0}
        }))
        .unwrap()
    };
    let send = |cmd: &'static str, args: serde_json::Value| {
        let (state, app) = (state.clone(), app.clone());
        async move {
            edit_command(&state, &app, cmd, &args)
                .await
                .expect("a canvas command")
        }
    };
    let name = "my variable";
    send(
        "add_strand",
        serde_json::json!({"x": 20, "y": 30, "instruction": variable_block(), "variableToCreate": name}),
    )
    .await
    .unwrap();
    {
        let s = state.lock().unwrap();
        assert_eq!(s.current_macro.as_ref().unwrap().variables.len(), 1);
        assert_eq!(
            s.variable_values.lock().unwrap().get("my variable"),
            Some(&Evaluated::Number(0.0))
        );
    }
    undo(&state, &app).unwrap();
    {
        let s = state.lock().unwrap();
        assert!(s.current_macro.as_ref().unwrap().variables.is_empty());
        assert_eq!(
            s.current_macro.as_ref().unwrap().strands.len(),
            initial_strands
        );
        assert!(s.variable_values.lock().unwrap().is_empty());
    }
    redo(&state, &app).unwrap();
    {
        let mut s = state.lock().unwrap();
        s.current_macro.as_mut().unwrap().variables[0].value = Evaluated::Number(42.0);
        sync_variable_values(&mut s);
    }
    send(
        "add_strand",
        serde_json::json!({"instruction": variable_block(), "variableToCreate": name}),
    )
    .await
    .unwrap();
    {
        let s = state.lock().unwrap();
        assert_eq!(s.current_macro.as_ref().unwrap().variables.len(), 1);
        assert_eq!(
            s.variable_values.lock().unwrap().get("my variable"),
            Some(&Evaluated::Number(42.0))
        );
    }
    // Start a fresh canvas and attach the variable block to an existing strand.
    state.lock().unwrap().current_macro =
        Some(Macro::new("Snapped variables".into(), "".into(), vec![]));
    let strand = send("add_strand", serde_json::json!({}))
        .await
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let rejected = serde_json::json!({"id": "break", "type": "EscapeLoop"});
    let add = |instruction: serde_json::Value| serde_json::json!({"strandId": strand, "path": top(0), "instruction": instruction, "variableToCreate": name});
    assert!(send("add_instruction", add(rejected)).await.is_err());
    assert!(
        state
            .lock()
            .unwrap()
            .current_macro
            .as_ref()
            .unwrap()
            .variables
            .is_empty()
    );
    send(
        "add_instruction",
        add(serde_json::to_value(variable_block()).unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(
        state
            .lock()
            .unwrap()
            .current_macro
            .as_ref()
            .unwrap()
            .variables
            .len(),
        1
    );
    undo(&state, &app).unwrap();
    let s = state.lock().unwrap();
    let mac = s.current_macro.as_ref().unwrap();
    assert!(mac.variables.is_empty());
    assert!(mac.strand(&strand).unwrap().instructions.is_empty());
}

#[test]
fn create_variable_adds_variable_starting_at_zero() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let name = mac.create_variable("score").unwrap();
    assert_eq!(name, "score");
    assert_eq!(
        mac.variables,
        vec![VariableDef {
            name: "score".into(),
            value: Evaluated::Number(0.0)
        }]
    );
}

#[test]
fn create_variable_trims_whitespace() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let name = mac.create_variable("  score  ").unwrap();
    assert_eq!(name, "score");
}

#[test]
fn create_variable_rejects_empty_name() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    assert!(mac.create_variable("   ").is_err());
}

#[test]
fn create_variable_rejects_duplicate_name() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    mac.create_variable("score").unwrap();
    assert!(mac.create_variable("score").is_err());
    assert_eq!(mac.variables.len(), 1);
}

#[test]
fn rename_variable_renames_and_updates_references() {
    let mut mac = Macro::new(
        "Test".into(),
        "".into(),
        vec![Instruction::new(InstructionKind::SetVariable(
            "score".to_string(),
            Value::number(1.0),
        ))],
    );
    mac.create_variable("score").unwrap();
    let name = mac.rename_variable("score", "points").unwrap();
    assert_eq!(name, "points");
    assert_eq!(mac.variables[0].name, "points");
    assert_eq!(
        mac.strands[0].instructions[1],
        Instruction::new(InstructionKind::SetVariable(
            "points".to_string(),
            Value::number(1.0)
        ))
    );
}

#[test]
fn rename_variable_trims_whitespace() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    mac.create_variable("score").unwrap();
    let name = mac.rename_variable("score", "  points  ").unwrap();
    assert_eq!(name, "points");
}

#[test]
fn rename_variable_rejects_empty_name() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    mac.create_variable("score").unwrap();
    assert!(mac.rename_variable("score", "   ").is_err());
}

#[test]
fn rename_variable_rejects_duplicate_name() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    mac.create_variable("score").unwrap();
    mac.create_variable("points").unwrap();
    assert!(mac.rename_variable("score", "points").is_err());
}

#[test]
fn rename_variable_allows_renaming_to_its_own_current_name() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    mac.create_variable("score").unwrap();
    assert_eq!(mac.rename_variable("score", "score").unwrap(), "score");
    assert_eq!(mac.variables.len(), 1);
}

#[test]
fn remove_variable_removes_declaration_but_leaves_references() {
    let mut mac = Macro::new(
        "Test".into(),
        "".into(),
        vec![Instruction::new(InstructionKind::Token(InputToken::Text(
            Value::Var {
                name: "score".to_string(),
            }
            .into(),
        )))],
    );
    mac.create_variable("score").unwrap();
    mac.remove_variable("score");
    assert!(mac.variables.is_empty());
    assert_eq!(
        mac.strands[0].instructions[1],
        Instruction::new(InstructionKind::Token(InputToken::Text(
            Value::Var {
                name: "score".to_string()
            }
            .into()
        )))
    );
}

// ─── Custom blocks ("My Blocks") ────────────────────────────────────────

fn label(text: &str) -> BlockPiece {
    BlockPiece::Label {
        id: format!("id-{text}"),
        text: text.to_string(),
    }
}
fn input(id: &str, name: &str) -> BlockPiece {
    BlockPiece::Input {
        component: false,
        id: id.to_string(),
        name: name.to_string(),
        value_type: Default::default(),
        default_value: None,
        multiline: false,
    }
}

#[test]
fn validate_pieces_accepts_a_well_formed_prototype() {
    assert!(BlockDef::validate_pieces(&[label("double"), input("i1", "n")]).is_ok());
}

#[test]
fn validate_pieces_rejects_all_blank_labels() {
    assert!(BlockDef::validate_pieces(&[label("  "), input("i1", "n")]).is_err());
}

#[test]
fn validate_pieces_rejects_blank_input_name() {
    assert!(BlockDef::validate_pieces(&[label("double"), input("i1", "  ")]).is_err());
}

#[test]
fn validate_pieces_rejects_duplicate_input_names() {
    assert!(
        BlockDef::validate_pieces(&[label("add"), input("i1", "n"), input("i2", "n")]).is_err()
    );
}

#[test]
fn create_block_appends_def_and_empty_header_strand() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![label("double"), input("i1", "n")],
        BlockShape::ReturnsValue,
        "#4C97FF".into(),
        100,
        0,
    );
    assert_eq!(mac.block_defs.len(), 1);
    assert_eq!(mac.block_defs[0].id, id);
    assert_eq!(mac.block_defs[0].shape, BlockShape::ReturnsValue);
    let header_strand = mac.strands.iter().find(|s| {
        s.instructions == vec![Instruction::new(InstructionKind::BlockHeader(id.clone()))]
    });
    assert!(header_strand.is_some());
}

#[test]
fn reordering_branches_keeps_each_call_body_with_its_branch() {
    let branch = |id: &str, name: &str| BlockPiece::Branch {
        id: id.into(),
        name: name.into(),
    };
    let say = |text: &str| Instruction::new(InstructionKind::Log(Value::text(text)));
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![label("either"), branch("b1", "yes"), branch("b2", "no")],
        BlockShape::Normal,
        "#4C97FF".into(),
        0,
        0,
    );
    mac.strands.push(Strand {
        id: "caller".into(),
        x: 0,
        y: 0,
        instructions: vec![Instruction::new(InstructionKind::CallBlock {
            block_id: id.clone(),
            args: vec![],
            branches: vec![vec![say("yes")], vec![say("no")]],
        })],
    });
    mac.update_block(
        &id,
        vec![label("either"), branch("b2", "no"), branch("b1", "yes")],
        BlockShape::Normal,
        "#4C97FF",
    )
    .unwrap();
    let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
    let InstructionKind::CallBlock { branches, .. } = &caller.instructions[0].kind else {
        panic!("expected CallBlock")
    };
    assert_eq!(branches, &vec![vec![say("no")], vec![say("yes")]]);
}

#[test]
fn reconcile_block_call_args_preserves_value_across_rename_by_id() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![input("i1", "a")],
        BlockShape::Normal,
        "#4C97FF".into(),
        0,
        0,
    );
    // A CallBlock call site with one arg bound to the "a" slot.
    mac.strands.push(Strand {
        id: "caller".into(),
        x: 0,
        y: 0,
        instructions: vec![Instruction::new(InstructionKind::CallBlock {
            block_id: id.clone(),
            args: vec![Value::number(5.0)],
            branches: vec![],
        })],
    });
    let old_pieces = mac.block_defs[0].pieces.clone();
    let new_pieces = vec![input("i1", "b")]; // same id, renamed
    mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
    let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
    let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
        panic!("expected CallBlock")
    };
    assert_eq!(args, &vec![Value::number(5.0)]);
}

#[test]
fn reconcile_block_call_args_drops_removed_input_and_keeps_survivor() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![input("i1", "a"), input("i2", "b")],
        BlockShape::Normal,
        "#4C97FF".into(),
        0,
        0,
    );
    mac.strands.push(Strand {
        id: "caller".into(),
        x: 0,
        y: 0,
        instructions: vec![Instruction::new(InstructionKind::CallBlock {
            block_id: id.clone(),
            args: vec![Value::number(1.0), Value::number(2.0)],
            branches: vec![],
        })],
    });
    let old_pieces = mac.block_defs[0].pieces.clone();
    let new_pieces = vec![input("i2", "b")]; // "a" (i1) removed
    mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
    let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
    let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
        panic!("expected CallBlock")
    };
    assert_eq!(args, &vec![Value::number(2.0)]);
}

#[test]
fn reconcile_block_call_args_defaults_a_newly_added_input_to_zero() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![input("i1", "a")],
        BlockShape::Normal,
        "#4C97FF".into(),
        0,
        0,
    );
    mac.strands.push(Strand {
        id: "caller".into(),
        x: 0,
        y: 0,
        instructions: vec![Instruction::new(InstructionKind::CallBlock {
            block_id: id.clone(),
            args: vec![Value::number(5.0)],
            branches: vec![],
        })],
    });
    let old_pieces = mac.block_defs[0].pieces.clone();
    let new_pieces = vec![input("i1", "a"), input("i2", "b")]; // "b" newly added
    mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
    let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
    let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
        panic!("expected CallBlock")
    };
    assert_eq!(args, &vec![Value::number(5.0), Value::number(0.0)]);
}

#[test]
fn reconcile_block_call_args_defaults_a_newly_added_bool_input_to_blank_bool() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![input("i1", "a")],
        BlockShape::Normal,
        "#4C97FF".into(),
        0,
        0,
    );
    mac.strands.push(Strand {
        id: "caller".into(),
        x: 0,
        y: 0,
        instructions: vec![Instruction::new(InstructionKind::CallBlock {
            block_id: id.clone(),
            args: vec![Value::number(5.0)],
            branches: vec![],
        })],
    });
    let old_pieces = mac.block_defs[0].pieces.clone();
    let new_pieces = vec![
        input("i1", "a"),
        BlockPiece::Input {
            component: false,
            id: "i2".into(),
            name: "b".into(),
            value_type: InputValueType::Bool,
            default_value: None,
            multiline: false,
        },
    ];
    mac.reconcile_block_call_args(&id, &old_pieces, &new_pieces);
    let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
    let InstructionKind::CallBlock { args, .. } = &caller.instructions[0].kind else {
        panic!("expected CallBlock")
    };
    assert_eq!(args, &vec![Value::number(5.0), Value::Bool]);
}

#[test]
fn remove_block_scrubs_call_block_and_call_references() {
    let mut mac = Macro::new("Test".into(), "".into(), vec![]);
    let id = mac.create_block(
        vec![input("i1", "n")],
        BlockShape::ReturnsValue,
        "#4C97FF".into(),
        0,
        0,
    );
    mac.strands.push(Strand {
        id: "caller".into(),
        x: 0,
        y: 0,
        instructions: vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::Call {
                block_id: id.clone(),
                args: vec![],
                branches: vec![],
                saved: Box::new(Value::number(0.0)),
            },
        ))],
    });
    mac.remove_block(&id);
    assert!(mac.block_defs.is_empty());
    assert!(!mac.strands.iter().any(|s| matches!(
        s.instructions.first().map(|i| &i.kind),
        Some(InstructionKind::BlockHeader(_))
    )));
    let caller = mac.strands.iter().find(|s| s.id == "caller").unwrap();
    assert_eq!(
        caller.instructions[0],
        Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::number(0.0)
        ))
    );
}

/// An unresolved `Call` node must degrade to an ordinary `Err`, never panic.
#[test]
fn preview_value_with_env_errors_on_unresolved_call() {
    let dto = Value::Call {
        block_id: "missing".into(),
        args: vec![],
        branches: vec![],
        saved: Box::new(Value::Number { value: 0.0 }),
    };
    assert!(preview_value_with_env(&dto, &HashMap::new()).is_err());
}

#[test]
fn apply_value_kind_number_best_effort_defaults_to_zero_for_unresolved_call() {
    let mut node = Value::Call {
        block_id: "missing".into(),
        args: vec![],
        branches: vec![],
        saved: Box::new(Value::number(0.0)),
    };
    apply_value_kind(&mut node, "Number", &HashMap::new()).unwrap();
    assert_eq!(node, Value::number(0.0));
}

fn wait_field(path: Vec<u8>) -> ValueLocation {
    ValueLocation::Field {
        strand_id: "s1".into(),
        index: top(1),
        field_id: FieldId::WaitDuration.to_string(),
        path,
    }
}

#[test]
fn typed_spelling_survives_in_an_operand_through_the_shared_edit() {
    let mut mac = test_macro();
    mac.put_value(
        &wait_field(vec![]),
        Value::Op {
            op: Op::Add,
            args: vec![Value::number(1.0), Value::number(2.0)],
            saved: Box::new(Value::number(0.0)),
        },
    );
    for (path, text, stored) in [
        (vec![0], "09", Value::text("09")),
        (vec![1], "1.50", Value::text("1.50")),
    ] {
        let outcome = mac
            .apply_edit(
                EditCommand::EditFieldText {
                    location: wait_field(path.clone()),
                    text: text.into(),
                },
                &EDIT_CONTEXT,
            )
            .unwrap();
        assert!(outcome.changed);
        assert_eq!(mac.value_at(&wait_field(path)), Some(stored));
    }
}

#[test]
fn a_shared_move_matches_the_old_take_then_put() {
    let floating = ValueLocation::Floating {
        floating_id: "f1".into(),
        path: vec![],
    };
    let mut old = test_macro();
    let mut new = old.clone();
    let value = old.take_value(&floating).unwrap();
    old.put_value(&wait_field(vec![]), value);

    new.apply_edit(
        EditCommand::MoveValue {
            from: floating,
            to: ValueDestination::Slot(wait_field(vec![])),
        },
        &EDIT_CONTEXT,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&new.graph).unwrap(),
        serde_json::to_value(&old.graph).unwrap()
    );
}

#[test]
fn a_rejected_move_leaves_the_macro_alone() {
    let mut mac = test_macro();
    let before = serde_json::to_value(&mac.graph).unwrap();
    let missing = ValueLocation::Floating {
        floating_id: "nope".into(),
        path: vec![],
    };
    let result = mac.apply_edit(
        EditCommand::MoveValue {
            from: missing,
            to: ValueDestination::Slot(wait_field(vec![])),
        },
        &EDIT_CONTEXT,
    );
    assert_eq!(
        String::from(result.unwrap_err()),
        "Nothing to take at that location"
    );
    assert_eq!(serde_json::to_value(&mac.graph).unwrap(), before);
}

#[test]
fn delete_tail_is_one_edit() {
    let mut mac = test_macro();
    let outcome = mac
        .apply_edit(
            EditCommand::RemoveInstructions {
                strand_id: "s1".into(),
                path: top(0),
                count: 2,
            },
            &EDIT_CONTEXT,
        )
        .unwrap();
    assert!(outcome.changed);
    assert!(mac.strand("s1").unwrap().instructions.is_empty());
}

/// Cloning walks every body the kind declares, `catch_body` included.
#[test]
fn duplicating_a_try_renames_its_on_error_part_and_copies_notes() {
    let mut mac = test_macro();
    let caught = Instruction::new(InstructionKind::Wait(Value::number(1.0)));
    let caught_id = caught.id.clone();
    let try_block = Instruction::new(InstructionKind::TryCatch {
        body: vec![Instruction::new(InstructionKind::Wait(Value::number(2.0)))],
        catch_body: vec![caught],
    });
    mac.insert_instruction("s1", &top(2), try_block).unwrap();
    mac.comments.push(blockwork_core::macros::Comment::new(
        4,
        4,
        "n".into(),
        Some(caught_id.clone()),
    ));
    let outcome = mac
        .apply_edit(
            EditCommand::DuplicateInstruction {
                strand_id: "s1".into(),
                path: top(2),
            },
            &EDIT_CONTEXT,
        )
        .unwrap();
    let list = &mac.strand("s1").unwrap().instructions;
    assert_eq!(list.len(), 4);
    let InstructionKind::TryCatch { catch_body, .. } = &list[3].kind else {
        panic!()
    };
    assert_ne!(catch_body[0].id, caught_id);
    assert_eq!(outcome.ids[&caught_id], catch_body[0].id);
    assert_eq!(mac.instruction_ids().len(), 2 + 3 + 3);
    assert_eq!(
        mac.comments.last().unwrap().attached_to.as_ref(),
        Some(&catch_body[0].id)
    );
}

#[test]
fn a_fragment_round_trips_through_the_frontend_dto() {
    let mut mac = test_macro();
    let wait_id = mac.strand("s1").unwrap().instructions[1].id.clone();
    mac.comments.push(blockwork_core::macros::Comment::new(
        1,
        2,
        "n".into(),
        Some(wait_id.clone()),
    ));
    let fragment = mac.copy_fragment("s1", &top(1), None).unwrap();
    let dto: FragmentDto =
        serde_json::from_value(serde_json::to_value(fragment_to_dto(&fragment)).unwrap()).unwrap();
    let back = dto_to_fragment(&dto).unwrap();
    assert_eq!(back, fragment);
    assert_eq!(back.instructions[0].id, wait_id);
    let outcome = mac
        .apply_edit(
            EditCommand::PasteFragment {
                fragment: back,
                x: 5,
                y: 6,
            },
            &EDIT_CONTEXT,
        )
        .unwrap();
    assert!(outcome.changed);
    assert_ne!(outcome.ids[&wait_id], wait_id);
    assert_eq!(mac.comments.len(), 2);
}
