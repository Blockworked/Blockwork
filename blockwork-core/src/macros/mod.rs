//! Blockwork's block vocabulary - the instructions its canvas speaks - and
//! the `Macro` document wrapped around blockstitch's [`BlockGraph`].
//! [`InstructionKind`]'s [`BlockKind`] impl is the hinge between the two.

use crate::input::schedule::{LegacyTimeSchedule, TimeSchedule};
use crate::input::types::InputToken;
use crate::input::value::{Op, Value};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

#[cfg(feature = "system")]
pub mod backend;
#[cfg(feature = "system")]
pub mod calls;
pub mod conditions;
#[cfg(feature = "system")]
pub mod debug;
pub mod fields;
#[cfg(feature = "system")]
pub mod gdr_import;
#[cfg(feature = "system")]
pub mod host_clock;
#[cfg(feature = "system")]
pub mod locks;
#[cfg(feature = "system")]
pub mod loop_control;
pub mod migrate;
pub mod placement;
#[cfg(feature = "system")]
pub mod priority;
#[cfg(feature = "system")]
pub mod run_registry;
#[cfg(feature = "system")]
pub mod runner;
#[cfg(feature = "system")]
pub mod thread_pool;
#[cfg(feature = "system")]
pub mod trace;
pub mod trigger;

pub use blockstitch_core::graph::{
    BlockDef, BlockGraph, BlockKind, BlockPiece, BlockShape, Comment, DictDef, FloatingValue,
    InputValueType, ListDef, ListItem, RefKind, VariableDef, default_block_color,
    normalize_block_color,
};
pub use fields::FieldId;

/// One instruction on the canvas: Blockwork's [`InstructionKind`] plus the
/// stable id blockstitch tracks it by.
pub type Instruction = blockstitch_core::graph::Instruction<InstructionKind>;
/// One draggable stack of Blockwork instructions.
pub type Strand = blockstitch_core::graph::Strand<InstructionKind>;

/// What a typed-in value is stored as: a number when it reads as one, else text.
pub fn typed_value(text: &str) -> crate::input::value::Evaluated {
    use crate::input::value::Evaluated;
    match text.trim().parse::<f64>() {
        Ok(n) if n.is_finite() && !text.trim().is_empty() => Evaluated::Number(n),
        _ => Evaluated::Text(text.to_string()),
    }
}
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

mod block_kind;
mod document_rules;
pub use document_rules::{
    BlockDraft, EDIT_CONTEXT, UNDO_LIMIT, UndoSnapshot, checked_block_color, clamp_speed,
    clean_folder,
};
mod saved_shapes;

mod info;
pub use info::{BlockInfo, Role};
mod kind;
pub use kind::*;

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
    #[serde(default)]
    pub signals: Vec<String>,
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
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize, Default)]
pub struct MacroSettings {
    /// When `true`, this macro's `WhenBattery*`/`WhenTime`/`WhenPower*`
    /// strands are watched by the background watchers (`watch::battery`/
    /// `watch::time` in the desktop app) even while a different macro is
    /// selected. By default only the currently selected macro's event
    /// strands are live.
    #[serde(default)]
    pub always_listen: bool,
    /// Stops a run (or a trigger-fired strand) after this many seconds;
    /// 0 means no limit.
    #[serde(default)]
    pub max_run_secs: u32,
    /// Stops any one loop after this many iterations; 0 means no limit.
    #[serde(default)]
    pub max_loop_iterations: u32,
    /// Triggers and hotkeys only start the macro while a window whose
    /// title or app contains this is focused; empty means any window.
    #[serde(default)]
    pub only_in_window: String,
    #[serde(default)]
    pub power: conditions::PowerCondition,
    /// Only from/until these minutes after midnight (may wrap past midnight).
    #[serde(default)]
    pub active_hours: Option<(u16, u16)>,
    /// Only on these weekdays, bit 0 = Sunday; 0 means every day.
    #[serde(default)]
    pub active_days: u8,
    /// Library folder; empty means the top level.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// Runs schedules and "once at" triggers it missed while Blockwork was
    /// closed or the computer slept, once, when it gets the chance.
    #[serde(default)]
    pub catch_up_missed: bool,
    /// Block packs this macro's blocks came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_packs: Vec<crate::block_packs::PackRecord>,
    /// Metadata for packs authored here, restored on the next export.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pack_exports: Vec<crate::block_packs::PackExport>,
}

/// Deserialization shape supporting both the current multi-strand format and
/// the legacy flat `code: Vec<Instruction>` format from older saves; legacy
/// macros become a single root strand. The canvas collections are listed out
/// rather than flattened, so the untagged split stays a field-by-field match.
// Only lives while a document loads.
#[allow(clippy::large_enum_variant)]
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
        signals: Vec<String>,
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
        #[serde(default)]
        lists: Vec<ListDef>,
        // Carried for round-trip only, Blockwork has no dict blocks or UI.
        #[serde(default)]
        dicts: Vec<DictDef>,
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
                signals,
                recording_target,
                speed_multiplier,
                floating_values,
                comments,
                variables,
                block_defs,
                settings,
                lists,
                dicts,
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
                        lists,
                        dicts,
                        block_defs,
                    },
                    signals,
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
                    signals: Vec::new(),
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
        mac.collect_named_signals();
        crate::block_packs::migrate_variables(&mut mac);
        mac
    }
}

