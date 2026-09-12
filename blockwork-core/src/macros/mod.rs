use crate::input::schedule::TimeSchedule;
use crate::input::types::InputToken;
use crate::input::value::{Evaluated, Op, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod backend;
pub mod loop_control;
pub mod priority;
pub mod run_registry;
pub mod runner;
pub mod thread_pool;

fn default_macro_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_strand_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_speed_multiplier() -> f64 {
    1.0
}

/// Id of the single implicit strand used before "When Ran" blocks existed.
/// Kept only so loading an old save file can find and migrate that strand.
const LEGACY_ROOT_STRAND_ID: &str = "root";

/// One draggable stack of instructions on the canvas. It's an entry point —
/// one of the possibly-many things a macro runs concurrently — when its
/// first instruction is `InstructionKind::WhenRan`; otherwise it stays persisted
/// but inert until dragged under a "When Ran" block.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct Strand {
    #[serde(default = "default_strand_id")]
    pub id: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub instructions: Vec<Instruction>,
}

/// A value block sitting free on the canvas, not embedded in any
/// instruction's field — the drag-and-drop "parking spot" for a value block
/// before/after it's placed into a field's slot.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct FloatingValue {
    #[serde(default = "default_floating_value_id")]
    pub id: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    pub value: Value,
    /// Which custom block's own header this value was dragged out of, if
    /// any — set only when the value came from a `Value::Param` reporter
    /// (see `commands::create_floating_value`), since that's the one value
    /// kind meaningless outside its declaring block. A floating value with
    /// no such origin (a plain number/operator/variable, or an older save
    /// from before this field existed — hence `#[serde(default)]`) is just
    /// `None`. Lets the frontend render a floating `Param` reporter with
    /// its real declared shape (e.g. a boolean hexagon) instead of guessing
    /// from name alone.
    #[serde(default)]
    pub origin_block_id: Option<String>,
}

fn default_floating_value_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// A floating, collapsible note on the canvas — freestanding (`attached_to:
/// None`, `x`/`y` an absolute canvas position, same convention as
/// `FloatingValue`) or pinned to an instruction (`attached_to: Some(id)`,
/// `x`/`y` an *offset* from that instruction's currently-rendered position
/// instead — the desktop app has no idea where a given instruction renders
/// on screen, that's purely a frontend DOM-measurement fact, so an attached
/// note's absolute position is computed frontend-side each render as anchor
/// row position + this offset, never stored as an absolute coordinate here).
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct Comment {
    #[serde(default = "default_comment_id")]
    pub id: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub attached_to: Option<String>,
}

fn default_comment_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn default_variable_value() -> Evaluated {
    Evaluated::Number(0.0)
}

/// A user-declared macro-wide variable and its current value, mutated by
/// `SetVariable`/`ChangeVariable` at runtime and persisted with the macro so
/// it survives an app restart.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct VariableDef {
    pub name: String,
    #[serde(default = "default_variable_value")]
    pub value: Evaluated,
}

/// What kind of value an input slot expects — drives the blank default a
/// fresh call site's argument gets (`Value::number(0.0)` vs `Value::Bool`,
/// see `reconcile_block_call_args`) and, transitively, whether that slot
/// renders as the ordinary rounded capsule or a boolean hexagon (purely a
/// function of the `Value` actually sitting there, same as every built-in
/// boolean slot — see `blockstitch`'s `ValueBlock.vue`'s `isBool`). `Any`
/// (number-or-text, free-typed) is the long-standing default; `#[serde(default)]`
/// on `BlockPiece::Input::value_type` lets an older save missing this field
/// deserialize as `Any` instead of failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum InputValueType {
    #[default]
    Any,
    Bool,
}

/// One piece of a custom block's prototype, in declaration order — either
/// static label text or a named input slot (read in the body via
/// `Value::Param`). `id` is a stable identifier assigned once and never
/// regenerated, since `name` changes on rename and can't serve as identity
/// when reconciling call sites in `reconcile_block_call_args`.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BlockPiece {
    Label {
        id: String,
        text: String,
    },
    Input {
        id: String,
        name: String,
        #[serde(default)]
        value_type: InputValueType,
    },
}

impl BlockPiece {
    fn id(&self) -> &str {
        match self {
            BlockPiece::Label { id, .. } | BlockPiece::Input { id, .. } => id,
        }
    }
}

/// What a custom block's own call site looks like: a plain stackable
/// instruction (`Normal`), a stackable instruction with no bottom notch so
/// nothing can be placed below it (`Ending` — same shape family as the
/// built-in `Return`/`EscapeLoop`/`ContinueLoop`), or a value-position
/// reporter returning either a number-or-text (`ReturnsValue`, an oval,
/// today's long-standing `returns_value: true`) or a boolean (`ReturnsBool`,
/// a hexagon — see `BlockPiece`'s `InputValueType` for the same oval/hexagon
/// split on an *input*). `Normal`/`Ending` and `ReturnsValue`/`ReturnsBool`
/// are each a mutually-exclusive pair in the "Make a Block" UI (a "returns a
/// value" checkbox swaps which pair the two shape buttons offer) — there's
/// no such thing as an `Ending` reporter or a `Normal` block that also
/// returns a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
pub enum BlockShape {
    #[default]
    Normal,
    Ending,
    ReturnsValue,
    ReturnsBool,
}

