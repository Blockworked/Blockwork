//! Native host observations of the focused Android application. This has no
//! relationship to desktop WhenApp's process start/exit watcher.
use crate::{Backend, scheduled_run};
use blockwork_core::macros::InstructionKind;
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct Observation {
    package: Option<String>,
    generation: u64,
}
impl Observation {
    pub(crate) fn reset(&mut self) {
        self.package = None;
        self.generation = self.generation.wrapping_add(1);
    }
}

fn conditions_supported(settings: &blockwork_core::macros::MacroSettings) -> bool {
    settings.power == blockwork_core::macros::conditions::PowerCondition::Any && settings.only_in_window.trim().is_empty()
}

impl Backend {
    /// Trusted host entry point, deliberately absent from editor/IPC dispatch.
    /// None resets the baseline after lock, disconnect or picker suspension.
    /// The first valid observation only establishes a baseline, never a run.
    pub fn observe_foreground_app(&self, package: Option<&str>) -> Result<usize, String> {
        if let Some(package) = package { blockwork_core::input::android_app::validate_package(package)?; }
        let mut last = self.foreground.lock().map_err(|e| e.to_string())?;
        let state = self.state.lock().map_err(|e| e.to_string())?;
        if state.capabilities.platform != "android" { return Err("Foreground app events require an Android host".into()); }
        let Some(package) = package else { last.reset(); return Ok(0); };
        if !state.capabilities.services.get("foreground_events").is_some_and(|c| c.ready) {
            last.reset(); return Ok(0);
        }
        if last.package.as_deref() == Some(package) { return Ok(0); }
        let previous = last.package.replace(package.into());
        let generation = last.generation;
        let Some(previous) = previous else { return Ok(0); };
        // Pausing still consumes transitions, so resuming cannot replay them.
        if state.triggers_paused { return Ok(0); }
        let Some(input) = state.emulator.as_ref().map(Arc::clone) else { return Ok(0); };
        let mut jobs = Vec::new();
        for mac in scheduled_run::live_macros(&state) {
            for strand in &mac.strands {
                let Some(InstructionKind::WhenForegroundApp { package: target, entered }) = strand.instructions.first().map(|i| &i.kind) else { continue };
                if (*entered && target == package) || (!*entered && target == &previous) {
                    if !conditions_supported(&mac.settings) {
                        blockwork_core::run_log::error(&blockwork_core::run_log::Source::for_macro(&mac.id, &mac.name),
                            "Android foreground trigger skipped: desktop power/window conditions are unavailable; remove those conditions");
                        continue;
                    }
                    jobs.push((mac.id.clone(), strand.instructions[1..].to_vec(), mac.variables.clone(), mac.lists.clone(), state.global_speed_multiplier));
                }
            }
        }
        drop(state); drop(last);
        let count = jobs.len();
        for (id, body, variables, lists, speed) in jobs {
            let backend = self.clone();
            let macro_id = id.clone();
            let gate: scheduled_run::HostGate = Arc::new(move || {
                let Ok(observed) = backend.foreground.lock() else { return false };
                let Ok(state) = backend.state.lock() else { return false };
                observed.generation == generation && !state.triggers_paused &&
                    scheduled_run::live_macros(&state).iter().any(|mac| mac.id == macro_id && conditions_supported(&mac.settings)) &&
                    state.capabilities.services.get("foreground_events").is_some_and(|c| c.ready)
            });
            scheduled_run::fire_with_host_gate(id, body, Arc::clone(&input), speed, &variables, &lists,
                Arc::clone(&self.state), self.app.clone(), None, Some(("screen", gate)));
        }
        Ok(count)
    }
}
