//! Concurrent strands, stopping, signals, locks and running other macros.

use super::*;

/// Three entry strands each waiting 150ms should finish in well under
/// 3×150ms if they're actually running concurrently rather than one
/// after another.
#[test]
fn run_executes_all_when_ran_strands_concurrently() {
    let mac = Macro {
        id: "m".into(),
        name: "Concurrent".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                when_ran_strand("a", 150.0),
                when_ran_strand("b", 150.0),
                when_ran_strand("c", 150.0),
                Strand {
                    id: "inert".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![Instruction::new(InstructionKind::Wait(Value::number(
                        150.0,
                    )))],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![],
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
        start.elapsed() < Duration::from_millis(400),
        "entry strands ran sequentially instead of concurrently"
    );
}

/// `RunOptions::offset` should backdate the first `Wait`'s deadline anchor
/// by `initial_offset` - a macro whose only strand waits 2000ms should
/// return in roughly (2000ms - offset) wall-clock time, proving the
/// anchor is `Instant::now() - initial_offset`, not just `Instant::now()`
/// with the offset silently ignored.
// A 1500ms separation distinguishes an ignored offset while allowing
// scheduler jitter on shared CI runners; the expected wait is 500ms.
#[test]
fn run_with_offset_backdates_the_first_wait_deadline() {
    let mac = Macro {
        id: "m".into(),
        name: "Offset".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![when_ran_strand("a", 2000.0)],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run_with(RunOptions {
        offset: Duration::from_millis(1500),
        ..RunOptions::new(noop_emulator(), empty_vars())
    });
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(1200),
        "offset didn't shorten the wait as expected: {elapsed:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(250),
        "returned suspiciously fast, offset may have overshot: {elapsed:?}"
    );
}

/// A stop flag flipped false mid-run should cut every concurrently
/// running entry strand short, not just one of them.
#[test]
fn stop_flag_stops_every_concurrent_strand() {
    let long_wait = 5_000.0;
    let mac = Macro {
        id: "m".into(),
        name: "Stoppable".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                when_ran_strand("a", long_wait),
                when_ran_strand("b", long_wait),
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let stop_flag = Arc::new(Mutex::new(true));
    let flag_clone = Arc::clone(&stop_flag);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        *flag_clone.lock().unwrap() = false;
    });
    let start = Instant::now();
    mac.run(noop_emulator(), Some(stop_flag), 1.0, empty_vars());
    assert!(
        start.elapsed() < Duration::from_millis(1000),
        "stop flag didn't stop both concurrent strands promptly"
    );
}

#[test]
fn run_macro_waits_for_the_other_run_to_end() {
    struct Host;
    impl crate::macros::calls::MacroHost for Host {
        fn find(&self, name: &str) -> Option<String> {
            (name == "Helper").then(|| "helper-id".to_string())
        }
        fn start(&self, _id: &str) -> Result<Arc<AtomicBool>, String> {
            let done = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&done);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(60));
                flag.store(true, Ordering::Relaxed);
            });
            Ok(done)
        }
    }
    crate::macros::calls::set_host(Arc::new(Host));
    let vars = empty_vars();
    let started = Instant::now();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::RunMacro {
                target: Value::text("Nope"),
                wait: true,
            }),
            Instruction::new(InstructionKind::RunMacro {
                target: Value::text("Helper"),
                wait: true,
            }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert!(
        started.elapsed() >= Duration::from_millis(60),
        "waited for the helper"
    );
    assert_eq!(
        vars.lock().unwrap().get("after"),
        Some(&Evaluated::Number(1.0)),
        "an unknown macro is a soft error"
    );
}

#[test]
fn signal_expressions_can_use_unregistered_computed_names() {
    let mut mac = Macro::new(
        "Dynamic signals".into(),
        String::new(),
        vec![
            set("target", Value::text("task:42")),
            Instruction::new(InstructionKind::SendSignal {
                name: Value::op(Op::Join, vec![Value::text("task:"), Value::text("42")]).into(),
                wait: true,
            }),
            set(
                "seen",
                Value::Var {
                    name: "received".into(),
                },
            ),
        ],
    );
    mac.graph.strands.push(Strand::with_instructions(
        0,
        200,
        vec![
            Instruction::new(InstructionKind::WhenSignal(
                Value::Var {
                    name: "target".into(),
                }
                .into(),
            )),
            set("received", Value::number(1.0)),
        ],
    ));
    mac.collect_named_signals();
    assert!(mac.signals.is_empty());
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("seen"),
        Some(&Evaluated::Number(1.0))
    );
}

