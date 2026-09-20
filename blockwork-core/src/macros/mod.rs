//! Blockwork's block vocabulary - the instructions its canvas speaks - and
//! the `Macro` document wrapped around blockstitch's [`BlockGraph`].
//! [`InstructionKind`]'s [`BlockKind`] impl is the hinge between the two.

use crate::input::schedule::TimeSchedule;
use crate::input::types::InputToken;
use crate::input::value::{Op, Value};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

pub mod backend;
pub mod fields;
pub mod loop_control;
pub mod priority;
pub mod run_registry;
pub mod runner;
pub mod thread_pool;

pub use blockstitch_core::graph::{
    BlockDef, BlockGraph, BlockKind, BlockPiece, BlockShape, Comment, FloatingValue,
    InputValueType, VariableDef, default_block_color, normalize_block_color,
};
pub use fields::FieldId;

/// One instruction on the canvas: Blockwork's [`InstructionKind`] plus the
/// stable id blockstitch tracks it by.
pub type Instruction = blockstitch_core::graph::Instruction<InstructionKind>;
/// One draggable stack of Blockwork instructions.
pub type Strand = blockstitch_core::graph::Strand<InstructionKind>;
/// Everything on a macro's canvas - see [`Macro`], which owns one.
pub type MacroGraph = BlockGraph<InstructionKind>;

fn default_macro_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_speed_multiplier() -> f64 {
    1.0
}

/// Id of the single implicit strand used before "When Ran" blocks existed.
/// Kept only so loading an old save file can find and migrate that strand.
const LEGACY_ROOT_STRAND_ID: &str = "root";

/// Blockwork's half of the block-editor contract: where its instructions
/// keep values and bodies, which are headers, and how field ids map onto
/// their slots. Every traversal built on this lives in blockstitch.
impl BlockKind for InstructionKind {
    const HEADER_LABEL: &'static str = "When Ran/Block Definition";

    fn visit_values_mut(&mut self, f: &mut dyn FnMut(&mut Value, InputValueType)) {
        match self {
            InstructionKind::Token(token) => token.visit_values_mut(f),
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value)
            | InstructionKind::SetVariable(_, value)
            | InstructionKind::ChangeVariable(_, value)
            | InstructionKind::Repeat { count: value, .. } => f(value, InputValueType::Any),
            // A call site's declared Boolean inputs live on the `BlockDef`,
            // not here - `BlockGraph::migrate_bool_slots` handles those.
            InstructionKind::CallBlock { args, .. } => {
                for arg in args {
                    f(arg, InputValueType::Any);
                }
            }
            InstructionKind::If { condition, .. }
            | InstructionKind::IfElse { condition, .. }
            | InstructionKind::While { condition, .. } => f(condition, InputValueType::Bool),
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::WhenTime(_)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::Forever { .. }
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => {}
        }
    }

    /// True for "header" blocks (`WhenRan`, `BlockHeader`, and the event
    /// entry points) - must be first in their strand, nothing may stack
    /// above them, and they render with a flat top edge.
    fn is_header(&self) -> bool {
        matches!(
            self,
            InstructionKind::WhenRan
                | InstructionKind::BlockHeader(_)
                | InstructionKind::WhenBatteryDischargedTo(_)
                | InstructionKind::WhenBatteryChargedTo(_)
                | InstructionKind::WhenTime(_)
                | InstructionKind::WhenPowerPluggedIn
                | InstructionKind::WhenPowerUnplugged
        )
    }

    fn body(&self, slot: u8) -> Option<&Vec<Instruction>> {
        match (self, slot) {
            (InstructionKind::If { body, .. }, 0) => Some(body),
            (InstructionKind::IfElse { then_body, .. }, 0) => Some(then_body),
            (InstructionKind::IfElse { else_body, .. }, 1) => Some(else_body),
            (InstructionKind::Repeat { body, .. }, 0) => Some(body),
            (InstructionKind::Forever { body }, 0) => Some(body),
            (InstructionKind::While { body, .. }, 0) => Some(body),
            _ => None,
        }
    }

    fn body_mut(&mut self, slot: u8) -> Option<&mut Vec<Instruction>> {
        match (self, slot) {
            (InstructionKind::If { body, .. }, 0) => Some(body),
            (InstructionKind::IfElse { then_body, .. }, 0) => Some(then_body),
            (InstructionKind::IfElse { else_body, .. }, 1) => Some(else_body),
            (InstructionKind::Repeat { body, .. }, 0) => Some(body),
            (InstructionKind::Forever { body }, 0) => Some(body),
            (InstructionKind::While { body, .. }, 0) => Some(body),
            _ => None,
        }
    }

    fn variable_target_mut(&mut self) -> Option<&mut String> {
        match self {
            InstructionKind::SetVariable(name, _) | InstructionKind::ChangeVariable(name, _) => {
                Some(name)
            }
            _ => None,
        }
    }

    fn calls_block(&self, block_id: &str) -> bool {
        matches!(self, InstructionKind::CallBlock { block_id: id, .. } if id == block_id)
    }

    fn call_args_mut(&mut self, block_id: &str) -> Option<&mut Vec<Value>> {
        match self {
            InstructionKind::CallBlock { block_id: id, args } if id == block_id => Some(args),
            _ => None,
        }
    }

    fn block_header_id(&self) -> Option<&str> {
        match self {
            InstructionKind::BlockHeader(id) => Some(id),
            _ => None,
        }
    }

    fn value_slot_mut(&mut self, field: &str) -> Option<&mut Value> {
        fields::value_slot_mut(self, field.parse().ok()?)
    }

    fn blank_field_value(&self, field: &str, blocks: &[BlockDef]) -> Option<Value> {
        fields::blank_field_value(self, field.parse().ok()?, blocks)
    }

    fn field_requires_integer(&self, field: &str) -> bool {
        field
            .parse()
            .is_ok_and(|field: FieldId| field.requires_integer())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "InstructionKindDe")]