impl BlockShape {
    /// True for either reporter shape — gates whether `Value::Call` nodes
    /// referencing this block are meaningful and whether a `Return` inside
    /// its body is valid placement (see `commands::check_return_placement`).
    /// Boolean-vs-number/text is a pure rendering concern (which shape the
    /// reporter draws as, and what a fresh call-site arg defaults to) with
    /// no effect on execution — `Value::eval`/`Evaluated` are already
    /// dynamically typed regardless of which reporter shape produced them.
    pub fn returns_value(self) -> bool {
        matches!(self, BlockShape::ReturnsValue | BlockShape::ReturnsBool)
    }

    /// True only for the no-bottom-notch command shape.
    pub fn is_ending(self) -> bool {
        matches!(self, BlockShape::Ending)
    }
}

/// Old saves only ever had a `returns_value: bool` field; this decodes
/// either that (`true` migrating to `ReturnsValue`, `false` to `Normal`) or
/// the current `shape: "Normal" | "Ending" | "ReturnsValue" | "ReturnsBool"`
/// tag, whichever `BlockDef`'s `#[serde(alias = "returns_value")]` field
/// actually finds on disk. Mirrors `Value`'s own legacy-tolerant
/// `Deserialize` impl further down this crate (`input/value.rs`).
impl<'de> Deserialize<'de> for BlockShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ShapeDe {
            Legacy(bool),
            Current(String),
        }
        Ok(match ShapeDe::deserialize(deserializer)? {
            ShapeDe::Legacy(true) => BlockShape::ReturnsValue,
            ShapeDe::Legacy(false) => BlockShape::Normal,
            ShapeDe::Current(s) => match s.as_str() {
                "Normal" => BlockShape::Normal,
                "Ending" => BlockShape::Ending,
                "ReturnsValue" => BlockShape::ReturnsValue,
                "ReturnsBool" => BlockShape::ReturnsBool,
                other => {
                    return Err(serde::de::Error::unknown_variant(
                        other,
                        &["Normal", "Ending", "ReturnsValue", "ReturnsBool"],
                    ));
                }
            },
        })
    }
}

/// A user-defined custom block ("My Blocks") — just the prototype/signature;
/// its body lives in a separate `Strand` whose `instructions[0]` is
/// `InstructionKind::BlockHeader(id)`.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct BlockDef {
    pub id: String,
    pub pieces: Vec<BlockPiece>,
    #[serde(alias = "returns_value")]
    pub shape: BlockShape,
    /// User-selected accent for the block's icon and hover outline. The
    /// default preserves the established blue treatment for older macros.
    #[serde(default = "default_block_color")]
    pub color: String,
}

/// The legacy/default custom-block accent. Kept as a function so serde can
/// supply it when loading macros saved before custom colors existed.
pub fn default_block_color() -> String {
    "#4C97FF".to_string()
}

