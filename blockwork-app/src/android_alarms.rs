//! AlarmManager's durable desired state. Claim an occurrence on disk before
//! starting any actions. Broadcast tokens are opaque and never command input.
use crate::{Backend, host::Capability, scheduled_run};
use blockwork_core::{
    input::{
        due::DueSchedule,
        value::{Op, Value},
    },
    macros::{InstructionKind, Macro},
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Alarm {
    pub token: String,
    pub at_ms: i64,
    pub exact: bool,
    pub macro_name: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Pending {
    alarm: Alarm,
    namespace: String,
    identity: String,
}
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
struct Record {
    macro_id: String,
    strand_id: String,
    definition: String,
    zone: String,
    cursor_ms: i64,
    completed: BTreeMap<String, String>,
    pending: Option<Pending>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Ledger {
    version: u32,
    records: Vec<Record>,
    last_seen_ms: i64,
    #[serde(default)]
    paused: bool,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            version: 1,
            records: vec![],
            last_seen_ms: 0,
            paused: false,
        }
    }
}
#[derive(Default)]
pub(crate) struct Store {
    path: Option<PathBuf>,
    ledger: Option<Ledger>,
    recovered: bool,
    saved_at: i64,
}
#[derive(Serialize)]
pub struct Plan {
    pub paused: bool,
    pub alarms: Vec<Alarm>,
    pub errors: Vec<String>,
}

fn durable_shape(ledger: &Ledger) -> Vec<u8> {
    let mut shape = ledger.clone();
    shape.last_seen_ms = 0;
    for record in &mut shape.records {
        record.cursor_ms = 0;
    }
    serde_json::to_vec(&shape).expect("ledger serialization")
}

impl Store {
    fn load(&mut self) -> Result<(), String> {
        if self.ledger.is_some() {
            return Ok(());
        }
        let path = self
            .path
            .as_ref()
            .ok_or("Android alarm storage is not configured")?;
        let ledger: Ledger = match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Alarm history is unreadable; schedules are blocked: {e}"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ledger::default(),
            Err(e) => return Err(e.to_string()),
        };
        if ledger.version != 1 {
            return Err("Unsupported Android alarm history version".into());
        }
        self.ledger = Some(ledger);
        Ok(())
    }
    fn save(&mut self, now: i64) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or("Android alarm storage is not configured")?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&serde_json::to_vec(self.ledger.as_ref().unwrap())?)?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)?;
            std::fs::File::open(path.parent().unwrap())?.sync_all()
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e: std::io::Error| e.to_string())?;
        self.saved_at = now;
        Ok(())
    }
}