pub enum InstructionKind {
    Token(InputToken),
    Wait(Value),
    Command(String),
    Comment(String),
    /// Marks a strand as an entry point (always at index 0): it runs as its
    /// own concurrent thread when the macro runs. A macro can have several.
    WhenRan,
    /// Header-only marker (like `WhenRan`/`BlockHeader`) for a strand whose
    /// body should run whenever the system's battery charge drops to (or
    /// below) the given percentage. Unlike `WhenRan`, this is *not* an
    /// entry point Run/Loop invokes - `runner::run_with_offset` skips these
    /// strands entirely. Instead they're driven independently by a
    /// long-running background watcher outside a macro run altogether (in
    /// the desktop app, `src-tauri`'s `battery_watch` module), which polls
    /// the battery, fires the strand's body (everything after this marker)
    /// the moment the condition holds, and won't fire it again until the
    /// battery recovers past the threshold and crosses it again.
    WhenBatteryDischargedTo(Value),
    /// Same as `WhenBatteryDischargedTo`, but fires when the battery charge
    /// rises to (or above) the given percentage instead.
    WhenBatteryChargedTo(Value),
    /// Header-only marker, same shape/semantics as `WhenBatteryDischargedTo`
    /// (excluded from Run/Loop, driven by a background watcher - `time_watch`
    /// in the desktop app) but for a recurring point in local time instead
    /// of a battery level. See `TimeSchedule` for the recurrence shapes.
    WhenTime(TimeSchedule),
    /// Header-only marker, no payload (like `WhenRan`) - excluded from
    /// Run/Loop and driven by the same background watcher as
    /// `WhenBattery*To` (`battery_watch` in the desktop app), which fires
    /// this strand's body the moment the system starts receiving external
    /// power. See `crate::battery::is_plugged_in`.
    WhenPowerPluggedIn,
    /// Same as `WhenPowerPluggedIn`, but fires when external power is lost
    /// instead - never fires at all on a system with no battery/UPS, since
    /// `is_plugged_in` is always `true` there.
    WhenPowerUnplugged,
    /// Launches an installed application, chosen via the desktop app's "Open
    /// App" picker (`src-tauri`'s `installed_apps` module lists candidates).
    /// `command` is the already-resolved, platform-specific launch string
    /// (a cleaned freedesktop `Exec=` line on Linux, a `.lnk` path on
    /// Windows, an `.app` bundle path on macOS) captured at pick time -
    /// running it later never re-queries the installed-app list. `name` and
    /// `icon` (a `data:` URI, when one was found) are cached at the same
    /// time purely for display, so the block keeps showing the right label
    /// and picture even if the app is later renamed or uninstalled.
    OpenApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    /// Same picker/payload shape as `OpenApp`, but terminates the app
    /// instead of launching it - `runner::close_app` derives a process
    /// matcher from `command` (and, on macOS, `name`) rather than executing
    /// it directly. `command`/`name`/`icon` are cached at pick time for the
    /// exact same reason `OpenApp`'s are.
    CloseApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    /// `set <name> to <value>` - overwrites the named variable.
    SetVariable(String, Value),
    /// `change <name> by <value>` - adds `value` to the named variable.
    /// No-op if `value` isn't numeric; the variable is coerced to `0` first
    /// if it wasn't already numeric.
    ChangeVariable(String, Value),
    /// Marks a strand as a custom block's body; the `String` is the
    /// `BlockDef::id`. Header-only, like `WhenRan`, but never auto-runs -
    /// only invoked via `CallBlock`/`Value::Call`.
    BlockHeader(String),
    /// Command-position invocation of a `Normal`/`Ending`-shaped custom
    /// block: runs its body inline with `args` bound to its inputs.
    CallBlock {
        block_id: String,
        args: Vec<Value>,
    },
    /// Only meaningful inside a `ReturnsValue`/`ReturnsBool`-shaped block's
    /// body: evaluates `Value` and halts execution, returning the result to
    /// the caller.
    Return(Value),
    /// `if <condition> then { body }` - runs `body` inline (same strand,
    /// same depth) when `condition` evaluates truthy.
    If {
        condition: Value,
        body: Vec<Instruction>,
    },
    /// `if <condition> then { then_body } else { else_body }`.
    IfElse {
        condition: Value,
        then_body: Vec<Instruction>,
        else_body: Vec<Instruction>,
    },
    /// `repeat <count> { body }` - runs `body` `count` times (rounded,
    /// clamped to non-negative).
    Repeat {
        count: Value,
        body: Vec<Instruction>,
    },
    /// `forever { body }` - runs `body` in an unconditional loop; only ends
    /// via `EscapeLoop`, a `Return` inside it, or the run being stopped.
    Forever {
        body: Vec<Instruction>,
    },
    /// `while <condition> { body }` - re-evaluates `condition` before every
    /// iteration, running `body` for as long as it's truthy.
    While {
        condition: Value,
        body: Vec<Instruction>,
    },
    /// Stops the nearest enclosing `Repeat`/`Forever`/`While` immediately.
    /// A no-op if not inside a loop.
    EscapeLoop,
    /// Skips straight to the next iteration of the nearest enclosing
    /// `Repeat`/`Forever`/`While`. A no-op if not inside a loop.
    ContinueLoop,
}

