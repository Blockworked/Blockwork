//! Fires battery/power strands. Edge-triggered with hysteresis.

use super::{StrandKey, Target, variable_env};
use crate::AppHandle;
use crate::scheduled_run;
use crate::state::SharedState;
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Poll interval.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    Armed,
    Disarmed,
}

/// Pure tick decision: returns (should fire, next state).
fn tick_decision(armed: Arm, holds: bool) -> (bool, Arm) {
    match (armed, holds) {
        (Arm::Armed, true) => (true, Arm::Disarmed),
        (Arm::Disarmed, false) => (false, Arm::Armed),
        (armed, _) => (false, armed),
    }
}

/// Spawns the watcher thread.
pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("battery-watch".into())
        .spawn(move || run(shared_state, app));
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> arm state. Rebuilt fresh each tick from
    // whatever strands currently exist, carrying over prior arm state by key
    // - so a deleted strand/macro just quietly drops out instead of leaking.
    let mut arm_state: HashMap<StrandKey, Arm> = HashMap::new();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        // `level` is `None` on a desktop with no battery at all - that only
        // rules out the two threshold blocks below, not plug/unplug (see
        // `battery::is_plugged_in`, which is always `true` there).
        let level = blockwork_core::battery::percentage().ok();
        let plugged_in = blockwork_core::battery::is_plugged_in();

        let (watched, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else {
                continue;
            };
            let mut watched = Vec::new();
            for mac in scheduled_run::live_macros(&s) {
                for strand in &mac.strands {
                    let holds = match strand.instructions.first().map(|i| &i.kind) {
                        Some(InstructionKind::WhenBatteryDischargedTo(v)) => {
                            let Some(level) = level else { continue };
                            // A variable in the threshold reads its saved
                            // value, as every other trigger's header does.
                            let Ok(threshold) = v.resolve_vars(&variable_env(mac)).eval_number()
                            else {
                                continue;
                            };
                            level <= threshold
                        }
                        Some(InstructionKind::WhenBatteryChargedTo(v)) => {
                            let Some(level) = level else { continue };
                            let Ok(threshold) = v.resolve_vars(&variable_env(mac)).eval_number()
                            else {
                                continue;
                            };
                            level >= threshold
                        }
                        Some(InstructionKind::WhenPowerPluggedIn) => plugged_in,
                        Some(InstructionKind::WhenPowerUnplugged) => !plugged_in,
                        _ => continue,
                    };
                    watched.push((Target::new(mac, strand), holds));
                }
            }
            (watched, emulator, s.global_speed_multiplier)
        };

        let mut next_arm_state = HashMap::with_capacity(arm_state.len());
        for (target, holds) in watched {
            let armed = arm_state.get(&target.key).copied().unwrap_or(Arm::Armed);
            let (should_fire, next) = tick_decision(armed, holds);
            if should_fire {
                target.fire(&emulator, speed_multiplier, &shared_state, &app);
            }
            next_arm_state.insert(target.key, next);
        }
        arm_state = next_arm_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_once_then_waits_for_recovery_before_rearming() {
        // First observation, condition already true (e.g. app just started
        // with the battery already past the threshold): fires immediately.
        let (fired, state) = tick_decision(Arm::Armed, true);
        assert!(fired);
        assert_eq!(state, Arm::Disarmed);

        // Stays disarmed - no refire - while the condition keeps holding on
        // later ticks.
        let (fired, state) = tick_decision(state, true);
        assert!(!fired);
        assert_eq!(state, Arm::Disarmed);

        // Recovers past the threshold: re-arms, but that's not itself a fire.
        let (fired, state) = tick_decision(state, false);
        assert!(!fired);
        assert_eq!(state, Arm::Armed);

        // Crosses again: fires again.
        let (fired, state) = tick_decision(state, true);
        assert!(fired);
        assert_eq!(state, Arm::Disarmed);
    }

    #[test]
    fn never_fires_while_condition_stays_false() {
        let (fired, state) = tick_decision(Arm::Armed, false);
        assert!(!fired);
        assert_eq!(state, Arm::Armed);

        let (fired, state) = tick_decision(state, false);
        assert!(!fired);
        assert_eq!(state, Arm::Armed);
    }
}
