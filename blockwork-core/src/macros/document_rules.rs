//! Checks and small edits every editor shares (the app and the web demo),
//! so a rule like "list names are unique" lives in one place.

use blockstitch_core::editor::PathStep;

use super::conditions::{PowerCondition, parse_clock};
use super::{BlockDef, BlockPiece, BlockShape, Instruction, InstructionKind, Macro, MacroSettings, placement};
use crate::input::value::Value;

/// How many steps undo remembers, in the app and the web demo.
pub const UNDO_LIMIT: usize = 50;

/// What undo restores: the canvas plus the macro's signals and block packs.
pub type UndoSnapshot = (blockstitch_core::graph::BlockGraph<InstructionKind>, Vec<String>, Vec<crate::block_packs::PackRecord>);

/// How every Blockwork editor applies typed edits: "09" typed into an
/// operator argument stays text.
pub const EDIT_CONTEXT: blockstitch_core::editor::EditContext =
    blockstitch_core::editor::EditContext { numbers: blockstitch_core::editor::NumberSpelling::KeepTypedInOperands };

/// A speed multiplier as stored, inside [`super::SPEED_MULTIPLIER_RANGE`].
pub fn clamp_speed(multiplier: f64) -> f64 {
    multiplier.clamp(*super::SPEED_MULTIPLIER_RANGE.start(), *super::SPEED_MULTIPLIER_RANGE.end())
}

/// A folder name as stored: no surrounding spaces or slashes, empty for the top level.
pub fn clean_folder(folder: &str) -> String {
    folder.trim().trim_matches('/').trim().to_string()
}

/// A custom block's color as stored: six-digit hex, so a shared macro can't
/// put arbitrary CSS into the editor.
pub fn checked_block_color(color: &str) -> Result<String, String> {
    blockstitch_core::graph::normalize_block_color(color).ok_or_else(|| "Choose a valid block color".to_string())
}

/// What "Make a Block" sends: a custom block's prototype and details.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BlockDraft {
    pub pieces: Vec<BlockPiece>,
    pub shape: BlockShape,
    pub color: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub identifier: String,
    #[serde(default)]
    pub description: String,
}

impl BlockDraft {
    /// Refuses bad pieces, identifier or color before anything changes.
    pub fn check(&self) -> Result<(), String> {
        BlockDef::validate_pieces(&self.pieces)?;
        crate::block_packs::check_identifier(&self.identifier)?;
        checked_block_color(&self.color)?;
        Ok(())
    }
}

impl Macro {
    /// The part of the macro undo checkpoints.
    pub fn undo_snapshot(&self) -> UndoSnapshot {
        (self.graph.clone(), self.signals.clone(), self.settings.block_packs.clone())
    }

    /// Puts an undo checkpoint back.
    pub fn restore_undo_snapshot(&mut self, snapshot: UndoSnapshot) {
        (self.graph, self.signals, self.settings.block_packs) = snapshot;
        self.ensure_id();
    }

    /// A copy with a new id, named "<name> copy".
    pub fn duplicate(&self) -> Macro {
        let mut copy = self.clone();
        copy.id = uuid::Uuid::new_v4().simple().to_string();
        copy.name = format!("{} copy", copy.name);
        copy
    }

    /// Defines a custom block from a checked draft, at `at` or beside the
    /// existing strands; returns its id.
    pub fn add_block(&mut self, draft: BlockDraft, at: Option<(i32, i32)>) -> Result<String, String> {
        let color = checked_block_color(&draft.color)?;
        let (x, y) = at.unwrap_or_else(|| self.next_editor_position());
        let id = self.create_block(draft.pieces, draft.shape, color, x, y);
        self.set_block_details(&id, &draft.identifier, &draft.description, &draft.icon)?;
        Ok(id)
    }

    /// Whether `block_id` may be edited into `draft` (a pack's blocks can't be).
    pub fn check_block_edit(&self, block_id: &str, draft: &BlockDraft) -> Result<(), String> {
        crate::block_packs::check_editable(self, block_id)?;
        draft.check()
    }

    /// Edits a custom block to a checked draft; call sites, body inputs and
    /// renamed branches follow (blockstitch's `update_block`).
    pub fn edit_block(&mut self, block_id: &str, draft: BlockDraft) -> Result<(), String> {
        self.update_block(block_id, draft.pieces, draft.shape, &draft.color)?;
        self.set_block_details(block_id, &draft.identifier, &draft.description, &draft.icon)
    }