#[test]
fn send_signal_and_wait_runs_the_receivers_first() {
    let mut mac = Macro::new(
        "Signals".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::SendSignal {
                name: Value::text("Go").into(),
                wait: true,
            }),
            set("seen", Value::Var { name: "x".into() }),
        ],
    );
    mac.graph.strands.push(Strand::with_instructions(
        0,
        200,
        vec![
            Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
            Instruction::new(InstructionKind::Wait(Value::number(40.0))),
            set("x", Value::number(5.0)),
        ],
    ));
    mac.graph.strands.push(Strand::with_instructions(
        0,
        400,
        vec![
            Instruction::new(InstructionKind::WhenSignal(Value::text("other").into())),
            set("wrong", Value::number(1.0)),
        ],
    ));
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("seen"), Some(&Evaluated::Number(5.0)));
    assert_eq!(vars.get("wrong"), None);
}

#[test]
fn run_strand_runs_one_strand_whatever_heads_it() {
    let mut mac = Macro::new(
        "One strand".into(),
        String::new(),
        vec![set("main", Value::number(1.0))],
    );
    let mut keyed = Strand::with_instructions(
        0,
        200,
        vec![
            Instruction::new(InstructionKind::WhenIdle(Value::number(5.0))),
            Instruction::new(InstructionKind::SendSignal {
                name: Value::text("go").into(),
                wait: true,
            }),
            set("keyed", Value::Var { name: "x".into() }),
        ],
    );
    keyed.id = "keyed".into();
    mac.graph.strands.push(keyed);
    mac.graph.strands.push(Strand::with_instructions(
        0,
        400,
        vec![
            Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
            set("x", Value::number(7.0)),
        ],
    ));
    assert!(!mac.can_run_strand("missing"));
    let vars = empty_vars();
    let lists: ListStore = Arc::new(Mutex::new(HashMap::new()));
    assert!(mac.run_one_strand(
        "keyed",
        RunOptions {
            lists: Some(lists),
            ..RunOptions::new(noop_emulator(), Arc::clone(&vars))
        }
    ));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("keyed"), Some(&Evaluated::Number(7.0)));
    assert_eq!(vars.get("main"), None);
}

#[cfg(unix)]
#[test]
fn run_command_moves_on_and_run_command_and_wait_waits() {
    let dir = std::env::temp_dir().join(format!("bw-command-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let run = |kind: InstructionKind| {
        Macro::new(
            "Commands".into(),
            String::new(),
            vec![Instruction::new(kind)],
        )
        .run(noop_emulator(), None, 1.0, empty_vars());
    };
    let (started, waited) = (dir.join("started"), dir.join("waited"));
    run(InstructionKind::Command(format!(
        "sleep 1; touch '{}'",
        started.display()
    )));
    assert!(!started.exists(), "run command didn't wait");
    run(InstructionKind::CommandWait(format!(
        "sleep 1; touch '{}'",
        waited.display()
    )));
    assert!(waited.exists(), "run command and wait waited");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !started.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(started.exists(), "the started command still ran");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lock_blocks_run_their_body_and_give_up_when_stopped() {
    let lock = |name: &str, body| {
        Instruction::new(InstructionKind::WithLock {
            name: Value::text(name),
            body,
        })
    };
    let vars = empty_vars();
    run_instructions(
        vec![lock("runner-free", vec![set("ran", Value::number(1.0))])],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(
        vars.lock().unwrap().get("ran"),
        Some(&Evaluated::Number(1.0))
    );
    assert!(!crate::macros::locks::is_held("runner-free"));

    // Keep the lock held until the stopped strand has returned. Sleeping
    // for a fixed holder lifetime made cancellation depend on CI scheduling.
    let held =
        crate::macros::locks::acquire("runner-busy", Duration::from_millis(5), || false).unwrap();
    let stop = Arc::new(Mutex::new(true));
    let worker_stop = Arc::clone(&stop);
    let vars = empty_vars();
    let worker_vars = Arc::clone(&vars);
    let instruction = lock("runner-busy", vec![set("ran", Value::number(1.0))]);
    let block_id = instruction.id.clone();
    let (finished, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        run_fired_strand(
            crate::run_log::Source::for_macro("runner-lock-cancellation", "Lock cancellation"),
            vec![instruction],
            noop_emulator(),
            Some(worker_stop),
            1.0,
            worker_vars,
            Arc::new(Mutex::new(HashMap::new())),
            Arc::new(HashMap::new()),
            0,
        );
        let _ = finished.send(());
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let reached_lock = loop {
        if crate::macros::trace::snapshot().iter().any(|active| {
            active.macro_id == "runner-lock-cancellation" && active.block_id == block_id
        }) {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    *stop.lock().unwrap() = false;
    let cancelled_while_held = completion.recv_timeout(Duration::from_secs(2)).is_ok();
    drop(held);
    worker.join().unwrap();
    assert!(reached_lock, "the strand never reached the held lock");
    assert!(
        cancelled_while_held,
        "the strand needed the lock released before it could stop"
    );
    assert_eq!(vars.lock().unwrap().get("ran"), None);
}

#[test]
fn stop_macro_stops_every_strand_of_the_run() {
    let mut stopper = when_ran_strand("b", 0.0);
    stopper
        .instructions
        .push(Instruction::new(InstructionKind::StopMacro));
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