/// Canonicalizes the one color format Blockwork persists and exposes to CSS.
/// `None` means the supplied value cannot safely be used as a block accent.
pub fn normalize_block_color(color: &str) -> Option<String> {
    let color = color.trim();
    (color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    .then(|| color.to_ascii_uppercase())
}

impl BlockDef {
    /// Declared input names, in prototype order — the positional key `Call`/
    /// `CallBlock`'s `args` line up against.
    pub fn input_names(&self) -> impl Iterator<Item = &str> {
        self.pieces.iter().filter_map(|p| match p {
            BlockPiece::Input { name, .. } => Some(name.as_str()),
            BlockPiece::Label { .. } => None,
        })
    }
}

fn default_block_id() -> String {
    Uuid::new_v4().simple().to_string()
}

impl InstructionKind {
    /// True for "header" blocks (`WhenRan`, `BlockHeader`, and the
    /// `WhenBattery*To` entry points) — must be first in their strand,
    /// nothing may stack above them, and they render with a flat top edge.
    pub fn is_header(&self) -> bool {
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

    /// Renames `Value::Var` reads, plus a `SetVariable`/`ChangeVariable`
    /// instruction's own target name.
    pub fn rename_var(&mut self, old: &str, new: &str) {
        match self {
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value) => value.rename_var(old, new),
            InstructionKind::Token(token) => token.rename_var(old, new),
            InstructionKind::SetVariable(name, value)
            | InstructionKind::ChangeVariable(name, value) => {
                if name == old {
                    *name = new.to_string();
                }
                value.rename_var(old, new);
            }
            InstructionKind::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.rename_var(old, new);
                }
            }
            InstructionKind::If { condition, body } => {
                condition.rename_var(old, new);
                for ins in body.iter_mut() {
                    ins.rename_var(old, new);
                }
            }
            InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                condition.rename_var(old, new);
                for ins in then_body.iter_mut().chain(else_body.iter_mut()) {
                    ins.rename_var(old, new);
                }
            }
            InstructionKind::Repeat { count, body } => {
                count.rename_var(old, new);
                for ins in body.iter_mut() {
                    ins.rename_var(old, new);
                }
            }
            InstructionKind::Forever { body } => {
                for ins in body.iter_mut() {
                    ins.rename_var(old, new);
                }
            }
            InstructionKind::While { condition, body } => {
                condition.rename_var(old, new);
                for ins in body.iter_mut() {
                    ins.rename_var(old, new);
                }
            }
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::WhenTime(_)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => {}
        }
    }

    /// Repairs boolean slots poisoned by the historical `Value::Bool`-less
    /// bug (see `Value::migrate_bool_slots`) — run once over every
    /// instruction when a macro loads (`From<MacroDe>`). An `If`/`IfElse`
    /// condition is the one position this module knows is boolean-typed by
    /// construction; everything else starts `false` and lets `Value::migrate_bool_slots`
    /// find any `And`/`Or`/`Not` operands nested further in on its own.
    pub fn migrate_bool_slots(&mut self) {
        match self {
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value) => value.migrate_bool_slots(false),
            InstructionKind::Token(token) => token.migrate_bool_slots(),
            InstructionKind::SetVariable(_, value) | InstructionKind::ChangeVariable(_, value) => {
                value.migrate_bool_slots(false)
            }
            InstructionKind::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.migrate_bool_slots(false);
                }
            }
            InstructionKind::If { condition, body } => {
                condition.migrate_bool_slots(true);
                for ins in body.iter_mut() {
                    ins.migrate_bool_slots();
                }
            }
            InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                condition.migrate_bool_slots(true);
                for ins in then_body.iter_mut().chain(else_body.iter_mut()) {
                    ins.migrate_bool_slots();
                }
            }
            InstructionKind::Repeat { count, body } => {
                count.migrate_bool_slots(false);
                for ins in body.iter_mut() {
                    ins.migrate_bool_slots();
                }
            }
            InstructionKind::Forever { body } => {
                for ins in body.iter_mut() {
                    ins.migrate_bool_slots();
                }
            }
            InstructionKind::While { condition, body } => {
                condition.migrate_bool_slots(true);
                for ins in body.iter_mut() {
                    ins.migrate_bool_slots();
                }
            }
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::WhenTime(_)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => {}
        }
    }

    /// Renames every `Value::Param` leaf reading `old` to `new`, keeping a
    /// block's body working after one of its inputs is renamed.
    pub fn rename_param(&mut self, old: &str, new: &str) {
        match self {
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value) => value.rename_param(old, new),
            InstructionKind::Token(token) => token.rename_param(old, new),
            InstructionKind::SetVariable(_, value) | InstructionKind::ChangeVariable(_, value) => {
                value.rename_param(old, new)
            }
            InstructionKind::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.rename_param(old, new);
                }
            }
            InstructionKind::If { condition, body } => {
                condition.rename_param(old, new);
                for ins in body.iter_mut() {
                    ins.rename_param(old, new);
                }
            }
            InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                condition.rename_param(old, new);
                for ins in then_body.iter_mut().chain(else_body.iter_mut()) {
                    ins.rename_param(old, new);
                }
            }
            InstructionKind::Repeat { count, body } => {
                count.rename_param(old, new);
                for ins in body.iter_mut() {
                    ins.rename_param(old, new);
                }
            }
            InstructionKind::Forever { body } => {
                for ins in body.iter_mut() {
                    ins.rename_param(old, new);
                }
            }
            InstructionKind::While { condition, body } => {
                condition.rename_param(old, new);
                for ins in body.iter_mut() {
                    ins.rename_param(old, new);
                }
            }
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::WhenTime(_)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => {}
        }
    }

    /// Applies `f` to the `args` of every `CallBlock`/`Value::Call` node
    /// referencing `block_id`, wherever nested. Used to keep call sites'
    /// argument lists aligned after a block's inputs change.
    pub fn for_each_call_args_mut(&mut self, block_id: &str, f: &mut dyn FnMut(&mut Vec<Value>)) {
        match self {
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value) => {
                value.for_each_call_args_mut(block_id, f)
            }
            InstructionKind::Token(token) => token.for_each_call_args_mut(block_id, f),
            InstructionKind::SetVariable(_, value) | InstructionKind::ChangeVariable(_, value) => {
                value.for_each_call_args_mut(block_id, f)
            }
            InstructionKind::CallBlock { block_id: id, args } => {
                if id == block_id {
                    f(args);
                }
                for a in args.iter_mut() {
                    a.for_each_call_args_mut(block_id, f);
                }
            }
            InstructionKind::If { condition, body } => {
                condition.for_each_call_args_mut(block_id, f);
                for ins in body.iter_mut() {
                    ins.for_each_call_args_mut(block_id, f);
                }
            }
            InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                condition.for_each_call_args_mut(block_id, f);
                for ins in then_body.iter_mut().chain(else_body.iter_mut()) {
                    ins.for_each_call_args_mut(block_id, f);
                }
            }
            InstructionKind::Repeat { count, body } => {
                count.for_each_call_args_mut(block_id, f);
                for ins in body.iter_mut() {
                    ins.for_each_call_args_mut(block_id, f);
                }
            }
            InstructionKind::Forever { body } => {
                for ins in body.iter_mut() {
                    ins.for_each_call_args_mut(block_id, f);
                }
            }
            InstructionKind::While { condition, body } => {
                condition.for_each_call_args_mut(block_id, f);
                for ins in body.iter_mut() {
                    ins.for_each_call_args_mut(block_id, f);
                }
            }
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::WhenTime(_)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => {}
        }
    }

    /// Replaces every `Value::Call` node referencing `block_id` with a plain
    /// `0` leaf (a `CallBlock` referencing it is left for the caller to drop
    /// entirely), so deleting a custom block never leaves a dangling ref.
    pub fn scrub_block_calls(&mut self, block_id: &str) {
        match self {
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value) => value.scrub_block_calls(block_id),
            InstructionKind::Token(token) => token.scrub_block_calls(block_id),
            InstructionKind::SetVariable(_, value) | InstructionKind::ChangeVariable(_, value) => {
                value.scrub_block_calls(block_id)
            }
            InstructionKind::CallBlock { args, .. } => {
                for a in args.iter_mut() {
                    a.scrub_block_calls(block_id);
                }
            }
            InstructionKind::If { condition, body } => {
                condition.scrub_block_calls(block_id);
                for ins in body.iter_mut() {
                    ins.scrub_block_calls(block_id);
                }
            }
            InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                condition.scrub_block_calls(block_id);
                for ins in then_body.iter_mut().chain(else_body.iter_mut()) {
                    ins.scrub_block_calls(block_id);
                }
            }
            InstructionKind::Repeat { count, body } => {
                count.scrub_block_calls(block_id);
                for ins in body.iter_mut() {
                    ins.scrub_block_calls(block_id);
                }
            }
            InstructionKind::Forever { body } => {
                for ins in body.iter_mut() {
                    ins.scrub_block_calls(block_id);
                }
            }
            InstructionKind::While { condition, body } => {
                condition.scrub_block_calls(block_id);
                for ins in body.iter_mut() {
                    ins.scrub_block_calls(block_id);
                }
            }
            InstructionKind::Command(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::WhenTime(_)
            | InstructionKind::WhenPowerPluggedIn
            | InstructionKind::WhenPowerUnplugged
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => {}
        }
    }

    /// Read-only counterpart to `body_mut`.
    pub fn body(&self, slot: u8) -> Option<&Vec<Instruction>> {
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

    /// The nested instruction list for compound-instruction `slot` — `If`'s
    /// single body (`slot == 0`), or `IfElse`'s `then_body`/`else_body`
    /// (`slot == 0`/`1`); `Repeat`/`Forever`/`While` each have a single body
    /// at `slot == 0`, same as `If`. `None` for anything else (including an
    /// out-of-range slot). The one primitive nested-instruction addressing
    /// builds on.
    pub fn body_mut(&mut self, slot: u8) -> Option<&mut Vec<Instruction>> {
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
}

impl Strand {
    pub fn starts_with_when_ran(&self) -> bool {
        self.instructions
            .first()
            .map_or(false, Instruction::is_header)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "MacroDe")]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub description: String,
    pub strands: Vec<Strand>,
    /// Strand explicitly chosen to receive freshly-recorded input; `None`
    /// falls back to the "first When Ran strand, else first strand" rule.
    /// Kept out of undo/redo (it's a preference, not an instruction edit).
    #[serde(default)]
    pub recording_target: Option<String>,
    /// Playback speed: every `Wait` duration is divided by this at runtime.
    /// 1.0 is normal, 2.0 is twice as fast. Clamped to `SPEED_MULTIPLIER_RANGE`.
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f64,
    /// Value blocks parked on open canvas — see `FloatingValue`.
    #[serde(default)]
    pub floating_values: Vec<FloatingValue>,
    /// Floating/attached notes — see `Comment`.
    #[serde(default)]
    pub comments: Vec<Comment>,
    /// User-declared macro-wide variables — see `VariableDef`.
    #[serde(default)]
    pub variables: Vec<VariableDef>,
    /// User-defined custom blocks ("My Blocks") — see `BlockDef`. Each
    /// def's body lives in its own header strand within `strands`.
    #[serde(default)]
    pub block_defs: Vec<BlockDef>,
    /// Settings edited from the "Macro Settings" popup — see `MacroSettings`.
    #[serde(default)]
    pub settings: MacroSettings,
}