    /// Whether `ins` may go at `path` in the strand: headers only on top,
    /// `Return` only in a reporter body, loop control only inside a loop.
    pub fn check_insert(&self, strand_id: &str, path: &[PathStep], ins: &Instruction) -> Result<(), String> {
        self.check_header_placement(strand_id, path, ins)?;
        if let Some(strand) = self.strand(strand_id) {
            placement::check_return(self, strand, ins)?;
            placement::check_loop_control(strand, path, ins)?;
        }
        Ok(())
    }

    /// Refuses to delete a signal that is missing or still named by a
    /// `When signal`/`Send signal` block.
    pub fn check_signal_deletable(&self, name: &str) -> Result<(), String> {
        if !self.signals.iter().any(|s| s == name) {
            return Err("Signal not found".into());
        }
        let mut used = false;
        self.graph.walk_instructions(&mut |ins| {
            if let InstructionKind::WhenSignal(target) | InstructionKind::SendSignal { name: target, .. } = &ins.kind
                && !target.expression
                && let Value::Text { value } = &target.value
            {
                used |= value.trim().eq_ignore_ascii_case(name);
            }
        });
        if used {
            return Err("This signal is still used by a named signal block. Change or remove those blocks first.".into());
        }
        Ok(())
    }

    /// The trimmed name a list would be created or renamed to (`old` is the
    /// list being renamed), or why it can't be.
    pub fn check_list_name(&self, name: &str, old: Option<&str>) -> Result<String, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("List name can't be empty".into());
        }
        if Some(trimmed) != old && self.lists.iter().any(|list| list.name == trimmed) {
            return Err(format!("A list named \"{trimmed}\" already exists"));
        }
        if let Some(old) = old
            && !self.lists.iter().any(|list| list.name == old)
        {
            return Err("List not found".into());
        }
        Ok(trimmed.to_string())
    }

    /// Adds a list with its editor window near the top left; returns its name.
    pub fn add_list(&mut self, name: &str) -> Result<String, String> {
        let trimmed = self.graph.create_list(name)?;
        if let Some(list) = self.graph.lists.iter_mut().find(|list| list.name == trimmed) {
            list.editor_x = 36;
            list.editor_y = 36;
        }
        Ok(trimmed)
    }
}

impl MacroSettings {
    /// Sets when the macro may run. Times are `HH:MM`; give both or neither.
    pub fn set_conditions(
        &mut self,
        only_in_window: &str,
        power: PowerCondition,
        active_from: &str,
        active_until: &str,
        active_days: u8,
    ) -> Result<(), String> {
        let active_hours = match (active_from.trim(), active_until.trim()) {
            ("", "") => None,
            ("", _) | (_, "") => return Err("set both a start and an end time, or neither".into()),
            (from, until) => Some((parse_clock(from)?, parse_clock(until)?)),
        };
        self.only_in_window = only_in_window.trim().to_string();
        self.power = power;
        self.active_hours = active_hours;
        self.active_days = active_days & 0x7f;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_names_are_trimmed_unique_and_must_exist_to_rename() {
        let mut mac = Macro::new("m".into(), String::new(), vec![]);
        assert_eq!(mac.check_list_name("  todo ", None).unwrap(), "todo");
        mac.add_list("todo").unwrap();
        assert!(mac.check_list_name("todo", None).is_err());
        assert!(mac.check_list_name(" ", None).is_err());
        assert_eq!(mac.check_list_name("todo", Some("todo")).unwrap(), "todo");
        assert!(mac.check_list_name("done", Some("missing")).is_err());
    }

    #[test]
    fn active_hours_need_both_ends() {
        let mut settings = Macro::new("m".into(), String::new(), vec![]).settings;
        assert!(settings.set_conditions("", PowerCondition::default(), "09:00", "", 0).is_err());
        settings.set_conditions(" Editor ", PowerCondition::default(), "09:00", "17:00", 0xff).unwrap();
        assert_eq!(settings.only_in_window, "Editor");
        assert!(settings.active_hours.is_some());
        assert_eq!(settings.active_days, 0x7f);
    }
}
