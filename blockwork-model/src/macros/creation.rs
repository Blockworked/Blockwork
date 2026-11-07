use super::{Macro, clamp_speed};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Options {
    title: Option<String>,
    speed_multiplier: Option<f64>,
    max_run_secs: u32,
    max_loop_iterations: u32,
    always_listen: bool,
    catch_up_missed: bool,
    only_in_window: String,
    power: super::conditions::PowerCondition,
    active_from: String,
    active_until: String,
    active_days: u8,
}

/// Build the complete draft before either host saves or selects it.
pub fn from_args(args: &serde_json::Value) -> Result<Macro, String> {
    let options: Options = if args.is_null() {
        Options::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|e| e.to_string())?
    };
    let title = options.title.unwrap_or_else(|| "New Macro".into());
    let title = title.trim();
    if title.is_empty() {
        return Err("Enter a title for the macro".into());
    }
    let mut mac = Macro::new(title.into(), String::new(), vec![]);
    mac.speed_multiplier = clamp_speed(options.speed_multiplier.unwrap_or(1.0));
    mac.settings.max_run_secs = options.max_run_secs;
    mac.settings.max_loop_iterations = options.max_loop_iterations;
    mac.settings.always_listen = options.always_listen;
    mac.settings.catch_up_missed = options.catch_up_missed;
    mac.settings.set_conditions(
        &options.only_in_window,
        options.power,
        &options.active_from,
        &options.active_until,
        options.active_days,
    )?;
    Ok(mac)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn creation_defaults_and_options() {
        let default = from_args(&json!({})).unwrap();
        assert_eq!(default.name, "New Macro");
        assert_eq!(
            from_args(&serde_json::Value::Null).unwrap().settings,
            default.settings
        );
        assert_eq!(default.speed_multiplier, 1.0);
        assert_eq!(default.settings, super::super::MacroSettings::default());
        let mac = from_args(&json!({"title": "  Work  ", "speedMultiplier": 2.5,
            "maxRunSecs": 30, "maxLoopIterations": 100, "alwaysListen": true,
            "catchUpMissed": true, "onlyInWindow": "Editor", "power": "PluggedIn",
            "activeFrom": "09:00", "activeUntil": "17:00", "activeDays": 62}))
        .unwrap();
        assert_eq!(mac.settings.active_hours, Some((540, 1020)));
        assert_eq!(mac.settings.active_days, 62);
        assert_eq!(mac.settings.only_in_window, "Editor");
        assert_eq!(mac.name, "Work");
        assert_eq!(mac.speed_multiplier, 2.5);
        assert_eq!(mac.settings.max_run_secs, 30);
        assert_eq!(mac.settings.max_loop_iterations, 100);
        assert!(mac.settings.always_listen && mac.settings.catch_up_missed);
        assert!(from_args(&json!({"title": "   "})).is_err());
        assert!(from_args(&json!({"maxRunSecs": -1})).is_err());
        assert!(from_args(&json!({"activeFrom": "09:00"})).is_err());
        let fast = from_args(&json!({"speedMultiplier": 1000})).unwrap();
        assert_eq!(fast.speed_multiplier, clamp_speed(1000.0));
    }
}
