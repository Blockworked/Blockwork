//! Image matching.

use super::*;

#[test]
fn image_matches_are_picked_by_number_or_by_nearness() {
    use crate::screen::ScreenMatch;
    // Reading order: two on the top row, one below; the middle one looks best.
    let matches = [
        ScreenMatch {
            x: 100.0,
            y: 50.0,
            diff: 4.0,
        },
        ScreenMatch {
            x: 400.0,
            y: 50.0,
            diff: 1.0,
        },
        ScreenMatch {
            x: 100.0,
            y: 300.0,
            diff: 1.0,
        },
    ];
    let x_of = |target| pick_match(&matches, target).unwrap().map(|m| (m.x, m.y));
    assert_eq!(
        x_of(MatchTarget::Best),
        Some((400.0, 50.0)),
        "ties go to reading order"
    );
    assert_eq!(x_of(MatchTarget::Nth(1.0)), Some((100.0, 50.0)));
    assert_eq!(x_of(MatchTarget::Nth(3.0)), Some((100.0, 300.0)));
    assert_eq!(
        x_of(MatchTarget::Nth(-1.0)),
        Some((100.0, 300.0)),
        "negative counts from the end"
    );
    assert_eq!(x_of(MatchTarget::Nth(-3.0)), Some((100.0, 50.0)));
    assert_eq!(x_of(MatchTarget::Nth(4.0)), None);
    assert_eq!(x_of(MatchTarget::Nth(-4.0)), None);
    assert!(pick_match(&matches, MatchTarget::Nth(0.0)).is_err());
    assert_eq!(x_of(MatchTarget::Near(120.0, 260.0)), Some((100.0, 300.0)));
    assert_eq!(x_of(MatchTarget::Near(390.0, 0.0)), Some((400.0, 50.0)));
    assert_eq!(pick_match(&[], MatchTarget::Near(0.0, 0.0)).unwrap(), None);
}

/// Every image pick against the real screen, for `$BW_LIVE_IMAGE` (a PNG
/// on screen more than once) near `$BW_LIVE_NEAR` (`x,y`); run by hand
/// with `--ignored`.
#[test]
#[ignore]
fn live_image_picks() {
    struct Moves(Arc<Mutex<Vec<(i32, i32)>>>);
    impl InputBackend for Moves {
        fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> {
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
        fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
            self.0.lock().unwrap().push((x, y));
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
    let image = std::env::var("BW_LIVE_IMAGE").expect("set BW_LIVE_IMAGE to a PNG");
    let near = std::env::var("BW_LIVE_NEAR").unwrap_or_else(|_| "0,0".into());
    let (nx, ny) = near
        .split_once(',')
        .map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap()))
        .unwrap();
    let picks = [
        ("best", None, 1.0),
        ("match 1", Some(ImagePick::Nth), 1.0),
        ("match 2", Some(ImagePick::Nth), 2.0),
        ("match -1", Some(ImagePick::Nth), -1.0),
        ("nearest mouse", Some(ImagePick::NearMouse), 1.0),
        ("nearest point", Some(ImagePick::NearPoint), 1.0),
    ];
    for (name, mode, index) in picks {
        let pick = mode.map(|mode| ImageMatch {
            mode,
            index: Value::number(index),
            near_x: Value::number(nx),
            near_y: Value::number(ny),
        });
        let vars = empty_vars();
        let moves = Arc::new(Mutex::new(Vec::new()));
        let search = |kind| Instruction::new(kind);
        run_instructions(
            vec![
                search(InstructionKind::FindImage {
                    image: image.clone(),
                    tolerance: Some(Value::number(10.0)),
                    x_variable: "x".into(),
                    y_variable: "y".into(),
                    pick: pick.clone(),
                    region: None,
                    count_variable: Some("count".into()),
                }),
                search(InstructionKind::ClickImage {
                    image: image.clone(),
                    tolerance: Some(Value::number(10.0)),
                    button: MacroButton::Left,
                    pick,
                    region: None,
                }),
            ],
            Arc::new(Mutex::new(Moves(Arc::clone(&moves)))),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        eprintln!(
            "{name}: x {:?} y {:?} count {:?}, click at {:?}",
            vars.get("x"),
            vars.get("y"),
            vars.get("count"),
            moves.lock().unwrap()
        );
    }
}

#[test]
fn image_blocks_without_an_image_fail_softly() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::FindImage {
                image: String::new(),
                tolerance: Some(Value::number(5.0)),
                x_variable: "x".into(),
                y_variable: "y".into(),
                pick: None,
                region: None,
                count_variable: None,
            }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("x"), None);
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
}
