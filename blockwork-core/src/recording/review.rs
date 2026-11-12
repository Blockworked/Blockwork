//! The review step between stopping a recording and inserting its blocks.
use crate::input::types::{Direction, InputToken};
use crate::input::value::Value;
use crate::macros::{Instruction, InstructionKind};
use serde::{Deserialize, Serialize};

/// How many rows the review lists; the counts always cover everything.
pub const PREVIEW_ROWS: usize = 300;

/// What a recorded block is, for counting and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Key,
    Button,
    Scroll,
    Movement,
    Wait,
    Touch,
    Controller,
    Other,
}

pub fn source(kind: &InstructionKind) -> Source {
    match kind {
        InstructionKind::Wait(_) => Source::Wait,
        InstructionKind::Token(InputToken::Key(..) | InputToken::Raw(..) | InputToken::Text(_)) => {
            Source::Key
        }
        InstructionKind::Token(InputToken::Button(..)) => Source::Button,
        InstructionKind::Token(InputToken::Scroll(..)) => Source::Scroll,
        InstructionKind::Token(InputToken::MoveMouse(..)) => Source::Movement,
        InstructionKind::TouchSequence { .. } | InstructionKind::TouchGesture { .. } => {
            Source::Touch
        }
        InstructionKind::GamepadButton { .. }
        | InstructionKind::GamepadStick { .. }
        | InstructionKind::GamepadTrigger { .. }
        | InstructionKind::ResetGamepad => Source::Controller,
        _ => Source::Other,
    }
}

fn wait_ms(kind: &InstructionKind) -> Option<f64> {
    match kind {
        InstructionKind::Wait(Value::Number { value }) => Some(*value),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Summary {
    pub total: usize,
    pub keys: usize,
    pub buttons: usize,
    pub scrolls: usize,
    pub movements: usize,
    pub waits: usize,
    pub touch: usize,
    pub controller: usize,
    pub other: usize,
    /// Every wait added up: how long the recording takes to play.
    pub duration_ms: f64,
    /// The wait before the first input, often just time to switch apps.
    pub leading_wait_ms: f64,
    pub longest_wait_ms: f64,
    /// Key or button presses that the recording never releases.
    pub unreleased: usize,
}

/// One listed row. QML words it, so the text stays translatable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    pub source: Source,
    /// `press`, `release`, `click` or empty.
    pub direction: &'static str,
    /// The key, button, amount or wait, as plain text.
    pub detail: String,
}

pub fn summarize(instructions: &[Instruction]) -> Summary {
    let mut summary = Summary {
        total: instructions.len(),
        ..Summary::default()
    };
    let mut held = std::collections::HashSet::new();
    let mut seen_input = false;
    for ins in instructions {
        let source = source(&ins.kind);
        match source {
            Source::Key => summary.keys += 1,
            Source::Button => summary.buttons += 1,
            Source::Scroll => summary.scrolls += 1,
            Source::Movement => summary.movements += 1,
            Source::Wait => summary.waits += 1,
            Source::Touch => summary.touch += 1,
            Source::Controller => summary.controller += 1,
            Source::Other => summary.other += 1,
        }
        if let Some(ms) = wait_ms(&ins.kind) {
            summary.duration_ms += ms;
            summary.longest_wait_ms = summary.longest_wait_ms.max(ms);
            if !seen_input {
                summary.leading_wait_ms += ms;
            }
        } else {
            seen_input = true;
        }
        if let Some((key, direction)) = held_key(&ins.kind) {
            match direction {
                Direction::Press => {
                    held.insert(key);
                }
                Direction::Release => {
                    held.remove(&key);
                }
                Direction::Click => {}
            }
        }
    }
    summary.unreleased = held.len();
    summary
}

fn held_key(kind: &InstructionKind) -> Option<(String, Direction)> {
    match kind {
        InstructionKind::Token(InputToken::Key(key, direction)) => {
            Some((format!("key:{key:?}"), direction.clone()))
        }
        InstructionKind::Token(InputToken::Button(button, direction)) => {
            Some((format!("button:{button:?}"), direction.clone()))
        }
        InstructionKind::Token(InputToken::Raw(code, direction)) => {
            Some((format!("raw:{code}"), direction.clone()))
        }
        _ => None,
    }
}

fn direction(d: &Direction) -> &'static str {
    match d {
        Direction::Press => "press",
        Direction::Release => "release",
        Direction::Click => "click",
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::Number { value } => {
            let rounded = (value * 100.0).round() / 100.0;
            format!("{rounded}")
        }
        Value::Text { value } => value.clone(),
        other => format!("{other:?}"),
    }
}

