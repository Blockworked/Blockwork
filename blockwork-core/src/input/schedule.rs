//! `TimeSchedule` - the recurrence of `InstructionKind::WhenTime`: daily,
//! weekly, monthly, or yearly, beside a time-of-day value. Backs the
//! background time watcher (e.g. blockwork-app's `time_watch` module), the same way `InstructionKind::WhenBatteryDischargedTo`/
//! `WhenBatteryChargedTo` back the battery watcher.

use chrono::{DateTime, Datelike, Timelike};
#[cfg(test)]
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl Weekday {
    fn matches(&self, day: chrono::Weekday) -> bool {
        matches!(
            (self, day),
            (Weekday::Sunday, chrono::Weekday::Sun)
                | (Weekday::Monday, chrono::Weekday::Mon)
                | (Weekday::Tuesday, chrono::Weekday::Tue)
                | (Weekday::Wednesday, chrono::Weekday::Wed)
                | (Weekday::Thursday, chrono::Weekday::Thu)
                | (Weekday::Friday, chrono::Weekday::Fri)
                | (Weekday::Saturday, chrono::Weekday::Sat)
        )
    }
}

/// The recurrence part of `InstructionKind::WhenTime`: which days it fires
/// on. The time of day is a separate value (see `parse_time_of_day`), so a
/// variable or reporter can supply it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TimeSchedule {
    /// Every day.
    Daily,
    /// Every week, on `weekday`.
    Weekly { weekday: Weekday },
    /// Every month, on the `day`th (1-31). A month shorter than `day` just
    /// never matches that month - no clamping/rollover.
    Monthly { day: u8 },
    /// Every year, on `month`/`day` (1-12/1-31).
    Yearly { month: u8, day: u8 },
}

/// The shape `WhenTime` was saved in before its time became a value: the
/// recurrence with a fixed 24-hour `hour`/`minute` beside it.
#[derive(Deserialize)]
pub(crate) struct LegacyTimeSchedule {
    #[serde(flatten)]
    pub schedule: TimeSchedule,
    pub hour: u8,
    pub minute: u8,
}

impl TimeSchedule {
    /// True if `now` (local time) is on one of this schedule's days and its
    /// clock reads `hour:minute` - i.e. this is a minute it should fire on.
    /// Doesn't know or care whether it already *has* fired for this exact
    /// occurrence; callers (the time watcher) dedup that themselves so a
    /// schedule fires once per matching minute, not once per poll tick within it.
    pub fn matches<Tz: chrono::TimeZone>(&self, now: &DateTime<Tz>, hour: u8, minute: u8) -> bool {
        if now.hour() as u8 != hour || now.minute() as u8 != minute {
            return false;
        }
        match self {
            TimeSchedule::Daily => true,
            TimeSchedule::Weekly { weekday } => weekday.matches(now.weekday()),
            TimeSchedule::Monthly { day } => now.day() as u8 == *day,
            TimeSchedule::Yearly { month, day } => now.month() as u8 == *month && now.day() as u8 == *day,
        }
    }
}