fn is_schedule(header: &InstructionKind) -> bool {
    matches!(
        header,
        InstructionKind::WhenTime(..) | InstructionKind::WhenCron(_) | InstructionKind::WhenAt(_)
    )
}
fn safe_value(value: &Value) -> bool {
    match value {
        Value::Op {
            op: Op::Ext(_) | Op::Random | Op::CurrentTime,
            ..
        }
        | Value::Call { .. }
        | Value::Param { .. } => false,
        Value::Op { args, .. } => args.iter().all(safe_value),
        _ => true,
    }
}
fn resolve(
    mac: &Macro,
    header: &InstructionKind,
    zone: Tz,
) -> Result<(DueSchedule, String, String), String> {
    let value = match header {
        InstructionKind::WhenTime(_, value)
        | InstructionKind::WhenCron(value)
        | InstructionKind::WhenAt(value) => value,
        _ => return Err("Not a schedule".into()),
    };
    if !safe_value(value) {
        return Err("Android schedules need a literal, saved variable or pure expression; live reporters/custom calls are unavailable in alarm times".into());
    }
    let env: HashMap<_, _> = mac
        .variables
        .iter()
        .map(|v| (v.name.clone(), v.value.clone()))
        .collect();
    let schedule = DueSchedule::resolve(header, &env, &zone)?.ok_or("Not a schedule")?;
    let namespace = match header {
        InstructionKind::WhenAt(value) => format!("once:{}", value.resolve_vars(&env).eval_text()?),
        InstructionKind::WhenCron(_) => "cron".into(),
        _ => "time".into(),
    };
    let definition = serde_json::to_value((header, &mac.settings, format!("{schedule:?}")))
        .map_err(|e| e.to_string())?
        .to_string();
    Ok((schedule, namespace, definition))
}
fn accepted(record: &Record, namespace: &str, key: &str) -> bool {
    record
        .completed
        .get(namespace)
        .is_none_or(|old| key > old.as_str())
}
fn claim(record: &mut Record, pending: &Pending) {
    let old = record
        .completed
        .entry(pending.namespace.clone())
        .or_default();
    if pending.identity > *old {
        *old = pending.identity.clone();
    }
}
fn future(
    schedule: &DueSchedule,
    namespace: &str,
    record: &Record,
    now: DateTime<Tz>,
) -> Option<DateTime<Tz>> {
    if matches!(schedule, DueSchedule::Once(_)) && record.completed.contains_key(namespace) {
        return None;
    }
    let mut after = now;
    if let Some(last) = record.completed.get(namespace) {
        let naive = if namespace == "time" {
            NaiveDate::parse_from_str(last, "%Y-%m-%d")
                .ok()?
                .and_hms_opt(23, 59, 59)?
        } else {
            NaiveDateTime::parse_from_str(last, "%Y-%m-%dT%H:%M").ok()?
        };
        if let Some(last) = after.timezone().from_local_datetime(&naive).latest() {
            after = after.max(last);
        }
    }
    schedule
        .next_after(&after)
        .filter(|at| accepted(record, namespace, &schedule.identity(at)))
}
fn unsupported(mac: &Macro) -> bool {
    mac.settings.power != blockwork_core::macros::conditions::PowerCondition::Any
        || !mac.settings.only_in_window.trim().is_empty()
}
fn uses_ui(mac: &Macro) -> bool {
    crate::commands::macro_capabilities(mac).iter().any(|c| {
        [
            "screen", "input", "commands", "windows", "macros", "browser",
        ]
        .contains(&c.kind.as_str())
    })
}
fn at(zone: Tz, millis: i64) -> Result<DateTime<Tz>, String> {
    zone.timestamp_millis_opt(millis)
        .single()
        .ok_or("Alarm time is out of range".into())
}

impl Backend {
    pub fn configure_android_alarms(&self, path: PathBuf) -> Result<(), String> {
        let mut store = self.alarms.lock().map_err(|e| e.to_string())?;
        if store.path.as_ref().is_some_and(|old| old != &path) {
            return Err("Alarm storage is already configured".into());
        }
        store.path = Some(path);
        let paused = match store.load() {
            Ok(()) => store.ledger.as_ref().unwrap().paused,
            Err(_) => true,
        };
        drop(store);
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        state.triggers_paused = paused;
        crate::state::emit_state_updated(&self.app, &state);
        Ok(())
    }
    pub(crate) fn persist_android_trigger_pause(&self, paused: bool) -> Result<(), String> {
        if self
            .state
            .lock()
            .map_err(|e| e.to_string())?
            .capabilities
            .platform
            != "android"
        {
            return Ok(());
        }
        let mut store = self.alarms.lock().map_err(|e| e.to_string())?;
        store.load()?;
        let previous = store.ledger.as_ref().unwrap().paused;
        store.ledger.as_mut().unwrap().paused = paused;
        if let Err(error) = store.save(chrono::Utc::now().timestamp_millis()) {
            store.ledger.as_mut().unwrap().paused = previous;
            return Err(format!("Could not persist trigger pause: {error}"));
        }
        Ok(())
    }