impl Macro {
    /// Whether a strand starts on its own, from a watcher. A signal header
    /// doesn't count: only a run of this macro sends one.
    pub fn has_triggers(&self) -> bool {
        self.strands.iter().any(|st| {
            st.instructions.first().is_some_and(|i| {
                i.kind.info().role == Role::Trigger
                    && !matches!(i.kind, InstructionKind::WhenSignal(_))
            })
        })
    }

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
            signals: Vec::new(),
            recording_target: None,
            speed_multiplier: default_speed_multiplier(),
            settings: MacroSettings::default(),
        }
    }

    /// Pack implementation strands stay available to playback, outside the editor.
    pub fn editor_strands(&self) -> impl Iterator<Item = &Strand> {
        self.strands
            .iter()
            .filter(|strand| !crate::block_packs::is_pack_strand(self, strand))
    }

    pub fn clear_editor(&mut self) {
        let ids: Vec<_> = self
            .editor_strands()
            .map(|strand| strand.id.clone())
            .collect();
        self.graph
            .strands
            .retain(|strand| !ids.contains(&strand.id));
        self.graph.prune_orphaned_comments();
    }

    pub fn next_editor_position(&self) -> (i32, i32) {
        (
            self.editor_strands()
                .map(|strand| strand.x)
                .max()
                .unwrap_or(0)
                + 260,
            0,
        )
    }

    /// Keep named references registered when importing or adding blocks.
    pub fn collect_named_signals(&mut self) {
        let mut names = Vec::new();
        self.graph.walk_instructions(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &ins.kind
                && !name.expression
                && let Value::Text { value } = &name.value
                && !value.trim().is_empty()
            {
                names.push(value.trim().to_string());
            }
        });
        let mut unique: Vec<String> = Vec::new();
        for name in self.signals.iter().chain(names.iter()) {
            let name = name.trim();
            if !name.is_empty() && !unique.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                unique.push(name.to_string());
            }
        }
        self.graph.walk_instructions_mut(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &mut ins.kind
                && !name.expression
                && let Value::Text { value } = &mut name.value
                && let Some(canonical) =
                    unique.iter().find(|s| s.eq_ignore_ascii_case(value.trim()))
            {
                *value = canonical.clone();
            }
        });
        self.signals = unique;
    }

    pub fn validate_signal_name(
        &self,
        name: &str,
        replacing: Option<&str>,
    ) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Signal name can't be empty".into());
        }
        if self
            .signals
            .iter()
            .any(|s| Some(s.as_str()) != replacing && s.eq_ignore_ascii_case(name))
        {
            return Err(format!("A signal named \"{name}\" already exists"));
        }
        Ok(name.to_string())
    }

    /// The checked new name when renaming `old` to `new` changes anything,
    /// `None` when it is the same name. Called before taking an undo step.
    pub fn check_signal_rename(&self, old: &str, new: &str) -> Result<Option<String>, String> {
        let name = self.validate_signal_name(new, Some(old))?;
        if !self.signals.iter().any(|s| s == old) {
            return Err("Signal not found".into());
        }
        Ok((name != old).then_some(name))
    }

    pub fn rename_signal(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = self.validate_signal_name(new, Some(old))?;
        let index = self
            .signals
            .iter()
            .position(|s| s == old)
            .ok_or("Signal not found")?;
        self.signals[index] = new.clone();
        self.graph.walk_instructions_mut(&mut |ins| {
            if let InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } =
                &mut ins.kind
                && !name.expression
                && let Value::Text { value } = &mut name.value
                && value.trim().eq_ignore_ascii_case(old)
            {
                *value = new.clone();
            }
        });
        Ok(())
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
        if let Some(id) = self.recording_target_id() {
            let pos = self
                .graph
                .strands
                .iter()
                .position(|strand| strand.id == id)
                .unwrap();
            return &mut self.graph.strands[pos];
        }
        self.graph.strands.push(Strand::new(0, 0));
        self.graph.strands.last_mut().unwrap()
    }

    /// Resolves the recording target without entering a pack implementation.
    pub fn recording_target_id(&self) -> Option<String> {
        if let Some(id) = &self.recording_target
            && self.editor_strands().any(|strand| &strand.id == id)
        {
            return Some(id.clone());
        }
        self.editor_strands()
            .find(|strand| strand.starts_with_header())
            .or_else(|| self.editor_strands().next())
            .map(|strand| strand.id.clone())
    }
}

impl std::hash::Hash for Macro {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.name.hash(state);
        self.description.hash(state);
        self.graph.hash(state);
        self.signals.hash(state);
        self.recording_target.hash(state);
        self.speed_multiplier.to_bits().hash(state);
        self.settings.hash(state);
    }
}

#[cfg(test)]
mod tests;