pub fn rows(instructions: &[Instruction]) -> Vec<Row> {
    instructions
        .iter()
        .take(PREVIEW_ROWS)
        .map(|ins| {
            let (dir, detail) = match &ins.kind {
                InstructionKind::Wait(v) => ("", text(v)),
                InstructionKind::Token(InputToken::Key(key, d)) => (direction(d), text(key)),
                InstructionKind::Token(InputToken::Raw(code, d)) => {
                    (direction(d), code.to_string())
                }
                InstructionKind::Token(InputToken::Button(b, d)) => {
                    (direction(d), format!("{b:?}"))
                }
                InstructionKind::Token(InputToken::Scroll(amount, axis)) => {
                    ("", format!("{} {axis:?}", text(amount)))
                }
                InstructionKind::Token(InputToken::MoveMouse(x, y, c)) => {
                    ("", format!("{}, {} {c:?}", text(x), text(y)))
                }
                InstructionKind::TouchSequence { sequence, .. } => {
                    let detail = match sequence.as_ref() {
                        Value::Text { value } => crate::input::touch::TouchSequence::parse(value)
                            .map(|s| {
                                let fingers =
                                    s.frames.iter().map(|f| f.contacts.len()).max().unwrap_or(0);
                                let ms = s.frames.last().map_or(0, |f| f.at_ms);
                                format!("{fingers}|{ms}")
                            })
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    ("", detail)
                }
                InstructionKind::GamepadButton {
                    button,
                    direction: d,
                } => (direction(d), format!("{button:?}")),
                InstructionKind::GamepadStick { stick, x, y } => {
                    ("", format!("{stick:?} {}, {}", text(x), text(y)))
                }
                InstructionKind::GamepadTrigger { left, amount } => (
                    "",
                    format!("{} {}", if *left { "Left" } else { "Right" }, text(amount)),
                ),
                InstructionKind::ResetGamepad => ("", "reset".into()),
                _ => ("", String::new()),
            };
            Row {
                source: source(&ins.kind),
                direction: dir,
                detail,
            }
        })
        .collect()
}

/// What the review keeps. Leaving something out keeps the timing of what's
/// left: waits around a removed block merge into one.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub keep_waits: bool,
    pub keep_keys: bool,
    pub keep_buttons: bool,
    pub keep_scrolls: bool,
    pub keep_movement: bool,
    pub keep_touch: bool,
    pub keep_controller: bool,
    pub trim_leading_wait: bool,
    /// Waits longer than this are shortened to it; 0 keeps them as recorded.
    pub max_wait_ms: f64,
    /// Adds a release for each key or button the recording leaves pressed.
    pub release_held: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            keep_waits: true,
            keep_keys: true,
            keep_buttons: true,
            keep_scrolls: true,
            keep_movement: true,
            keep_touch: true,
            keep_controller: true,
            trim_leading_wait: false,
            max_wait_ms: 0.0,
            release_held: true,
        }
    }
}

impl Options {
    fn keeps(&self, source: Source) -> bool {
        match source {
            Source::Key => self.keep_keys,
            Source::Button => self.keep_buttons,
            Source::Scroll => self.keep_scrolls,
            Source::Movement => self.keep_movement,
            Source::Touch => self.keep_touch,
            Source::Controller => self.keep_controller,
            Source::Wait | Source::Other => true,
        }
    }
}