/// Valid range for both the per-macro and global speed multipliers, enforced
/// wherever either is set from user input.
pub const SPEED_MULTIPLIER_RANGE: std::ops::RangeInclusive<f64> = 0.1..=10.0;

/// Per-macro settings edited from the "Macro Settings" popup next to the
/// macro dropdown — not part of the macro's own behavior, but affecting how
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
/// macros become a single root strand.
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
                if let Some(legacy) = strands.iter_mut().find(|s| s.id == LEGACY_ROOT_STRAND_ID) {
                    if !legacy.starts_with_when_ran() {
                        legacy
                            .instructions
                            .insert(0, Instruction::new(InstructionKind::WhenRan));
                    }
                }
                Self {
                    id,
                    name,
                    description,
                    strands,
                    recording_target,
                    speed_multiplier,
                    floating_values,
                    comments,
                    variables,
                    block_defs,
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
                let strand = Strand {
                    id: default_strand_id(),
                    x: 0,
                    y: 0,
                    instructions: code,
                };
                Self {
                    id,
                    name,
                    description,
                    strands: vec![strand],
                    recording_target: None,
                    speed_multiplier: default_speed_multiplier(),
                    floating_values: Vec::new(),
                    comments: Vec::new(),
                    variables: Vec::new(),
                    block_defs: Vec::new(),
                    settings: MacroSettings::default(),
                }
            }
        };
        // Repairs boolean slots poisoned by the historical `Value::Bool`-less
        // bug (see `Value::migrate_bool_slots`) — a save from before that fix
        // may have a raw number leaf sitting where a blank hexagon belongs.
        for strand in mac.strands.iter_mut() {
            for ins in strand.instructions.iter_mut() {
                ins.migrate_bool_slots();
            }
        }
        for fv in mac.floating_values.iter_mut() {
            fv.value.migrate_bool_slots(false);
        }
        // A macro file may have been created before this field existed (in
        // which case serde supplied blue), or hand-edited/imported with an
        // invalid color. Keep all persisted values safe to place in a CSS
        // custom property before the frontend ever sees them.
        for def in mac.block_defs.iter_mut() {
            def.color = normalize_block_color(&def.color).unwrap_or_else(default_block_color);
        }
        mac.migrate_legacy_comments();
        mac
    }
}

