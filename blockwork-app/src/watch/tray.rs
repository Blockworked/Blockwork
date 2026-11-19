//! `WhenTrayItem` strands: each one adds an item to the daemon's tray menu,
//! and clicking the item runs the strand.

use super::{Target, variable_env};
use crate::state::AppState;
use crate::{AppHandle, Backend, Event, scheduled_run};
use blockwork_core::macros::InstructionKind;
use std::sync::{Arc, Mutex};

/// One tray menu item: what it says and which strand it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItem {
    /// "macro id/strand id", passed back to [`Backend::run_tray_item`].
    pub id: String,
    pub label: String,
}

/// The items last sent, so a state change only sends them when they differ.
static LAST: Mutex<Vec<TrayItem>> = Mutex::new(Vec::new());

fn targets(s: &AppState) -> Vec<(TrayItem, Target)> {
    let mut out = Vec::new();
    for mac in scheduled_run::live_macros(s) {
        let env = variable_env(mac);
        for strand in &mac.strands {
            let Some(InstructionKind::WhenTrayItem(label)) =
                strand.instructions.first().map(|i| &i.kind)
            else {
                continue;
            };
            let label = label.resolve_vars(&env).eval_text().unwrap_or_default();
            let label = if label.trim().is_empty() {
                mac.name.clone()
            } else {
                label
            };
            let target = Target::new(mac, strand);
            let id = format!("{}/{}", target.key.0, target.key.1);
            out.push((TrayItem { id, label }, target));
        }
    }
    out
}

pub(crate) fn items(s: &AppState) -> Vec<TrayItem> {
    targets(s).into_iter().map(|(item, _)| item).collect()
}

/// Sends [`Event::TrayItems`] when the items changed since the last call.
pub(crate) fn refresh(app: &AppHandle, s: &AppState) {
    let now = items(s);
    let Ok(mut last) = LAST.lock() else { return };
    if *last != now {
        *last = now.clone();
        app.send(Event::TrayItems(now));
    }
}

impl Backend {
    /// The tray items live macros add right now.
    pub fn tray_items(&self) -> Vec<TrayItem> {
        self.state.lock().map(|s| items(&s)).unwrap_or_default()
    }

    /// Runs the strand behind a tray item (by [`TrayItem::id`]).
    pub fn run_tray_item(&self, id: &str) {
        let (target, emulator, speed) = {
            let Ok(s) = self.state.lock() else { return };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                return;
            };
            let Some((_, target)) = targets(&s).into_iter().find(|(item, _)| item.id == id) else {
                return;
            };
            (target, emulator, s.global_speed_multiplier)
        };
        target.fire(&emulator, speed, &self.state, &self.app);
    }
}

#[cfg(test)]
mod tests {
    use blockwork_core::input::value::Value;
    use blockwork_core::macros::{Instruction, InstructionKind, Macro};

    fn tray_macro(name: &str, label: &str, listen: bool) -> Macro {
        let mut mac = Macro::new(name.into(), "".into(), vec![]);
        mac.graph.strands[0].instructions = vec![
            Instruction::new(InstructionKind::WhenTrayItem(Value::text(label))),
            Instruction::new(InstructionKind::Log(Value::text("hi"))),
        ];
        mac.settings.always_listen = listen;
        mac
    }

    #[test]
    fn items_come_from_live_macros_and_blank_labels_use_the_name() {
        let dir = std::env::temp_dir().join(format!("blockwork-tray-{}", uuid::Uuid::new_v4()));
        let backend = crate::Backend::for_tests(dir.clone());
        {
            let mut s = backend.state.lock().unwrap();
            let open = tray_macro("Open", "Say hi", false);
            s.macros_list = vec![
                open.clone(),
                tray_macro("Listening", " ", true),
                tray_macro("Idle", "Hidden", false),
            ];
            s.current_macro = Some(open);
        }
        let labels: Vec<String> = backend.tray_items().into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Say hi", "Listening"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
