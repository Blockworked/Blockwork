use rand::RngExt;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Operator for a [`Value::Op`] block: arithmetic, [`Op::Random`],
/// [`Op::Join`] (text concat), a zero-arity text constant, or a text/lookup op.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    /// `args[0] % args[1]` via `rem_euclid` (not Rust's `%`), so a negative
    /// left-hand side still wraps normally. Errs on a zero right-hand side.
    Mod,
    /// `args[0]` rounded to the nearest whole number (half away from zero).
    Round,
    /// `args[0]`/`args[1]` are inclusive bounds (not operands), resampled
    /// fresh on every `eval`. Picks an integer if both bounds are whole
    /// numbers, otherwise a float.
    Random,
    /// Concatenates all of `args` as text — the only variable-arity operator
    /// (2 or 3 args depending on which palette entry it came from).
    Join,
    /// Zero-arity text constants — `args` is always empty.
    NewLine,
    Tab,
    /// 1-based index of `args[0]` (needle) in `args[1]` (haystack), or `0` if
    /// not found. Character-based, matching `LetterOf`'s indexing.
    IndexOf,
    /// Same as `IndexOf`, but the index of the last occurrence.
    LastIndexOf,
    /// The single character at `args[0]` (1-based) of `args[1]` (as text).
    /// Errs if the index is out of range.
    LetterOf,
    /// Character count of `args[0]` (as text).
    Length,
    /// Upper/lowercases `args[0]` based on `args[1]` (`"Upper"`/`"Lower"`) —
    /// a plain `Value::Text` leaf, but driven by an in-place dropdown rather
    /// than the drag/drop machinery.
    Case,
    /// `args[0] == args[1]` — numeric if both sides parse as a number,
    /// otherwise a text comparison (mirrors Scratch's loose `=`).
    Eq,
    /// Negation of [`Op::Eq`].
    Neq,
    /// `args[0] > args[1]`, numeric (same coercion as the arithmetic ops).
    Gt,
    /// `args[0] < args[1]`.
    Lt,
    /// `args[0] >= args[1]`.
    Gte,
    /// `args[0] <= args[1]`.
    Lte,
    /// `args[0] && args[1]`, short-circuiting.
    And,
    /// `args[0] || args[1]`, short-circuiting.
    Or,
    /// `!args[0]`.
    Not,
    /// Zero-arity `true` literal — a standalone block, not a toggle.
    True,
    /// Zero-arity `false` literal — a standalone block, not a toggle.
    False,
    /// Zero-arity — the system's current battery charge, 0-100. See
    /// `crate::battery::percentage`.
    BatteryPercentage,
    /// Zero-arity boolean — whether the system is currently receiving
    /// external power. See `crate::battery::is_plugged_in`.
    PluggedIn,
    /// `args[0]` (a fixed dropdown, like `Case`'s upper/lowercase toggle) is
    /// one of `"Year"`/`"Month"`/`"Date"`/`"DayOfWeek"`/`"Hour"`/`"Minute"`/
    /// `"Second"`, naming which local-clock component to read right now.
    /// Always numeric — `DayOfWeek` is 1 (Sunday) through 7 (Saturday),
    /// `Hour` is always 24-hour (0-23) regardless of the UI's display
    /// format, matching Scratch's own "current ()" sensing block.
    CurrentTime,
}

/// Recursive expression tree backing a numeric/text instruction field — a
/// number, text, or an operator over nested `args` (e.g. `5 + 3`).
///
/// `saved` holds the value an operator displaced when it took over the slot
/// (not an operand, not reachable via `get_mut`), so the UI can restore it
/// if the operator is later dragged back out.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Value {
    Number { value: f64 },
    Text { value: String },
    /// Bare boolean leaf with no value of its own — the "nothing plugged in
    /// here" state of a boolean-typed slot (an `If`'s condition, an
    /// `And`/`Or`/`Not` operand). Evaluates as `false`, same as Scratch's
    /// empty hexagon. Distinct from the standalone `Op::True`/`Op::False`
    /// blocks, which are explicit, draggable "I want this to literally read
    /// true/false" blocks rather than an unset slot.
    Bool,
    Op { op: Op, args: Vec<Value>, saved: Box<Value> },
    /// A read of a macro-wide variable by name, resolved against the running
    /// macro's variable store by [`Value::resolve_vars`] before `eval` sees it.
    Var { name: String },
    /// A read of the current custom-block invocation's bound parameter by
    /// name — resolved against that invocation's local parameter scope (not
    /// the macro-wide variable store), so nested/concurrent invocations of
    /// the same block don't share a slot.
    Param { name: String },
    /// Value-position invocation of a `ReturnsValue`/`ReturnsBool`-shaped custom block —
    /// mirrors `Op`'s shape (including `saved`) but names a block instead of
    /// a fixed [`Op`]. Resolved before `eval` sees it, like `Var`/`Param`.
    Call { block_id: String, args: Vec<Value>, saved: Box<Value> },
}

/// Result of evaluating a [`Value`] tree — a number or text. Also doubles as
/// the persisted representation of a variable's current value, hence the
/// extra derives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Evaluated {
    Number(f64),
    Text(String),
    Bool(bool),
}

/// Manual impl mirroring `Value`'s own — `f64` isn't `Hash`, so its bit
/// pattern stands in.
impl std::hash::Hash for Evaluated {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Evaluated::Number(n) => {
                0u8.hash(state);
                n.to_bits().hash(state);
            }
            Evaluated::Text(s) => {
                1u8.hash(state);
                s.hash(state);
            }
            Evaluated::Bool(b) => {
                2u8.hash(state);
                b.hash(state);
            }
        }
    }
}