impl Macro {
    pub fn new(name: String, description: String, mut code: Vec<Instruction>) -> Self {
        code.insert(0, Instruction::new(InstructionKind::WhenRan));
        let strand = Strand {
            id: default_strand_id(),
            x: 0,
            y: 0,
            instructions: code,
        };
        Self {
            id: default_macro_id(),
            name,
            description,
            strands: vec![strand],
            recording_target: None,
            speed_multiplier: default_speed_multiplier(),
            floating_values: Vec::new(),
            comments: Vec::new(),
            variables: Vec::new(),
            block_defs: Vec::new(),
            settings: MacroSettings::default(),
        }
    }

    pub fn ensure_id(&mut self) {
        if self.id.trim().is_empty() {
            self.id = default_macro_id();
        }
    }

    pub fn strand(&self, id: &str) -> Option<&Strand> {
        self.strands.iter().find(|s| s.id == id)
    }

    pub fn strand_mut(&mut self, id: &str) -> Option<&mut Strand> {
        self.strands.iter_mut().find(|s| s.id == id)
    }

    pub fn floating_value_mut(&mut self, id: &str) -> Option<&mut FloatingValue> {
        self.floating_values.iter_mut().find(|f| f.id == id)
    }

    pub fn comment_mut(&mut self, id: &str) -> Option<&mut Comment> {
        self.comments.iter_mut().find(|c| c.id == id)
    }

    /// Every instruction id currently reachable from any strand, including
    /// nested bodies (If/IfElse/Repeat/Forever/While) — the "still alive" set
    /// `prune_orphaned_comments` checks attachments against.
    fn all_instruction_ids(&self) -> std::collections::HashSet<String> {
        fn walk(list: &[Instruction], out: &mut std::collections::HashSet<String>) {
            for ins in list {
                out.insert(ins.id.clone());
                for slot in 0..2u8 {
                    if let Some(body) = ins.body(slot) {
                        walk(body, out);
                    }
                }
            }
        }
        let mut out = std::collections::HashSet::new();
        for strand in &self.strands {
            walk(&strand.instructions, &mut out);
        }
        out
    }

    /// Drops any comment attached to an instruction that no longer exists —
    /// "if the block gets deleted, the comment is deleted." Call after any
    /// mutation that can remove instructions or whole strands.
    pub fn prune_orphaned_comments(&mut self) {
        let live = self.all_instruction_ids();
        self.comments.retain(|c| {
            c.attached_to
                .as_deref()
                .map_or(true, |id| live.contains(id))
        });
    }

