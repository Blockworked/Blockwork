//! One browser tab's macros. Canvas edits run through `blockwork_model::editing`,
//! as they do in the app; this file adds the tab's in-memory macro list, its
//! key capture, preview and state, without disk, daemon or runner.

use std::collections::HashMap;

use blockstitch_core::editor::{History, PathStep, ValueBuffers};
use blockstitch_core::graph::{ListItem, resolve_list_reporters};
use blockstitch_ffi::{Host, arg};
use blockwork_model::editing;
use blockwork_model::frontend::{InvalidFieldDto, KeyCaptureDto, macro_to_dto};
use blockwork_model::input::key_names::{format_key_combo, match_key};
use blockwork_model::input::types::{InputToken, MacroKey};
use blockwork_model::input::value::{
    Evaluated, Op, Value, has_side_effect_operator, is_pure_operator,
};
use blockwork_model::key_mapping::{web_code_to_macro_key, web_key_to_macro_key};
use blockwork_model::macros::conditions::PowerCondition;
use blockwork_model::macros::{
    Instruction, InstructionKind, Macro, UndoSnapshot, clamp_speed, migrate,
};
use serde_json::{Value as Json, json};

enum KeyCapture {
    Strand(String, Vec<PathStep>),
    Standalone,
}

/// The page's document: every macro in the tab, the open one, its history.
pub struct Demo {
    macros: Vec<Macro>,
    selected: Option<usize>,
    history: History<UndoSnapshot>,
    buffers: ValueBuffers,
    key_capture: Option<KeyCapture>,
    standalone_key: Option<String>,
    palette_layout: String,
}

impl Default for Demo {
    fn default() -> Self {
        // The pure text and date reporters; host ones stay unregistered.
        blockwork_model::init();
        Demo {
            macros: Vec::new(),
            selected: None,
            history: History::new(blockwork_model::macros::UNDO_LIMIT),
            buffers: ValueBuffers::new(),
            key_capture: None,
            standalone_key: None,
            palette_layout: "rail".into(),
        }
    }
}

/// The commands only the page answers; [`commands`] adds the shared ones.
const PAGE_COMMANDS: &[&str] = &[
    "load",
    "state",
    "commands",
    "preview_value",
    "new_macro",
    "select_macro_by_id",
    "duplicate_macro",
    "delete_macro",
    "remove_macro",
    "set_macro_folder",
    "set_title",
    "set_macro_speed_multiplier",
    "set_macro_always_listen",
    "set_macro_catch_up",
    "set_macro_conditions",
    "set_macro_max_run_secs",
    "set_macro_max_loop_iterations",
    "set_palette_layout",
    "start_key_capture",
    "start_standalone_key_capture",
    "key_capture_event",
    "clear_standalone_key_capture",
];

/// Every command `dispatch` answers, for the page's contract check.
pub fn commands() -> Vec<&'static str> {
    PAGE_COMMANDS
        .iter()
        .chain(editing::COMMANDS)
        .copied()
        .collect()
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Json, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// Page commands that only read; the rest tidy signal names as the app's
/// `auto_save` does. Shared canvas commands tidy through `changed`.
const READS: &[&str] = &["state", "commands", "preview_value", "select_macro_by_id"];

impl editing::Document for Demo {
    fn parts(&mut self) -> editing::Parts<'_> {
        editing::Parts {
            open: self.selected.and_then(|i| self.macros.get_mut(i)),
            history: &mut self.history,
            buffers: &mut self.buffers,
        }
    }

    /// The app tidies signal names on every save; the page has no disk but
    /// keeps the same document.
    fn changed(&mut self) {
        if let Some(mac) = self.open_mut() {
            mac.collect_named_signals();
        }
    }
}

impl Host for Demo {
    fn dispatch(&mut self, command: &str, args: &Json) -> Result<Json, String> {
        if let Some(result) = editing::dispatch(self, command, args) {
            return result;
        }
        let result = self.run(command, args);
        if result.is_ok()
            && !READS.contains(&command)
            && let Some(mac) = self.open_mut()
        {
            mac.collect_named_signals();
        }
        result
    }
}