pub fn apply(instructions: Vec<Instruction>, options: &Options) -> Vec<Instruction> {
    let mut out: Vec<Instruction> = Vec::with_capacity(instructions.len());
    let mut held: Vec<(String, InputToken)> = Vec::new();
    for ins in instructions {
        if let Some(ms) = wait_ms(&ins.kind) {
            if !options.keep_waits || !ms.is_finite() || ms <= 0.0 {
                continue;
            }
            if let Some(previous) = out.last_mut().filter(|p| wait_ms(&p.kind).is_some()) {
                let sum = wait_ms(&previous.kind).unwrap_or(0.0) + ms;
                previous.kind = InstructionKind::Wait(Value::number(sum));
            } else {
                out.push(ins);
            }
            continue;
        }
        if !options.keeps(source(&ins.kind)) {
            continue;
        }
        if let (Some((key, direction)), InstructionKind::Token(token)) =
            (held_key(&ins.kind), &ins.kind)
        {
            match direction {
                Direction::Press => held.push((key, token.clone())),
                Direction::Release => held.retain(|(k, _)| *k != key),
                Direction::Click => {}
            }
        }
        out.push(ins);
    }
    if options.trim_leading_wait && out.first().is_some_and(|i| wait_ms(&i.kind).is_some()) {
        out.remove(0);
    }
    if options.max_wait_ms > 0.0 {
        for ins in &mut out {
            if let Some(ms) = wait_ms(&ins.kind)
                && ms > options.max_wait_ms
            {
                ins.kind = InstructionKind::Wait(Value::number(options.max_wait_ms));
            }
        }
    }
    // A trailing wait only delays the end of the strand.
    while out.last().is_some_and(|i| wait_ms(&i.kind).is_some()) {
        out.pop();
    }
    if options.release_held {
        for (_, token) in held.into_iter().rev() {
            let release = match token {
                InputToken::Key(key, _) => InputToken::Key(key, Direction::Release),
                InputToken::Button(button, _) => InputToken::Button(button, Direction::Release),
                InputToken::Raw(code, _) => InputToken::Raw(code, Direction::Release),
                _ => continue,
            };
            out.push(Instruction::new(InstructionKind::Token(release)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::types::{Axis, Coordinate, MacroButton};

    fn wait(ms: f64) -> Instruction {
        Instruction::new(InstructionKind::Wait(Value::number(ms)))
    }
    fn key(name: &str, d: Direction) -> Instruction {
        Instruction::new(InstructionKind::Token(InputToken::Key(
            Value::text(name),
            d,
        )))
    }
    fn mv() -> Instruction {
        Instruction::new(InstructionKind::Token(InputToken::MoveMouse(
            Value::number(1.0),
            Value::number(2.0),
            Coordinate::Abs,
        )))
    }
    fn sample() -> Vec<Instruction> {
        vec![
            wait(1500.0),
            key("A", Direction::Press),
            wait(20.0),
            mv(),
            wait(30.0),
            key("A", Direction::Release),
            wait(10.0),
            Instruction::new(InstructionKind::Token(InputToken::Button(
                MacroButton::Left,
                Direction::Press,
            ))),
            Instruction::new(InstructionKind::Token(InputToken::Scroll(
                Value::number(1.0),
                Axis::Vertical,
            ))),
            Instruction::new(InstructionKind::ResetGamepad),
            wait(5.0),
        ]
    }

    #[test]
    fn summaries_count_every_source_and_the_timeline() {
        let summary = summarize(&sample());
        assert_eq!(summary.total, 11);
        assert_eq!((summary.keys, summary.movements, summary.waits), (2, 1, 5));
        assert_eq!(
            (summary.buttons, summary.scrolls, summary.controller),
            (1, 1, 1)
        );
        assert_eq!(summary.duration_ms, 1565.0);
        assert_eq!(summary.leading_wait_ms, 1500.0);
        assert_eq!(summary.longest_wait_ms, 1500.0);
        assert_eq!(summary.unreleased, 1);
        let rows = rows(&sample());
        assert_eq!(rows[1].direction, "press");
        assert_eq!(rows[1].detail, "A");
        assert_eq!(rows[0].detail, "1500");
    }

    #[test]
    fn leaving_blocks_out_keeps_the_timing_of_the_rest() {
        let options = Options {
            keep_movement: false,
            trim_leading_wait: true,
            ..Options::default()
        };
        let out = apply(sample(), &options);
        let waits: Vec<_> = out.iter().filter_map(|i| wait_ms(&i.kind)).collect();
        // 20 + 30 merge around the removed movement; the trailing wait goes.
        assert_eq!(waits, vec![50.0, 10.0]);
        assert!(matches!(
            out[0].kind,
            InstructionKind::Token(InputToken::Key(..))
        ));
        // The held left button is released at the end.
        assert!(matches!(
            out.last().unwrap().kind,
            InstructionKind::Token(InputToken::Button(MacroButton::Left, Direction::Release))
        ));
    }

    #[test]
    fn waits_can_be_capped_or_dropped() {
        let capped = apply(
            sample(),
            &Options {
                max_wait_ms: 100.0,
                release_held: false,
                ..Options::default()
            },
        );
        assert_eq!(wait_ms(&capped[0].kind), Some(100.0));
        assert!(!matches!(
            capped.last().unwrap().kind,
            InstructionKind::Token(InputToken::Button(_, Direction::Release))
        ));
        let none = apply(
            sample(),
            &Options {
                keep_waits: false,
                keep_controller: false,
                ..Options::default()
            },
        );
        assert!(none.iter().all(|i| wait_ms(&i.kind).is_none()));
        assert!(none.iter().all(|i| source(&i.kind) != Source::Controller));
    }
}