    /// One-time upgrade for saves from before floating/attached comments
    /// existed: pulls every legacy inline `Comment` instruction out of the
    /// instruction stream and re-homes it as a freestanding `Comment` parked
    /// near its old strand. Idempotent — a save with no legacy `Comment`
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
        for strand in &mut self.strands {
            let mut texts = Vec::new();
            extract(&mut strand.instructions, &mut texts);
            for (i, text) in texts.into_iter().enumerate() {
                self.comments.push(Comment {
                    id: default_comment_id(),
                    x: strand.x + 40,
                    y: strand.y + 40 + i as i32 * 30,
                    text,
                    collapsed: false,
                    attached_to: None,
                });
            }
        }
    }

    /// Writes live runtime variable values back into this macro's
    /// `variables` before it's saved to disk, once a run finishes.
    pub fn sync_variables_from(&mut self, values: &HashMap<String, Evaluated>) {
        for var in &mut self.variables {
            if let Some(v) = values.get(&var.name) {
                var.value = v.clone();
            }
        }
    }

    /// Renames a declared variable and every reference to it (`Value::Var`
    /// reads, `SetVariable`/`ChangeVariable` targets) across all strands and
    /// floating values. No-op if `old` isn't declared.
    pub fn rename_variable(&mut self, old: &str, new: &str) {
        if let Some(var) = self.variables.iter_mut().find(|v| v.name == old) {
            var.name = new.to_string();
        } else {
            return;
        }
        for strand in &mut self.strands {
            for ins in &mut strand.instructions {
                ins.rename_var(old, new);
            }
        }
        for fv in &mut self.floating_values {
            fv.value.rename_var(old, new);
        }
    }

    /// Defines a new custom block: appends the `BlockDef` and creates its
    /// (initially empty) header strand at `(x, y)`. Caller validates
    /// `pieces` beforehand.
    pub fn create_block(
        &mut self,
        pieces: Vec<BlockPiece>,
        shape: BlockShape,
        color: String,
        x: i32,
        y: i32,
    ) -> String {
        let id = default_block_id();
        self.block_defs.push(BlockDef {
            id: id.clone(),
            pieces,
            shape,
            color,
        });
        self.strands.push(Strand {
            id: default_strand_id(),
            x,
            y,
            instructions: vec![Instruction::new(InstructionKind::BlockHeader(id.clone()))],
        });
        id
    }

    /// Renames every `Value::Param` leaf reading `old` to `new`, scoped to
    /// `block_id`'s own body. The body-side half of reconciling a renamed
    /// input; caller still needs to update `BlockDef::pieces` separately.
    pub fn rename_block_input_body(&mut self, block_id: &str, old: &str, new: &str) {
        for strand in &mut self.strands {
            if matches!(strand.instructions.first().map(|i| &i.kind), Some(InstructionKind::BlockHeader(id)) if id == block_id)
            {
                for ins in &mut strand.instructions {
                    ins.rename_param(old, new);
                }
            }
        }
    }

    /// Rebuilds every call site's `args` to line up with `new_pieces`' input
    /// order, carrying over each surviving input's value by matching
    /// `BlockPiece::id` (identity survives a rename); removed inputs drop
    /// their value, added ones get a fresh blank matching their declared
    /// `value_type` (`0` for `Any`, an empty `Value::Bool` hexagon for
    /// `Bool`). Call before overwriting `BlockDef::pieces` — `old_pieces`
    /// must be the pieces beforehand.
    pub fn reconcile_block_call_args(
        &mut self,
        block_id: &str,
        old_pieces: &[BlockPiece],
        new_pieces: &[BlockPiece],
    ) {
        let old_input_ids: Vec<&str> = old_pieces
            .iter()
            .filter(|p| matches!(p, BlockPiece::Input { .. }))
            .map(BlockPiece::id)
            .collect();
        let new_inputs: Vec<(&str, InputValueType)> = new_pieces
            .iter()
            .filter_map(|p| match p {
                BlockPiece::Input { id, value_type, .. } => Some((id.as_str(), *value_type)),
                BlockPiece::Label { .. } => None,
            })
            .collect();
        // For each new input slot, which old slot (if any) it carries over from.
        let mapping: Vec<(Option<usize>, InputValueType)> = new_inputs
            .iter()
            .map(|(id, value_type)| (old_input_ids.iter().position(|old| old == id), *value_type))
            .collect();

        let mut rebuild = |args: &mut Vec<Value>| {
            *args = mapping
                .iter()
                .map(|(old_idx, value_type)| {
                    old_idx
                        .and_then(|i| args.get(i).cloned())
                        .unwrap_or_else(|| match value_type {
                            InputValueType::Any => Value::number(0.0),
                            InputValueType::Bool => Value::Bool,
                        })
                })
                .collect();
        };
        for strand in &mut self.strands {
            for ins in &mut strand.instructions {
                ins.for_each_call_args_mut(block_id, &mut rebuild);
            }
        }
        for fv in &mut self.floating_values {
            fv.value.for_each_call_args_mut(block_id, &mut rebuild);
        }
    }

    /// Deletes a custom block entirely: its `BlockDef`, its header strand,
    /// every `CallBlock` instruction calling it, and every `Value::Call`
    /// node calling it (collapsed to a plain `0` leaf) so nothing is left
    /// dangling.
    pub fn remove_block(&mut self, block_id: &str) {
        self.block_defs.retain(|b| b.id != block_id);
        self.strands.retain(|s| !matches!(s.instructions.first().map(|i| &i.kind), Some(InstructionKind::BlockHeader(id)) if id == block_id));
        for strand in &mut self.strands {
            strand.instructions.retain(|ins| !matches!(&ins.kind, InstructionKind::CallBlock { block_id: id, .. } if id == block_id));
            for ins in &mut strand.instructions {
                ins.scrub_block_calls(block_id);
            }
        }
        for fv in &mut self.floating_values {
            fv.value.scrub_block_calls(block_id);
        }
        self.prune_orphaned_comments();
    }

    /// Strand that freshly-recorded input gets appended to: the explicit
    /// `recording_target` if it still exists, else the first "When Ran"
    /// strand, else the first strand (creating one if the macro is empty).
    pub fn recording_target_mut(&mut self) -> &mut Strand {
        if let Some(id) = &self.recording_target {
            if let Some(pos) = self.strands.iter().position(|s| &s.id == id) {
                return &mut self.strands[pos];
            }
        }
        if let Some(pos) = self.strands.iter().position(Strand::starts_with_when_ran) {
            return &mut self.strands[pos];
        }
        if self.strands.is_empty() {
            self.strands.push(Strand {
                id: default_strand_id(),
                x: 0,
                y: 0,
                instructions: vec![],
            });
        }
        &mut self.strands[0]
    }

    /// Read-only counterpart to `recording_target_mut`: same resolution
    /// order, but never creates a strand.
    pub fn recording_target_id(&self) -> Option<String> {
        if let Some(id) = &self.recording_target {
            if self.strands.iter().any(|s| &s.id == id) {
                return Some(id.clone());
            }
        }
        if let Some(strand) = self.strands.iter().find(|s| s.starts_with_when_ran()) {
            return Some(strand.id.clone());
        }
        self.strands.first().map(|s| s.id.clone())
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
    /// entry point Run/Loop invokes — `runner::run_with_offset` skips these
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
    /// (excluded from Run/Loop, driven by a background watcher — `time_watch`
    /// in the desktop app) but for a recurring point in local time instead
    /// of a battery level. See `TimeSchedule` for the recurrence shapes.
    WhenTime(TimeSchedule),
    /// Header-only marker, no payload (like `WhenRan`) — excluded from
    /// Run/Loop and driven by the same background watcher as
    /// `WhenBattery*To` (`battery_watch` in the desktop app), which fires
    /// this strand's body the moment the system starts receiving external
    /// power. See `crate::battery::is_plugged_in`.
    WhenPowerPluggedIn,
    /// Same as `WhenPowerPluggedIn`, but fires when external power is lost
    /// instead — never fires at all on a system with no battery/UPS, since
    /// `is_plugged_in` is always `true` there.
    WhenPowerUnplugged,
    /// Launches an installed application, chosen via the desktop app's "Open
    /// App" picker (`src-tauri`'s `installed_apps` module lists candidates).
    /// `command` is the already-resolved, platform-specific launch string
    /// (a cleaned freedesktop `Exec=` line on Linux, a `.lnk` path on
    /// Windows, an `.app` bundle path on macOS) captured at pick time —
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
    /// instead of launching it — `runner::close_app` derives a process
    /// matcher from `command` (and, on macOS, `name`) rather than executing
    /// it directly. `command`/`name`/`icon` are cached at pick time for the
    /// exact same reason `OpenApp`'s are.
    CloseApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    /// `set <name> to <value>` — overwrites the named variable.
    SetVariable(String, Value),
    /// `change <name> by <value>` — adds `value` to the named variable.
    /// No-op if `value` isn't numeric; the variable is coerced to `0` first
    /// if it wasn't already numeric.
    ChangeVariable(String, Value),
    /// Marks a strand as a custom block's body; the `String` is the
    /// `BlockDef::id`. Header-only, like `WhenRan`, but never auto-runs —
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
    /// `if <condition> then { body }` — runs `body` inline (same strand,
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
    /// `repeat <count> { body }` — runs `body` `count` times (rounded,
    /// clamped to non-negative).
    Repeat {
        count: Value,
        body: Vec<Instruction>,
    },
    /// `forever { body }` — runs `body` in an unconditional loop; only ends
    /// via `EscapeLoop`, a `Return` inside it, or the run being stopped.
    Forever {
        body: Vec<Instruction>,
    },
    /// `while <condition> { body }` — re-evaluates `condition` before every
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

impl std::hash::Hash for Macro {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);
        self.description.hash(state);
        self.strands.hash(state);
        self.recording_target.hash(state);
        self.speed_multiplier.to_bits().hash(state);
        self.floating_values.hash(state);
        self.comments.hash(state);
        self.variables.hash(state);
        self.block_defs.hash(state);
        self.settings.hash(state);
    }
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

