use super::*;

#[test]
fn stop_this_strand_unwinds_loops_and_custom_blocks_only() {
    let mut mac = macro_with_double_block(vec![
        Instruction::new(InstructionKind::Forever {
            body: vec![Instruction::new(InstructionKind::CallBlock {
                block_id: "double".into(),
                args: vec![],
                branches: vec![],
            })],
        }),
        set("after", Value::number(1.0)),
    ]);
    mac.graph.strands[1].instructions.truncate(1);
    mac.graph.strands[1].instructions.extend([
        Instruction::new(InstructionKind::Stop {
            scope: StopScope::ThisStrand,
        }),
        set("inside_after", Value::number(1.0)),
    ]);
    let mut sibling = when_ran_strand("sibling", 30.0);
    sibling
        .instructions
        .push(set("sibling", Value::number(1.0)));
    mac.graph.strands.push(sibling);
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("sibling"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("after"), None);
    assert_eq!(vars.get("inside_after"), None);
}

#[test]
fn stop_other_strands_cancels_waits_and_keeps_current_strand_running() {
    let mut mac = Macro::new(
        "Stop others".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::Stop {
                scope: StopScope::OtherStrands,
            }),
            set("current", Value::number(1.0)),
        ],
    );
    let mut sibling = when_ran_strand("sibling", 5_000.0);
    sibling
        .instructions
        .push(set("sibling", Value::number(1.0)));
    mac.graph.strands.push(sibling);
    let vars = empty_vars();
    let flag = Arc::new(Mutex::new(true));
    let start = Instant::now();
    mac.run(
        noop_emulator(),
        Some(Arc::clone(&flag)),
        1.0,
        Arc::clone(&vars),
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(
        *flag.lock().unwrap(),
        "stopping siblings must not stop the run"
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("current"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("sibling"), None);
}

#[test]
fn stop_scopes_include_signal_strands_and_allow_new_signals() {
    for scope in [
        StopScope::All,
        StopScope::ThisStrand,
        StopScope::OtherStrands,
    ] {
        let mut mac = Macro::new(
            "Signal stops".into(),
            String::new(),
            vec![
                Instruction::new(InstructionKind::SendSignal {
                    name: Value::text("go").into(),
                    wait: true,
                }),
                set("parent", Value::number(1.0)),
                Instruction::new(InstructionKind::SendSignal {
                    name: Value::text("later").into(),
                    wait: true,
                }),
            ],
        );
        mac.graph.strands.push(Strand::with_instructions(
            0,
            200,
            vec![
                Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
                Instruction::new(InstructionKind::Stop { scope }),
                set("receiver", Value::number(1.0)),
                Instruction::new(InstructionKind::SendSignal {
                    name: Value::text("later").into(),
                    wait: true,
                }),
            ],
        ));
        mac.graph.strands.push(Strand::with_instructions(
            0,
            400,
            vec![
                Instruction::new(InstructionKind::WhenSignal(Value::text("later").into())),
                set("later", Value::number(1.0)),
            ],
        ));
        let vars = empty_vars();
        mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
        let vars = vars.lock().unwrap();
        assert_eq!(
            vars.contains_key("parent"),
            scope == StopScope::ThisStrand,
            "{scope:?}"
        );
        assert_eq!(
            vars.contains_key("receiver"),
            scope == StopScope::OtherStrands,
            "{scope:?}"
        );
        assert_eq!(
            vars.contains_key("later"),
            scope != StopScope::All,
            "{scope:?}"
        );
    }
}

#[test]
fn stop_all_leaves_another_macro_running() {
    let vars = empty_vars();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            run_instructions(
                vec![
                    Instruction::new(InstructionKind::Wait(Value::number(40.0))),
                    set("unrelated", Value::number(1.0)),
                ],
                noop_emulator(),
                None,
                1.0,
                Arc::clone(&vars),
            );
        });
        run_instructions(
            vec![Instruction::new(InstructionKind::Stop {
                scope: StopScope::All,
            })],
            noop_emulator(),
            None,
            1.0,
            empty_vars(),
        );
    });
    assert_eq!(
        vars.lock().unwrap().get("unrelated"),
        Some(&Evaluated::Number(1.0))
    );
}

#[test]
fn stop_macro_stops_every_strand_of_the_run() {
    let mut stopper = when_ran_strand("b", 0.0);
    stopper
        .instructions
        .push(Instruction::new(InstructionKind::Stop {
            scope: StopScope::All,
        }));
    let mac = Macro {
        id: "m".into(),
        name: "Stops itself".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![when_ran_strand("a", 5_000.0), stopper],
            ..MacroGraph::new()
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run(noop_emulator(), None, 1.0, empty_vars());
    assert!(
        start.elapsed() < Duration::from_millis(1000),
        "stop macro didn't end the waiting strand"
    );
}
