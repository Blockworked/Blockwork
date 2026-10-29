//! Fires `WhenTime` strands (once per date), `WhenCron` strands (once per
//! matching minute) and `WhenAt` strands (once, at their moment). Macros
//! with "catch up" on also get one run for times missed while Blockwork
//! was closed or the computer slept.

use crate::scheduled_run;
use crate::state::SharedState;
use crate::AppHandle;
use blockwork_core::input::cron::Cron;
use blockwork_core::input::schedule::parse_time_of_day;
use blockwork_core::macros::{InstructionKind, Macro, Strand};
use blockwork_core::run_log;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Timelike};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Poll interval.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// A gap between ticks longer than this means time was missed.
const MISSED_AFTER_SECS: i64 = 90;
/// How far back a catch-up looks.
/// How often the last-seen time is saved, in ticks.
const SAVE_EVERY: u32 = 30;
const STATE_FILE: &str = "schedule_state.json";

/// Spawns the watcher thread.
pub(crate) fn start(shared_state: SharedState, app: AppHandle) {
    let _ = std::thread::Builder::new().name("time-watch".into()).spawn(move || run(shared_state, app));
}

fn state_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join(blockwork_core::config::APP_ID).join(STATE_FILE))
}

/// When the watcher last ran, from the previous session.
fn load_last_seen() -> Option<DateTime<Local>> {
    let text = std::fs::read_to_string(state_path()?).ok()?;
    let secs = serde_json::from_str::<serde_json::Value>(&text).ok()?.get("last_seen")?.as_i64()?;
    Local.timestamp_opt(secs, 0).single()
}

fn save_last_seen(now: DateTime<Local>) {
    let Some(path) = state_path() else { return };
    let _ = std::fs::write(path, serde_json::json!({ "last_seen": now.timestamp() }).to_string());
}

/// Whether `strand`'s schedule had a time in (`from`, `to`], checked minute by minute.
fn missed(strand: &Strand, env: &HashMap<String, blockwork_core::input::value::Evaluated>, from: DateTime<Local>, to: DateTime<Local>) -> bool {
    strand.instructions.first().and_then(|header| blockwork_core::input::due::DueSchedule::resolve(&header.kind, env, &Local).ok().flatten())
        .is_some_and(|schedule| schedule.latest_between(&from, &to).is_some())
}

/// A `WhenAt` header's moment.
fn at_time(at: &blockwork_core::input::value::Value, env: &HashMap<String, blockwork_core::input::value::Evaluated>) -> Option<DateTime<Local>> {
    let text = at.resolve_vars(env).eval_text().ok()?;
    let secs = blockwork_core::text::parse_date(&text, "").ok()?;
    Local.timestamp_opt(secs.floor() as i64, 0).single()
}

fn fire(mac: &Macro, strand: &Strand, emulator: &Arc<std::sync::Mutex<dyn blockwork_core::macros::backend::InputBackend>>, speed: f64, shared_state: &SharedState, app: &AppHandle) {
    scheduled_run::fire(mac.id.clone(), strand.instructions[1..].to_vec(), Arc::clone(emulator), speed, &mac.variables, &mac.lists, Arc::clone(shared_state), app.clone());
}