fn default_instruction_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// The wrapper every instruction is actually stored as — `id` is a stable
/// identity (unlike position/path, survives drags/splits/merges/reorders)
/// that comments attach to (`Comment::attached_to`); `kind` is the actual
/// instruction data, unchanged in shape from before this wrapper existed.
/// Equality/hashing deliberately ignore `id` and compare `kind` only — the
/// rest of this module (block-header lookups, dedup, tests) all compare
/// instructions structurally, the same as when there was no id at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "InstructionEnvelope")]
pub struct Instruction {
    pub id: String,
    pub kind: InstructionKind,
}

impl Instruction {
    pub fn new(kind: InstructionKind) -> Self {
        Self {
            id: default_instruction_id(),
            kind,
        }
    }

    pub fn is_header(&self) -> bool {
        self.kind.is_header()
    }
    pub fn rename_var(&mut self, old: &str, new: &str) {
        self.kind.rename_var(old, new)
    }
    pub fn migrate_bool_slots(&mut self) {
        self.kind.migrate_bool_slots()
    }
    pub fn rename_param(&mut self, old: &str, new: &str) {
        self.kind.rename_param(old, new)
    }
    pub fn for_each_call_args_mut(&mut self, block_id: &str, f: &mut dyn FnMut(&mut Vec<Value>)) {
        self.kind.for_each_call_args_mut(block_id, f)
    }
    pub fn scrub_block_calls(&mut self, block_id: &str) {
        self.kind.scrub_block_calls(block_id)
    }
    pub fn body(&self, slot: u8) -> Option<&Vec<Instruction>> {
        self.kind.body(slot)
    }
    pub fn body_mut(&mut self, slot: u8) -> Option<&mut Vec<Instruction>> {
        self.kind.body_mut(slot)
    }
}

