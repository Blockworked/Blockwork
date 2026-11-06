//! Input emulation failures.

use super::*;

#[test]
fn without_input_emulation_other_blocks_run_and_input_reports_once() {
    let _log = crate::run_log::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let text = || Instruction::new(InstructionKind::Token(InputToken::Text(Value::text("hi"))));
    let first = text();
    let vars = empty_vars();
    run_fired_strand(
        crate::run_log::Source::for_macro("no-input-test", "No input"),
        vec![first.clone(), text(), set("after", Value::number(1.0))],
        Arc::new(Mutex::new(crate::macros::backend::UnavailableBackend::new())),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        0,
    );
    assert_eq!(
        vars.lock().unwrap().get("after"),
        Some(&Evaluated::Number(1.0))
    );
    let errors: Vec<_> = crate::run_log::snapshot()
        .into_iter()
        .filter(|e| e.macro_id == "no-input-test" && e.level == crate::run_log::Level::Error)
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].block_id, first.id);
    assert!(
        errors[0]
            .message
            .contains("input emulation isn't available"),
        "{}",
        errors[0].message
    );
}

#[test]
fn keyboard_backend_failures_are_visible_and_catchable_after_combo_release() {
    let _log = crate::run_log::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    struct RejectKey(Arc<Mutex<Vec<(MacroKey, Direction)>>>);
    impl InputBackend for RejectKey {
        fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
            self.0.lock().unwrap().push((key.clone(), dir));
            if key == MacroKey::F21 {
                Err("F21 has no native macOS virtual-key code".into())
            } else {
                Ok(())
            }
        }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> {
            Ok(())
        }
        fn text(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            None
        }
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let backend: Arc<Mutex<dyn InputBackend>> =
        Arc::new(Mutex::new(RejectKey(Arc::clone(&events))));
    let input = Instruction::new(InstructionKind::Token(InputToken::Key(
        Value::text("Control+F21"),
        Direction::Click,
    )));
    let vars = empty_vars();
    run_fired_strand(
        crate::run_log::Source::for_macro("keyboard-error-regression", "Keyboard error"),
        vec![input.clone(), set("after", Value::number(1.0))],
        Arc::clone(&backend),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        0,
    );
    assert_eq!(
        vars.lock().unwrap().get("after"),
        Some(&Evaluated::Number(1.0))
    );
    let errors: Vec<_> = crate::run_log::snapshot()
        .into_iter()
        .filter(|e| {
            e.macro_id == "keyboard-error-regression" && e.level == crate::run_log::Level::Error
        })
        .collect();
    assert_eq!(errors.len(), 1, "one visible block error is required");
    assert_eq!(errors[0].block_id, input.id);
    assert!(
        errors[0]
            .message
            .contains("F21 has no native macOS virtual-key code")
    );
    assert_eq!(
        events.lock().unwrap().last(),
        Some(&(MacroKey::Control, Direction::Release))
    );
    crate::input::value::register_blockwork_operators();
    run_instructions(
        vec![Instruction::new(InstructionKind::TryCatch {
            body: vec![input, set("missed", Value::number(1.0))],
            catch_body: vec![set("caught", Value::op(Op::from_name("LastError"), vec![]))],
        })],
        backend,
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("missed"), None);
    assert!(
        matches!(vars.get("caught"), Some(Evaluated::Text(e)) if e.contains("F21 has no native macOS virtual-key code"))
    );
    assert_eq!(
        events.lock().unwrap().last(),
        Some(&(MacroKey::Control, Direction::Release))
    );
}

#[test]
fn combo_letters_dont_add_shift() {
    #[derive(Default)]
    struct Keys(Arc<Mutex<Vec<(MacroKey, Direction)>>>);
    impl InputBackend for Keys {
        fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
            self.0.lock().unwrap().push((key, dir));
            Ok(())
        }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> {
            Ok(())
        }
        fn text(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            None
        }
    }
    let keys = Keys::default();
    let seen = Arc::clone(&keys.0);
    run_instructions(
        vec![Instruction::new(InstructionKind::Token(InputToken::Key(
            Value::text("Control+A"),
            Direction::Click,
        )))],
        Arc::new(Mutex::new(keys)),
        None,
        1.0,
        empty_vars(),
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[1], (MacroKey::Unicode('a'), Direction::Press));
}