fn run(shared_state: SharedState, app: AppHandle) {
    // (macro_id, strand_id) -> the minute it last fired. `WhenTime` only
    // compares the date part.
    let mut last_fired: HashMap<(String, String), NaiveDateTime> = HashMap::new();
    // Cron expressions and "once at" times already reported as broken.
    let mut reported: HashMap<(String, String), String> = HashMap::new();
    let mut previous_tick = load_last_seen().filter(|t| *t < Local::now()).unwrap_or_else(Local::now);
    let mut ticks = 0u32;

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let now = Local::now();
        let Some(minute) = now.naive_local().with_second(0).and_then(|t| t.with_nanosecond(0)) else { continue };
        let gap_from = previous_tick;
        previous_tick = now;
        ticks += 1;
        if ticks % SAVE_EVERY == 1 {
            save_last_seen(now);
        }

        let (macros, emulator, speed_multiplier) = {
            let Ok(s) = shared_state.lock() else { continue };
            let Some(emulator) = s.emulator.as_ref().map(Arc::clone) else { continue };
            let macros: Vec<_> = scheduled_run::live_macros(&s).into_iter().cloned().collect();
            (macros, emulator, s.global_speed_multiplier)
        };
        // Times before the current minute that were never checked.
        let missed_window = ((now - gap_from).num_seconds() > MISSED_AFTER_SECS).then(|| (gap_from, now - chrono::Duration::minutes(1)));

        let mut next_last_fired = HashMap::with_capacity(last_fired.len());
        for mac in &macros {
            let env: HashMap<_, _> = mac.variables.iter().map(|v| (v.name.clone(), v.value.clone())).collect();
            for strand in &mac.strands {
                let key = (mac.id.clone(), strand.id.clone());
                let previous = last_fired.get(&key).copied();
                let mut report = |text: String, problem: String| {
                    if reported.get(&key) != Some(&text) {
                        run_log::error(&run_log::Source::for_macro(&mac.id, &mac.name), problem);
                        reported.insert(key.clone(), text);
                    }
                };
                let due = match strand.instructions.first().map(|i| &i.kind) {
                    Some(InstructionKind::WhenTime(schedule, time)) => {
                        let text = time.resolve_vars(&env).eval_text().unwrap_or_default();
                        match parse_time_of_day(&text) {
                            Ok((hour, minute_of_hour)) => {
                                previous.map(|t| t.date()) != Some(minute.date()) && schedule.matches(&now, hour, minute_of_hour)
                            }
                            Err(e) => {
                                report(text, format!("When: {e}"));
                                false
                            }
                        }
                    }
                    Some(InstructionKind::WhenCron(expression)) => {
                        let text = expression.resolve_vars(&env).eval_text().unwrap_or_default();
                        match text.parse::<Cron>() {
                            Ok(cron) => previous != Some(minute) && cron.matches(&now),
                            Err(e) => {
                                report(text, format!("Schedule: {e}"));
                                false
                            }
                        }
                    }
                    Some(InstructionKind::WhenAt(at)) => match at_time(at, &env) {
                        // Fires on the tick that passes it, unless that tick
                        // came after a gap, which catch-up handles.
                        Some(at) => gap_from < at && at <= now && missed_window.is_none(),
                        None => {
                            let text = at.resolve_vars(&env).eval_text().unwrap_or_default();
                            report(text.clone(), format!("Once at: \"{text}\" isn't a date and time like 2026-12-24 18:00"));
                            false
                        }
                    },
                    _ => continue,
                };
                let caught_up = !due
                    && mac.settings.catch_up_missed
                    && missed_window.is_some_and(|(from, to)| missed(strand, &env, from, to));
                if caught_up {
                    run_log::info(&run_log::Source::for_macro(&mac.id, &mac.name), "Running a schedule it missed");
                }
                if due || caught_up {
                    fire(mac, strand, &emulator, speed_multiplier, &shared_state, &app);
                    next_last_fired.insert(key, minute);
                } else if let Some(previous) = previous {
                    next_last_fired.insert(key, previous);
                }
            }
        }
        last_fired = next_last_fired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockwork_core::input::value::Value;
    use blockwork_core::macros::Instruction;

    fn strand(kind: InstructionKind) -> Strand {
        Strand::with_instructions(0, 0, vec![Instruction::new(kind)])
    }

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 3, 10, h, m, 0).unwrap()
    }

    #[test]
    fn finds_schedules_inside_a_gap() {
        let env = HashMap::new();
        let cron = strand(InstructionKind::WhenCron(Value::text("30 9 * * *")));
        assert!(missed(&cron, &env, at(8, 0), at(10, 0)));
        assert!(!missed(&cron, &env, at(9, 30), at(10, 0)), "the start is exclusive");
        assert!(!missed(&cron, &env, at(10, 0), at(11, 0)));

        let once = strand(InstructionKind::WhenAt(Value::text("2026-03-10 09:15")));
        assert!(missed(&once, &env, at(9, 0), at(9, 20)));
        assert!(!missed(&once, &env, at(9, 20), at(9, 40)));

        let daily = strand(InstructionKind::WhenTime(blockwork_core::input::schedule::TimeSchedule::Daily, Value::text("9:45")));
        assert!(missed(&daily, &env, at(9, 0), at(10, 0)));
        assert!(!missed(&daily, &env, at(10, 0), at(11, 0)));
    }
}