impl PartialEq for Instruction {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl std::hash::Hash for Instruction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}

/// Wire shape for `Instruction`: today's shape (`{"id": "...", "kind": ...}`)
/// or, for a save from before ids existed, the bare `InstructionKind` value
/// with no envelope at all — same "try new shape, fall back to old" pattern
/// as `WaitDe` above, just one level up. A legacy instruction gets a fresh id
/// generated on load; harmless since nothing could have referenced it by id yet.
#[derive(Deserialize)]
#[serde(untagged)]
enum InstructionEnvelope {
    Current { id: String, kind: InstructionKind },
    Legacy(InstructionKind),
}

impl From<InstructionEnvelope> for Instruction {
    fn from(env: InstructionEnvelope) -> Self {
        match env {
            InstructionEnvelope::Current { id, kind } => Instruction { id, kind },
            InstructionEnvelope::Legacy(kind) => Instruction::new(kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(mac.strands[0].starts_with_when_ran());
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
        assert!(root.starts_with_when_ran());
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

        // Remove the Wait instruction (index 1 — index 0 is the WhenRan header).
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
        // The poisoned `Number` can be arbitrarily deep — here inside an
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
    fn migrate_bool_slots_leaves_legitimately_numeric_fields_alone() {
        // A `Wait` duration is never boolean-typed — a `Number` there is
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

        mac.rename_variable("x", "y");

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
    fn rename_variable_is_a_no_op_for_undeclared_name() {
        let mut mac = Macro::new(
            "Test".into(),
            "".into(),
            vec![Instruction::new(InstructionKind::Token(InputToken::Text(
                Value::Var {
                    name: "x".to_string(),
                },
            )))],
        );
        mac.rename_variable("x", "y");
        assert_eq!(
            mac.strands[0].instructions[1],
            Instruction::new(InstructionKind::Token(InputToken::Text(Value::Var {
                name: "x".to_string()
            })))
        );
    }

    #[test]
    fn rename_var_reaches_into_if_body_and_condition() {
        let mut ins = InstructionKind::If {
            condition: Value::Var {
                name: "x".to_string(),
            },
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Var {
                    name: "x".to_string(),
                },
            ))],
        };
        ins.rename_var("x", "y");
        match &ins {
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
        let mut ins = InstructionKind::IfElse {
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
        };
        ins.rename_var("x", "y");
        match &ins {
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
        let mut ins = InstructionKind::If {
            condition: Value::number(1.0),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::Call {
                    block_id: "gone".to_string(),
                    args: vec![],
                    saved: Box::new(Value::number(0.0)),
                },
            ))],
        };
        ins.scrub_block_calls("gone");
        match &ins {
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