impl std::hash::Hash for InstructionKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Token(t) => {
                0u8.hash(state);
                t.hash(state);
            }
            Self::Wait(d) => {
                1u8.hash(state);
                d.hash(state);
            }
            Self::Command(s) => {
                2u8.hash(state);
                s.hash(state);
            }
            Self::Comment(s) => {
                3u8.hash(state);
                s.hash(state);
            }
            Self::WhenRan => {
                4u8.hash(state);
            }
            Self::SetVariable(n, v) => {
                5u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Self::ChangeVariable(n, v) => {
                6u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Self::BlockHeader(id) => {
                7u8.hash(state);
                id.hash(state);
            }
            Self::CallBlock { block_id, args } => {
                8u8.hash(state);
                block_id.hash(state);
                args.hash(state);
            }
            Self::Return(v) => {
                9u8.hash(state);
                v.hash(state);
            }
            Self::If { condition, body } => {
                10u8.hash(state);
                condition.hash(state);
                body.hash(state);
            }
            Self::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                11u8.hash(state);
                condition.hash(state);
                then_body.hash(state);
                else_body.hash(state);
            }
            Self::Repeat { count, body } => {
                12u8.hash(state);
                count.hash(state);
                body.hash(state);
            }
            Self::Forever { body } => {
                13u8.hash(state);
                body.hash(state);
            }
            Self::While { condition, body } => {
                14u8.hash(state);
                condition.hash(state);
                body.hash(state);
            }
            Self::EscapeLoop => {
                15u8.hash(state);
            }
            Self::ContinueLoop => {
                16u8.hash(state);
            }
            Self::WhenBatteryDischargedTo(v) => {
                17u8.hash(state);
                v.hash(state);
            }
            Self::WhenBatteryChargedTo(v) => {
                18u8.hash(state);
                v.hash(state);
            }
            Self::WhenTime(s) => {
                19u8.hash(state);
                s.hash(state);
            }
            Self::WhenPowerPluggedIn => {
                20u8.hash(state);
            }
            Self::WhenPowerUnplugged => {
                21u8.hash(state);
            }
            Self::OpenApp {
                command,
                name,
                icon,
            } => {
                22u8.hash(state);
                command.hash(state);
                name.hash(state);
                icon.hash(state);
            }
            Self::CloseApp {
                command,
                name,
                icon,
            } => {
                23u8.hash(state);
                command.hash(state);
                name.hash(state);
                icon.hash(state);
            }
        }
    }
}

#[derive(Deserialize)]
enum InstructionKindDe {
    Token(InputToken),
    Wait(WaitDe),
    Command(String),
    Comment(String),
    WhenRan,
    WhenBatteryDischargedTo(Value),
    WhenBatteryChargedTo(Value),
    WhenTime(TimeSchedule),
    WhenPowerPluggedIn,
    WhenPowerUnplugged,
    OpenApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    CloseApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    SetVariable(String, Value),
    ChangeVariable(String, Value),
    BlockHeader(String),
    CallBlock {
        block_id: String,
        args: Vec<Value>,
    },
    Return(Value),
    If {
        condition: Value,
        body: Vec<Instruction>,
    },
    IfElse {
        condition: Value,
        then_body: Vec<Instruction>,
        else_body: Vec<Instruction>,
    },
    Repeat {
        count: Value,
        body: Vec<Instruction>,
    },
    Forever {
        body: Vec<Instruction>,
    },
    While {
        condition: Value,
        body: Vec<Instruction>,
    },
    EscapeLoop,
    ContinueLoop,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WaitDe {
    /// Oldest save shape: a bare duration, no randomness field existed yet.
    LegacyNumber(u64),
    /// Pre-`Op::Random` save shape: `[duration, randomness]`, migrated into
    /// `Op::Random` below so old macros keep the same spread of wait times.
    LegacyWithRandomness(Value, Value),
    /// Current shape: a single `Value` (a plain duration, or a duration
    /// wrapped in any operator including `Op::Random`).
    Current(Value),
}

/// Folds a legacy `[duration, randomness]` `Wait` into today's single-`Value`
/// shape: `duration` alone if randomness was zero, otherwise `duration`
/// wrapped in `Op::Random` spanning `[duration - randomness, duration + randomness]`.
fn migrate_wait_duration(duration: Value, randomness: Value) -> Value {
    if randomness == Value::number(0.0) {
        return duration;
    }
    let zero = || Box::new(Value::number(0.0));
    Value::Op {
        op: Op::Random,
        args: vec![
            Value::Op {
                op: Op::Sub,
                args: vec![duration.clone(), randomness.clone()],
                saved: zero(),
            },
            Value::Op {
                op: Op::Add,
                args: vec![duration, randomness],
                saved: zero(),
            },
        ],
        saved: zero(),
    }
}

impl From<InstructionKindDe> for InstructionKind {
    fn from(de: InstructionKindDe) -> Self {
        match de {
            InstructionKindDe::Token(t) => InstructionKind::Token(t),
            InstructionKindDe::Wait(WaitDe::LegacyNumber(d)) => {
                InstructionKind::Wait(Value::number(d as f64))
            }
            InstructionKindDe::Wait(WaitDe::LegacyWithRandomness(d, r)) => {
                InstructionKind::Wait(migrate_wait_duration(d, r))
            }
            InstructionKindDe::Wait(WaitDe::Current(d)) => InstructionKind::Wait(d),
            InstructionKindDe::Command(s) => InstructionKind::Command(s),
            InstructionKindDe::Comment(s) => InstructionKind::Comment(s),
            InstructionKindDe::WhenRan => InstructionKind::WhenRan,
            InstructionKindDe::WhenBatteryDischargedTo(v) => {
                InstructionKind::WhenBatteryDischargedTo(v)
            }
            InstructionKindDe::WhenBatteryChargedTo(v) => InstructionKind::WhenBatteryChargedTo(v),
            InstructionKindDe::WhenTime(s) => InstructionKind::WhenTime(s),
            InstructionKindDe::WhenPowerPluggedIn => InstructionKind::WhenPowerPluggedIn,
            InstructionKindDe::WhenPowerUnplugged => InstructionKind::WhenPowerUnplugged,
            InstructionKindDe::OpenApp {
                command,
                name,
                icon,
            } => InstructionKind::OpenApp {
                command,
                name,
                icon,
            },
            InstructionKindDe::CloseApp {
                command,
                name,
                icon,
            } => InstructionKind::CloseApp {
                command,
                name,
                icon,
            },
            InstructionKindDe::SetVariable(n, v) => InstructionKind::SetVariable(n, v),
            InstructionKindDe::ChangeVariable(n, v) => InstructionKind::ChangeVariable(n, v),
            InstructionKindDe::BlockHeader(id) => InstructionKind::BlockHeader(id),
            InstructionKindDe::CallBlock { block_id, args } => {
                InstructionKind::CallBlock { block_id, args }
            }
            InstructionKindDe::Return(v) => InstructionKind::Return(v),
            InstructionKindDe::If { condition, body } => InstructionKind::If { condition, body },
            InstructionKindDe::IfElse {
                condition,
                then_body,
                else_body,
            } => InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            },
            InstructionKindDe::Repeat { count, body } => InstructionKind::Repeat { count, body },
            InstructionKindDe::Forever { body } => InstructionKind::Forever { body },
            InstructionKindDe::While { condition, body } => {
                InstructionKind::While { condition, body }
            }
            InstructionKindDe::EscapeLoop => InstructionKind::EscapeLoop,
            InstructionKindDe::ContinueLoop => InstructionKind::ContinueLoop,
        }
    }
}

