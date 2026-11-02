//! Plain-text names for trigger headers, for the run log's "Trigger fired" line.

use super::InstructionKind;
use crate::input::schedule::{TimeSchedule, Weekday};
use crate::input::types::{Direction, MacroButton};
use crate::input::value::Value;

/// A literal as written, quoted when it's text; a variable by name; anything
/// else as "(reporter)".
fn show(value: &Value) -> String {
    match value {
        Value::Text { value } => format!("\"{value}\""),
        Value::Number { value } => value.to_string(),
        Value::Var { name } => name.clone(),
        _ => "(reporter)".to_string(),
    }
}

/// `show`, but text unquoted: times and schedules read better bare.
fn bare(value: &Value) -> String {
    match value {
        Value::Text { value } => value.clone(),
        v => show(v),
    }
}

fn weekday(day: Weekday) -> &'static str {
    match day {
        Weekday::Sunday => "Sunday",
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
    }
}

fn button(b: &MacroButton) -> String {
    match b {
        MacroButton::Left => "left button".into(),
        MacroButton::Right => "right button".into(),
        MacroButton::Middle => "middle button".into(),
        MacroButton::Back => "back button".into(),
        MacroButton::Forward => "forward button".into(),
        MacroButton::ScrollUp => "scroll up".into(),
        MacroButton::ScrollDown => "scroll down".into(),
        MacroButton::ScrollLeft => "scroll left".into(),
        MacroButton::ScrollRight => "scroll right".into(),
        MacroButton::Other(n) => format!("button {n}"),
    }
}

impl InstructionKind {
    /// What a trigger header reads as, like `when key "F8" is pressed`.
    /// `None` for blocks that aren't watcher triggers.
    pub fn trigger_label(&self) -> Option<String> {
        use InstructionKind as K;
        Some(match self {
            K::WhenBatteryDischargedTo(v) => format!("when battery drops to {}%", show(v)),
            K::WhenBatteryChargedTo(v) => format!("when battery charges to {}%", show(v)),
            K::WhenTime(schedule, time) => {
                let when = match schedule {
                    TimeSchedule::Daily => "every day".to_string(),
                    TimeSchedule::Weekly { weekday: d } => format!("every {}", weekday(*d)),
                    TimeSchedule::Monthly { day } => format!("every month on day {day}"),
                    TimeSchedule::Yearly { month, day } => format!("every year on {month}/{day}"),
                };
                format!("{when} at {}", bare(time))
            }
            K::WhenPowerPluggedIn => "when power is plugged in".into(),
            K::WhenPowerUnplugged => "when power is unplugged".into(),
            K::WhenClipboardChanged => "when clipboard changes".into(),
            K::WhenKey(key, dir) => {
                let dir = if *dir == Direction::Release { "released" } else { "pressed" };
                format!("when key {} is {dir}", show(key))
            }
            K::WhenWindow { pattern, focused } => {
                format!("when window {} is {}", show(pattern), if *focused { "focused" } else { "unfocused" })
            }
            K::WhenFileChanged(path) => format!("when file {} changes", show(path)),
            K::WhenTyped { text, .. } => format!("when I type {}", show(text)),
            K::WhenApp { pattern, started } => format!("when app {} {}", show(pattern), if *started { "starts" } else { "exits" }),
            K::WhenIdle(minutes) => format!("when idle for {} minutes", show(minutes)),
            K::WhenScreenLock { locked } => format!("when screen is {}", if *locked { "locked" } else { "unlocked" }),
            K::WhenCron(expr) => format!("on schedule {}", bare(expr)),
            K::WhenNetwork { connected } => format!("when network {}", if *connected { "connects" } else { "disconnects" }),
            K::WhenUsb { pattern, connected } => {
                let device = match pattern {
                    Value::Text { value } if value.trim().is_empty() => "any USB device".to_string(),
                    p => format!("USB device {}", show(p)),
                };
                format!("when {device} is {}", if *connected { "plugged in" } else { "unplugged" })
            }
            K::WhenMouseButton { button: b, pressed, modifiers } => {
                let with = match modifiers {
                    Some(Value::Text { value }) if value.trim().is_empty() => String::new(),
                    Some(m) => format!(" with {}", show(m)),
                    None => String::new(),
                };
                format!("when {} is {}{with}", button(b), if *pressed { "pressed" } else { "released" })
            }
            K::WhenAt(at) => format!("once at {}", bare(at)),
            K::WhenSignal(name) => format!("when I receive {}", show(&name.value)),
            K::WhenWebhook { path, .. } => format!("when webhook {} is called", show(path)),
            _ => return None,
        })
    }
}

/// The run log line for a fired trigger.
pub fn fired_message(trigger: Option<&InstructionKind>) -> String {
    match trigger.and_then(InstructionKind::trigger_label) {
        Some(label) => format!("Trigger fired: {label}"),
        None => "Trigger fired".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_trigger() {
        let key = InstructionKind::WhenKey(Value::text("F8"), Direction::Press);
        assert_eq!(fired_message(Some(&key)), "Trigger fired: when key \"F8\" is pressed");
        let time = InstructionKind::WhenTime(TimeSchedule::Weekly { weekday: Weekday::Monday }, Value::text("09:30"));
        assert_eq!(time.trigger_label().unwrap(), "every Monday at 09:30");
        let usb = InstructionKind::WhenUsb { pattern: Value::text(""), connected: false };
        assert_eq!(usb.trigger_label().unwrap(), "when any USB device is unplugged");
        assert_eq!(InstructionKind::WhenIdle(Value::number(5.0)).trigger_label().unwrap(), "when idle for 5 minutes");
        assert_eq!(fired_message(None), "Trigger fired");
        assert_eq!(InstructionKind::WhenRan.trigger_label(), None);
    }
}
