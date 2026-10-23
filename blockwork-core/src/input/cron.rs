//! Cron expressions for `WhenCron`: the usual five fields (minute, hour,
//! day of month, month, day of week) with `*`, lists, ranges, steps and
//! names, plus the `@hourly`-style shortcuts.

use chrono::{DateTime, Datelike, Duration, Local, Timelike};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cron {
    minutes: u64,
    hours: u64,
    days: u64,
    months: u64,
    weekdays: u64,
    // Standard cron: when both day fields are restricted, either may match.
    days_restricted: bool,
    weekdays_restricted: bool,
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const WEEKDAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

impl std::str::FromStr for Cron {
    type Err = String;

    fn from_str(text: &str) -> Result<Cron, String> {
        let text = text.trim();
        let expanded = match text.to_ascii_lowercase().as_str() {
            "@yearly" | "@annually" => "0 0 1 1 *".to_string(),
            "@monthly" => "0 0 1 * *".to_string(),
            "@weekly" => "0 0 * * 0".to_string(),
            "@daily" | "@midnight" => "0 0 * * *".to_string(),
            "@hourly" => "0 * * * *".to_string(),
            other if other.starts_with('@') => return Err(format!("unknown schedule '{text}'")),
            _ => text.to_string(),
        };
        let fields: Vec<&str> = expanded.split_whitespace().collect();
        let [minute, hour, day, month, weekday] = fields[..] else {
            return Err(format!(
                "'{text}' needs 5 parts: minute hour day month weekday (like '*/15 9-17 * * mon-fri')"
            ));
        };
        let mut weekdays = field(weekday, 0, 7, &WEEKDAYS, "weekday")?;
        // 7 is Sunday too.
        if weekdays & (1 << 7) != 0 {
            weekdays = (weekdays | 1) & !(1 << 7);
        }
        let cron = Cron {
            minutes: field(minute, 0, 59, &[], "minute")?,
            hours: field(hour, 0, 23, &[], "hour")?,
            days: field(day, 1, 31, &[], "day")?,
            months: field(month, 1, 12, &MONTHS, "month")?,
            weekdays,
            days_restricted: !day.starts_with('*'),
            weekdays_restricted: !weekday.starts_with('*'),
        };
        // Days only count alone when the weekday is `*`; then a day no picked
        // month has (like 31 2, February 31st) would never come.
        if cron.days_restricted && !cron.weekdays_restricted {
            const LONGEST: [u32; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            let possible = (1..=12u32).any(|m| has(cron.months, m) && (1..=LONGEST[m as usize - 1]).any(|d| has(cron.days, d)));
            if !possible {
                return Err(format!("'{text}' never runs: none of its months has that day"));
            }
        }
        Ok(cron)
    }
}

/// One field as a bitmask of the values it allows.
fn field(text: &str, min: u32, max: u32, names: &[&str], what: &str) -> Result<u64, String> {
    let value = |part: &str| -> Result<u32, String> {
        let lower = part.to_ascii_lowercase();
        // Names count from `min` (jan = 1, sun = 0).
        if let Some(i) = names.iter().position(|n| *n == lower) {
            return Ok(i as u32 + min);
        }
        let n: u32 = part.parse().map_err(|_| format!("'{part}' isn't a valid {what}"))?;
        if n < min || n > max {
            return Err(format!("{what} {n} is out of range ({min}-{max})"));
        }
        Ok(n)
    };
    let mut mask = 0u64;
    for item in text.split(',') {
        let (range, step) = match item.split_once('/') {
            Some((range, step)) => {
                let step: u32 = step.parse().map_err(|_| format!("'{step}' isn't a valid step"))?;
                if step == 0 {
                    return Err("a step can't be 0".into());
                }
                (range, step)
            }
            None => (item, 1),
        };
        let (from, to) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (value(a)?, value(b)?)
        } else {
            let start = value(range)?;
            // "5/10" means from 5 to the end.
            (start, if item.contains('/') { max } else { start })
        };
        if from > to {
            return Err(format!("'{range}' runs backwards"));
        }
        mask |= (from..=to).step_by(step as usize).fold(0, |m, v| m | 1 << v);
    }
    Ok(mask)
}

impl Cron {
    /// Whether `at`'s minute is one this schedule fires on.
    pub fn matches(&self, at: &DateTime<Local>) -> bool {
        has(self.minutes, at.minute()) && has(self.hours, at.hour()) && has(self.months, at.month()) && self.day_matches(at)
    }

    fn day_matches(&self, at: &DateTime<Local>) -> bool {
        let day = has(self.days, at.day());
        let weekday = has(self.weekdays, at.weekday().num_days_from_sunday());
        if self.days_restricted && self.weekdays_restricted { day || weekday } else { day && weekday }
    }

    /// The first minute after `at` this fires on, within about four years.
    pub fn next_after(&self, at: &DateTime<Local>) -> Option<DateTime<Local>> {
        let mut t = at.with_second(0)?.with_nanosecond(0)? + Duration::minutes(1);
        let end = t + Duration::days(366 * 4 + 1);
        while t < end {
            // Skip whole months, days and hours that can't match.
            if !has(self.months, t.month()) {
                let (y, m) = if t.month() == 12 { (t.year() + 1, 1) } else { (t.year(), t.month() + 1) };
                t = local_midnight(y, m, 1)?;
            } else if !self.day_matches(&t) {
                let next = t.date_naive().succ_opt()?;
                t = local_midnight(next.year(), next.month(), next.day())?;
            } else if !has(self.hours, t.hour()) {
                t = t.with_minute(0)? + Duration::hours(1);
            } else if has(self.minutes, t.minute()) {
                return Some(t);
            } else {
                t += Duration::minutes(1);
            }
        }
        None
    }
}

fn has(mask: u64, value: u32) -> bool {
    mask & (1 << value) != 0
}

fn local_midnight(year: i32, month: u32, day: u32) -> Option<DateTime<Local>> {
    use chrono::TimeZone;
    Local.with_ymd_and_hms(year, month, day, 0, 0, 0).earliest()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    fn cron(text: &str) -> Cron {
        text.parse().unwrap()
    }

    #[test]
    fn plain_fields_and_steps() {
        let every_15 = cron("*/15 9-17 * * mon-fri");
        // 2026-10-15 is a Thursday.
        assert!(every_15.matches(&at(2026, 10, 15, 9, 0)));
        assert!(every_15.matches(&at(2026, 10, 15, 17, 45)));
        assert!(!every_15.matches(&at(2026, 10, 15, 9, 10)));
        assert!(!every_15.matches(&at(2026, 10, 15, 18, 0)));
        assert!(!every_15.matches(&at(2026, 10, 17, 9, 0)));

        let list = cron("5,35 */6 1 jan,JUL *");
        assert!(list.matches(&at(2026, 7, 1, 18, 35)));
        assert!(!list.matches(&at(2026, 7, 1, 19, 35)));
        assert!(!list.matches(&at(2026, 8, 1, 18, 35)));
    }

    #[test]
    fn day_fields_combine_like_cron() {
        // Either the 13th or any Friday.
        let either = cron("0 0 13 * fri");
        assert!(either.matches(&at(2026, 10, 13, 0, 0)));
        assert!(either.matches(&at(2026, 10, 16, 0, 0)));
        assert!(!either.matches(&at(2026, 10, 15, 0, 0)));
        // Sunday as 7.
        assert!(cron("0 12 * * 7").matches(&at(2026, 10, 18, 12, 0)));
    }

    #[test]
    fn shortcuts_and_errors() {
        assert_eq!(cron("@daily"), cron("0 0 * * *"));
        assert!(cron("@hourly").matches(&at(2026, 1, 1, 7, 0)));
        // A weekday makes it run on those days anyway.
        assert!("0 0 31 2 fri".parse::<Cron>().is_ok());
        for bad in ["", "* * * *", "60 * * * *", "*/0 * * * *", "5-1 * * * *", "* * * smarch *", "@sometimes", "0 0 31 2 *", "0 0 30-31 feb *"] {
            assert!(bad.parse::<Cron>().is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn finds_the_next_run() {
        let c = cron("30 8 * * mon");
        assert_eq!(c.next_after(&at(2026, 10, 15, 12, 0)), Some(at(2026, 10, 19, 8, 30)));
        assert_eq!(cron("0 0 29 2 *").next_after(&at(2026, 3, 1, 0, 0)), Some(at(2028, 2, 29, 0, 0)));
        assert_eq!(cron("* * * * *").next_after(&at(2026, 1, 1, 0, 0)), Some(at(2026, 1, 1, 0, 1)));
    }
}