/// Reads a time of day like `09:30`, `9:30`, `21:05`, `9` or `9:30 pm` as a
/// 24-hour `(hour, minute)`. Seconds (`09:30:00`) are allowed and ignored.
pub fn parse_time_of_day(text: &str) -> Result<(u8, u8), String> {
    let invalid = || format!("\"{}\" isn't a time of day like 09:30", text.trim());
    let lower = text.trim().to_ascii_lowercase();
    let (clock, meridiem) = if let Some(rest) = lower.strip_suffix("am").or_else(|| lower.strip_suffix("a.m.")) {
        (rest.trim_end(), Some(false))
    } else if let Some(rest) = lower.strip_suffix("pm").or_else(|| lower.strip_suffix("p.m.")) {
        (rest.trim_end(), Some(true))
    } else {
        (lower.as_str(), None)
    };
    let mut parts = clock.split([':', '.']);
    let number = |part: Option<&str>| -> Result<Option<u32>, String> {
        match part {
            None => Ok(None),
            Some(p) if !p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit()) => Ok(p.parse().ok()),
            Some(_) => Err(invalid()),
        }
    };
    let hour = number(parts.next())?.ok_or_else(invalid)?;
    let minute = number(parts.next())?.unwrap_or(0);
    let second = number(parts.next())?.unwrap_or(0);
    if parts.next().is_some() || minute > 59 || second > 59 {
        return Err(invalid());
    }
    let hour = match meridiem {
        None if hour <= 23 => hour,
        Some(pm) if (1..=12).contains(&hour) => hour % 12 + if pm { 12 } else { 0 },
        _ => return Err(invalid()),
    };
    Ok((hour as u8, minute as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn daily_matches_any_date_at_the_right_time() {
        let sched = TimeSchedule::Daily;
        assert!(sched.matches(&at(2026, 3, 5, 9, 30), 9, 30));
        assert!(sched.matches(&at(2026, 11, 1, 9, 30), 9, 30));
        assert!(!sched.matches(&at(2026, 3, 5, 9, 31), 9, 30));
        assert!(!sched.matches(&at(2026, 3, 5, 10, 30), 9, 30));
    }

    #[test]
    fn weekly_matches_only_the_chosen_weekday() {
        // 2026-03-05 is a Thursday.
        let sched = TimeSchedule::Weekly { weekday: Weekday::Thursday };
        assert!(sched.matches(&at(2026, 3, 5, 8, 0), 8, 0));
        assert!(!sched.matches(&at(2026, 3, 6, 8, 0), 8, 0));
    }

    #[test]
    fn monthly_matches_only_the_chosen_day_of_month() {
        let sched = TimeSchedule::Monthly { day: 15 };
        assert!(sched.matches(&at(2026, 3, 15, 12, 0), 12, 0));
        assert!(sched.matches(&at(2026, 4, 15, 12, 0), 12, 0));
        assert!(!sched.matches(&at(2026, 3, 16, 12, 0), 12, 0));
    }

    #[test]
    fn monthly_never_matches_a_shorter_month() {
        let sched = TimeSchedule::Monthly { day: 31 };
        assert!(!sched.matches(&at(2026, 4, 30, 0, 0), 0, 0));
        assert!(sched.matches(&at(2026, 5, 31, 0, 0), 0, 0));
    }

    #[test]
    fn yearly_matches_only_the_chosen_month_and_day() {
        let sched = TimeSchedule::Yearly { month: 12, day: 25 };
        assert!(sched.matches(&at(2026, 12, 25, 0, 0), 0, 0));
        assert!(sched.matches(&at(2027, 12, 25, 0, 0), 0, 0));
        assert!(!sched.matches(&at(2026, 12, 24, 0, 0), 0, 0));
        assert!(!sched.matches(&at(2026, 11, 25, 0, 0), 0, 0));
    }

    #[test]
    fn reads_times_of_day() {
        assert_eq!(parse_time_of_day("09:30"), Ok((9, 30)));
        assert_eq!(parse_time_of_day(" 9:05 "), Ok((9, 5)));
        assert_eq!(parse_time_of_day("21"), Ok((21, 0)));
        assert_eq!(parse_time_of_day("23:59:30"), Ok((23, 59)));
        assert_eq!(parse_time_of_day("9:30 pm"), Ok((21, 30)));
        assert_eq!(parse_time_of_day("12am"), Ok((0, 0)));
        assert_eq!(parse_time_of_day("12:15 PM"), Ok((12, 15)));
        assert!(parse_time_of_day("24:00").is_err());
        assert!(parse_time_of_day("9:60").is_err());
        assert!(parse_time_of_day("13pm").is_err());
        assert!(parse_time_of_day("noon").is_err());
        assert!(parse_time_of_day("").is_err());
    }

    #[test]
    fn reads_the_old_saved_shape() {
        let old: LegacyTimeSchedule = serde_json::from_str(r#"{"kind":"Weekly","weekday":"Friday","hour":7,"minute":45}"#).unwrap();
        assert_eq!(old.schedule, TimeSchedule::Weekly { weekday: Weekday::Friday });
        assert_eq!((old.hour, old.minute), (7, 45));
    }
}
