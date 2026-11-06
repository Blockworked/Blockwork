//! If, loops and return.

use super::*;

#[test]
fn if_runs_body_when_condition_true() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: true_cond(),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn if_skips_body_when_condition_false() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: false_cond(),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

#[test]
fn if_else_runs_the_matching_branch() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::IfElse {
            condition: true_cond(),
            then_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
            else_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(2.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));

    let vars2 = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::IfElse {
            condition: false_cond(),
            then_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
            else_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(2.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars2),
    );
    assert_eq!(
        vars2.lock().unwrap().get("x"),
        Some(&Evaluated::Number(2.0))
    );
}

#[test]
fn nested_if_inside_if_runs_only_when_both_true() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: true_cond(),
            body: vec![Instruction::new(InstructionKind::If {
                condition: true_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));

    let vars2 = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: true_cond(),
            body: vec![Instruction::new(InstructionKind::If {
                condition: false_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars2),
    );
    assert_eq!(vars2.lock().unwrap().get("x"), None);
}

/// A `Return` inside an `If` branch, within a custom block called via
/// `Value::Call`, should halt the block's body and hand its value back
/// to the caller - proving `run_block`'s `Ok(Some(_))` bubbles up
/// through nested `If` the same way it already does through `CallBlock`.
#[test]
fn return_inside_if_branch_bubbles_up_through_reporter_block() {
    let vars = empty_vars();
    let block_id = "cond_return".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "CondReturn".into(),
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
                    id: "body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::If {
                            condition: true_cond(),
                            body: vec![Instruction::new(InstructionKind::Return(Value::number(
                                42.0,
                            )))],
                        }),
                        // Never reached if the branch's Return correctly halted the body.
                        Instruction::new(InstructionKind::Return(Value::number(0.0))),
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
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(42.0))
    );
}

#[test]
fn repeat_runs_body_n_times() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(5.0),
            body: vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn repeat_with_zero_or_negative_count_never_runs_body() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(-3.0),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

#[test]
fn escape_loop_stops_repeat_early() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(10.0),
            body: vec![
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::If {
                    condition: true_cond(),
                    body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                }),
            ],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn continue_loop_skips_rest_of_iteration_but_keeps_looping() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(3.0),
            body: vec![
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::ContinueLoop),
                // Never reached - proves ContinueLoop halted this iteration.
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(100.0),
                )),
            ],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(3.0)));
}

#[test]
fn while_loop_runs_until_condition_goes_false() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(0.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::While {
            condition: Value::Op {
                op: crate::input::value::Op::Lt,
                args: vec![
                    Value::Var {
                        name: "x".to_string(),
                    },
                    Value::number(5.0),
                ],
                saved: Box::new(Value::Bool),
            },
            body: vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn while_loop_with_false_condition_never_runs_body() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::While {
            condition: false_cond(),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

#[test]
fn forever_loop_runs_until_escape_loop() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(0.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::Forever {
            body: vec![
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::IfElse {
                    condition: Value::Op {
                        op: crate::input::value::Op::Gte,
                        args: vec![
                            Value::Var {
                                name: "x".to_string(),
                            },
                            Value::number(3.0),
                        ],
                        saved: Box::new(Value::Bool),
                    },
                    then_body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                    else_body: vec![],
                }),
            ],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(3.0)));
}

/// A `Forever` loop with no `Break`/`Return` must still be interruptible
/// via the external stop flag rather than spinning the thread forever.
#[test]
fn forever_loop_is_interrupted_by_stop_flag() {
    let stop_flag = Arc::new(Mutex::new(true));
    let flag_clone = Arc::clone(&stop_flag);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        *flag_clone.lock().unwrap() = false;
    });
    let start = Instant::now();
    run_instructions(
        vec![Instruction::new(InstructionKind::Forever { body: vec![] })],
        noop_emulator(),
        Some(stop_flag),
        1.0,
        empty_vars(),
    );
    assert!(
        start.elapsed() < Duration::from_millis(1000),
        "forever loop wasn't stopped by the stop flag"
    );
}

/// `Return` inside a `Repeat` must unwind straight through the loop,
/// same as it already does through `If` - proving loop instructions
/// forward `Flow::Return` rather than swallowing it like `Break`.
#[test]
fn return_inside_repeat_bubbles_up_through_reporter_block() {
    let vars = empty_vars();
    let block_id = "loop_return".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "LoopReturn".into(),
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
                    id: "body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Repeat {
                            count: Value::number(10.0),
                            body: vec![Instruction::new(InstructionKind::Return(Value::number(
                                7.0,
                            )))],
                        }),
                        // Never reached if Return correctly halted the loop and the body.
                        Instruction::new(InstructionKind::Return(Value::number(0.0))),
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
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
}

/// `EscapeLoop`/`ContinueLoop` with no enclosing loop at all (a malformed
/// path, since the UI itself refuses to place one there) should be a
/// harmless no-op, mirroring how a stray `Return` at strand level today
/// is silently discarded.
#[test]
fn escape_loop_with_no_enclosing_loop_is_a_harmless_no_op() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::EscapeLoop),
            Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            )),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    // EscapeLoop with nothing to catch it halts the whole strand, same as
    // Return does today - the SetVariable after it never runs.
    assert_eq!(vars.lock().unwrap().get("x"), None);
}
