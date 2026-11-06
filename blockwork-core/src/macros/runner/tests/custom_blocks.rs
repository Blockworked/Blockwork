//! Custom blocks, packs and value calls.

use super::*;

#[test]
fn custom_block_runs_the_callback_supplied_to_its_branch() {
    let vars = empty_vars();
    let block_id = "with_branch".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Branches".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::CallBlock {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![vec![Instruction::new(InstructionKind::SetVariable(
                                "ran".into(),
                                Value::number(1.0),
                            ))]],
                        }),
                    ],
                },
                Strand {
                    id: "body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::RunBranch("callback".into())),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![BlockPiece::Branch {
                    id: "b1".into(),
                    name: "callback".into(),
                }],
                shape: BlockShape::Normal,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("ran"),
        Some(&Evaluated::Number(1.0))
    );
}

#[test]
fn pack_state_is_isolated_persistent_and_available_to_reporters_and_triggers() {
    use crate::block_packs::{self, PackInfo, VariableInterface, VariableRole};
    use crate::macros::{BlockPiece, BlockShape, VariableDef};
    let mut mac = Macro::new("user".into(), String::new(), vec![]);
    mac.graph.variables.push(VariableDef {
        name: "counter".into(),
        value: Evaluated::Number(99.0),
    });
    let mut calls = Vec::new();
    for name in ["first", "second"] {
        let mut author = Macro::new("author".into(), String::new(), vec![]);
        author.graph.variables.push(VariableDef {
            name: "counter".into(),
            value: Evaluated::Number(5.0),
        });
        let block = author.create_block(
            vec![BlockPiece::Label {
                id: "label".into(),
                text: name.into(),
            }],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
        );
        author
            .graph
            .strands
            .iter_mut()
            .find(|s| s.block_header_id() == Some(block.as_str()))
            .unwrap()
            .instructions
            .push(Instruction::new(InstructionKind::ChangeVariable(
                "counter".into(),
                Value::number(1.0),
            )));
        let mut pack = block_packs::pack_from_macro(
            &author,
            std::slice::from_ref(&block),
            PackInfo {
                id: name.into(),
                name: name.into(),
                ..Default::default()
            },
        )
        .unwrap();
        pack.variable_roles.push(VariableInterface {
            name: "counter".into(),
            role: VariableRole::Output,
        });
        block_packs::merge_into(&mut mac, &pack, "");
        calls.push(Instruction::new(InstructionKind::CallBlock {
            block_id: block,
            args: vec![],
            branches: vec![],
        }));
    }
    let first = mac.settings.block_packs[0].clone();
    let second = mac.settings.block_packs[1].clone();
    let reporter = Value::Call {
        block_id: first.output_id("counter"),
        args: vec![],
        branches: vec![],
        saved: Box::new(Value::number(0.0)),
    };
    let mut body = calls.clone();
    body.push(Instruction::new(InstructionKind::SetVariable(
        "result".into(),
        reporter,
    )));
    mac.graph.strands.push(Strand::with_instructions(
        0,
        0,
        [
            vec![Instruction::new(InstructionKind::WhenRan)],
            body.clone(),
        ]
        .concat(),
    ));
    let vars = Arc::new(Mutex::new(mac.variable_values()));
    mac.clone()
        .run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    {
        let values = vars.lock().unwrap();
        assert_eq!(values.get("counter"), Some(&Evaluated::Number(99.0)));
        assert_eq!(values.get("result"), Some(&Evaluated::Number(6.0)));
        assert_eq!(
            values.get(&first.variable_key("counter")),
            Some(&Evaluated::Number(6.0))
        );
        assert_eq!(
            values.get(&second.variable_key("counter")),
            Some(&Evaluated::Number(6.0))
        );
        mac.sync_variables_from(&values);
    }
    let (loaded, _) =
        crate::macros::migrate::from_value(crate::macros::migrate::to_value(&mac).unwrap())
            .unwrap();
    let triggered = empty_vars();
    run_fired_strand(
        crate::run_log::Source::default(),
        body,
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&triggered),
        Arc::new(Mutex::new(HashMap::new())),
        loaded.block_table(),
        0,
    );
    assert_eq!(
        triggered.lock().unwrap().get("result"),
        Some(&Evaluated::Number(7.0))
    );
}