/// One saved macro: a [`MacroGraph`] canvas plus what only Blockwork cares
/// about - its name, playback speed and recording target. `graph` is
/// flattened on the wire and [`Deref`]ed, so both shapes are unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "MacroDe")]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(flatten)]
    pub graph: MacroGraph,
    /// Strand explicitly chosen to receive freshly-recorded input; `None`
    /// falls back to the "first When Ran strand, else first strand" rule.
    /// Kept out of undo/redo (it's a preference, not an instruction edit).
    #[serde(default)]
    pub recording_target: Option<String>,
    /// Playback speed: every `Wait` duration is divided by this at runtime.
    /// 1.0 is normal, 2.0 is twice as fast. Clamped to [`SPEED_MULTIPLIER_RANGE`].
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f64,
    /// Settings edited from the "Macro Settings" popup - see [`MacroSettings`].
    #[serde(default)]
    pub settings: MacroSettings,
}

impl Deref for Macro {
    type Target = MacroGraph;

    fn deref(&self) -> &MacroGraph {
        &self.graph
    }
}

impl DerefMut for Macro {
    fn deref_mut(&mut self) -> &mut MacroGraph {
        &mut self.graph
    }
}

/// Valid range for both the per-macro and global speed multipliers, enforced
/// wherever either is set from user input.
pub const SPEED_MULTIPLIER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;

/// Per-macro settings edited from the "Macro Settings" popup next to the
/// macro dropdown - not part of the macro's own behavior, but affecting how
/// the app treats it. Persisted and exported/imported with the macro like
/// everything else in `Macro`, so a new field here needs no separate wiring
/// to survive a save/export round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct MacroSettings {
    /// When `true`, this macro's `WhenBattery*`/`WhenTime`/`WhenPower*`
    /// strands are watched by the background watchers (`battery_watch`/
    /// `time_watch` in the desktop app) even while a different macro is
    /// selected. By default only the currently selected macro's event
    /// strands are live.
    #[serde(default)]
    pub always_listen: bool,
}

/// Deserialization shape supporting both the current multi-strand format and
/// the legacy flat `code: Vec<Instruction>` format from older saves; legacy
/// macros become a single root strand. The canvas collections are listed out
/// rather than flattened, so the untagged split stays a field-by-field match.
#[derive(Deserialize)]
#[serde(untagged)]
enum MacroDe {
    Current {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        strands: Vec<Strand>,
        #[serde(default)]
        recording_target: Option<String>,
        #[serde(default = "default_speed_multiplier")]
        speed_multiplier: f64,
        #[serde(default)]
        floating_values: Vec<FloatingValue>,
        #[serde(default)]
        comments: Vec<Comment>,
        #[serde(default)]
        variables: Vec<VariableDef>,
        #[serde(default)]
        block_defs: Vec<BlockDef>,
        #[serde(default)]
        settings: MacroSettings,
    },
    Legacy {
        #[serde(default = "default_macro_id")]
        id: String,
        name: String,
        description: String,
        code: Vec<Instruction>,
    },
}