impl Evaluated {
    /// Coerces to a number, parsing text loosely (so a `Text` leaf like
    /// `"5"` still works as an operand); errs with a readable message otherwise.
    pub fn as_number(&self) -> Result<f64, String> {
        match self {
            Evaluated::Number(n) => Ok(*n),
            Evaluated::Text(s) => s.trim().parse::<f64>().map_err(|_| format!("\"{s}\" is not a number")),
            Evaluated::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        }
    }

    /// Coerces to a boolean — always succeeds, same "loose" spirit as
    /// `as_number`: a nonzero number or a nonempty, non-`"false"` string
    /// counts as true.
    pub fn as_bool(&self) -> bool {
        match self {
            Evaluated::Bool(b) => *b,
            Evaluated::Number(n) => *n != 0.0,
            Evaluated::Text(s) => !s.is_empty() && s != "false",
        }
    }

    /// Stringifies for display/comparison — like `eval_text`, but from an
    /// already-evaluated result (no re-evaluation).
    fn as_text(&self) -> String {
        match self {
            Evaluated::Text(s) => s.clone(),
            Evaluated::Number(n) => n.to_string(),
            Evaluated::Bool(b) => b.to_string(),
        }
    }

    /// Rebuilds a `Value` leaf holding this result — `Bool` becomes a
    /// `True`/`False` op node, since there's no bare boolean `Value` leaf.
    pub fn into_value(self) -> Value {
        match self {
            Evaluated::Number(value) => Value::Number { value },
            Evaluated::Text(value) => Value::Text { value },
            Evaluated::Bool(b) => Value::Op {
                op: if b { Op::True } else { Op::False },
                args: vec![],
                saved: Box::new(Value::number(0.0)),
            },
        }
    }
}

/// Per-palette-entry metadata for default construction when a block is
/// dropped from the sidebar. Arity lives on the kind string, not `Op`, since
/// `Op::Join` alone covers both the 2- and 3-arg palette entries.
pub struct OperatorKindSpec {
    pub kind: &'static str,
    pub op: Op,
    pub arity: usize,
    /// Builds the default `args` vec — a function (not a fixed `Vec`, since
    /// `Value` isn't `Const`) so mixed-type operators like `LetterOf` can
    /// give each slot its own default.
    pub default_args: fn() -> Vec<Value>,
}

fn text_default() -> Value {
    Value::Text { value: String::new() }
}

/// Default for a boolean-typed operand slot — blank (`Value::Bool`), same
/// spirit as `text_default`, not a pre-filled `false`.
fn bool_default() -> Value {
    Value::Bool
}

