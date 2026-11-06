//! The editor's frontend shape: a macro as the QML canvas reads it and the
//! flat `{"type": ...}` instructions it sends back. The desktop app and the
//! website's in-browser editor both speak exactly this, so it lives with the
//! vocabulary rather than in either host.

use crate::block_packs;
use crate::input::schedule::TimeSchedule;
use crate::input::types::{Axis, Coordinate, Direction, InputToken};
use crate::input::value::Value;
use crate::input::{get_mouse_button_names, mouse_button_to_index};
use crate::macros::{
    BlockDef, Comment, FloatingValue, Instruction, InstructionKind, LaunchOptions, Macro, MacroSettings, Strand,
};
use blockstitch_core::editor::{InstrPath, ValueLocation};
use blockstitch_core::graph::{Fragment, ListDef, ListItem};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", content = "value")]
pub enum ListItemDto {
    Number(f64),
    Text(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListDto {
    pub name: String,
    pub items: Vec<ListItemDto>,
    pub editor_visible: bool,
    pub editor_x: i32,
    pub editor_y: i32,
}

pub fn list_to_dto(list: &ListDef) -> ListDto {
    ListDto {
        name: list.name.clone(),
        items: list
            .items
            .iter()
            .map(|item| match item {
                ListItem::Number(value) => ListItemDto::Number(*value),
                ListItem::Text(value) => ListItemDto::Text(value.clone()),
            })
            .collect(),
        editor_visible: list.editor_visible,
        editor_x: list.editor_x,
        editor_y: list.editor_y,
    }
}

pub fn dto_to_list_item(item: &ListItemDto) -> ListItem {
    match item {
        ListItemDto::Number(value) => ListItem::Number(*value),
        ListItemDto::Text(value) => ListItem::Text(value.clone()),
    }
}

#[derive(Serialize, Clone)]
pub struct MacroDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub strands: Vec<StrandDto>,
    pub recording_target_strand_id: Option<String>,
    pub speed_multiplier: f64,
    pub floating_values: Vec<FloatingValue>,
    /// Floating/attached notes - see `Comment`.
    pub comments: Vec<Comment>,
    /// Declared variable names only, for the sidebar/dropdowns - current
    /// values aren't surfaced to the frontend.
    pub variables: Vec<String>,
    pub pack_data: Vec<PackDataDto>,
    pub pack_exports: Vec<block_packs::PackExport>,
    /// Macro-scoped lists, including their literal items for the list editor.
    pub lists: Vec<ListDto>,
    pub signals: Vec<String>,
    /// User-defined custom blocks ("My Blocks").
    pub block_defs: Vec<BlockDefinitionDto>,
    /// Settings edited from the "Macro Settings" popup - see `MacroSettingsDto`.
    pub settings: MacroSettingsDto,
}

#[derive(Serialize, Clone)]
pub struct PackDataDto {
    pub id: String,
    pub name: String,
    pub variables: Vec<PackVariableDto>,
}

#[derive(Serialize, Clone)]
pub struct PackVariableDto {
    pub name: String,
    pub label: String,
    pub value: crate::input::value::Evaluated,
    pub role: block_packs::VariableRole,
}

#[derive(Serialize, Clone)]
pub struct BlockDefinitionDto {
    #[serde(flatten)]
    pub definition: BlockDef,
    pub read_only: bool,
    pub pack_name: String,
    pub packs: Vec<BlockPackMembershipDto>,
}

#[derive(Serialize, Clone)]
pub struct BlockPackMembershipDto {
    pub id: String,
    pub name: String,
}

/// Wire shape for `MacroSettings` - see there for field meanings.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MacroSettingsDto {
    pub always_listen: bool,
    pub max_run_secs: u32,
    pub max_loop_iterations: u32,
    pub only_in_window: String,
    pub power: crate::macros::conditions::PowerCondition,
    /// `HH:MM`, both empty when there are no active hours.
    pub active_from: String,
    pub active_until: String,
    pub active_days: u8,
    pub folder: String,
    pub catch_up_missed: bool,
}

pub fn macro_settings_to_dto(settings: &MacroSettings) -> MacroSettingsDto {
    use crate::macros::conditions::clock;
    let (active_from, active_until) = settings.active_hours.map(|(a, b)| (clock(a), clock(b))).unwrap_or_default();
    MacroSettingsDto {
        always_listen: settings.always_listen,
        max_run_secs: settings.max_run_secs,
        max_loop_iterations: settings.max_loop_iterations,
        only_in_window: settings.only_in_window.clone(),
        power: settings.power,
        active_from,
        active_until,
        active_days: settings.active_days,
        folder: settings.folder.clone(),
        catch_up_missed: settings.catch_up_missed,
    }
}