impl From<MacroDe> for Macro {
    fn from(de: MacroDe) -> Self {
        let mut mac = match de {
            MacroDe::Current {
                id,
                name,
                description,
                mut strands,
                recording_target,
                speed_multiplier,
                floating_values,
                comments,
                variables,
                block_defs,
                settings,
            } => {
                // Pre-"When Ran" saves have a strand id=="root" that was the
                // implicit entry point; give it a real WhenRan on upgrade.
                if let Some(legacy) = strands.iter_mut().find(|s| s.id == LEGACY_ROOT_STRAND_ID)
                    && !legacy.starts_with_header()
                {
                    legacy
                        .instructions
                        .insert(0, Instruction::new(InstructionKind::WhenRan));
                }
                Self {
                    id,
                    name,
                    description,
                    graph: MacroGraph {
                        strands,
                        floating_values,
                        comments,
                        variables,
                        block_defs,
                    },
                    recording_target,
                    speed_multiplier,
                    settings,
                }
            }
            MacroDe::Legacy {
                id,
                name,
                description,
                mut code,
            } => {
                code.insert(0, Instruction::new(InstructionKind::WhenRan));
                Self {
                    id,
                    name,
                    description,
                    graph: MacroGraph {
                        strands: vec![Strand::with_instructions(0, 0, code)],
                        ..MacroGraph::new()
                    },
                    recording_target: None,
                    speed_multiplier: default_speed_multiplier(),
                    settings: MacroSettings::default(),
                }
            }
        };
        // Repairs boolean slots poisoned by the historical `Value::Bool`-less
        // bug (see `Value::migrate_bool_slots`) - a save from before that fix
        // may have a raw number leaf sitting where a blank hexagon belongs.
        mac.graph.migrate_bool_slots();
        // A macro file may have been created before block colors existed (in
        // which case serde supplied blue), or hand-edited/imported with an
        // invalid color. Keep all persisted values safe to place in a CSS
        // custom property before the frontend ever sees them.
        mac.graph.normalize_block_colors();
        mac.migrate_legacy_comments();
        mac
    }
}

impl Macro {
    pub fn new(name: String, description: String, mut code: Vec<Instruction>) -> Self {
        code.insert(0, Instruction::new(InstructionKind::WhenRan));
        Self {
            id: default_macro_id(),
            name,
            description,
            graph: MacroGraph {
                strands: vec![Strand::with_instructions(0, 0, code)],
                ..MacroGraph::new()
            },
            recording_target: None,
            speed_multiplier: default_speed_multiplier(),
            settings: MacroSettings::default(),
        }
    }

    /// Whether any instruction, including nested bodies, moves to an
    /// absolute screen position.
    pub fn has_absolute_mouse_move(&self) -> bool {
        let mut found = false;
        self.graph.walk_instructions(&mut |ins| {
            found |= matches!(
                &ins.kind,
                InstructionKind::Token(InputToken::MoveMouse(
                    _,
                    _,
                    crate::input::types::Coordinate::Abs
                ))
            );
        });
        found
    }

    pub fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
    }

    /// Defines a new custom block, with Blockwork's own header instruction
    /// at the top of its body strand.
    pub fn create_block(
        &mut self,
        pieces: Vec<BlockPiece>,
        shape: BlockShape,
        color: String,
        x: i32,
        y: i32,
    ) -> String {
        self.graph.create_block(pieces, shape, color, x, y, |id| {
            InstructionKind::BlockHeader(id.to_string())
        })
    }

    /// One-time upgrade for saves from before floating/attached comments
    /// existed: pulls every legacy inline `Comment` instruction out of the
    /// instruction stream and re-homes it as a freestanding `Comment` parked
    /// near its old strand. Idempotent - a save with no legacy `Comment`
    /// instructions left is a no-op.
    fn migrate_legacy_comments(&mut self) {
        fn extract(list: &mut Vec<Instruction>, out: &mut Vec<String>) {
            list.retain_mut(|ins| {
                for slot in 0..2u8 {
                    if let Some(body) = ins.body_mut(slot) {
                        extract(body, out);
                    }
                }
                if let InstructionKind::Comment(text) = &ins.kind {
                    out.push(text.clone());
                    false
                } else {
                    true
                }
            });
        }
        for strand in &mut self.graph.strands {
            let mut texts = Vec::new();
            extract(&mut strand.instructions, &mut texts);
            for (i, text) in texts.into_iter().enumerate() {
                self.graph.comments.push(Comment::new(
                    strand.x + 40,
                    strand.y + 40 + i as i32 * 30,
                    text,
                    None,
                ));
            }
        }
    }

    /// Strand that freshly-recorded input gets appended to: the explicit
    /// `recording_target` if it still exists, else the first "When Ran"
    /// strand, else the first strand (creating one if the macro is empty).
    pub fn recording_target_mut(&mut self) -> &mut Strand {
        if let Some(id) = &self.recording_target
            && let Some(pos) = self.graph.strands.iter().position(|s| &s.id == id)
        {
            return &mut self.graph.strands[pos];
        }
        if let Some(pos) = self
            .graph
            .strands
            .iter()
            .position(Strand::starts_with_header)
        {
            return &mut self.graph.strands[pos];
        }
        if self.graph.strands.is_empty() {
            self.graph.strands.push(Strand::new(0, 0));
        }
        &mut self.graph.strands[0]
    }

    /// Read-only counterpart to `recording_target_mut`: same resolution
    /// order, but never creates a strand.
    pub fn recording_target_id(&self) -> Option<String> {
        if let Some(id) = &self.recording_target
            && self.graph.strands.iter().any(|s| &s.id == id)
        {
            return Some(id.clone());
        }
        if let Some(strand) = self.graph.strands.iter().find(|s| s.starts_with_header()) {
            return Some(strand.id.clone());
        }
        self.graph.strands.first().map(|s| s.id.clone())
    }
}

