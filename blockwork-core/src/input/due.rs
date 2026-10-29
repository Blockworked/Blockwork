//! Shared recurrence calculation for desktop polling and native alarm hosts.
use super::{
    cron::Cron,
    schedule::{TimeSchedule, parse_time_of_day},
    value::{Evaluated, Value},
};
use crate::macros::InstructionKind;
use chrono::{DateTime, Duration, TimeZone};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum DueSchedule {
    Time(TimeSchedule, u8, u8),
    Cron(Cron),
    Once(i64),
}
impl DueSchedule {
    pub fn resolve<Tz: TimeZone>(
        header: &InstructionKind,
        env: &HashMap<String, Evaluated>,
        zone: &Tz,
    ) -> Result<Option<Self>, String> {
        let text = |value: &Value| value.resolve_vars(env).eval_text();
        Ok(Some(match header {
            InstructionKind::WhenTime(schedule, value) => {
                let invalid = match schedule {
                    TimeSchedule::Monthly { day } => !(1..=31).contains(day),
                    TimeSchedule::Yearly { month, day } => {
                        chrono::NaiveDate::from_ymd_opt(2000, (*month).into(), (*day).into())
                            .is_none()
                    }
                    _ => false,
                };
                if invalid {
                    return Err("Schedule has an invalid month or day".into());
                }
                let (h, m) = parse_time_of_day(&text(value)?)?;
                Self::Time(*schedule, h, m)
            }
            InstructionKind::WhenCron(value) => Self::Cron(text(value)?.parse()?),
            InstructionKind::WhenAt(value) => Self::Once(
                crate::text::parse_date_in_zone(&text(value)?, "", zone)?.floor() as i64 * 1000,
            ),
            _ => return Ok(None),
        }))
    }
    /// First occurrence strictly after a moment. Nonexistent wall minutes
    /// during a spring-forward gap are skipped; repeated minutes are real
    /// candidates, with duplicate local identities suppressed by the host.
    pub fn next_after<Tz: TimeZone>(&self, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        match self {
            Self::Once(ms) => after
                .timezone()
                .timestamp_millis_opt(*ms)
                .single()
                .filter(|at| at > after),
            Self::Cron(cron) => cron.next_after(after),
            Self::Time(schedule, hour, minute) => {
                let zone = after.timezone();
                let mut date = after.date_naive();
                for _ in 0..366 * 8 {
                    let local = date.and_hms_opt((*hour).into(), (*minute).into(), 0)?;
                    let wall = zone.from_local_datetime(&local);
                    let mut candidates = [wall.clone().earliest(), wall.latest()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    candidates.sort();
                    if let Some(at) = candidates
                        .into_iter()
                        .find(|at| at > after && schedule.matches(at, *hour, *minute))
                    {
                        return Some(at);
                    }
                    date = date.succ_opt()?;
                }
                None
            }
        }
    }
    /// Most recent occurrence in (from, to], bounded to one week, matching
    /// the desktop catch-up policy. Iterating UTC minutes handles DST gaps.
    pub fn latest_between<Tz: TimeZone>(
        &self,
        from: &DateTime<Tz>,
        to: &DateTime<Tz>,
    ) -> Option<DateTime<Tz>> {
        let start = from.clone().max(to.clone() - Duration::days(7));
        if let Self::Once(ms) = self {
            return to
                .timezone()
                .timestamp_millis_opt(*ms)
                .single()
                .filter(|at| at > &start && at <= to);
        }
        let mut at = to
            .timezone()
            .timestamp_opt(to.timestamp().div_euclid(60) * 60, 0)
            .single()?;
        while at > start {
            let matches = match self {
                Self::Time(s, h, m) => s.matches(&at, *h, *m),
                Self::Cron(c) => c.matches(&at),
                _ => false,
            };
            if matches {
                return Some(at);
            }
            at -= Duration::minutes(1);
        }
        None
    }
    pub fn identity<Tz: TimeZone>(&self, at: &DateTime<Tz>) -> String {
        match self {
            Self::Time(..) => at.naive_local().format("%Y-%m-%d").to_string(),
            Self::Cron(_) => at.naive_local().format("%Y-%m-%dT%H:%M").to_string(),
            Self::Once(_) => "done".into(),
        }
    }
}
