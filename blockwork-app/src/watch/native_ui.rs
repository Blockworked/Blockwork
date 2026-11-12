//! Coalesced native UI changes; only property fingerprints survive each poll.
use super::{ReportOnce, StrandKey, Target, variable_env};
use crate::{AppHandle, scheduled_run, state::SharedState};
use blockwork_core::{
    accessibility,
    input::ui::{UiProperty, UiSelector},
    macros::InstructionKind,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
struct Watched {
    target: Target,
    selector: UiSelector,
    property: UiProperty,
    signature: String,
}
pub(crate) fn start(state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("native-ui-watch".into())
        .spawn(move || {
            let mut last: HashMap<StrandKey, (String, u64)> = HashMap::new();
            let mut revision = None;
            let mut errors = ReportOnce::default();
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let (watched, emulator, speed) = {
                    let Ok(s) = state.lock() else { continue };
                    if !s.ui_automation_enabled || s.triggers_paused {
                        last.clear();
                        revision = None;
                        continue;
                    }
                    let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                        continue;
                    };
                    let mut watched = vec![];
                    for mac in scheduled_run::live_macros(&s) {
                        if !super::permitted(mac, "ui_automation") {
                            continue;
                        }
                        let env = variable_env(mac);
                        for strand in &mac.strands {
                            if let Some(InstructionKind::WhenNativeUiChanged {
                                selector,
                                property,
                            }) = strand.instructions.first().map(|i| &i.kind)
                            {
                                let Some(selector) = errors.check(
                                    "Native UI selector",
                                    super::header_value(selector, &env)
                                        .and_then(|v| v.eval_text())
                                        .and_then(|s| UiSelector::parse(&s)),
                                ) else {
                                    continue;
                                };
                                let signature = format!(
                                    "{}:{property:?}",
                                    serde_json::to_string(&selector).unwrap_or_default()
                                );
                                watched.push(Watched {
                                    target: Target::new(mac, strand),
                                    selector,
                                    property: *property,
                                    signature,
                                });
                            }
                        }
                    }
                    (watched, emulator, s.global_speed_multiplier)
                };
                if watched.is_empty() {
                    last.clear();
                    revision = None;
                    continue;
                }
                let Some(now) = errors.check("Native UI events", accessibility::event_revision())
                else {
                    last.clear();
                    revision = None;
                    continue;
                };
                let changed = revision != Some(now);
                revision = Some(now);
                for w in &watched {
                    if !changed
                        && last
                            .get(&w.target.key)
                            .is_some_and(|(signature, _)| signature == &w.signature)
                    {
                        continue;
                    }
                    let query = match w.property {
                        UiProperty::Exists => accessibility::Query::Exists,
                        UiProperty::Enabled => accessibility::Query::Enabled,
                        UiProperty::Text => accessibility::Query::Text,
                        UiProperty::Value => accessibility::Query::Value,
                    };
                    let Some(value) = errors.check(
                        "Native UI property",
                        accessibility::query(&w.selector, query),
                    ) else {
                        last.remove(&w.target.key);
                        continue;
                    };
                    let mut hash = std::collections::hash_map::DefaultHasher::new();
                    value.hash(&mut hash);
                    let hash = hash.finish();
                    if last
                        .get(&w.target.key)
                        .is_some_and(|(signature, old)| signature == &w.signature && *old != hash)
                    {
                        w.target.fire(&emulator, speed, &state, &app);
                    }
                    last.insert(w.target.key.clone(), (w.signature.clone(), hash));
                }
                last.retain(|key, _| watched.iter().any(|w| &w.target.key == key));
            }
        });
}