impl std::hash::Hash for Macro {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);
        self.description.hash(state);
        self.graph.hash(state);
        self.recording_target.hash(state);
        self.speed_multiplier.to_bits().hash(state);
        self.settings.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::value::Evaluated;
    use crate::input::types::Coordinate;

    #[test]
    fn block_def_migrates_legacy_returns_value_true_to_returns_value_shape() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"returns_value":true}"#).unwrap();
        assert_eq!(def.shape, BlockShape::ReturnsValue);
    }

    #[test]
    fn block_def_migrates_legacy_returns_value_false_to_normal_shape() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"returns_value":false}"#).unwrap();
        assert_eq!(def.shape, BlockShape::Normal);
    }

    #[test]
    fn block_def_reads_current_shape_field() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"shape":"ReturnsBool"}"#).unwrap();
        assert_eq!(def.shape, BlockShape::ReturnsBool);
    }

    #[test]
    fn block_def_round_trips_shape_through_serialize() {
        let def = BlockDef {
            id: "b1".into(),
            pieces: vec![],
            shape: BlockShape::Ending,
            color: default_block_color(),
        };
        let json = serde_json::to_string(&def).unwrap();
        assert!(
            json.contains(r#""shape":"Ending""#),
            "expected serialized shape field, got: {json}"
        );
        let round_tripped: BlockDef = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped.shape, BlockShape::Ending);
    }

    #[test]
    fn block_def_defaults_color_for_older_macro_files() {
        let def: BlockDef =
            serde_json::from_str(r#"{"id":"b1","pieces":[],"shape":"Normal"}"#).unwrap();
        assert_eq!(def.color, default_block_color());
    }

    #[test]
    fn block_def_color_is_normalized_when_loading_a_macro() {
        let mac: Macro = serde_json::from_str(r##"{"id":"m1","name":"Test","description":"","strands":[],"block_defs":[{"id":"b1","pieces":[],"shape":"Normal","color":"#beef00"}]}"##).unwrap();
        assert_eq!(mac.block_defs[0].color, "#BEEF00");
    }

    #[test]
    fn block_def_color_falls_back_when_loading_an_invalid_color() {
        let mac: Macro = serde_json::from_str(r#"{"id":"m1","name":"Test","description":"","strands":[],"block_defs":[{"id":"b1","pieces":[],"shape":"Normal","color":"not a color"}]}"#).unwrap();
        assert_eq!(mac.block_defs[0].color, default_block_color());
    }

    #[test]
    fn new_macro_defaults_to_one_when_ran_strand() {
        let mac = Macro::new("Test".into(), "".into(), vec![]);
        assert_eq!(mac.strands.len(), 1);
        assert!(mac.strands[0].starts_with_header());
    }

    #[test]
    fn detects_absolute_mouse_moves_in_nested_bodies() {
        let mut mac = Macro::new("Test".into(), "".into(), vec![]);
        mac.strands[0]
            .instructions
            .push(Instruction::new(InstructionKind::If {
                condition: Value::Bool,
                body: vec![Instruction::new(InstructionKind::Token(
                    InputToken::MoveMouse(
                        Value::number(10.0),
                        Value::number(20.0),
                        Coordinate::Abs,
                    ),
                ))],
            }));

        assert!(mac.has_absolute_mouse_move());
    }

    #[test]
    fn legacy_flat_code_migrates_to_when_ran_strand() {
        let json = r#"{"name":"Old","description":"","code":[{"Comment":"hi"}]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(mac.strands.len(), 1);
        // The legacy inline `Comment` instruction is pulled out into a
        // freestanding `Comment` note, not left in the instruction stream.
        assert_eq!(
            mac.strands[0].instructions,
            vec![Instruction::new(InstructionKind::WhenRan)]
        );
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "hi");
        assert_eq!(mac.comments[0].attached_to, None);
    }

    #[test]
    fn legacy_root_strand_gains_when_ran_on_load() {
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"Comment":"hi"}]},
            {"id":"stray","x":10,"y":10,"instructions":[]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let root = mac.strand("root").unwrap();
        assert!(root.starts_with_header());
        assert_eq!(
            root.instructions,
            vec![Instruction::new(InstructionKind::WhenRan)]
        );
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "hi");
        // Untouched, non-entry strand should survive as-is.
        assert!(mac.strand("stray").unwrap().instructions.is_empty());
    }

    #[test]
    fn already_migrated_root_strand_is_not_double_prepended() {
        let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Comment":"hi"}]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(
            mac.strand("root").unwrap().instructions,
            vec![Instruction::new(InstructionKind::WhenRan)]
        );
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "hi");
    }

    #[test]
    fn migrate_legacy_comments_reaches_into_nested_if_body() {
        let json = r#"{"id":"m1","name":"New","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{"kind":"Bool"},"body":[{"Comment":"nested"}]}}
            ]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let root = mac.strand("root").unwrap();
        match &root.instructions[1].kind {
            InstructionKind::If { body, .. } => assert!(
                body.is_empty(),
                "nested Comment should be extracted, not left in the If body"
            ),
            other => panic!("expected If, got {other:?}"),
        }
        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].text, "nested");
        assert_eq!(mac.comments[0].attached_to, None);
    }

    #[test]
    fn prune_orphaned_comments_drops_comment_attached_to_removed_instruction() {
        let wait = Instruction::new(InstructionKind::Wait(Value::number(1000.0)));
        let wait_id = wait.id.clone();
        let mut mac = Macro::new("Test".into(), "".into(), vec![wait]);
        mac.comments.push(Comment {
            id: "c1".into(),
            x: 0,
            y: 0,
            text: "hi".into(),
            collapsed: false,
            attached_to: Some(wait_id),
        });
        mac.comments.push(Comment {
            id: "c2".into(),
            x: 0,
            y: 0,
            text: "freestanding".into(),
            collapsed: false,
            attached_to: None,
        });

        // Remove the Wait instruction (index 1 - index 0 is the WhenRan header).
        mac.strands[0].instructions.remove(1);
        mac.prune_orphaned_comments();

        assert_eq!(mac.comments.len(), 1);
        assert_eq!(mac.comments[0].id, "c2");
    }

    #[test]
    fn prune_orphaned_comments_cascades_into_nested_wrap_body() {
        let inner = Instruction::new(InstructionKind::Wait(Value::number(1.0)));
        let inner_id = inner.id.clone();
        let if_ins = Instruction::new(InstructionKind::If {
            condition: Value::Bool,
            body: vec![inner],
        });
        let mut mac = Macro::new("Test".into(), "".into(), vec![if_ins]);
        mac.comments.push(Comment {
            id: "c1".into(),
            x: 0,
            y: 0,
            text: "nested".into(),
            collapsed: false,
            attached_to: Some(inner_id),
        });

        // Deleting the whole If block (index 1) takes its nested body with it.
        mac.strands[0].instructions.remove(1);
        mac.prune_orphaned_comments();

        assert!(mac.comments.is_empty());
    }

    #[test]
    fn prune_orphaned_comments_keeps_comment_attached_to_surviving_instruction() {
        let wait = Instruction::new(InstructionKind::Wait(Value::number(1000.0)));
        let wait_id = wait.id.clone();
        let mut mac = Macro::new("Test".into(), "".into(), vec![wait]);
        mac.comments.push(Comment {
            id: "c1".into(),
            x: 0,
            y: 0,
            text: "hi".into(),
            collapsed: false,
            attached_to: Some(wait_id),
        });

        mac.prune_orphaned_comments();

        assert_eq!(mac.comments.len(), 1);
    }

    #[test]
    fn migrate_bool_slots_repairs_poisoned_if_condition_on_load() {
        // Pre-`Value::Bool` save: dragging the default boolean block out of
        // an `If`'s condition once left a bare `Number` behind.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{"kind":"Number","value":0.0},"body":[]}}
            ]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        match &mac.strand("root").unwrap().instructions[1].kind {
            InstructionKind::If { condition, .. } => assert_eq!(condition, &Value::Bool),
            other => panic!("expected If, got {other:?}"),
        }
    }

    #[test]
    fn migrate_bool_slots_repairs_poisoned_operand_nested_inside_condition() {
        // The poisoned `Number` can be arbitrarily deep - here inside an
        // `And` that itself is the `If`'s condition. Its sibling (a real
        // comparison) must survive untouched.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",
                {"If":{"condition":{
                    "kind":"Op","op":"And",
                    "args":[
                        {"kind":"Number","value":0.0},
                        {"kind":"Op","op":"Eq","args":[{"kind":"Number","value":1.0},{"kind":"Number","value":1.0}],"saved":{"kind":"Number","value":0.0}}
                    ],
                    "saved":{"kind":"Number","value":0.0}
                },"body":[]}}
            ]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        match &mac.strand("root").unwrap().instructions[1].kind {
            InstructionKind::If {
                condition: Value::Op {
                    op: Op::And, args, ..
                },
                ..
            } => {
                assert_eq!(args[0], Value::Bool);
                assert_eq!(
                    args[1],
                    Value::Op {
                        op: Op::Eq,
                        args: vec![Value::number(1.0), Value::number(1.0)],
                        saved: Box::new(Value::Bool)
                    }
                );
            }
            other => panic!("expected If(And(..)), got {other:?}"),
        }
    }

    #[test]
    fn migrate_bool_slots_repairs_custom_block_boolean_arguments() {
        // The Boolean type of a custom-block argument lives in its
        // definition, not at the call site. A prior drag-out bug saved a zero
        // here, so loading must recover the blank hexagon from that type.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":[{"CallBlock":{
                "block_id":"b1","args":[{"kind":"Number","value":0.0}]
            }}]}
        ],"block_defs":[{"id":"b1","pieces":[
            {"kind":"Input","id":"i1","name":"flag","value_type":"Bool"}
        ],"shape":"Normal"}]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        let args = mac.strands[0]
            .instructions
            .iter()
            .find_map(|instruction| match &instruction.kind {
                InstructionKind::CallBlock { args, .. } => Some(args),
                _ => None,
            })
            .expect("expected CallBlock");
        assert_eq!(args, &vec![Value::Bool]);
    }

    #[test]
    fn migrate_bool_slots_leaves_legitimately_numeric_fields_alone() {
        // A `Wait` duration is never boolean-typed - a `Number` there is
        // always legitimate and must not be touched.
        let json = r#"{"id":"m1","name":"Old","description":"","strands":[
            {"id":"root","x":0,"y":0,"instructions":["WhenRan",{"Wait":{"kind":"Number","value":0.0}}]}
        ]}"#;
        let mac: Macro = serde_json::from_str(json).unwrap();
        assert_eq!(
            mac.strand("root").unwrap().instructions[1],
            Instruction::new(InstructionKind::Wait(Value::number(0.0)))
        );
    }

    #[test]
    fn legacy_wait_with_randomness_migrates_to_random_op() {
        let json = r#"{"Wait":[1000.0,50.0]}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Wait(Value::Op {
                op: Op::Random,
                args: vec![
                    Value::Op {
                        op: Op::Sub,
                        args: vec![Value::number(1000.0), Value::number(50.0)],
                        saved: Box::new(Value::number(0.0)),
                    },
                    Value::Op {
                        op: Op::Add,
                        args: vec![Value::number(1000.0), Value::number(50.0)],
                        saved: Box::new(Value::number(0.0)),
                    },
                ],
                saved: Box::new(Value::number(0.0)),
            }))
        );
    }

    #[test]
    fn legacy_wait_with_zero_randomness_migrates_to_plain_duration() {
        let json = r#"{"Wait":[1000.0,0.0]}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Wait(Value::number(1000.0)))
        );
    }

    #[test]
    fn legacy_single_arg_wait_migrates_to_value() {
        let json = r#"{"Wait":1000}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Wait(Value::number(1000.0)))
        );
    }

    #[test]
    fn legacy_bare_number_move_mouse_fields_migrate_to_value() {
        let json = r#"{"Token":{"MoveMouse":[5,10,"Rel"]}}"#;
        let ins: Instruction = serde_json::from_str(json).unwrap();
        assert_eq!(
            ins,
            Instruction::new(InstructionKind::Token(InputToken::MoveMouse(
                Value::number(5.0),
                Value::number(10.0),
                Coordinate::Rel
            ))),
        );
    }

    #[test]
    fn rename_variable_renames_declaration_and_every_reference() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![
                Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::Var {
                        name: "x".to_string(),
                    },
                )),
                Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                    name: "x".to_string(),
                }))),
            ],
        );
        mac.variables.push(VariableDef {
            name: "x".to_string(),
            value: Evaluated::Number(0.0),
        });
        mac.floating_values.push(FloatingValue {
            id: "f1".into(),
            x: 0,
            y: 0,
            value: Value::Var {
                name: "x".to_string(),
            },
            origin_block_id: None,
        });

        mac.rename_variable("x", "y").unwrap();

        assert_eq!(mac.variables[0].name, "y");
        let strand = &mac.strands[0];
        assert_eq!(
            strand.instructions[1],
            Instruction::new(InstructionKind::SetVariable(
                "y".to_string(),
                Value::number(1.0)
            ))
        );
        assert_eq!(
            strand.instructions[2],
            Instruction::new(InstructionKind::ChangeVariable(
                "y".to_string(),
                Value::Var {
                    name: "y".to_string()
                }
            ))
        );
        assert_eq!(
            strand.instructions[3],
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "y".to_string()
            })))
        );
        assert_eq!(
            mac.floating_values[0].value,
            Value::Var {
                name: "y".to_string()
            }
        );
    }

    #[test]
    fn rename_variable_rejects_an_undeclared_name_and_changes_nothing() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![Instruction::new(InstructionKind::Token(InputToken::Text(
                Value::Var {
                    name: "x".to_string(),
                },
            )))],
        );
        assert!(mac.rename_variable("x", "y").is_err());
        assert_eq!(
            mac.strands[0].instructions[1],
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "x".to_string()
            })))
        );
    }

    #[test]
    fn rename_var_reaches_into_if_body_and_condition() {
        let mut ins = Instruction::new(InstructionKind::If {
            condition: Value::Var {
                name: "x".to_string(),
            },
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Var {
                    name: "x".to_string(),
                },
            ))],
        });
        ins.rename_var("x", "y");
        match &ins.kind {
            InstructionKind::If { condition, body } => {
                assert_eq!(
                    *condition,
                    Value::Var {
                        name: "y".to_string()
                    }
                );
                assert_eq!(
                    body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "y".to_string(),
                        Value::Var {
                            name: "y".to_string()
                        }
                    ))
                );
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn rename_var_reaches_into_if_else_both_branches() {
        let mut ins = Instruction::new(InstructionKind::IfElse {
            condition: Value::Var {
                name: "x".to_string(),
            },
            then_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
            else_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(2.0),
            ))],
        });
        ins.rename_var("x", "y");
        match &ins.kind {
            InstructionKind::IfElse {
                then_body,
                else_body,
                ..
            } => {
                assert_eq!(
                    then_body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "y".to_string(),
                        Value::number(1.0)
                    ))
                );
                assert_eq!(
                    else_body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "y".to_string(),
                        Value::number(2.0)
                    ))
                );
            }
            _ => panic!("expected IfElse"),
        }
    }

    #[test]
    fn scrub_block_calls_reaches_into_nested_if_body() {
        let mut ins = Instruction::new(InstructionKind::If {
            condition: Value::number(1.0),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Call {
                    block_id: "gone".to_string(),
                    args: vec![],
                    saved: Box::new(Value::number(0.0)),
                },
            ))],
        });
        ins.scrub_block_calls("gone");
        match &ins.kind {
            InstructionKind::If { body, .. } => {
                assert_eq!(
                    body[0],
                    Instruction::new(InstructionKind::SetVariable(
                        "x".to_string(),
                        Value::number(0.0)
                    ))
                );
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn body_mut_addresses_if_and_if_else_slots() {
        let mut if_ins = InstructionKind::If {
            condition: Value::number(1.0),
            body: vec![Instruction::new(InstructionKind::Comment("a".into()))],
        };
        assert_eq!(
            if_ins.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "a".into()
            ))])
        );
        assert_eq!(if_ins.body_mut(1), None);

        let mut if_else = InstructionKind::IfElse {
            condition: Value::number(1.0),
            then_body: vec![Instruction::new(InstructionKind::Comment("then".into()))],
            else_body: vec![Instruction::new(InstructionKind::Comment("else".into()))],
        };
        assert_eq!(
            if_else.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "then".into()
            ))])
        );
        assert_eq!(
            if_else.body_mut(1),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "else".into()
            ))])
        );
        assert_eq!(if_else.body_mut(2), None);
    }

    #[test]
    fn body_mut_addresses_loop_slots() {
        let mut repeat = InstructionKind::Repeat {
            count: Value::number(3.0),
            body: vec![Instruction::new(InstructionKind::Comment("a".into()))],
        };
        assert_eq!(
            repeat.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "a".into()
            ))])
        );
        assert_eq!(repeat.body_mut(1), None);

        let mut forever = InstructionKind::Forever {
            body: vec![Instruction::new(InstructionKind::Comment("b".into()))],
        };
        assert_eq!(
            forever.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "b".into()
            ))])
        );

        let mut while_ins = InstructionKind::While {
            condition: Value::Bool,
            body: vec![Instruction::new(InstructionKind::Comment("c".into()))],
        };
        assert_eq!(
            while_ins.body_mut(0),
            Some(&mut vec![Instruction::new(InstructionKind::Comment(
                "c".into()
            ))])
        );

        assert_eq!(InstructionKind::EscapeLoop.body_mut(0), None);
        assert_eq!(InstructionKind::ContinueLoop.body_mut(0), None);
    }
}