    /// Trusted native host API, deliberately absent from editor/IPC dispatch.
    pub fn android_alarm_plan(
        &self,
        now_ms: i64,
        zone_name: &str,
        exact_access: bool,
    ) -> Result<Plan, String> {
        let zone: Tz = zone_name
            .parse()
            .map_err(|_| format!("Unsupported time zone: {zone_name}"))?;
        let now = at(zone, now_ms)?;
        self.update_host_services(vec![
            ("schedules".into(), Capability::ready()),
            (
                "exact_alarms".into(),
                if exact_access {
                    Capability::ready()
                } else {
                    Capability::unavailable(true, "Allow exact alarms in Android settings")
                },
            ),
        ])?;
        let (macros, paused) = {
            let s = self.state.lock().map_err(|e| e.to_string())?;
            if s.capabilities.platform != "android" {
                return Err("AlarmManager needs an Android host".into());
            }
            (
                scheduled_run::live_macros(&s)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>(),
                s.triggers_paused,
            )
        };
        let mut store = self.alarms.lock().map_err(|e| e.to_string())?;
        store.load()?;
        let recover = !store.recovered;
        let before = durable_shape(store.ledger.as_ref().unwrap());
        let ledger = store.ledger.as_mut().unwrap();
        let mut live = Vec::new();
        let mut plan = Plan {
            paused,
            alarms: vec![],
            errors: vec![],
        };
        for mac in macros {
            for strand in &mac.strands {
                let Some(header) = strand
                    .instructions
                    .first()
                    .map(|i| &i.kind)
                    .filter(|i| is_schedule(i))
                else {
                    continue;
                };
                live.push((mac.id.clone(), strand.id.clone()));
                let index = ledger
                    .records
                    .iter()
                    .position(|r| r.macro_id == mac.id && r.strand_id == strand.id);
                let fresh = index.is_none();
                let index = index.unwrap_or_else(|| {
                    ledger.records.push(Record {
                        macro_id: mac.id.clone(),
                        strand_id: strand.id.clone(),
                        cursor_ms: now_ms,
                        ..Default::default()
                    });
                    ledger.records.len() - 1
                });
                let record = &mut ledger.records[index];
                let resolved = resolve(&mac, header, zone);
                let blocked = if paused {
                    Some("Triggers are paused".into())
                } else if unsupported(&mac) {
                    Some(
                        "Desktop power/window schedule constraints are unavailable on Android"
                            .into(),
                    )
                } else if mac.settings.android_exact_time && !exact_access {
                    Some(
                        "Exact-time schedule is blocked: allow exact alarms in Android settings"
                            .into(),
                    )
                } else {
                    resolved.as_ref().err().cloned()
                };
                if let Some(error) = blocked {
                    if let Some(pending) = record.pending.take() {
                        if pending.alarm.at_ms <= now_ms {
                            claim(record, &pending);
                        }
                    }
                    record.cursor_ms = now_ms;
                    if !paused {
                        plan.errors.push(format!("{}: {error}", mac.name));
                    }
                    continue;
                }
                let (schedule, namespace, definition) = resolved.unwrap();
                if record.definition != definition || record.zone != zone_name {
                    record.pending = None;
                    record.definition = definition;
                    record.zone = zone_name.into();
                    // Edits replace future delivery; they never catch up an old definition.
                    if !fresh {
                        record.cursor_ms = now_ms;
                    }
                }
                if recover
                    && record
                        .pending
                        .as_ref()
                        .is_some_and(|p| p.alarm.at_ms < now_ms - 90_000)
                {
                    if let Some(pending) = record.pending.take() {
                        if !mac.settings.catch_up_missed {
                            claim(record, &pending);
                            record.cursor_ms = now_ms;
                        }
                    }
                }
                if record.pending.is_none() {
                    let from = at(zone, record.cursor_ms.max(now_ms - 7 * 24 * 3600 * 1000))?;
                    let recent = if mac.settings.catch_up_missed && !fresh {
                        schedule.latest_between(&from, &now)
                    } else if fresh && !matches!(schedule, DueSchedule::Once(_)) {
                        schedule.latest_between(
                            &at(zone, now_ms.div_euclid(60_000) * 60_000 - 1)?,
                            &now,
                        )
                    } else {
                        None
                    };
                    let due = recent
                        .filter(|t| accepted(record, &namespace, &schedule.identity(t)))
                        .or_else(|| future(&schedule, &namespace, record, now));
                    record.pending = due.map(|due| Pending {
                        identity: schedule.identity(&due),
                        namespace: namespace.clone(),
                        alarm: Alarm {
                            token: uuid::Uuid::new_v4().simple().to_string(),
                            at_ms: due.timestamp_millis(),
                            exact: mac.settings.android_exact_time,
                            macro_name: mac.name.clone(),
                        },
                    });
                    record.cursor_ms = now_ms;
                }
                if let Some(pending) = &record.pending {
                    plan.alarms.push(pending.alarm.clone());
                }
            }
        }
        for record in &mut ledger.records {
            if !live
                .iter()
                .any(|(m, s)| m == &record.macro_id && s == &record.strand_id)
            {
                if let Some(pending) = record.pending.take() {
                    if pending.alarm.at_ms <= now_ms {
                        claim(record, &pending);
                    }
                }
                record.cursor_ms = now_ms;
            }
        }
        // Cursor checkpoints are bounded to once every 30 seconds unless
        // an alarm/definition changes. Never rewrite the history every UI tick.
        let changed = before != durable_shape(ledger);
        if changed || (now_ms - store.saved_at).abs() >= 30_000 {
            store.ledger.as_mut().unwrap().last_seen_ms = now_ms;
            store.save(now_ms)?;
        }
        store.recovered = true;
        Ok(plan)
    }