#[test]
fn shipped_text_helpers_pack_runs() {
    crate::input::value::register_blockwork_operators();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../blockwork-core/tests/fixtures/blocks/packs/text-helpers.json"
    );
    let pack = crate::block_packs::read_pack(&std::fs::read_to_string(path).unwrap()).unwrap();
    let id = |name: &str| {
        pack.content
            .block_defs
            .iter()
            .find(|b| crate::block_packs::block_name(b) == name)
            .unwrap()
            .id
            .clone()
    };
    let call = |name: &str, text: &str| Value::Call {
        block_id: id(name),
        args: vec![Value::text(text)],
        branches: vec![],
        saved: Box::new(Value::number(0.0)),
    };
    let set = |var: &str, value: Value| {
        Instruction::new(InstructionKind::SetVariable(var.to_string(), value))
    };
    let mut mac = Macro::new(
        "t".into(),
        String::new(),
        vec![
            set("cap", call("capitalize (text)", "hello")),
            set("empty", call("capitalize (text)", "")),
            set("last", call("last letter of (text)", "abc")),
            set("blank", call("(text) is empty?", "  ")),
        ],
    );
    crate::block_packs::merge_into(&mut mac, &pack, "");
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("cap"), Some(&Evaluated::Text("Hello".into())));
    assert_eq!(vars.get("empty"), Some(&Evaluated::Text(String::new())));
    assert_eq!(vars.get("last"), Some(&Evaluated::Text("c".into())));
    assert_eq!(vars.get("blank"), Some(&Evaluated::Bool(true)));
}

#[test]
fn value_call_runs_its_supplied_branch() {
    let mut mac = macro_with_double_block(vec![Instruction::new(InstructionKind::SetVariable(
        "result".into(),
        Value::Call {
            block_id: "double".into(),
            args: vec![Value::number(21.0)],
            branches: vec![vec![
                serde_json::to_value(crate::frontend::instruction_to_dto(&Instruction::new(
                    InstructionKind::SetVariable("branch_ran".into(), Value::number(7.0)),
                )))
                .unwrap(),
            ]],
            saved: Box::new(Value::number(0.0)),
        },
    ))]);
    mac.block_defs[0].pieces.push(BlockPiece::Branch {
        id: "callback".into(),
        name: "callback".into(),
    });
    let body = mac
        .strands
        .iter_mut()
        .find(|s| {
            s.instructions
                .first()
                .is_some_and(|i| matches!(&i.kind, InstructionKind::BlockHeader(_)))
        })
        .unwrap();
    body.instructions.insert(
        1,
        Instruction::new(InstructionKind::RunBranch("callback".into())),
    );
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("branch_ran"), Some(&Evaluated::Number(7.0)));
    assert_eq!(vars.get("result"), Some(&Evaluated::Number(42.0)));
}

#[test]
fn custom_components_bind_their_enabled_parameter_in_commands_and_reporters() {
    for reporter in [false, true] {
        for enabled in [false, true] {
            let vars = empty_vars();
            let args = vec![Evaluated::Bool(enabled).into_value(), Value::number(21.0)];
            let call = if reporter {
                InstructionKind::SetVariable(
                    "x".into(),
                    Value::Call {
                        block_id: "double".into(),
                        args,
                        branches: vec![],
                        saved: Box::new(Value::number(0.0)),
                    },
                )
            } else {
                InstructionKind::CallBlock {
                    block_id: "double".into(),
                    args,
                    branches: vec![],
                }
            };
            let mut mac = macro_with_double_block(vec![Instruction::new(call)]);
            mac.graph.block_defs[0].pieces = serde_json::from_value(serde_json::json!([
                {"kind":"Label","id":"name","text":"example"},
                {"kind":"Input","id":"enabled","name":"optional","value_type":"Bool","component":true},
                {"kind":"Input","id":"value","name":"n"}
            ])).unwrap();
            mac.graph.block_defs[0].shape = if reporter {
                BlockShape::ReturnsValue
            } else {
                BlockShape::Normal
            };
            let result = |value| {
                Instruction::new(if reporter {
                    InstructionKind::Return(value)
                } else {
                    InstructionKind::SetVariable("x".into(), value)
                })
            };
            mac.graph.strands[1].instructions.truncate(1);
            mac.graph.strands[1]
                .instructions
                .push(Instruction::new(InstructionKind::IfElse {
                    condition: Value::Param {
                        name: "optional".into(),
                    },
                    then_body: vec![result(Value::Param { name: "n".into() })],
                    else_body: vec![result(Value::number(7.0))],
                }));
            let (mac, _) =
                crate::macros::migrate::from_value(crate::macros::migrate::to_value(&mac).unwrap())
                    .unwrap();
            mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
            assert_eq!(
                vars.lock().unwrap().get("x"),
                Some(&Evaluated::Number(if enabled { 21.0 } else { 7.0 }))
            );
        }
    }
}

