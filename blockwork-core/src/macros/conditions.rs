//! Per-macro conditions: when triggers and hotkeys may start a macro.

use super::MacroSettings;
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PowerCondition {
    #[default]
    Any,
    PluggedIn,
    OnBattery,
}

impl MacroSettings {
    pub fn has_conditions(&self) -> bool {
        !self.only_in_window.trim().is_empty() || self.power != PowerCondition::Any || self.active_hours.is_some() || self.active_days != 0
    }

    /// Why a trigger or hotkey can't start the macro right now, or `None` if it can.
    pub fn blocked_by(&self) -> Option<String> {
        if !self.has_conditions() {
            return None;
        }
        self.check(&Local::now(), crate::battery::is_plugged_in, crate::window::active)
    }

    fn check(
        &self,
        now: &DateTime<Local>,
        plugged_in: impl FnOnce() -> bool,
        window: impl FnOnce() -> Result<crate::window::ActiveWindow, String>,
    ) -> Option<String> {
        if self.active_days != 0 && self.active_days & (1 << now.weekday().num_days_from_sunday()) == 0 {
            return Some("not on this day of the week".into());
        }
        if let Some((from, until)) = self.active_hours {
            let minute = (now.hour() * 60 + now.minute()) as u16;
            // A range like 22:00-06:00 runs past midnight.
            let inside = if from <= until { minute >= from && minute < until } else { minute >= from || minute < until };
            if !inside {
                return Some(format!("only runs from {} to {}", clock(from), clock(until)));
            }
        }
        if self.power != PowerCondition::Any {
            let plugged_in = plugged_in();
            if self.power == PowerCondition::PluggedIn && !plugged_in {
                return Some("only runs while plugged in".into());
            }
            if self.power == PowerCondition::OnBattery && plugged_in {
                return Some("only runs on battery".into());
            }
        }
        let pattern = self.only_in_window.trim();
        if !pattern.is_empty() {
            let focused = window().map(|w| w.matches(pattern, crate::window::MatchBy::Any)).unwrap_or(false);
            if !focused {
                return Some(format!("only runs while '{pattern}' is focused"));
            }
        }
        None
    }
}

/// Minutes after midnight as `HH:MM`.
pub fn clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// `HH:MM` (or `H:MM`) as minutes after midnight.
pub fn parse_clock(text: &str) -> Result<u16, String> {
    let (h, m) = text.trim().split_once(':').ok_or_else(|| format!("'{text}' isn't a time like 09:30"))?;
    let (h, m): (u16, u16) = (h.parse().map_err(|_| format!("'{text}' isn't a time"))?, m.parse().map_err(|_| format!("'{text}' isn't a time"))?);
    if h > 23 || m > 59 {
        return Err(format!("'{text}' isn't a time of day"));
    }
    Ok(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::ActiveWindow;
    use chrono::TimeZone;

    fn at(d: u32, h: u32, mi: u32) -> DateTime<Local> {
        // 2026-10-15 is a Thursday.
        Local.with_ymd_and_hms(2026, 10, d, h, mi, 0).unwrap()
    }

    fn window(app: &str) -> Result<ActiveWindow, String> {
        Ok(ActiveWindow { title: String::new(), app: app.into() })
    }

    #[test]
    fn each_condition_can_block() {
        let mut s = MacroSettings::default();
        assert!(!s.has_conditions());
        assert_eq!(s.check(&at(15, 3, 0), || false, || window("x")), None);

        s.active_hours = Some((parse_clock("22:00").unwrap(), parse_clock("6:00").unwrap()));
        assert_eq!(s.check(&at(15, 23, 30), || true, || window("x")), None);
        assert_eq!(s.check(&at(15, 5, 59), || true, || window("x")), None);
        assert_eq!(s.check(&at(15, 6, 0), || true, || window("x")).unwrap(), "only runs from 22:00 to 06:00");

        s.active_hours = None;
        s.active_days = 1 << 4; // Thursday
        assert_eq!(s.check(&at(15, 12, 0), || true, || window("x")), None);
        assert!(s.check(&at(16, 12, 0), || true, || window("x")).is_some());

        s.active_days = 0;
        s.power = PowerCondition::OnBattery;
        assert!(s.check(&at(15, 12, 0), || true, || window("x")).is_some());
        assert_eq!(s.check(&at(15, 12, 0), || false, || window("x")), None);

        s.power = PowerCondition::Any;
        s.only_in_window = "Firefox".into();
        assert_eq!(s.check(&at(15, 12, 0), || true, || window("firefox")), None);
        assert!(s.check(&at(15, 12, 0), || true, || window("code")).is_some());
        assert!(s.check(&at(15, 12, 0), || true, || Err("no window".into())).is_some());
    }

    #[test]
    fn clock_times() {
        assert_eq!(parse_clock(" 9:05 "), Ok(545));
        assert_eq!(clock(545), "09:05");
        assert!(parse_clock("24:00").is_err());
        assert!(parse_clock("noon").is_err());
    }
}
