//! `TimeSchedule` - the payload of `InstructionKind::WhenTime`: a recurring
//! point in local time (daily, weekly, monthly, or yearly) plus a
//! time-of-day. Backs the background time watcher (e.g. src-tauri's
//! `time_watch` module), the same way `InstructionKind::WhenBatteryDischargedTo`/
//! `WhenBatteryChargedTo` back the battery watcher.

use chrono::{DateTime, Datelike, Local, Timelike};
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

/// A recurring point in local time - the payload of `InstructionKind::WhenTime`.
/// `hour`/`minute` are always 24-hour (0-23/0-59); which clock format the UI
/// shows them in is purely a display concern, not part of the saved shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TimeSchedule {
    /// Every day at `hour:minute`.
    Daily { hour: u8, minute: u8 },
    /// Every week, on `weekday`, at `hour:minute`.
    Weekly { weekday: Weekday, hour: u8, minute: u8 },
    /// Every month, on the `day`th (1-31) at `hour:minute`. A month shorter
    /// than `day` just never matches that month - no clamping/rollover.
    Monthly { day: u8, hour: u8, minute: u8 },
    /// Every year, on `month`/`day` (1-12/1-31) at `hour:minute`.
    Yearly { month: u8, day: u8, hour: u8, minute: u8 },
}

impl TimeSchedule {
    pub fn hour(&self) -> u8 {
        match *self {
            TimeSchedule::Daily { hour, .. }
            | TimeSchedule::Weekly { hour, .. }
            | TimeSchedule::Monthly { hour, .. }
            | TimeSchedule::Yearly { hour, .. } => hour,
        }
    }

    pub fn minute(&self) -> u8 {
        match *self {
            TimeSchedule::Daily { minute, .. }
            | TimeSchedule::Weekly { minute, .. }
            | TimeSchedule::Monthly { minute, .. }
            | TimeSchedule::Yearly { minute, .. } => minute,
        }
    }

    /// True if `now` (local time) satisfies both this schedule's recurrence
    /// field (weekday/day-of-month/month+day) and its `hour:minute` - i.e.
    /// this is a minute this schedule should fire on. Doesn't know or care
    /// whether it already *has* fired for this exact occurrence; callers
    /// (the time watcher) dedup that themselves so a schedule fires once per
    /// matching minute, not once per poll tick within it.
    pub fn matches(&self, now: &DateTime<Local>) -> bool {
        if now.hour() as u8 != self.hour() || now.minute() as u8 != self.minute() {
            return false;
        }
        match self {
            TimeSchedule::Daily { .. } => true,
            TimeSchedule::Weekly { weekday, .. } => weekday.matches(now.weekday()),
            TimeSchedule::Monthly { day, .. } => now.day() as u8 == *day,
            TimeSchedule::Yearly { month, day, .. } => now.month() as u8 == *month && now.day() as u8 == *day,
        }
    }
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
        let sched = TimeSchedule::Daily { hour: 9, minute: 30 };
        assert!(sched.matches(&at(2026, 3, 5, 9, 30)));
        assert!(sched.matches(&at(2026, 11, 1, 9, 30)));
        assert!(!sched.matches(&at(2026, 3, 5, 9, 31)));
        assert!(!sched.matches(&at(2026, 3, 5, 10, 30)));
    }

    #[test]
    fn weekly_matches_only_the_chosen_weekday() {
        // 2026-03-05 is a Thursday.
        let sched = TimeSchedule::Weekly { weekday: Weekday::Thursday, hour: 8, minute: 0 };
        assert!(sched.matches(&at(2026, 3, 5, 8, 0)));
        assert!(!sched.matches(&at(2026, 3, 6, 8, 0)));
    }

    #[test]
    fn monthly_matches_only_the_chosen_day_of_month() {
        let sched = TimeSchedule::Monthly { day: 15, hour: 12, minute: 0 };
        assert!(sched.matches(&at(2026, 3, 15, 12, 0)));
        assert!(sched.matches(&at(2026, 4, 15, 12, 0)));
        assert!(!sched.matches(&at(2026, 3, 16, 12, 0)));
    }

    #[test]
    fn monthly_never_matches_a_shorter_month() {
        let sched = TimeSchedule::Monthly { day: 31, hour: 0, minute: 0 };
        assert!(!sched.matches(&at(2026, 4, 30, 0, 0)));
        assert!(sched.matches(&at(2026, 5, 31, 0, 0)));
    }

    #[test]
    fn yearly_matches_only_the_chosen_month_and_day() {
        let sched = TimeSchedule::Yearly { month: 12, day: 25, hour: 0, minute: 0 };
        assert!(sched.matches(&at(2026, 12, 25, 0, 0)));
        assert!(sched.matches(&at(2027, 12, 25, 0, 0)));
        assert!(!sched.matches(&at(2026, 12, 24, 0, 0)));
        assert!(!sched.matches(&at(2026, 11, 25, 0, 0)));
    }
}
