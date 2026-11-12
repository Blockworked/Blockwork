use super::*;
use blockwork_core::input::types::{Direction, InputToken};
use blockwork_core::recording;

fn key(name: &str, direction: Direction) -> Instruction {
    Instruction::new(InstructionKind::Token(InputToken::Key(
        Value::text(name),
        direction,
    )))
}

fn queue(instructions: Vec<Instruction>) {
    recording::get_recording_queue()
        .lock()
        .unwrap()
        .extend(instructions);
}

#[test]
fn stopped_recordings_wait_for_review_and_insert_as_one_undo_step() {
    let dir = std::env::temp_dir().join(format!("blockwork-review-{}", uuid::Uuid::new_v4()));
    let backend = crate::Backend::for_tests(dir);
    let state = &backend.state;
    let app = &backend.app;
    let mac = Macro::new("Recorded".into(), "".into(), vec![]);
    {
        let mut s = state.lock().unwrap();
        s.macros_list = vec![mac.clone()];
        s.current_macro = Some(mac.clone());
        assert!(s.review_recordings, "review is on by default");
    }
    queue(vec![
        Instruction::new(InstructionKind::Wait(Value::number(900.0))),
        key("A", Direction::Press),
        Instruction::new(InstructionKind::Wait(Value::number(40.0))),
        key("A", Direction::Release),
    ]);
    {
        let mut s = state.lock().unwrap();
        assert!(stop_recording_impl(&mut s).is_none());
        let target_len = s
            .current_macro
            .as_mut()
            .unwrap()
            .recording_target_mut()
            .instructions
            .len();
        assert_eq!(target_len, 1, "nothing is inserted before the review");
        let dto = crate::state::build_state_dto(&s);
        let review = dto.recording_review.expect("a review is pending");
        assert_eq!(review.summary.keys, 2);
        assert_eq!(review.summary.leading_wait_ms, 900.0);
        assert_eq!(review.macro_name, "Recorded");
    }
    // A second recording into the same macro joins the same review.
    queue(vec![key("B", Direction::Click)]);
    stop_recording_impl(&mut state.lock().unwrap());
    let options = recording::review::Options {
        trim_leading_wait: true,
        ..Default::default()
    };
    let inserted = apply_recording_review(state, app, options).unwrap();
    assert_eq!(inserted, 4);
    let mut s = state.lock().unwrap();
    assert!(s.recording_review.is_none());
    assert!(s.history.can_undo());
    let target = &s
        .current_macro
        .as_mut()
        .unwrap()
        .recording_target_mut()
        .instructions;
    assert!(matches!(
        target[1].kind,
        InstructionKind::Token(InputToken::Key(..))
    ));
    drop(s);
    assert!(apply_recording_review(state, app, Default::default()).is_err());
}

#[test]
fn reviews_stay_with_their_macro_and_can_be_discarded() {
    let dir = std::env::temp_dir().join(format!("blockwork-review-{}", uuid::Uuid::new_v4()));
    let backend = crate::Backend::for_tests(dir);
    let state = &backend.state;
    let first = Macro::new("First".into(), "".into(), vec![]);
    let second = Macro::new("Second".into(), "".into(), vec![]);
    {
        let mut s = state.lock().unwrap();
        s.macros_list = vec![first.clone(), second.clone()];
        s.current_macro = Some(first.clone());
        s.recording_review = Some((first.id.clone(), vec![key("A", Direction::Click)]));
        s.current_macro = Some(second);
    }
    assert!(apply_recording_review(state, &backend.app, Default::default()).is_err());
    assert!(state.lock().unwrap().recording_review.is_some());
    discard_recording_review(state, &backend.app).unwrap();
    assert!(state.lock().unwrap().recording_review.is_none());
    // With review off, blocks go straight in as before.
    {
        let mut s = state.lock().unwrap();
        s.review_recordings = false;
        s.current_macro = Some(first);
    }
    queue(vec![key("C", Direction::Click)]);
    assert!(stop_recording_impl(&mut state.lock().unwrap()).is_some());
}