#[test]
fn value_call_to_reporter_block_resolves_and_runs() {
    let vars = empty_vars();
    let mac = macro_with_double_block(vec![Instruction::new(InstructionKind::SetVariable(
        "x".to_string(),
        Value::Call {
            block_id: "double".to_string(),
            args: vec![Value::number(21.0)],
            branches: vec![],
            saved: Box::new(Value::number(0.0)),
        },
    ))]);
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(42.0))
    );
}

/// A reporter block whose body never hits `Return` should leave
/// the caller's `SetVariable` un-applied rather than panicking or
/// defaulting to some value.
#[test]
fn value_call_to_block_without_return_leaves_variable_unset() {
    let vars = empty_vars();
    let block_id = "empty".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Empty".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "empty_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![Instruction::new(InstructionKind::BlockHeader(
                        block_id.clone(),
                    ))],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

/// A `CallBlock` with real `Wait`s inside its body should take real
/// time, proving the callee's instructions genuinely execute inline.
#[test]
fn call_block_runs_real_instructions_including_wait() {
    let block_id = "waiter".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Waiter".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::CallBlock {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![],
                        }),
                    ],
                },
                Strand {
                    id: "waiter_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Wait(Value::number(150.0))),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::Normal,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run(noop_emulator(), None, 1.0, empty_vars());
    assert!(
        start.elapsed() >= Duration::from_millis(140),
        "CallBlock's nested Wait didn't actually take real time"
    );
}

/// A watcher-fired trigger body can call the macro's custom blocks.
#[test]
fn fired_strand_can_call_custom_block() {
    let block_id = "setter".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Fired".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "trigger".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenClipboardChanged),
                        Instruction::new(InstructionKind::CallBlock {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![],
                        }),
                    ],
                },
                Strand {
                    id: "setter_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::number(1.0),
                        )),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::Normal,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let vars = empty_vars();
    let body = mac.strands[0].instructions[1..].to_vec();
    run_fired_strand(
        crate::run_log::Source::app("Fired"),
        body,
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        mac.block_table(),
        0,
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
}

/// A block that calls itself should error past `MAX_CALL_DEPTH` instead
/// of overflowing the stack.
#[test]
fn self_recursive_reporter_block_errs_past_max_depth() {
    let vars = empty_vars();
    let block_id = "loop".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Loop".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "loop_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Call {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    // Should return promptly (erroring out at MAX_CALL_DEPTH) rather than
    // hanging or crashing the test process via a stack overflow.
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

/// One reporter block calling another in its own `Return` expression
/// should compose correctly, proving multi-level `Value::Call`
/// resolution nests rather than working one level deep.
#[test]
fn reporter_block_composes_with_another_reporter_block() {
    let vars = empty_vars();
    let double_id = "double".to_string();
    let triple_id = "triple".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Compose".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: triple_id.clone(),
                                args: vec![Value::number(2.0)],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "double_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(double_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Op {
                            op: crate::input::value::Op::Mul,
                            args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
                Strand {
                    id: "triple_body".into(),
                    x: 0,
                    y: 0,
                    // triple(n) = double(n) + n  =>  triple(2) = 4 + 2 = 6
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(triple_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Op {
                            op: crate::input::value::Op::Add,
                            args: vec![
                                Value::Call {
                                    block_id: double_id.clone(),
                                    args: vec![Value::Param { name: "n".into() }],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                                Value::Param { name: "n".into() },
                            ],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![
                BlockDef {
                    id: double_id,
                    pieces: vec![BlockPiece::Input {
                        component: false,
                        id: "p1".into(),
                        name: "n".into(),
                        value_type: InputValueType::Any,
                        default_value: None,
                        multiline: false,
                    }],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                },
                BlockDef {
                    id: triple_id,
                    pieces: vec![BlockPiece::Input {
                        component: false,
                        id: "p1".into(),
                        name: "n".into(),
                        value_type: InputValueType::Any,
                        default_value: None,
                        multiline: false,
                    }],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                    identifier: String::new(),
                    description: String::new(),
                },
            ],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(6.0)));
}