pub const OPERATOR_KINDS: &[OperatorKindSpec] = &[
    OperatorKindSpec { kind: "Add", op: Op::Add, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Sub", op: Op::Sub, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Mul", op: Op::Mul, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Div", op: Op::Div, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Mod", op: Op::Mod, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Round", op: Op::Round, arity: 1, default_args: || vec![Value::number(0.0)] },
    OperatorKindSpec { kind: "Random", op: Op::Random, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Join", op: Op::Join, arity: 2, default_args: || vec![text_default(), text_default()] },
    OperatorKindSpec { kind: "Join3", op: Op::Join, arity: 3, default_args: || vec![text_default(), text_default(), text_default()] },
    // `default_args` is unused for these — arity 0 means it's never called.
    OperatorKindSpec { kind: "NewLine", op: Op::NewLine, arity: 0, default_args: Vec::new },
    OperatorKindSpec { kind: "Tab", op: Op::Tab, arity: 0, default_args: Vec::new },
    OperatorKindSpec { kind: "IndexOf", op: Op::IndexOf, arity: 2, default_args: || vec![text_default(), text_default()] },
    OperatorKindSpec { kind: "LastIndexOf", op: Op::LastIndexOf, arity: 2, default_args: || vec![text_default(), text_default()] },
    OperatorKindSpec { kind: "LetterOf", op: Op::LetterOf, arity: 2, default_args: || vec![Value::number(1.0), text_default()] },
    OperatorKindSpec { kind: "Length", op: Op::Length, arity: 1, default_args: || vec![text_default()] },
    // `args[1]` defaults to the dropdown's "uppercase" option — see
    // `Op::Case`'s doc comment.
    OperatorKindSpec {
        kind: "Case",
        op: Op::Case,
        arity: 2,
        default_args: || vec![text_default(), Value::Text { value: "Upper".to_string() }],
    },
    OperatorKindSpec { kind: "Eq", op: Op::Eq, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Neq", op: Op::Neq, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Gt", op: Op::Gt, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Lt", op: Op::Lt, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Gte", op: Op::Gte, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "Lte", op: Op::Lte, arity: 2, default_args: || vec![Value::number(0.0), Value::number(0.0)] },
    OperatorKindSpec { kind: "And", op: Op::And, arity: 2, default_args: || vec![bool_default(), bool_default()] },
    OperatorKindSpec { kind: "Or", op: Op::Or, arity: 2, default_args: || vec![bool_default(), bool_default()] },
    OperatorKindSpec { kind: "Not", op: Op::Not, arity: 1, default_args: || vec![bool_default()] },
    // Zero-arity, like NewLine/Tab — a standalone "true"/"false" block, not a toggle.
    OperatorKindSpec { kind: "True", op: Op::True, arity: 0, default_args: Vec::new },
    OperatorKindSpec { kind: "False", op: Op::False, arity: 0, default_args: Vec::new },
    // Zero-arity, like NewLine/Tab — evaluates to the live system battery percentage.
    OperatorKindSpec { kind: "BatteryPercentage", op: Op::BatteryPercentage, arity: 0, default_args: Vec::new },
    // Zero-arity, like True/False — evaluates to whether the system is
    // currently on external power.
    OperatorKindSpec { kind: "PluggedIn", op: Op::PluggedIn, arity: 0, default_args: Vec::new },
    // `args[0]` defaults to the dropdown's first ("Year") option — same
    // shape as `Case`, and matching valueOps.ts's CURRENT_TIME_OPTIONS order
    // (the frontend's own default for a freshly-dragged block always picks
    // the enumArg's first option).
    OperatorKindSpec { kind: "CurrentTime", op: Op::CurrentTime, arity: 1, default_args: || vec![Value::Text { value: "Year".to_string() }] },
];

/// 1-based char index of the first occurrence of `needle` in `haystack`, or
/// `0` if not found/empty/too long. Scans by char, not byte, for multi-byte text.
fn char_index_of(haystack: &str, needle: &str) -> usize {
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || n.len() > h.len() {
        return 0;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()] == n[..]).map_or(0, |i| i + 1)
}

/// Same as [`char_index_of`], but the last occurrence.
fn char_last_index_of(haystack: &str, needle: &str) -> usize {
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || n.len() > h.len() {
        return 0;
    }
    (0..=h.len() - n.len()).rev().find(|&i| h[i..i + n.len()] == n[..]).map_or(0, |i| i + 1)
}

/// `Op::Eq`'s comparison rule: numeric if both sides parse as a number,
/// otherwise a text comparison (mirrors Scratch's loose `=`).
fn values_equal(l: &Evaluated, r: &Evaluated) -> bool {
    match (l.as_number(), r.as_number()) {
        (Ok(ln), Ok(rn)) => ln == rn,
        _ => l.as_text() == r.as_text(),
    }
}

impl Value {
    pub fn number(value: f64) -> Self {
        Value::Number { value }
    }

    pub fn eval(&self) -> Result<Evaluated, String> {
        match self {
            Value::Number { value } => Ok(Evaluated::Number(*value)),
            Value::Text { value } => Ok(Evaluated::Text(value.clone())),
            // The "nothing plugged in" state of a boolean slot acts as false.
            Value::Bool => Ok(Evaluated::Bool(false)),
            // Every real call site resolves vars via `resolve_vars` first; this
            // only fires if that step was skipped (a bug, not something a
            // macro can trigger).
            Value::Var { .. } => Err("unresolved variable reference".to_string()),
            // Same invariant as `Var` — only the runner can resolve these
            // (resolving `Call` requires actually executing instructions,
            // which this module can't do). That also keeps preview's
            // best-effort `resolve_vars().eval()` safe: a block call just
            // errors instead of running anything.
            Value::Param { .. } => Err("unresolved parameter reference".to_string()),
            Value::Call { .. } => Err("custom block calls can't be evaluated directly".to_string()),
            Value::Op { op: Op::Join, args, .. } => {
                let mut s = String::new();
                for a in args {
                    s.push_str(&a.eval_text()?);
                }
                Ok(Evaluated::Text(s))
            }
            Value::Op { op: Op::NewLine, .. } => Ok(Evaluated::Text("\n".to_string())),
            Value::Op { op: Op::Tab, .. } => Ok(Evaluated::Text("\t".to_string())),
            Value::Op { op: Op::Length, args, .. } => Ok(Evaluated::Number(args[0].eval_text()?.chars().count() as f64)),
            Value::Op { op: Op::IndexOf, args, .. } => {
                let needle = args[0].eval_text()?;
                let haystack = args[1].eval_text()?;
                Ok(Evaluated::Number(char_index_of(&haystack, &needle) as f64))
            }
            Value::Op { op: Op::LastIndexOf, args, .. } => {
                let needle = args[0].eval_text()?;
                let haystack = args[1].eval_text()?;
                Ok(Evaluated::Number(char_last_index_of(&haystack, &needle) as f64))
            }
            Value::Op { op: Op::LetterOf, args, .. } => {
                let index = args[0].eval_number()? as i64;
                let text = args[1].eval_text()?;
                let chars: Vec<char> = text.chars().collect();
                if index < 1 || index as usize > chars.len() {
                    return Err(format!("letter {index} is out of range for a {}-character value", chars.len()));
                }
                Ok(Evaluated::Text(chars[index as usize - 1].to_string()))
            }
            Value::Op { op: Op::Case, args, .. } => {
                let text = args[0].eval_text()?;
                let upper = args[1].eval_text()? == "Upper";
                Ok(Evaluated::Text(if upper { text.to_uppercase() } else { text.to_lowercase() }))
            }
            Value::Op { op: Op::Round, args, .. } => Ok(Evaluated::Number(args[0].eval_number()?.round())),
            Value::Op { op: Op::True, .. } => Ok(Evaluated::Bool(true)),
            Value::Op { op: Op::False, .. } => Ok(Evaluated::Bool(false)),
            Value::Op { op: Op::BatteryPercentage, .. } => Ok(Evaluated::Number(crate::battery::percentage()?)),
            Value::Op { op: Op::PluggedIn, .. } => Ok(Evaluated::Bool(crate::battery::is_plugged_in())),
            Value::Op { op: Op::CurrentTime, args, .. } => {
                use chrono::{Datelike, Timelike};
                let now = chrono::Local::now();
                let n = match args[0].eval_text()?.as_str() {
                    "Year" => now.year() as f64,
                    "Month" => now.month() as f64,
                    "Date" => now.day() as f64,
                    // 1 (Sunday) through 7 (Saturday), matching Scratch's own
                    // "current (day of week)" numbering.
                    "DayOfWeek" => now.weekday().num_days_from_sunday() as f64 + 1.0,
                    "Hour" => now.hour() as f64,
                    "Minute" => now.minute() as f64,
                    "Second" => now.second() as f64,
                    other => return Err(format!("unknown current-time component '{other}'")),
                };
                Ok(Evaluated::Number(n))
            }
            Value::Op { op: Op::Not, args, .. } => Ok(Evaluated::Bool(!args[0].eval()?.as_bool())),
            Value::Op { op: Op::And, args, .. } => Ok(Evaluated::Bool(args[0].eval()?.as_bool() && args[1].eval()?.as_bool())),
            Value::Op { op: Op::Or, args, .. } => Ok(Evaluated::Bool(args[0].eval()?.as_bool() || args[1].eval()?.as_bool())),
            Value::Op { op: Op::Eq, args, .. } => Ok(Evaluated::Bool(values_equal(&args[0].eval()?, &args[1].eval()?))),
            Value::Op { op: Op::Neq, args, .. } => Ok(Evaluated::Bool(!values_equal(&args[0].eval()?, &args[1].eval()?))),
            Value::Op { op: Op::Gt, args, .. } => Ok(Evaluated::Bool(args[0].eval()?.as_number()? > args[1].eval()?.as_number()?)),
            Value::Op { op: Op::Lt, args, .. } => Ok(Evaluated::Bool(args[0].eval()?.as_number()? < args[1].eval()?.as_number()?)),
            Value::Op { op: Op::Gte, args, .. } => Ok(Evaluated::Bool(args[0].eval()?.as_number()? >= args[1].eval()?.as_number()?)),
            Value::Op { op: Op::Lte, args, .. } => Ok(Evaluated::Bool(args[0].eval()?.as_number()? <= args[1].eval()?.as_number()?)),
            Value::Op { op, args, .. } => {
                let l = args[0].eval()?.as_number()?;
                let r = args[1].eval()?.as_number()?;
                let result = match op {
                    Op::Add => l + r,
                    Op::Sub => l - r,
                    Op::Mul => l * r,
                    Op::Div => {
                        if r == 0.0 {
                            return Err("division by zero".to_string());
                        }
                        l / r
                    }
                    Op::Mod => {
                        if r == 0.0 {
                            return Err("mod by zero".to_string());
                        }
                        l.rem_euclid(r)
                    }
                    Op::Random => {
                        let (lo, hi) = (l.min(r), l.max(r));
                        if l.fract() == 0.0 && r.fract() == 0.0 {
                            rand::rng().random_range(lo as i64..=hi as i64) as f64
                        } else {
                            rand::rng().random_range(lo..=hi)
                        }
                    }
                    Op::Join | Op::NewLine | Op::Tab | Op::Length | Op::IndexOf | Op::LastIndexOf | Op::LetterOf | Op::Case
                    | Op::Round | Op::True | Op::False | Op::Not | Op::And | Op::Or | Op::Eq | Op::Neq | Op::Gt | Op::Lt
                    | Op::Gte | Op::Lte | Op::BatteryPercentage | Op::PluggedIn | Op::CurrentTime => unreachable!("matched above"),
                };
                Ok(Evaluated::Number(result))
            }
        }
    }

    /// Evaluates the tree and coerces the result to a number — the entry
    /// point every numeric instruction field actually calls.
    pub fn eval_number(&self) -> Result<f64, String> {
        self.eval()?.as_number()
    }

    /// Evaluates the tree to a string — the entry point the `Text`
    /// instruction calls. Text results pass through verbatim; numeric
    /// results get stringified.
    pub fn eval_text(&self) -> Result<String, String> {
        Ok(match self.eval()? {
            Evaluated::Text(s) => s,
            Evaluated::Number(n) => n.to_string(),
            Evaluated::Bool(b) => b.to_string(),
        })
    }

    /// Walks `path` (`n` steps into `args[n]` at each `Op`) down to the
    /// addressed node; an empty path returns `self`.
    pub fn get_mut(&mut self, path: &[u8]) -> Option<&mut Value> {
        match path.split_first() {
            None => Some(self),
            Some((&step, rest)) => match self {
                Value::Op { args, .. } | Value::Call { args, .. } => args.get_mut(step as usize)?.get_mut(rest),
                _ => None,
            },
        }
    }

    /// Renames every `Value::Var` leaf reading `old` (including in `saved`)
    /// to `new`, so existing blocks keep working after a variable rename.
    pub fn rename_var(&mut self, old: &str, new: &str) {
        match self {
            Value::Var { name } => {
                if name == old {
                    *name = new.to_string();
                }
            }
            Value::Op { args, saved, .. } | Value::Call { args, saved, .. } => {
                for arg in args.iter_mut() {
                    arg.rename_var(old, new);
                }
                saved.rename_var(old, new);
            }
            Value::Number { .. } | Value::Text { .. } | Value::Bool | Value::Param { .. } => {}
        }
    }

    /// Renames every `Value::Param` leaf reading `old` to `new` (including in
    /// `saved`) — counterpart to `rename_var`, for when a block input is renamed.
    pub fn rename_param(&mut self, old: &str, new: &str) {
        match self {
            Value::Param { name } => {
                if name == old {
                    *name = new.to_string();
                }
            }
            Value::Op { args, saved, .. } | Value::Call { args, saved, .. } => {
                for arg in args.iter_mut() {
                    arg.rename_param(old, new);
                }
                saved.rename_param(old, new);
            }
            Value::Number { .. } | Value::Text { .. } | Value::Bool | Value::Var { .. } => {}
        }
    }

    /// Applies `f` to the `args` of every `Value::Call` node referencing
    /// `block_id` (including nested calls) — keeps call sites' argument
    /// lists aligned with the block's current pieces.
    pub fn for_each_call_args_mut(&mut self, block_id: &str, f: &mut dyn FnMut(&mut Vec<Value>)) {
        match self {
            Value::Number { .. } | Value::Text { .. } | Value::Bool | Value::Var { .. } | Value::Param { .. } => {}
            Value::Op { args, saved, .. } => {
                for a in args.iter_mut() {
                    a.for_each_call_args_mut(block_id, f);
                }
                saved.for_each_call_args_mut(block_id, f);
            }
            Value::Call { block_id: id, args, saved } => {
                if id == block_id {
                    f(args);
                }
                for a in args.iter_mut() {
                    a.for_each_call_args_mut(block_id, f);
                }
                saved.for_each_call_args_mut(block_id, f);
            }
        }
    }

    /// Replaces every `Value::Call` node referencing `block_id` with a plain
    /// `0` leaf, so deleting a block scrubs references instead of leaving
    /// dangling ones.
    pub fn scrub_block_calls(&mut self, block_id: &str) {
        match self {
            Value::Number { .. } | Value::Text { .. } | Value::Bool | Value::Var { .. } | Value::Param { .. } => {}
            Value::Op { args, saved, .. } => {
                for a in args.iter_mut() {
                    a.scrub_block_calls(block_id);
                }
                saved.scrub_block_calls(block_id);
            }
            Value::Call { block_id: id, args, saved } => {
                if id == block_id {
                    *self = Value::number(0.0);
                } else {
                    for a in args.iter_mut() {
                        a.scrub_block_calls(block_id);
                    }
                    saved.scrub_block_calls(block_id);
                }
            }
        }
    }

    /// Repairs a "poisoned" boolean slot left by a historical bug: before
    /// `Value::Bool` existed, a boolean-typed slot's blank state was a
    /// standalone `False` op whose `saved` fallback was a plain `Number(0)`
    /// — so dragging that (or any other block) out of an `If`/`IfElse`
    /// condition or an `And`/`Or`/`Not` operand could reveal a raw number
    /// leaf instead of the blank hexagon it should be. `expects_bool` is
    /// true exactly at the positions a boolean value belongs — the caller
    /// (an `Instruction`'s own `migrate_bool_slots`) passes `true` for an
    /// `If`/`IfElse` condition, `false` everywhere else; from there this
    /// threads it into `And`/`Or`/`Not` operands (and along every `saved`
    /// chain, which occupies the same slot as whatever displaced it) on its
    /// own. Only a bare `Number` gets converted — anything else already has
    /// a real shape, boolean-looking or not, so there's nothing to fix.
    pub fn migrate_bool_slots(&mut self, expects_bool: bool) {
        if expects_bool && matches!(self, Value::Number { .. }) {
            *self = Value::Bool;
            return;
        }
        match self {
            Value::Op { op, args, saved } => {
                let args_are_bool = matches!(op, Op::And | Op::Or | Op::Not);
                for arg in args.iter_mut() {
                    arg.migrate_bool_slots(args_are_bool);
                }
                saved.migrate_bool_slots(expects_bool);
            }
            Value::Call { args, saved, .. } => {
                for arg in args.iter_mut() {
                    arg.migrate_bool_slots(false);
                }
                saved.migrate_bool_slots(expects_bool);
            }
            Value::Number { .. } | Value::Text { .. } | Value::Bool | Value::Var { .. } | Value::Param { .. } => {}
        }
    }

    /// Rebuilds this tree with every `Value::Var` leaf replaced by its
    /// current value from `env` (defaulting to `0` if missing, so a stray
    /// reference doesn't error the whole tree). Must run before `eval`.
    pub fn resolve_vars(&self, env: &HashMap<String, Evaluated>) -> Value {
        match self {
            Value::Number { value } => Value::Number { value: *value },
            Value::Text { value } => Value::Text { value: value.clone() },
            Value::Bool => Value::Bool,
            Value::Op { op, args, saved } => Value::Op {
                op: *op,
                args: args.iter().map(|a| a.resolve_vars(env)).collect(),
                saved: Box::new(saved.resolve_vars(env)),
            },
            Value::Var { name } => match env.get(name) {
                Some(e) => e.clone().into_value(),
                None => Value::number(0.0),
            },
            // Left untouched — `Param`/`Call` need execution capability this
            // module doesn't have; only their nested `args` (which may
            // contain `Var` reads) get recursed into.
            Value::Param { name } => Value::Param { name: name.clone() },
            Value::Call { block_id, args, saved } => Value::Call {
                block_id: block_id.clone(),
                args: args.iter().map(|a| a.resolve_vars(env)).collect(),
                saved: Box::new(saved.resolve_vars(env)),
            },
        }
    }
}

/// Manual impl mirroring `Instruction`'s own manual `Hash` impl
/// (`macros/mod.rs`) — `f64` isn't `Hash`, so its bit pattern stands in.
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Number { value } => {
                0u8.hash(state);
                value.to_bits().hash(state);
            }
            Value::Text { value } => {
                1u8.hash(state);
                value.hash(state);
            }
            Value::Bool => {
                6u8.hash(state);
            }
            Value::Op { op, args, saved } => {
                2u8.hash(state);
                op.hash(state);
                args.hash(state);
                saved.hash(state);
            }
            Value::Var { name } => {
                3u8.hash(state);
                name.hash(state);
            }
            Value::Param { name } => {
                4u8.hash(state);
                name.hash(state);
            }
            Value::Call { block_id, args, saved } => {
                5u8.hash(state);
                block_id.hash(state);
                args.hash(state);
                saved.hash(state);
            }
        }
    }
}

