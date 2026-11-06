//! Errors: try, retry, limits and the run log.

use super::*;

#[test]
fn try_catch_runs_catch_body_with_the_error_message() {
    crate::input::value::register_blockwork_operators();
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::TryCatch {
            body: vec![
                set("a", Value::number(1.0)),
                Instruction::new(InstructionKind::ThrowError(Value::text("boom"))),
                set("a", Value::number(2.0)),
            ],
            catch_body: vec![set("e", Value::op(Op::from_name("LastError"), vec![]))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("a"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("e"), Some(&Evaluated::Text("boom".into())));
}

#[test]
fn failing_block_is_caught_inside_try_and_skipped_outside() {
    let vars = empty_vars();
    run_instructions(
        vec![
            set("y", bad_number()),
            set("after_skip", Value::number(1.0)),
            Instruction::new(InstructionKind::TryCatch {
                body: vec![set("y", bad_number()), set("after_error", Value::number(1.0))],
                catch_body: vec![set("caught", Value::number(1.0))],
            }),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("after_skip"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("after_error"), None);
    assert_eq!(vars.get("caught"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn retry_runs_the_body_again_until_attempts_run_out() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::TryCatch {
                body: vec![Instruction::new(InstructionKind::Retry {
                    count: Value::number(3.0),
                    delay: Value::number(0.0),
                    body: vec![bump("tries"), Instruction::new(InstructionKind::ThrowError(Value::text("nope")))],
                })],
                catch_body: vec![set("caught", Value::number(1.0))],
            }),
            Instruction::new(InstructionKind::Retry {
                count: Value::number(5.0),
                delay: Value::number(0.0),
                body: vec![bump("ok")],
            }),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("tries"), Some(&Evaluated::Number(3.0)));
    assert_eq!(vars.get("caught"), Some(&Evaluated::Number(1.0)), "giving up is an error a try catches");
    assert_eq!(vars.get("ok"), Some(&Evaluated::Number(1.0)), "a body that works runs once");
}

#[test]
fn loop_limit_stops_a_runaway_loop() {
    let vars = empty_vars();
    run_fired_strand(
        crate::run_log::Source::default(),
        vec![
            Instruction::new(InstructionKind::Forever { body: vec![bump("n")] }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        10,
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("n"), Some(&Evaluated::Number(10.0)));
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)), "outside a try the strand carries on");
}

#[test]
fn wait_until_returns_when_true_and_fails_after_its_timeout() {
    let vars = empty_vars();
    let started = Instant::now();
    let gt = |a: Value, b: Value| Value::op(Op::from_name("Gt"), vec![a, b]);
    run_instructions(
        vec![
            Instruction::new(InstructionKind::WaitUntil { condition: gt(Value::number(2.0), Value::number(1.0)), timeout: None, error_timeout: Some(Value::number(0.0)) }),
            Instruction::new(InstructionKind::TryCatch {
                body: vec![
                    Instruction::new(InstructionKind::WaitUntil { condition: gt(Value::number(1.0), Value::number(2.0)), timeout: None, error_timeout: Some(Value::number(150.0)) }),
                    set("missed", Value::number(1.0)),
                ],
                catch_body: vec![set("timed_out", Value::number(1.0))],
            }),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("timed_out"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("missed"), None);
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(150) && elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn wait_limits_continue_or_end_the_strand_and_can_be_caught() {
    let wait = |timeout: Option<f64>, error_timeout: Option<f64>| Instruction::new(InstructionKind::WaitUntil {
        condition: Value::op(Op::from_name("Gt"), vec![Value::number(0.0), Value::number(1.0)]),
        timeout: timeout.map(Value::number),
        error_timeout: error_timeout.map(Value::number),
    });
    // A normal deadline continues even inside try; an error deadline stops the strand.
    let vars = empty_vars();
    run_instructions(vec![
        Instruction::new(InstructionKind::TryCatch {
            body: vec![wait(Some(1.0), None), set("continued", Value::number(1.0))],
            catch_body: vec![set("caught", Value::number(1.0))],
        }),
        wait(None, Some(1.0)),
        set("after_error", Value::number(1.0)),
    ], noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("continued"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("caught"), None);
    assert_eq!(vars.get("after_error"), None);
    drop(vars);

    // Both deadlines start together. The earlier wins, with errors winning a tie.
    for (timeout, error_timeout, caught) in [(1.0, 1000.0, false), (1000.0, 1.0, true), (1.0, 1.0, true)] {
        let vars = empty_vars();
        run_instructions(vec![Instruction::new(InstructionKind::TryCatch {
            body: vec![wait(Some(timeout), Some(error_timeout)), set("continued", Value::number(1.0))],
            catch_body: vec![set("caught", Value::number(1.0))],
        })], noop_emulator(), None, 1.0, Arc::clone(&vars));
        let vars = vars.lock().unwrap();
        assert_eq!(vars.contains_key("caught"), caught);
        assert_eq!(vars.contains_key("continued"), !caught);
    }
}

#[test]
fn uncaught_throw_ends_the_strand() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::ThrowError(Value::text("stop here"))),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("after"), None);
}

#[test]
fn run_log_errors_name_the_failing_block() {
    let _log = crate::run_log::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let throw = Instruction::new(InstructionKind::ThrowError(Value::text("boom")));
    let source = crate::run_log::Source::for_macro("blame-test", "Blame");
    run_fired_strand(
        source,
        vec![Instruction::new(InstructionKind::Log(Value::text("hi"))), throw.clone()],
        noop_emulator(),
        None,
        1.0,
        empty_vars(),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        0,
    );
    let errors: Vec<_> = crate::run_log::snapshot()
        .into_iter()
        .filter(|e| e.macro_id == "blame-test" && e.level == crate::run_log::Level::Error)
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].block_id, throw.id);
    assert_eq!(errors[0].message, "boom");
}