    pub fn deliver_android_alarm(
        &self,
        token: &str,
        now_ms: i64,
        current_zone: &str,
        ui_ready: scheduled_run::HostGate,
    ) -> Result<bool, String> {
        let (macros, input, speed, paused, exact_access) = {
            let s = self.state.lock().map_err(|e| e.to_string())?;
            (
                scheduled_run::live_macros(&s)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>(),
                s.emulator.as_ref().map(Arc::clone),
                s.global_speed_multiplier,
                s.triggers_paused,
                s.capabilities
                    .services
                    .get("exact_alarms")
                    .is_some_and(|c| c.ready),
            )
        };
        let mut store = self.alarms.lock().map_err(|e| e.to_string())?;
        store.load()?;
        let ledger = store.ledger.as_mut().unwrap();
        let Some(index) = ledger
            .records
            .iter()
            .position(|r| r.pending.as_ref().is_some_and(|p| p.alarm.token == token))
        else {
            return Ok(false);
        };
        let pending = ledger.records[index].pending.clone().unwrap();
        if pending.alarm.at_ms > now_ms + 1000 {
            return Err("Alarm arrived before its scheduled time".into());
        }
        let record = ledger.records[index].clone();
        let mac = macros.iter().find(|m| m.id == record.macro_id);
        let strand = mac.and_then(|m| m.strands.iter().find(|s| s.id == record.strand_id));
        let zone: Tz = record
            .zone
            .parse()
            .map_err(|_| "Invalid saved alarm time zone")?;
        let valid = mac.zip(strand).filter(|(m, s)| {
            if record.zone != current_zone {
                return false;
            }

            s.instructions
                .first()
                .and_then(|h| resolve(m, &h.kind, zone).ok())
                .is_some_and(|(_, _, definition)| definition == record.definition)
        });
        let reason = if valid.is_none() {
            Some("Schedule was edited, deleted or disabled")
        } else if paused {
            Some("Triggers are paused")
        } else if pending.alarm.exact && !exact_access {
            Some("Exact alarm access was revoked")
        } else if mac.is_some_and(unsupported) {
            Some("Desktop power/window schedule constraints are unavailable")
        } else if mac.is_some_and(|m| uses_ui(m)) && !ui_ready() {
            Some(
                "UI automation skipped: unlock the device, enable accessibility and close the picker",
            )
        } else {
            None
        };
        let backup = ledger.clone();
        // Removing the opaque token rejects every duplicate. A stale edit or
        // timezone delivery must not consume a different schedule's local day.
        if valid.is_some() {
            claim(&mut ledger.records[index], &pending);
        }
        ledger.records[index].pending = None;
        ledger.records[index].cursor_ms = now_ms;
        if let Err(error) = store.save(now_ms) {
            store.ledger = Some(backup);
            return Err(error);
        }
        store.recovered = true;
        drop(store);
        let source =
            blockwork_core::run_log::Source::for_macro(&record.macro_id, &pending.alarm.macro_name);
        if let Some(reason) = reason {
            blockwork_core::run_log::error(&source, format!("Schedule skipped: {reason}"));
            return Ok(false);
        }
        let (mac, strand) = valid.unwrap();
        let Some(input) = input else { return Ok(false) };
        let body = strand.instructions[1..].to_vec();
        let needs_ui = uses_ui(mac);
        let backend = self.clone();
        let id = mac.id.clone();
        let strand_id = strand.id.clone();
        let definition = record.definition;
        let expected_zone = record.zone.clone();
        let expected_macro = mac.id.clone();
        let expected_strand = strand.id.clone();
        let gate: scheduled_run::HostGate = Arc::new(move || {
            let zone_matches = backend
                .alarms
                .lock()
                .ok()
                .and_then(|store| {
                    store.ledger.as_ref().map(|ledger| {
                        ledger.records.iter().any(|r| {
                            r.macro_id == expected_macro
                                && r.strand_id == expected_strand
                                && r.zone == expected_zone
                        })
                    })
                })
                .unwrap_or(false);
            if !zone_matches {
                return false;
            }
            if needs_ui && !ui_ready() {
                return false;
            }
            let Ok(s) = backend.state.lock() else {
                return false;
            };
            !s.triggers_paused
                && s.capabilities
                    .services
                    .get("schedules")
                    .is_some_and(|c| c.ready)
                && (!pending.alarm.exact
                    || s.capabilities
                        .services
                        .get("exact_alarms")
                        .is_some_and(|c| c.ready))
                && scheduled_run::live_macros(&s).iter().any(|m| {
                    m.id == id
                        && !unsupported(m)
                        && m.strands
                            .iter()
                            .find(|s| s.id == strand_id)
                            .and_then(|s| s.instructions.first())
                            .and_then(|h| resolve(m, &h.kind, zone).ok())
                            .is_some_and(|(_, _, current)| current == definition)
                })
        });
        // Host gates run even for logic-only schedules; their grant is only
        // needed when a body actually uses UI. Other body grants stay in runner.
        scheduled_run::fire_with_host_gate(
            mac.id.clone(),
            body,
            input,
            speed,
            &mac.variables,
            &mac.lists,
            Arc::clone(&self.state),
            self.app.clone(),
            None,
            Some((if needs_ui { "screen" } else { "" }, gate)),
        );
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockwork_core::macros::{Instruction, Strand};
    use chrono::{Datelike, Timelike};
    fn instant(text: &str) -> i64 {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .timestamp_millis()
    }
    fn backend(kind: InstructionKind) -> Backend {
        let mut state = crate::initial_state(
            &blockwork_core::config::AppSettings::default(),
            None,
            false,
            false,
        );
        state.capabilities = crate::host::Capabilities::android();
        let mut mac = Macro::new("Alarm test".into(), "".into(), vec![]);
        mac.strands = vec![Strand::with_instructions(
            0,
            0,
            vec![Instruction::new(kind)],
        )];
        state.current_macro = Some(mac);
        let (events, _) = tokio::sync::broadcast::channel(16);
        let backend = Backend {
            state: Arc::new(std::sync::Mutex::new(state)),
            app: crate::AppHandle { events },
            foreground: Arc::new(std::sync::Mutex::new(Default::default())),
            alarms: Arc::new(std::sync::Mutex::new(Default::default())),
        };
        let dir = std::env::temp_dir().join(format!("blockwork-alarms-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        backend
            .configure_android_alarms(dir.join("alarms.json"))
            .unwrap();
        backend
    }
    fn daily() -> InstructionKind {
        InstructionKind::WhenTime(
            blockwork_core::input::schedule::TimeSchedule::Daily,
            Value::text("09:30"),
        )
    }
    #[test]
    fn persisted_claims_and_clock_rollback_do_not_replay() {
        let backend = backend(daily());
        let now = instant("2026-03-10T08:00:00Z");
        let alarm = backend
            .android_alarm_plan(now, "UTC", true)
            .unwrap()
            .alarms
            .remove(0);
        assert!(
            !backend
                .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                .unwrap()
        ); // no input backend in this unit host
        assert!(
            !backend
                .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                .unwrap()
        );
        let path = backend.alarms.lock().unwrap().path.clone().unwrap();
        *backend.alarms.lock().unwrap() = Store {
            path: Some(path),
            ..Default::default()
        };
        let next = backend
            .android_alarm_plan(now, "UTC", true)
            .unwrap()
            .alarms
            .remove(0);
        assert_eq!(next.at_ms, instant("2026-03-11T09:30:00Z"));
    }
    #[test]
    fn exact_access_never_downgrades_and_revoked_tokens_are_stale() {
        let backend = backend(daily());
        backend
            .state
            .lock()
            .unwrap()
            .current_macro
            .as_mut()
            .unwrap()
            .settings
            .android_exact_time = true;
        let now = instant("2026-03-10T08:00:00Z");
        let blocked = backend.android_alarm_plan(now, "UTC", false).unwrap();
        assert!(blocked.alarms.is_empty());
        assert!(!blocked.errors.is_empty());
        let alarm = backend
            .android_alarm_plan(now, "UTC", true)
            .unwrap()
            .alarms
            .remove(0);
        assert!(alarm.exact);
        assert!(
            backend
                .android_alarm_plan(now, "UTC", false)
                .unwrap()
                .alarms
                .is_empty()
        );
        assert!(
            !backend
                .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                .unwrap()
        );
        assert_ne!(
            backend.android_alarm_plan(now, "UTC", true).unwrap().alarms[0].token,
            alarm.token
        );
    }
    #[test]
    fn recovery_catches_up_once_only_when_enabled() {
        for catch_up in [false, true] {
            let backend = backend(daily());
            backend
                .state
                .lock()
                .unwrap()
                .current_macro
                .as_mut()
                .unwrap()
                .settings
                .catch_up_missed = catch_up;
            backend
                .android_alarm_plan(instant("2026-03-10T08:00:00Z"), "UTC", true)
                .unwrap();
            let path = backend.alarms.lock().unwrap().path.clone().unwrap();
            *backend.alarms.lock().unwrap() = Store {
                path: Some(path),
                ..Default::default()
            };
            let next = backend
                .android_alarm_plan(instant("2026-03-12T12:00:00Z"), "UTC", true)
                .unwrap()
                .alarms
                .remove(0);
            assert_eq!(
                next.at_ms,
                instant(if catch_up {
                    "2026-03-12T09:30:00Z"
                } else {
                    "2026-03-13T09:30:00Z"
                })
            );
        }
    }
    #[test]
    fn edits_disable_old_broadcasts_and_pausing_does_not_catch_up() {
        let backend = backend(daily());
        let now = instant("2026-03-10T08:00:00Z");
        let alarm = backend
            .android_alarm_plan(now, "UTC", true)
            .unwrap()
            .alarms
            .remove(0);
        backend
            .state
            .lock()
            .unwrap()
            .current_macro
            .as_mut()
            .unwrap()
            .strands[0]
            .instructions[0]
            .kind = InstructionKind::WhenAt(Value::text("2026-03-12 12:00"));
        assert!(
            !backend
                .deliver_android_alarm(&alarm.token, alarm.at_ms, "UTC", Arc::new(|| true))
                .unwrap()
        );
        assert_eq!(
            backend
                .android_alarm_plan(alarm.at_ms, "UTC", true)
                .unwrap()
                .alarms[0]
                .at_ms,
            instant("2026-03-12T12:00:00Z")
        );
        backend.state.lock().unwrap().triggers_paused = true;
        assert!(
            backend
                .android_alarm_plan(instant("2026-03-12T13:00:00Z"), "UTC", true)
                .unwrap()
                .alarms
                .is_empty()
        );
        backend.state.lock().unwrap().triggers_paused = false;
        assert!(
            backend
                .android_alarm_plan(instant("2026-03-12T13:00:00Z"), "UTC", true)
                .unwrap()
                .alarms
                .is_empty()
        );
    }
    #[test]
    fn dst_gaps_and_folds_have_stable_local_occurrence_ids() {
        let zone: Tz = "America/Chicago".parse().unwrap();
        let spring = DueSchedule::Time(blockwork_core::input::schedule::TimeSchedule::Daily, 2, 30);
        let next = spring
            .next_after(&at(zone, instant("2026-03-08T00:00:00-06:00")).unwrap())
            .unwrap();
        assert_eq!((next.day(), next.hour(), next.minute()), (9, 2, 30));
        let backend = backend(InstructionKind::WhenTime(
            blockwork_core::input::schedule::TimeSchedule::Daily,
            Value::text("01:30"),
        ));
        let now = instant("2026-11-01T00:00:00-05:00");
        let first = backend
            .android_alarm_plan(now, "America/Chicago", true)
            .unwrap()
            .alarms
            .remove(0);
        assert_eq!(first.at_ms, instant("2026-11-01T01:30:00-05:00"));
        backend
            .deliver_android_alarm(
                &first.token,
                first.at_ms,
                "America/Chicago",
                Arc::new(|| true),
            )
            .unwrap();
        let next = backend
            .android_alarm_plan(first.at_ms + 1, "America/Chicago", true)
            .unwrap()
            .alarms
            .remove(0);
        assert_eq!(next.at_ms, instant("2026-11-02T01:30:00-06:00"));
        let cron: blockwork_core::input::cron::Cron = "30 1 * * *".parse().unwrap();
        let after = at(zone, instant("2026-11-01T01:45:00-05:00")).unwrap();
        assert_eq!(
            cron.next_after(&after).unwrap().timestamp_millis(),
            instant("2026-11-01T01:30:00-06:00")
        );
    }
    #[test]
    fn unreconciled_timezone_and_time_edits_cannot_fire_or_consume_new_occurrences() {
        let backend = backend(daily());
        let now = instant("2026-03-10T08:00:00Z");
        let old = backend
            .android_alarm_plan(now, "UTC", true)
            .unwrap()
            .alarms
            .remove(0);
        assert!(
            !backend
                .deliver_android_alarm(&old.token, old.at_ms, "America/Chicago", Arc::new(|| true))
                .unwrap()
        );
        let new = backend
            .android_alarm_plan(old.at_ms, "America/Chicago", true)
            .unwrap()
            .alarms
            .remove(0);
        assert_eq!(new.at_ms, instant("2026-03-10T09:30:00-05:00"));
        backend
            .state
            .lock()
            .unwrap()
            .current_macro
            .as_mut()
            .unwrap()
            .strands[0]
            .instructions[0]
            .kind = InstructionKind::WhenTime(
            blockwork_core::input::schedule::TimeSchedule::Daily,
            Value::text("10:00"),
        );
        assert!(
            !backend
                .deliver_android_alarm(&new.token, new.at_ms, "America/Chicago", Arc::new(|| true))
                .unwrap()
        );
        assert_eq!(
            backend
                .android_alarm_plan(new.at_ms, "America/Chicago", true)
                .unwrap()
                .alarms[0]
                .at_ms,
            instant("2026-03-10T10:00:00-05:00")
        );
    }

    #[test]
    fn emergency_pause_survives_restart_and_requires_explicit_resume() {
        let backend = backend(daily());
        let now = instant("2026-03-10T08:00:00Z");
        backend.android_alarm_plan(now, "UTC", true).unwrap();
        backend.persist_android_trigger_pause(true).unwrap();
        let path = backend.alarms.lock().unwrap().path.clone().unwrap();
        *backend.alarms.lock().unwrap() = Store {
            path: Some(path.clone()),
            ..Default::default()
        };
        backend.configure_android_alarms(path).unwrap();
        let plan = backend.android_alarm_plan(now + 1, "UTC", true).unwrap();
        assert!(plan.paused);
        assert!(plan.alarms.is_empty());
        backend.persist_android_trigger_pause(false).unwrap();
        backend.state.lock().unwrap().triggers_paused = false;
        assert_eq!(
            backend
                .android_alarm_plan(now + 2, "UTC", true)
                .unwrap()
                .alarms
                .len(),
            1
        );
    }

    #[test]
    fn timezone_changes_rearm_wall_time_and_reject_old_tokens() {
        let backend = backend(daily());
        let now = instant("2026-03-10T08:00:00Z");
        let old = backend
            .android_alarm_plan(now, "UTC", true)
            .unwrap()
            .alarms
            .remove(0);
        let new = backend
            .android_alarm_plan(now, "America/Chicago", true)
            .unwrap()
            .alarms
            .remove(0);
        assert_eq!(new.at_ms, instant("2026-03-10T09:30:00-05:00"));
        assert_ne!(old.token, new.token);
        assert!(
            !backend
                .deliver_android_alarm(&old.token, new.at_ms, "UTC", Arc::new(|| true))
                .unwrap()
        );
        assert!(
            backend
                .deliver_android_alarm(&new.token, new.at_ms - 60000, "UTC", Arc::new(|| true))
                .is_err()
        );
        assert_eq!(
            backend
                .android_alarm_plan(now, "America/Chicago", true)
                .unwrap()
                .alarms[0]
                .token,
            new.token
        );
    }

    #[test]
    fn invalid_live_reporters_and_corrupt_history_block_scheduling() {
        let backend = backend(InstructionKind::WhenCron(Value::op(
            Op::Ext("HttpGet".into()),
            vec![Value::text("https://invalid.test")],
        )));
        assert!(
            !backend
                .android_alarm_plan(instant("2026-03-10T08:00:00Z"), "UTC", true)
                .unwrap()
                .errors
                .is_empty()
        );
        for header in [
            InstructionKind::WhenTime(
                blockwork_core::input::schedule::TimeSchedule::Daily,
                Value::op(
                    Op::Ext("HttpGet".into()),
                    vec![Value::text("https://invalid.test")],
                ),
            ),
            InstructionKind::WhenAt(Value::op(
                Op::Ext("HttpGet".into()),
                vec![Value::text("https://invalid.test")],
            )),
        ] {
            backend
                .state
                .lock()
                .unwrap()
                .current_macro
                .as_mut()
                .unwrap()
                .strands[0]
                .instructions[0]
                .kind = header;
            assert!(
                backend
                    .android_alarm_plan(instant("2026-03-10T08:00:00Z"), "UTC", true)
                    .unwrap()
                    .errors
                    .iter()
                    .any(|error| error.contains("live reporters"))
            );
        }

        let mut store = backend.alarms.lock().unwrap();
        let path = store.path.clone().unwrap();
        std::fs::write(&path, "broken").unwrap();
        *store = Store {
            path: Some(path),
            ..Default::default()
        };
        drop(store);
        assert!(
            backend
                .android_alarm_plan(instant("2026-03-10T08:00:00Z"), "UTC", true)
                .is_err()
        );
    }
    #[test]
    fn monthly_short_months_leap_years_and_catch_up_bound() {
        let zone: Tz = "UTC".parse().unwrap();
        let monthly = DueSchedule::Time(
            blockwork_core::input::schedule::TimeSchedule::Monthly { day: 31 },
            9,
            0,
        );
        assert_eq!(
            monthly
                .next_after(&at(zone, instant("2026-04-01T00:00:00Z")).unwrap())
                .unwrap()
                .timestamp_millis(),
            instant("2026-05-31T09:00:00Z")
        );
        let yearly = DueSchedule::Time(
            blockwork_core::input::schedule::TimeSchedule::Yearly { month: 2, day: 29 },
            9,
            0,
        );
        assert_eq!(
            yearly
                .next_after(&at(zone, instant("2026-04-01T00:00:00Z")).unwrap())
                .unwrap()
                .year(),
            2028
        );
        let once = DueSchedule::Once(instant("2026-03-01T09:00:00Z"));
        assert!(
            once.latest_between(
                &at(zone, instant("2026-02-01T00:00:00Z")).unwrap(),
                &at(zone, instant("2026-03-10T00:00:00Z")).unwrap()
            )
            .is_none()
        );
    }
}
