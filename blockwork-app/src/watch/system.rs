//! Fires `WhenApp`, `WhenIdle`, `WhenScreenLock`, `WhenNetwork` and
//! `WhenUsb` strands. Polls like `watch::window`: compare each condition with
//! the last poll and fire the strand when it becomes what the header waits
//! for. USB fires once per matching device that comes or goes.

use super::{ReportOnce, StrandKey, Target, fires_on_change, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::macros::InstructionKind;
use blockwork_core::system;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

enum Condition {
    App { pattern: String, started: bool },
    Idle { secs: f64 },
    Locked(bool),
    Network(bool),
    Usb { pattern: String, connected: bool },
}

struct Watched {
    target: Target,
    condition: Condition,
}

pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("system-watch".into())
        .spawn(move || run(shared_state, app));
}

/// Devices in `now` that weren't in `before`, and the other way round.
fn device_changes(
    now: &[system::UsbDevice],
    before: &[system::UsbDevice],
) -> (Vec<system::UsbDevice>, Vec<system::UsbDevice>) {
    let added = now
        .iter()
        .filter(|d| !before.iter().any(|b| b.id == d.id))
        .cloned()
        .collect();
    let removed = before
        .iter()
        .filter(|b| !now.iter().any(|d| d.id == b.id))
        .cloned()
        .collect();
    (added, removed)
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> whether the condition held last poll.
    let mut last: HashMap<StrandKey, bool> = HashMap::new();
    // Devices at the last poll; `None` until something watches USB.
    let mut last_usb: Option<Vec<system::UsbDevice>> = None;
    let mut errors = ReportOnce::default();
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let (watched, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                continue;
            };
            let mut watched = Vec::new();
            for mac in scheduled_run::live_macros(&s) {
                let env = variable_env(mac);
                for strand in &mac.strands {
                    let condition = match strand.instructions.first().map(|i| &i.kind) {
                        Some(InstructionKind::WhenApp { pattern, started }) => Condition::App {
                            pattern: pattern.resolve_vars(&env).eval_text().unwrap_or_default(),
                            started: *started,
                        },
                        Some(InstructionKind::WhenIdle(minutes)) => {
                            match minutes.resolve_vars(&env).eval_number() {
                                Ok(m) if m > 0.0 => Condition::Idle { secs: m * 60.0 },
                                _ => continue,
                            }
                        }
                        Some(InstructionKind::WhenScreenLock { locked }) => {
                            Condition::Locked(*locked)
                        }
                        Some(InstructionKind::WhenNetwork { connected }) => {
                            Condition::Network(*connected)
                        }
                        Some(InstructionKind::WhenUsb { pattern, connected }) => Condition::Usb {
                            pattern: pattern.resolve_vars(&env).eval_text().unwrap_or_default(),
                            connected: *connected,
                        },
                        _ => continue,
                    };
                    watched.push(Watched {
                        target: Target::new(mac, strand),
                        condition,
                    });
                }
            }
            (watched, emulator, s.global_speed_multiplier)
        };
        if watched.is_empty() {
            last.clear();
            last_usb = None;
            continue;
        }

        // Only ask the system for what some trigger needs.
        let wants = |f: fn(&Condition) -> bool| watched.iter().any(|w| f(&w.condition));
        let apps = if wants(|c| matches!(c, Condition::App { .. })) {
            errors.check("App trigger", system::running_apps())
        } else {
            None
        };
        let idle = if wants(|c| matches!(c, Condition::Idle { .. })) {
            errors.check("Idle trigger", system::idle_time())
        } else {
            None
        };
        let locked = if wants(|c| matches!(c, Condition::Locked(_))) {
            errors.check("Screen lock trigger", system::screen_locked())
        } else {
            None
        };
        let online = if wants(|c| matches!(c, Condition::Network(_))) {
            errors.check("Network trigger", system::network_connected())
        } else {
            None
        };
        let usb = if wants(|c| matches!(c, Condition::Usb { .. })) {
            errors.check("USB trigger", system::usb_devices())
        } else {
            None
        };
        // Devices that came and went since the last poll.
        let (added, removed) = match (&usb, &last_usb) {
            (Some(now), Some(before)) => device_changes(now, before),
            _ => (Vec::new(), Vec::new()),
        };
        if usb.is_some() || !wants(|c| matches!(c, Condition::Usb { .. })) {
            last_usb = usb;
        }

        let mut next = HashMap::with_capacity(watched.len());
        for w in watched {
            let (now, fire_on) = match &w.condition {
                Condition::App { pattern, started } => match &apps {
                    Some(apps) => (system::app_matches(apps, pattern), *started),
                    None => continue,
                },
                Condition::Idle { secs } => match idle {
                    Some(idle) => (idle.as_secs_f64() >= *secs, true),
                    None => continue,
                },
                Condition::Locked(want) => match locked {
                    Some(locked) => (locked, *want),
                    None => continue,
                },
                Condition::Network(want) => match online {
                    Some(online) => (online, *want),
                    None => continue,
                },
                Condition::Usb { pattern, connected } => {
                    let changed: &Vec<system::UsbDevice> =
                        if *connected { &added } else { &removed };
                    for _ in changed.iter().filter(|d| d.matches(pattern)) {
                        w.target
                            .fire(&emulator, speed_multiplier, &shared_state, &app);
                    }
                    continue;
                }
            };
            if fires_on_change(last.get(&w.target.key).copied(), now, fire_on) {
                w.target
                    .fire(&emulator, speed_multiplier, &shared_state, &app);
            }
            next.insert(w.target.key, now);
        }
        last = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str) -> system::UsbDevice {
        system::UsbDevice {
            id: id.into(),
            name: String::new(),
            maker: String::new(),
            ids: String::new(),
        }
    }

    #[test]
    fn usb_changes_are_counted_by_device_id() {
        let (added, removed) =
            device_changes(&[device("a"), device("c")], &[device("a"), device("b")]);
        assert_eq!(
            added.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            ["c"]
        );
        assert_eq!(
            removed.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            ["b"]
        );
        let (added, removed) = device_changes(&[device("a")], &[device("a")]);
        assert!(added.is_empty() && removed.is_empty());
    }
}
