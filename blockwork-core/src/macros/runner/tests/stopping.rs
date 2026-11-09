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

/// Each trigger fire is its own run, so a Stop in one has to reach the
/// macro's strands that other fires started.
fn fire_in_macro(macro_id: &str, instructions: Vec<Instruction>, vars: &VariableStore) -> bool {
    let flag = crate::macros::run_registry::begin_macro_run(macro_id);
    run_fired_strand(
        crate::run_log::Source::for_macro(macro_id, "Fired"),
        instructions,
        noop_emulator(),
        Some(Arc::clone(&flag)),
        1.0,
        Arc::clone(vars),
        ListStore::default(),
        Arc::new(HashMap::new()),
        0,
    );
    crate::macros::run_registry::end_run(&flag);
    *flag.lock().unwrap()
}

#[test]
fn stop_in_a_fired_strand_reaches_strands_other_fires_started() {
    for (id, scope) in [
        ("stop-fired-others", StopScope::OtherStrands),
        ("stop-fired-all", StopScope::All),
    ] {
        let vars = empty_vars();
        let other = Arc::new(Mutex::new(None));
        let start = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                let running = fire_in_macro(
                    id,
                    vec![
                        Instruction::new(InstructionKind::Wait(Value::number(5_000.0))),
                        set("looped", Value::number(1.0)),
                    ],
                    &vars,
                );
                *other.lock().unwrap() = Some(running);
            });
            // Let the long strand register and start waiting.
            while !crate::macros::run_registry::is_running(id) {
                std::thread::yield_now();
            }
            let stopper_running = fire_in_macro(
                id,
                vec![
                    Instruction::new(InstructionKind::Stop { scope }),
                    set("stopper", Value::number(1.0)),
                ],
                &vars,
            );
            assert_eq!(
                stopper_running,
                scope == StopScope::OtherStrands,
                "{scope:?}"
            );
        });
        assert!(start.elapsed() < Duration::from_secs(2), "{scope:?}");
        assert_eq!(*other.lock().unwrap(), Some(false), "{scope:?}");
        let vars = vars.lock().unwrap();
        assert_eq!(vars.get("looped"), None, "{scope:?}");
        assert_eq!(
            vars.contains_key("stopper"),
            scope == StopScope::OtherStrands,
            "{scope:?}"
        );
    }
}

#[test]
fn stop_in_a_fired_strand_leaves_other_macros_alone() {
    let vars = empty_vars();
    std::thread::scope(|s| {
        s.spawn(|| {
            fire_in_macro(
                "stop-spared",
                vec![
                    Instruction::new(InstructionKind::Wait(Value::number(60.0))),
                    set("spared", Value::number(1.0)),
                ],
                &vars,
            );
        });
        while !crate::macros::run_registry::is_running("stop-spared") {
            std::thread::yield_now();
        }
        fire_in_macro(
            "stop-stopper",
            vec![Instruction::new(InstructionKind::Stop {
                scope: StopScope::OtherStrands,
            })],
            &vars,
        );
    });
    assert_eq!(
        vars.lock().unwrap().get("spared"),
        Some(&Evaluated::Number(1.0))
    );
}