impl Demo {
    fn run(&mut self, command: &str, args: &Json) -> Result<Json, String> {
        let a = args;
        match command {
            "state" => Ok(self.state()),
            "commands" => to_json(commands()),
            "load" => {
                let macros: Vec<Json> = arg(a, "macros")?;
                self.load(macros)?;
                Ok(Json::Null)
            }
            "preview_value" => self.preview(arg(a, "value")?).map(Json::String),

            // Macro library: in memory, one history for whichever is open.
            "new_macro" => {
                self.add_and_select(blockwork_model::macros::creation::from_args(a)?);
                Ok(Json::Null)
            }
            "select_macro_by_id" => {
                let id: String = arg(a, "id")?;
                let index = self
                    .macros
                    .iter()
                    .position(|m| m.id == id)
                    .ok_or("Macro not found")?;
                self.select(Some(index));
                Ok(Json::Null)
            }
            "duplicate_macro" => {
                let id: String = arg(a, "id")?;
                let copy = self.find(&id)?.duplicate();
                self.add_and_select(copy);
                Ok(Json::Null)
            }
            "delete_macro" => {
                let id: String = arg(a, "id")?;
                let index = self
                    .macros
                    .iter()
                    .position(|m| m.id == id)
                    .ok_or("Macro not found")?;
                self.delete(index);
                Ok(Json::Null)
            }
            "remove_macro" => {
                if let Some(index) = self.selected {
                    self.delete(index);
                }
                Ok(Json::Null)
            }
            "set_macro_folder" => {
                let (id, folder): (String, String) = (arg(a, "id")?, arg(a, "folder")?);
                let index = self
                    .macros
                    .iter()
                    .position(|m| m.id == id)
                    .ok_or("Macro not found")?;
                self.macros[index].settings.folder = blockwork_model::macros::clean_folder(&folder);
                Ok(Json::Null)
            }
            "set_title" => {
                let title: String = arg(a, "title")?;
                if let Some(mac) = self.open_mut() {
                    mac.name = title;
                }
                Ok(Json::Null)
            }
            "set_macro_speed_multiplier" => {
                let multiplier: f64 = arg(a, "multiplier")?;
                if let Some(mac) = self.open_mut() {
                    mac.speed_multiplier = clamp_speed(multiplier);
                }
                Ok(Json::Null)
            }
            "set_macro_always_listen" | "set_macro_catch_up" => {
                let enabled: bool = arg(a, "enabled")?;
                if let Some(mac) = self.open_mut() {
                    if command == "set_macro_always_listen" {
                        mac.settings.always_listen = enabled;
                    } else {
                        mac.settings.catch_up_missed = enabled;
                    }
                }
                Ok(Json::Null)
            }
            "set_macro_conditions" => {
                let window: String = arg(a, "onlyInWindow")?;
                let power: PowerCondition = arg(a, "power")?;
                let (from, until): (String, String) =
                    (arg(a, "activeFrom")?, arg(a, "activeUntil")?);
                let days: u8 = arg(a, "activeDays")?;
                if let Some(mac) = self.open_mut() {
                    mac.settings
                        .set_conditions(&window, power, &from, &until, days)?;
                }
                Ok(Json::Null)
            }
            "set_macro_max_run_secs" => {
                let seconds: u32 = arg(a, "seconds")?;
                if let Some(mac) = self.open_mut() {
                    mac.settings.max_run_secs = seconds;
                }
                Ok(Json::Null)
            }
            "set_macro_max_loop_iterations" => {
                let iterations: u32 = arg(a, "iterations")?;
                if let Some(mac) = self.open_mut() {
                    mac.settings.max_loop_iterations = iterations;
                }
                Ok(Json::Null)
            }
            "set_palette_layout" => {
                let layout: String = arg(a, "layout")?;
                self.palette_layout = if layout == "drawer" {
                    layout
                } else {
                    "rail".into()
                };
                Ok(Json::Null)
            }

            // Key capture: the page's key events, named as the app names them.
            "start_key_capture" => {
                self.key_capture = Some(KeyCapture::Strand(arg(a, "strandId")?, arg(a, "path")?));
                Ok(Json::Null)
            }
            "start_standalone_key_capture" => {
                self.key_capture = Some(KeyCapture::Standalone);
                Ok(Json::Null)
            }
            "clear_standalone_key_capture" => {
                self.standalone_key = None;
                Ok(Json::Null)
            }
            "key_capture_event" => {
                let keys: Vec<(String, String)> = arg(a, "keys")?;
                self.key_capture_event(&keys);
                Ok(Json::Null)
            }

            other => Err(format!("unsupported command `{other}`")),
        }
    }
}

/// Whether `value` uses a reporter nothing on the page can answer (a host
/// reporter such as the battery or the clipboard).
fn needs_app(value: &Value) -> bool {
    match value {
        Value::Op { op, args, .. } => {
            // Not the registry: a native build with `system` unified in has
            // the machine's reporters registered too.
            matches!(op, Op::Ext(name) if !is_pure_operator(name)) || args.iter().any(needs_app)
        }
        Value::Call { args, .. } => args.iter().any(needs_app),
        _ => false,
    }
}

impl Demo {
    fn open(&self) -> Result<&Macro, String> {
        self.selected
            .and_then(|i| self.macros.get(i))
            .ok_or_else(|| "No macro selected".to_string())
    }

    fn open_mut(&mut self) -> Option<&mut Macro> {
        self.selected.and_then(|i| self.macros.get_mut(i))
    }

    fn find(&self, id: &str) -> Result<&Macro, String> {
        self.macros
            .iter()
            .find(|m| m.id == id)
            .ok_or_else(|| "Macro not found".to_string())
    }

    fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|&i| i < self.macros.len());
        self.history.clear();
        self.buffers.clear();
        self.key_capture = None;
    }

    fn add_and_select(&mut self, mac: Macro) {
        self.macros.push(mac);
        self.select(Some(self.macros.len() - 1));
    }

    /// Deleting the open macro leaves nothing open, as in the app.
    fn delete(&mut self, index: usize) {
        self.macros.remove(index);
        match self.selected {
            Some(open) if open == index => self.select(None),
            Some(open) if open > index => self.selected = Some(open - 1),
            _ => {}
        }
    }

    /// Replaces the tab's macros with saved ones (the page's examples) and
    /// opens the first. Saved files go through the app's own migrations.
    fn load(&mut self, saved: Vec<Json>) -> Result<(), String> {
        let mut macros = Vec::with_capacity(saved.len());
        for value in saved {
            let (mut mac, _) = migrate::parse(&value.to_string())?;
            mac.ensure_id();
            macros.push(mac);
        }
        self.macros = macros;
        self.select((!self.macros.is_empty()).then_some(0));
        Ok(())
    }

    /// Variables at their declared values, which is what the app's preview
    /// reads while nothing runs.
    fn variable_values(&self) -> HashMap<String, Evaluated> {
        self.open()
            .map(|mac| {
                mac.variables
                    .iter()
                    .map(|v| (v.name.clone(), v.value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The app's click-to-preview over the shared evaluator. Host reporters
    /// are not simulated: they say they need the app.
    fn preview(&self, value: Value) -> Result<String, String> {
        if has_side_effect_operator(&value) {
            return Err("Preview is off for blocks that run commands or go online".into());
        }
        if needs_app(&value) {
            return Err("This reporter needs the Blockwork app to show a value".into());
        }
        let lists: HashMap<String, Vec<ListItem>> = self
            .open()
            .map(|mac| {
                mac.lists
                    .iter()
                    .map(|l| (l.name.clone(), l.items.clone()))
                    .collect()
            })
            .unwrap_or_default();
        resolve_list_reporters(&value.resolve_vars(&self.variable_values()), &lists)?.eval_text()
    }

    fn key_capture_event(&mut self, keys: &[(String, String)]) {
        let mut combo: Vec<MacroKey> = Vec::new();
        for (code, key) in keys {
            if let Some(mk) = web_code_to_macro_key(code).or_else(|| web_key_to_macro_key(key))
                && !combo.iter().any(|k| match_key(k) == match_key(&mk))
            {
                combo.push(mk);
            }
        }
        if combo.is_empty() {
            return;
        }
        let text = format_key_combo(&combo);
        match self.key_capture.take() {
            Some(KeyCapture::Strand(strand_id, path)) => {
                if let Some(mac) = self.open_mut()
                    && let Some(Instruction {
                        kind:
                            InstructionKind::Token(InputToken::Key(value, _))
                            | InstructionKind::WhenKey(value, _),
                        ..
                    }) = mac.instruction_at_mut(&strand_id, &path)
                {
                    *value = Value::Text { value: text };
                }
            }
            Some(KeyCapture::Standalone) => self.standalone_key = Some(text),
            None => {}
        }
    }

    /// The app's `StateDto`, with the fields the editor reads; what only the
    /// app has (runs, recording, globals, permissions) stays empty.
    pub fn state(&self) -> Json {
        let open = self.selected.and_then(|i| self.macros.get(i));
        let buffers: Vec<InvalidFieldDto> = self
            .buffers
            .iter()
            .map(|(location, text)| InvalidFieldDto {
                location: location.clone(),
                text: text.clone(),
            })
            .collect();
        let key_capture = self.key_capture.as_ref().map(|target| match target {
            KeyCapture::Strand(strand_id, index) => KeyCaptureDto {
                kind: "Strand".into(),
                strand_id: Some(strand_id.clone()),
                index: Some(index.clone()),
            },
            KeyCapture::Standalone => KeyCaptureDto {
                kind: "Standalone".into(),
                strand_id: None,
                index: None,
            },
        });
        let library: Vec<Json> = self
            .macros
            .iter()
            .map(|mac| {
                let has_triggers = mac.has_triggers();
                json!({ "id": mac.id, "name": mac.name, "folder": mac.settings.folder,
                        "always_listen": mac.settings.always_listen, "has_triggers": has_triggers, "restricted": false })
            })
            .collect();
        json!({
            "macro_names": self.macros.iter().map(|m| &m.name).collect::<Vec<_>>(),
            "macro_selected": self.selected,
            "current_macro": open.map(macro_to_dto),
            "macros_data": [],
            "global_speed_multiplier": 1.0,
            "is_looping": false,
            "page": "Main",
            "onboarding_done": true,
            "palette_layout": self.palette_layout,
            "key_capture": key_capture,
            "standalone_key": self.standalone_key,
            "combo_capture": null,
            "can_undo": self.history.can_undo(),
            "can_redo": self.history.can_redo(),
            "recording_phase": { "phase": "Idle", "countdown": null },
            "recording_countdown_secs": 3,
            "record_mouse_relative": true,
            "record_mouse_movement": false,
            "absolute_mouse_position_available": true,
            "invalid_field_buffers": buffers,
            "triggers_paused": false,
            "run_log": [],
            "templates": [],
            "secret_names": [],
            "globals": [],
            "hotkey_bindings": [],
            "permissions": null,
            "breakpoints": [],
            "paused_macros": [],
            "paused_strands": [],
            "library": library,
            "language": "",
        })
    }
}