#[derive(Serialize, Clone)]
pub struct StrandDto {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub instructions: Vec<InstructionDto>,
}

#[derive(Serialize, Clone)]
pub struct KeyCaptureDto {
    pub kind: String,
    pub strand_id: Option<String>,
    pub index: Option<InstrPath>,
}

/// Copied blocks and the notes pinned to them, as the editor's copy buffer
/// holds them.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FragmentDto {
    pub instructions: Vec<InstructionDto>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

pub fn fragment_to_dto(fragment: &Fragment<InstructionKind>) -> FragmentDto {
    FragmentDto {
        instructions: fragment.instructions.iter().map(instruction_to_dto).collect(),
        comments: fragment.comments.clone(),
    }
}

pub fn dto_to_fragment(dto: &FragmentDto) -> Option<Fragment<InstructionKind>> {
    Some(Fragment {
        instructions: dto
            .instructions
            .iter()
            .map(dto_to_instruction)
            .collect::<Option<_>>()?,
        comments: dto.comments.clone(),
    })
}
mod instructions;
mod reporter_branches;
pub use reporter_branches::edit_reporter_branch;
pub use instructions::*;

pub fn strand_to_dto(strand: &Strand) -> StrandDto {
    StrandDto {
        id: strand.id.clone(),
        x: strand.x,
        y: strand.y,
        instructions: strand.instructions.iter().map(instruction_to_dto).collect(),
    }
}

pub fn macro_to_dto(mac: &Macro) -> MacroDto {
    let mut hidden_instruction_ids = HashSet::new();
    for strand in mac.strands.iter().filter(|strand| block_packs::is_pack_strand(mac, strand)) {
        for ins in &strand.instructions {
            ins.walk(&mut |ins| { hidden_instruction_ids.insert(ins.id.clone()); });
        }
    }
    MacroDto {
        id: mac.id.clone(),
        name: mac.name.clone(),
        description: mac.description.clone(),
        strands: mac.editor_strands()
            .map(strand_to_dto).collect(),
        recording_target_strand_id: mac.recording_target_id(),
        speed_multiplier: mac.speed_multiplier,
        floating_values: mac.floating_values.clone(),
        comments: mac.comments.iter().filter(|comment| {
            comment.attached_to.as_ref().is_none_or(|id| !hidden_instruction_ids.contains(id))
        }).cloned().collect(),
        variables: mac.variables.iter().map(|v| v.name.clone()).collect(),
        pack_exports: mac.settings.pack_exports.clone(),
        pack_data: mac.settings.block_packs.iter().map(|pack| PackDataDto {
            id: pack.id.clone(), name: pack.name.clone(),
            variables: pack.variables.iter().map(|var| PackVariableDto {
                name: var.name.clone(), label: block_packs::variable_label(&var.name).into(),
                value: match &var.value {
                    #[cfg(feature = "system")]
                    crate::input::value::Evaluated::Text(text) => crate::input::value::Evaluated::Text(crate::secrets::redact(text)),
                    _ => var.value.clone(),
                }, role: var.role,
            }).collect(),
        }).collect(),
        lists: mac.lists.iter().map(list_to_dto).collect(),
        signals: mac.signals.clone(),
        block_defs: mac.block_defs.iter().map(|def| {
            let pack = block_packs::owner(mac, &def.id);
            let mut definition = def.clone();
            if let Some(pack) = pack {
                let name = if definition.identifier.is_empty() {
                    block_packs::block_name(def)
                } else { definition.identifier.clone() };
                definition.identifier = block_packs::details_identifier(&pack.author, &name);
                if definition.description.is_empty() { definition.description = pack.description.clone(); }
            }
            BlockDefinitionDto {
                definition,
                read_only: pack.is_some(),
                pack_name: pack.map(|p| p.name.clone()).unwrap_or_default(),
                packs: mac.settings.block_packs.iter().filter(|pack| pack.blocks.contains(&def.id))
                    .map(|pack| BlockPackMembershipDto { id: pack.id.clone(), name: pack.name.clone() }).collect(),
            }
        }).collect(),
        settings: macro_settings_to_dto(&mac.settings),
    }
}

/// Decodes reporter bodies stored as frontend instructions or older native records.
pub fn reporter_branch_to_instructions(body: &[serde_json::Value]) -> Result<Vec<Instruction>, String> {
    body.iter().map(|value| {
        serde_json::from_value::<InstructionDto>(value.clone()).ok()
            .and_then(|dto| dto_to_instruction(&dto))
            .or_else(|| serde_json::from_value::<Instruction>(value.clone()).ok())
            .ok_or_else(|| "Invalid instruction in a custom reporter branch".to_string())
    }).collect()
}