/// Accepts either the tagged `{"kind": ...}` shape or a bare number/string —
/// old save files have plain values in slots `Value` now occupies, so this
/// keeps them loading. Mirrors the `WaitDe`/`InstructionDe` untagged-enum
/// pattern in `macros/mod.rs`.
impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        fn default_saved() -> Box<Value> {
            Box::new(Value::number(0.0))
        }

        #[derive(Deserialize)]
        #[serde(tag = "kind")]
        enum Tagged {
            Number { value: f64 },
            Text { value: String },
            Bool,
            Op {
                op: Op,
                args: Vec<Value>,
                // Older save files predate `saved` entirely — falls back to
                // a plain zero, same spirit as `ValueDe::Legacy` below.
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
            Var { name: String },
            Param { name: String },
            Call {
                block_id: String,
                args: Vec<Value>,
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
            // Legacy tags predating the `BinaryOp`/`Join` → `Op` unification —
            // kept so old saves still load, migrated into `Value::Op` below.
            BinaryOp {
                op: Op,
                lhs: Box<Value>,
                rhs: Box<Value>,
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
            Join {
                args: Vec<Value>,
                #[serde(default = "default_saved")]
                saved: Box<Value>,
            },
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ValueDe {
            Legacy(f64),
            // Pre-`Value` shape of `InputToken::Text` — a bare JSON string,
            // from when it held a plain `String` rather than a tree.
            LegacyText(String),
            Current(Tagged),
        }

        Ok(match ValueDe::deserialize(deserializer)? {
            ValueDe::Legacy(n) => Value::Number { value: n },
            ValueDe::LegacyText(s) => Value::Text { value: s },
            ValueDe::Current(Tagged::Number { value }) => Value::Number { value },
            ValueDe::Current(Tagged::Text { value }) => Value::Text { value },
            ValueDe::Current(Tagged::Bool) => Value::Bool,
            ValueDe::Current(Tagged::Op { op, args, saved }) => Value::Op { op, args, saved },
            ValueDe::Current(Tagged::Var { name }) => Value::Var { name },
            ValueDe::Current(Tagged::Param { name }) => Value::Param { name },
            ValueDe::Current(Tagged::Call { block_id, args, saved }) => Value::Call { block_id, args, saved },
            ValueDe::Current(Tagged::BinaryOp { op, lhs, rhs, saved }) => Value::Op { op, args: vec![*lhs, *rhs], saved },
            ValueDe::Current(Tagged::Join { args, saved }) => Value::Op { op: Op::Join, args, saved },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_bare_number_deserializes_to_number() {
        let v: Value = serde_json::from_str("5.0").unwrap();
        assert_eq!(v, Value::number(5.0));
    }

    #[test]
    fn legacy_bare_string_deserializes_to_text() {
        // Pre-`Value` save shape of `InputToken::Text`, from back when it
        // held a plain `String` — must keep loading old macros.
        let v: Value = serde_json::from_str("\"hello\"").unwrap();
        assert_eq!(v, Value::Text { value: "hello".into() });
    }

    #[test]
    fn eval_text_passes_through_text_leaf() {
        let v = Value::Text { value: "hi".into() };
        assert_eq!(v.eval_text(), Ok("hi".to_string()));
    }

    #[test]
    fn eval_text_stringifies_numeric_result() {
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::number(2.0), Value::number(3.0)],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_text(), Ok("5".to_string()));
    }

    #[test]
    fn tagged_shape_round_trips() {
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::number(2.0), Value::Text { value: "3".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn legacy_binary_op_tag_migrates_to_op() {
        let json = r#"{"kind":"BinaryOp","op":"Add","lhs":{"kind":"Number","value":2.0},"rhs":{"kind":"Number","value":3.0},"saved":{"kind":"Number","value":0.0}}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            v,
            Value::Op { op: Op::Add, args: vec![Value::number(2.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) }
        );
    }

    #[test]
    fn legacy_join_tag_migrates_to_op() {
        let json = r#"{"kind":"Join","args":[{"kind":"Text","value":"a"},{"kind":"Text","value":"b"}],"saved":{"kind":"Number","value":0.0}}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            v,
            Value::Op {
                op: Op::Join,
                args: vec![Value::Text { value: "a".into() }, Value::Text { value: "b".into() }],
                saved: Box::new(Value::number(0.0)),
            }
        );
    }

    #[test]
    fn eval_number_nested_expression() {
        // (2 + 3) * 4 == 20
        let v = Value::Op {
            op: Op::Mul,
            args: vec![
                Value::Op { op: Op::Add, args: vec![Value::number(2.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) },
                Value::number(4.0),
            ],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_number(), Ok(20.0));
    }

    #[test]
    fn eval_number_division_by_zero_errs() {
        let v = Value::Op { op: Op::Div, args: vec![Value::number(1.0), Value::number(0.0)], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn eval_number_random_integer_stays_in_range_when_both_bounds_whole() {
        let v = Value::Op { op: Op::Random, args: vec![Value::number(1.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) };
        for _ in 0..100 {
            let n = v.eval_number().unwrap();
            assert_eq!(n.fract(), 0.0);
            assert!((1.0..=3.0).contains(&n));
        }
    }

    #[test]
    fn eval_number_random_float_when_a_bound_has_a_fraction() {
        let v = Value::Op { op: Op::Random, args: vec![Value::number(1.0), Value::number(2.5)], saved: Box::new(Value::number(0.0)) };
        for _ in 0..100 {
            let n = v.eval_number().unwrap();
            assert!((1.0..=2.5).contains(&n));
        }
    }

    #[test]
    fn eval_number_random_handles_reversed_bounds() {
        let v = Value::Op { op: Op::Random, args: vec![Value::number(5.0), Value::number(1.0)], saved: Box::new(Value::number(0.0)) };
        for _ in 0..100 {
            let n = v.eval_number().unwrap();
            assert!((1.0..=5.0).contains(&n));
        }
    }

    #[test]
    fn eval_number_coerces_numeric_text() {
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::Text { value: "5".into() }, Value::number(3.0)],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_number(), Ok(8.0));
    }

    #[test]
    fn eval_number_non_numeric_text_errs() {
        let v = Value::Text { value: "abc".into() };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn get_mut_walks_path() {
        let mut v = Value::Op {
            op: Op::Add,
            args: vec![
                Value::number(1.0),
                Value::Op { op: Op::Mul, args: vec![Value::number(2.0), Value::number(3.0)], saved: Box::new(Value::number(0.0)) },
            ],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.get_mut(&[1, 0]), Some(&mut Value::number(2.0)));
        assert_eq!(v.get_mut(&[0]), Some(&mut Value::number(1.0)));
        assert_eq!(v.get_mut(&[5]), None);
    }

    #[test]
    fn eval_text_joins_two_args() {
        let v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "foo".into() }, Value::Text { value: "bar".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_text(), Ok("foobar".to_string()));
    }

    #[test]
    fn eval_text_joins_three_args_and_stringifies_numbers() {
        let v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "a".into() }, Value::number(2.0), Value::Text { value: "c".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.eval_text(), Ok("a2c".to_string()));
    }

    #[test]
    fn get_mut_walks_join_args() {
        let mut v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "a".into() }, Value::Text { value: "b".into() }, Value::Text { value: "c".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        assert_eq!(v.get_mut(&[2]), Some(&mut Value::Text { value: "c".into() }));
        assert_eq!(v.get_mut(&[3]), None);
    }

    #[test]
    fn eval_text_new_line_is_constant_regardless_of_args() {
        let v = Value::Op { op: Op::NewLine, args: vec![], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("\n".to_string()));
    }

    #[test]
    fn eval_text_tab_is_constant_regardless_of_args() {
        let v = Value::Op { op: Op::Tab, args: vec![], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("\t".to_string()));
    }

    #[test]
    fn join_tagged_shape_round_trips() {
        let v = Value::Op {
            op: Op::Join,
            args: vec![Value::Text { value: "a".into() }, Value::Text { value: "b".into() }],
            saved: Box::new(Value::number(0.0)),
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    fn text(s: &str) -> Value {
        Value::Text { value: s.to_string() }
    }

    #[test]
    fn eval_number_mod_wraps_like_rem_euclid() {
        let v = Value::Op { op: Op::Mod, args: vec![Value::number(-1.0), Value::number(5.0)], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(4.0));
    }

    #[test]
    fn eval_number_mod_by_zero_errs() {
        let v = Value::Op { op: Op::Mod, args: vec![Value::number(1.0), Value::number(0.0)], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn eval_number_round_rounds_half_away_from_zero() {
        let v = Value::Op { op: Op::Round, args: vec![Value::number(2.5)], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(3.0));
    }

    #[test]
    fn eval_number_length_counts_chars_not_bytes() {
        let v = Value::Op { op: Op::Length, args: vec![text("héllo")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(5.0));
    }

    #[test]
    fn eval_number_index_of_is_one_based() {
        let v = Value::Op { op: Op::IndexOf, args: vec![text("lo"), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(4.0));
    }

    #[test]
    fn eval_number_index_of_not_found_is_zero() {
        let v = Value::Op { op: Op::IndexOf, args: vec![text("xyz"), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(0.0));
    }

    #[test]
    fn eval_number_last_index_of_finds_final_occurrence() {
        let v = Value::Op { op: Op::LastIndexOf, args: vec![text("l"), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_number(), Ok(4.0));
    }

    #[test]
    fn eval_text_letter_of_is_one_based() {
        let v = Value::Op { op: Op::LetterOf, args: vec![Value::number(1.0), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("h".to_string()));
    }

    #[test]
    fn eval_text_letter_of_out_of_range_errs() {
        let v = Value::Op { op: Op::LetterOf, args: vec![Value::number(6.0), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_text().is_err());
    }

    #[test]
    fn eval_text_letter_of_rejects_zero_index() {
        let v = Value::Op { op: Op::LetterOf, args: vec![Value::number(0.0), text("hello")], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_text().is_err());
    }

    #[test]
    fn current_time_components_stay_in_range() {
        let current = |which: &str| {
            let v = Value::Op { op: Op::CurrentTime, args: vec![text(which)], saved: Box::new(Value::number(0.0)) };
            v.eval_number().unwrap()
        };
        assert!(current("Year") >= 2020.0);
        assert!((1.0..=12.0).contains(&current("Month")));
        assert!((1.0..=31.0).contains(&current("Date")));
        assert!((1.0..=7.0).contains(&current("DayOfWeek")));
        assert!((0.0..=23.0).contains(&current("Hour")));
        assert!((0.0..=59.0).contains(&current("Minute")));
        assert!((0.0..=59.0).contains(&current("Second")));
    }

    #[test]
    fn current_time_rejects_unknown_component() {
        let v = Value::Op { op: Op::CurrentTime, args: vec![text("Fortnight")], saved: Box::new(Value::number(0.0)) };
        assert!(v.eval_number().is_err());
    }

    #[test]
    fn eval_text_case_uppercases_by_default_flag() {
        let v = Value::Op { op: Op::Case, args: vec![text("Hello"), text("Upper")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("HELLO".to_string()));
    }

    #[test]
    fn eval_text_case_lowercases_when_flagged() {
        let v = Value::Op { op: Op::Case, args: vec![text("Hello"), text("Lower")], saved: Box::new(Value::number(0.0)) };
        assert_eq!(v.eval_text(), Ok("hello".to_string()));
    }

    #[test]
    fn resolve_vars_replaces_leaf_with_current_number_value() {
        let env = HashMap::from([("x".to_string(), Evaluated::Number(5.0))]);
        let v = Value::Var { name: "x".to_string() };
        assert_eq!(v.resolve_vars(&env), Value::number(5.0));
    }

    #[test]
    fn resolve_vars_replaces_leaf_with_current_text_value() {
        let env = HashMap::from([("s".to_string(), Evaluated::Text("hi".to_string()))]);
        let v = Value::Var { name: "s".to_string() };
        assert_eq!(v.resolve_vars(&env), text("hi"));
    }

    #[test]
    fn resolve_vars_defaults_missing_name_to_zero() {
        let v = Value::Var { name: "missing".to_string() };
        assert_eq!(v.resolve_vars(&HashMap::new()), Value::number(0.0));
    }

    #[test]
    fn resolve_vars_recurses_into_op_args_and_saved() {
        let env = HashMap::from([("x".to_string(), Evaluated::Number(2.0))]);
        let v = Value::Op {
            op: Op::Add,
            args: vec![Value::Var { name: "x".to_string() }, Value::number(3.0)],
            saved: Box::new(Value::Var { name: "x".to_string() }),
        };
        let resolved = v.resolve_vars(&env);
        assert_eq!(resolved.eval_number(), Ok(5.0));
        match resolved {
            Value::Op { saved, .. } => assert_eq!(*saved, Value::number(2.0)),
            _ => panic!("expected Op"),
        }
    }

    #[test]
    fn eval_errs_on_unresolved_var() {
        let v = Value::Var { name: "x".to_string() };
        assert!(v.eval().is_err());
    }

    #[test]
    fn rename_var_renames_matching_leaf() {
        let mut v = Value::Var { name: "x".to_string() };
        v.rename_var("x", "y");
        assert_eq!(v, Value::Var { name: "y".to_string() });
    }

    #[test]
    fn rename_var_ignores_non_matching_leaf() {
        let mut v = Value::Var { name: "x".to_string() };
        v.rename_var("z", "y");
        assert_eq!(v, Value::Var { name: "x".to_string() });
    }

    #[test]
    fn rename_var_recurses_into_op_args_and_saved() {
        let mut v = Value::Op {
            op: Op::Add,
            args: vec![Value::Var { name: "x".to_string() }, Value::number(3.0)],
            saved: Box::new(Value::Var { name: "x".to_string() }),
        };
        v.rename_var("x", "y");
        match v {
            Value::Op { args, saved, .. } => {
                assert_eq!(args[0], Value::Var { name: "y".to_string() });
                assert_eq!(*saved, Value::Var { name: "y".to_string() });
            }
            _ => panic!("expected Op"),
        }
    }

    fn bool_op(op: Op, args: Vec<Value>) -> Value {
        Value::Op { op, args, saved: Box::new(Value::number(0.0)) }
    }

    #[test]
    fn eval_true_and_false_are_fixed_constants() {
        assert_eq!(bool_op(Op::True, vec![]).eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(bool_op(Op::False, vec![]).eval(), Ok(Evaluated::Bool(false)));
    }

    #[test]
    fn eval_not_negates() {
        assert_eq!(bool_op(Op::Not, vec![bool_op(Op::True, vec![])]).eval(), Ok(Evaluated::Bool(false)));
        assert_eq!(bool_op(Op::Not, vec![bool_op(Op::False, vec![])]).eval(), Ok(Evaluated::Bool(true)));
    }

    #[test]
    fn eval_and_or_combine_operands() {
        let t = || bool_op(Op::True, vec![]);
        let f = || bool_op(Op::False, vec![]);
        assert_eq!(bool_op(Op::And, vec![t(), t()]).eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(bool_op(Op::And, vec![t(), f()]).eval(), Ok(Evaluated::Bool(false)));
        assert_eq!(bool_op(Op::Or, vec![f(), f()]).eval(), Ok(Evaluated::Bool(false)));
        assert_eq!(bool_op(Op::Or, vec![f(), t()]).eval(), Ok(Evaluated::Bool(true)));
    }

    #[test]
    fn eval_not_coerces_non_boolean_operand() {
        // A nonzero number is truthy, so `not 5` is false.
        assert_eq!(bool_op(Op::Not, vec![Value::number(5.0)]).eval(), Ok(Evaluated::Bool(false)));
        assert_eq!(bool_op(Op::Not, vec![Value::number(0.0)]).eval(), Ok(Evaluated::Bool(true)));
    }

    #[test]
    fn eval_comparisons_are_numeric() {
        assert_eq!(bool_op(Op::Gt, vec![Value::number(5.0), Value::number(3.0)]).eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(bool_op(Op::Lt, vec![Value::number(5.0), Value::number(3.0)]).eval(), Ok(Evaluated::Bool(false)));
        assert_eq!(bool_op(Op::Gte, vec![Value::number(3.0), Value::number(3.0)]).eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(bool_op(Op::Lte, vec![Value::number(3.0), Value::number(3.0)]).eval(), Ok(Evaluated::Bool(true)));
    }

    #[test]
    fn eval_eq_compares_numerically_when_both_sides_are_numeric() {
        // "5" and 5.0 are numerically equal even though one is text.
        assert_eq!(bool_op(Op::Eq, vec![text("5"), Value::number(5.0)]).eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(bool_op(Op::Neq, vec![text("5"), Value::number(5.0)]).eval(), Ok(Evaluated::Bool(false)));
    }

    #[test]
    fn eval_eq_falls_back_to_text_comparison_for_non_numeric_text() {
        assert_eq!(bool_op(Op::Eq, vec![text("hello"), text("hello")]).eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(bool_op(Op::Eq, vec![text("hello"), text("world")]).eval(), Ok(Evaluated::Bool(false)));
        assert_eq!(bool_op(Op::Eq, vec![text("hello"), Value::number(5.0)]).eval(), Ok(Evaluated::Bool(false)));
    }

    #[test]
    fn as_bool_coerces_loosely() {
        assert!(Evaluated::Number(1.0).as_bool());
        assert!(!Evaluated::Number(0.0).as_bool());
        assert!(Evaluated::Text("hi".to_string()).as_bool());
        assert!(!Evaluated::Text(String::new()).as_bool());
        assert!(!Evaluated::Text("false".to_string()).as_bool());
    }

    #[test]
    fn into_value_rebuilds_bool_as_true_false_op() {
        assert_eq!(Evaluated::Bool(true).into_value().eval(), Ok(Evaluated::Bool(true)));
        assert_eq!(Evaluated::Bool(false).into_value().eval(), Ok(Evaluated::Bool(false)));
    }
}
