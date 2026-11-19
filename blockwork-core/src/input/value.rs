//! The reporters that read the machine. The value system and the pure
//! reporters are `blockwork_model::input::value`, re-exported here.

pub use blockwork_model::input::value::*;

/// Blockwork's own operators, in blockstitch's extension shape. They read
/// live system state, so none takes operands - see [`crate::battery`] and
/// [`crate::clipboard`]. The wire names are what they serialized as before,
/// so saves keep loading.
static OPERATORS: &[ExtOperator] = &[
    ExtOperator {
        kind: "NativeUiExists",
        op: "NativeUiExists",
        arity: 1,
        default_args: || vec![Value::text("scope=Focused;name=Save;role=push button")],
        eval: |args| {
            crate::permissions::require("ui_automation")?;
            crate::accessibility::query(
                &crate::input::ui::UiSelector::parse(&args[0].as_text())?,
                crate::accessibility::Query::Exists,
            )
        },
    },
    ExtOperator {
        kind: "NativeUiEnabled",
        op: "NativeUiEnabled",
        arity: 1,
        default_args: || vec![Value::text("scope=Focused;name=Save;role=push button")],
        eval: |args| {
            crate::permissions::require("ui_automation")?;
            crate::accessibility::query(
                &crate::input::ui::UiSelector::parse(&args[0].as_text())?,
                crate::accessibility::Query::Enabled,
            )
        },
    },
    ExtOperator {
        kind: "NativeUiText",
        op: "NativeUiText",
        arity: 1,
        default_args: || vec![Value::text("scope=Focused;name=Save;role=push button")],
        eval: |args| {
            crate::permissions::require("ui_automation")?;
            crate::accessibility::query(
                &crate::input::ui::UiSelector::parse(&args[0].as_text())?,
                crate::accessibility::Query::Text,
            )
        },
    },
    ExtOperator {
        kind: "NativeUiValue",
        op: "NativeUiValue",
        arity: 1,
        default_args: || vec![Value::text("scope=Focused;name=Save;role=push button")],
        eval: |args| {
            crate::permissions::require("ui_automation")?;
            crate::accessibility::query(
                &crate::input::ui::UiSelector::parse(&args[0].as_text())?,
                crate::accessibility::Query::Value,
            )
        },
    },
    ExtOperator {
        kind: "GamepadConnected",
        op: "GamepadConnected",
        arity: 1,
        default_args: || vec![Value::text("")],
        eval: |args| {
            crate::permissions::require("input")?;
            let controllers = crate::input::controllers::list()?;
            Ok(Evaluated::Bool(
                crate::input::controllers::select(&controllers, &args[0].as_text())?.is_some(),
            ))
        },
    },
    ExtOperator {
        kind: "GamepadButtonDown",
        op: "GamepadButtonDown",
        arity: 2,
        default_args: || vec![Value::text(""), Value::text("South")],
        eval: |args| {
            crate::permissions::require("input")?;
            let control: crate::input::gamepad::GamepadButton =
                serde_json::from_value(serde_json::json!(args[1].as_text()))
                    .map_err(|_| "Unknown controller button")?;
            let controllers = crate::input::controllers::list()?;
            Ok(Evaluated::Bool(
                crate::input::controllers::select(&controllers, &args[0].as_text())?
                    .and_then(|g| g.buttons.get(&control))
                    .copied()
                    .unwrap_or(false),
            ))
        },
    },
    ExtOperator {
        kind: "GamepadAxisValue",
        op: "GamepadAxisValue",
        arity: 2,
        default_args: || vec![Value::text(""), Value::text("LeftX")],
        eval: |args| {
            crate::permissions::require("input")?;
            let control: crate::input::gamepad::GamepadAxis =
                serde_json::from_value(serde_json::json!(args[1].as_text()))
                    .map_err(|_| "Unknown controller axis")?;
            let controllers = crate::input::controllers::list()?;
            let value = crate::input::controllers::select(&controllers, &args[0].as_text())?
                .and_then(|g| g.axes.get(&control))
                .copied()
                .unwrap_or(0.0);
            Ok(Evaluated::Number(crate::input::gamepad::deadzone(
                value,
                crate::config::load_settings()
                    .controller_deadzone
                    .unwrap_or(0.12),
            )))
        },
    },
    ExtOperator {
        kind: "BatteryPercentage",
        op: "BatteryPercentage",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Number(crate::battery::percentage()?)),
    },
    ExtOperator {
        kind: "PluggedIn",
        op: "PluggedIn",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::battery::is_plugged_in())),
    },
    ExtOperator {
        kind: "ClipboardText",
        op: "ClipboardText",
        arity: 0,
        default_args: Vec::new,
        eval: |_| {
            crate::permissions::require("clipboard")?;
            Ok(Evaluated::Text(crate::clipboard::get_text()?))
        },
    },
    ExtOperator {
        kind: "ClipboardHasImage",
        op: "ClipboardHasImage",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::clipboard::has_image())),
    },
    ExtOperator {
        kind: "ClipboardHasFiles",
        op: "ClipboardHasFiles",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::clipboard::has_file_list())),
    },
    ExtOperator {
        kind: "ReadFile",
        op: "ReadFile",
        arity: 1,
        default_args: || vec![Value::text("~/notes.txt")],
        eval: |args| {
            crate::permissions::require("files_read")?;
            Ok(Evaluated::Text(crate::files::read_text(
                &args[0].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "FileExists",
        op: "FileExists",
        arity: 1,
        default_args: || vec![Value::text("~/notes.txt")],
        eval: |args| Ok(Evaluated::Bool(crate::files::exists(&args[0].as_text()))),
    },
    ExtOperator {
        kind: "ListDirectory",
        op: "ListDirectory",
        arity: 1,
        default_args: || vec![Value::text("~")],
        eval: |args| {
            crate::permissions::require("files_read")?;
            Ok(Evaluated::Text(crate::files::list_dir(&args[0].as_text())?))
        },
    },
    ExtOperator {
        kind: "FileInfo",
        op: "FileInfo",
        arity: 2,
        default_args: || vec![Value::text("size"), Value::text("~/notes.txt")],
        eval: |args| {
            crate::permissions::require("files_read")?;
            let path = args[1].as_text();
            Ok(Evaluated::Number(match args[0].as_text().as_str() {
                "modified" => crate::files::modified(&path)?,
                _ => crate::files::size(&path)?,
            }))
        },
    },
    ExtOperator {
        kind: "HttpGet",
        op: "HttpGet",
        arity: 1,
        default_args: || vec![Value::text("https://example.com")],
        eval: |args| {
            crate::permissions::require("network")?;
            Ok(Evaluated::Text(crate::http::get(&args[0].as_text())?))
        },
    },
    ExtOperator {
        kind: "JsonGet",
        op: "JsonGet",
        arity: 2,
        default_args: || vec![Value::text("name"), Value::text(r#"{"name": "Blockwork"}"#)],
        eval: |args| {
            Ok(Evaluated::Text(crate::http::json_get(
                &args[1].as_text(),
                &args[0].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "ShellOutput",
        op: "ShellOutput",
        arity: 1,
        default_args: || vec![Value::text("echo hello")],
        eval: |args| {
            crate::permissions::require("commands")?;
            Ok(Evaluated::Text(
                crate::shell::run(&args[0].as_text())?.stdout,
            ))
        },
    },
    ExtOperator {
        kind: "ShellSucceeded",
        op: "ShellSucceeded",
        arity: 1,
        default_args: || vec![Value::text("true")],
        eval: |args| {
            crate::permissions::require("commands")?;
            Ok(Evaluated::Bool(
                crate::shell::run(&args[0].as_text())?.success,
            ))
        },
    },
    ExtOperator {
        kind: "CommandExitCode",
        op: "CommandExitCode",
        arity: 1,
        default_args: || vec![Value::text("true")],
        eval: |args| {
            crate::permissions::require("commands")?;
            Ok(Evaluated::Number(f64::from(
                crate::shell::run(&args[0].as_text())?.code,
            )))
        },
    },
    ExtOperator {
        kind: "ActiveWindowTitle",
        op: "ActiveWindowTitle",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Text(crate::window::active()?.title)),
    },
    ExtOperator {
        kind: "ActiveWindowApp",
        op: "ActiveWindowApp",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Text(crate::window::active()?.app)),
    },
    ExtOperator {
        kind: "WindowExists",
        op: "WindowExists",
        arity: 1,
        default_args: || vec![Value::text("Firefox")],
        eval: |args| Ok(Evaluated::Bool(crate::window::exists(&args[0].as_text())?)),
    },
    ExtOperator {
        kind: "WindowInfo",
        op: "WindowInfo",
        arity: 2,
        default_args: || vec![Value::text("Firefox"), Value::text("center x")],
        eval: |args| {
            let w = crate::window::bounds(&args[0].as_text(), crate::window::MatchBy::Any)?;
            Ok(Evaluated::Number(match args[1].as_text().as_str() {
                "x" => w.x,
                "y" => w.y,
                "width" => w.width,
                "height" => w.height,
                "center y" => w.y + w.height / 2.0,
                _ => w.x + w.width / 2.0,
            }))
        },
    },
    ExtOperator {
        kind: "Detection",
        op: "Detection",
        arity: 2,
        default_args: || {
            let item = Value::op(
                Op::from_name("ListItem"),
                vec![Value::number(1.0), Value::text("detections")],
            );
            vec![Value::text("center x"), item]
        },
        eval: |args| {
            crate::detect::Detection::from_item(&args[1].as_text())?.field(&args[0].as_text())
        },
    },
    ExtOperator {
        kind: "MacroRunning",
        op: "MacroRunning",
        arity: 1,
        default_args: one_text,
        eval: |args| {
            Ok(Evaluated::Bool(crate::macros::calls::is_running(
                &args[0].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "GlobalVariable",
        op: "GlobalVariable",
        arity: 1,
        default_args: one_text,
        eval: |args| {
            crate::permissions::require("globals")?;
            Ok(crate::globals::get(&args[0].as_text()))
        },
    },
    ExtOperator {
        kind: "PixelColor",
        op: "PixelColor",
        arity: 2,
        default_args: || numbers(&[100.0, 100.0]),
        eval: |args| {
            crate::permissions::require("screen")?;
            let (ox, oy) = crate::screen::origin()?;
            Ok(Evaluated::Text(crate::screen::pixel_color(
                args[0].as_number()? + ox,
                args[1].as_number()? + oy,
            )?))
        },
    },
    ExtOperator {
        kind: "AverageColor",
        op: "AverageColor",
        arity: 4,
        default_args: || numbers(&[0.0, 0.0, 100.0, 100.0]),
        eval: |args| {
            crate::permissions::require("screen")?;
            let n = |i: usize| args[i].as_number();
            let (ox, oy) = crate::screen::origin()?;
            Ok(Evaluated::Text(crate::screen::average_color(
                n(0)? + ox,
                n(1)? + oy,
                n(2)?,
                n(3)?,
            )?))
        },
    },
    ExtOperator {
        kind: "ColorsMatch",
        op: "ColorsMatch",
        arity: 3,
        default_args: || {
            vec![
                Value::text("#ffffff"),
                Value::text("#fafafa"),
                Value::number(10.0),
            ]
        },
        eval: |args| {
            let a = crate::screen::parse_color(&args[0].as_text())?;
            let b = crate::screen::parse_color(&args[1].as_text())?;
            Ok(Evaluated::Bool(
                crate::screen::color_distance(a, b) <= args[2].as_number()?,
            ))
        },
    },
    ExtOperator {
        kind: "ScreenText",
        op: "ScreenText",
        arity: 4,
        default_args: || numbers(&[0.0, 0.0, 400.0, 100.0]),
        eval: |args| {
            crate::permissions::require("screen")?;
            let n = |i: usize| args[i].as_number();
            let (ox, oy) = crate::screen::origin()?;
            Ok(Evaluated::Text(crate::screen::read_text(
                n(0)? + ox,
                n(1)? + oy,
                n(2)?,
                n(3)?,
            )?))
        },
    },
    ExtOperator {
        kind: "MousePosition",
        op: "MousePosition",
        arity: 1,
        default_args: || vec![Value::text("x")],
        eval: |args| {
            let (x, y) = crate::screen::cursor()?;
            let (ox, oy) = crate::screen::origin()?;
            Ok(Evaluated::Number(if args[0].as_text() == "y" {
                y as f64 - oy
            } else {
                x as f64 - ox
            }))
        },
    },
    ExtOperator {
        kind: "KeyHeld",
        op: "KeyHeld",
        arity: 1,
        default_args: || vec![Value::text("Shift")],
        eval: |args| {
            crate::permissions::require("input")?;
            let combo = crate::input::key_names::parse_trigger_combo(&args[0].as_text());
            if combo.is_empty() {
                return Err(format!("no key in '{}'", args[0].as_text()));
            }
            Ok(Evaluated::Bool(crate::recording::key_rules::combo_is_held(
                &combo,
            )))
        },
    },
    ExtOperator {
        kind: "ScreenSize",
        op: "ScreenSize",
        arity: 1,
        default_args: || vec![Value::text("width")],
        eval: |args| {
            let desktop = crate::screen::desktop()?;
            Ok(Evaluated::Number(if args[0].as_text() == "height" {
                desktop.height
            } else {
                desktop.width
            }))
        },
    },
    ExtOperator {
        kind: "MonitorCount",
        op: "MonitorCount",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Number(crate::screen::monitors().len() as f64)),
    },
    ExtOperator {
        kind: "MonitorInfo",
        op: "MonitorInfo",
        arity: 2,
        default_args: || vec![Value::number(1.0), Value::text("width")],
        eval: |args| {
            let monitors = crate::screen::monitors();
            let n = args[0].as_number()?;
            let m = monitors
                .get((n.round() as usize).wrapping_sub(1))
                .ok_or_else(|| format!("there's no monitor {n} ({} found)", monitors.len()))?;
            Ok(Evaluated::Number(match args[1].as_text().as_str() {
                "x" => m.x,
                "y" => m.y,
                "height" => m.height,
                _ => m.width,
            }))
        },
    },
    ExtOperator {
        kind: "AppRunning",
        op: "AppRunning",
        arity: 1,
        default_args: || vec![Value::text("firefox")],
        eval: |args| {
            Ok(Evaluated::Bool(crate::system::app_running(
                &args[0].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "WindowList",
        op: "WindowList",
        arity: 0,
        default_args: Vec::new,
        eval: |_| {
            let titles: Vec<String> = crate::window::list()?
                .into_iter()
                .map(|w| w.title)
                .filter(|t| !t.trim().is_empty())
                .collect();
            Ok(Evaluated::Text(titles.join("\n")))
        },
    },
    ExtOperator {
        kind: "WindowPid",
        op: "WindowPid",
        arity: 1,
        default_args: || vec![Value::text("Firefox")],
        eval: |args| {
            Ok(Evaluated::Number(f64::from(crate::window::pid(
                &args[0].as_text(),
                crate::window::MatchBy::Any,
            )?)))
        },
    },
    ExtOperator {
        kind: "AppPid",
        op: "AppPid",
        arity: 1,
        default_args: || vec![Value::text("firefox")],
        eval: |args| {
            Ok(Evaluated::Number(
                crate::system::app_pid(&args[0].as_text())?.map_or(-1.0, f64::from),
            ))
        },
    },
    ExtOperator {
        kind: "SystemInfo",
        op: "SystemInfo",
        arity: 1,
        default_args: || vec![Value::text("user name")],
        eval: |args| Ok(Evaluated::Text(crate::system::info(&args[0].as_text())?)),
    },
    ExtOperator {
        kind: "EnvVar",
        op: "EnvVar",
        arity: 1,
        default_args: || vec![Value::text("HOME")],
        eval: |args| {
            Ok(Evaluated::Text(
                std::env::var(args[0].as_text().trim()).unwrap_or_default(),
            ))
        },
    },
    ExtOperator {
        kind: "Volume",
        op: "Volume",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Number(crate::sound::volume()?)),
    },
    ExtOperator {
        kind: "Muted",
        op: "Muted",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::sound::muted()?)),
    },
    ExtOperator {
        kind: "IdleSeconds",
        op: "IdleSeconds",
        arity: 0,
        default_args: Vec::new,
        eval: |_| {
            Ok(Evaluated::Number(
                crate::system::idle_time()?.as_secs_f64().floor(),
            ))
        },
    },
    ExtOperator {
        kind: "ScreenLocked",
        op: "ScreenLocked",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::system::screen_locked()?)),
    },
    ExtOperator {
        kind: "NetworkConnected",
        op: "NetworkConnected",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::system::network_connected()?)),
    },
    ExtOperator {
        kind: "UsbConnected",
        op: "UsbConnected",
        arity: 1,
        default_args: one_text,
        eval: |args| {
            let pattern = args[0].as_text();
            Ok(Evaluated::Bool(
                crate::system::usb_devices()?
                    .iter()
                    .any(|d| d.matches(&pattern)),
            ))
        },
    },
    ExtOperator {
        kind: "Secret",
        op: "Secret",
        arity: 1,
        default_args: || vec![Value::text("api token")],
        eval: |args| {
            crate::permissions::require("secrets")?;
            Ok(Evaluated::Text(crate::secrets::get(&args[0].as_text())?))
        },
    },
    ExtOperator {
        kind: "BrowserConnected",
        op: "BrowserConnected",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Bool(crate::browser::connected())),
    },
    ExtOperator {
        kind: "PageUrl",
        op: "PageUrl",
        arity: 0,
        default_args: Vec::new,
        eval: |_| {
            Ok(Evaluated::Text(crate::browser::text_field(
                &browser("info", serde_json::json!({}))?,
                "url",
            )))
        },
    },
    ExtOperator {
        kind: "PageTitle",
        op: "PageTitle",
        arity: 0,
        default_args: Vec::new,
        eval: |_| {
            Ok(Evaluated::Text(crate::browser::text_field(
                &browser("info", serde_json::json!({}))?,
                "title",
            )))
        },
    },
    ExtOperator {
        kind: "ElementText",
        op: "ElementText",
        arity: 1,
        default_args: || vec![Value::text("h1")],
        eval: |args| {
            let read = browser(
                "read",
                serde_json::json!({ "selector": args[0].as_text(), "wait": 0 }),
            )?;
            Ok(Evaluated::Text(crate::browser::text_field(&read, "value")))
        },
    },
    ExtOperator {
        kind: "ElementExists",
        op: "ElementExists",
        arity: 1,
        default_args: || vec![Value::text("text=Sign in")],
        eval: |args| Ok(Evaluated::Bool(element_count(&args[0].as_text())? > 0.0)),
    },
    ExtOperator {
        kind: "ElementCount",
        op: "ElementCount",
        arity: 1,
        default_args: || vec![Value::text("button")],
        eval: |args| Ok(Evaluated::Number(element_count(&args[0].as_text())?)),
    },
];

/// A browser reporter's request, behind the browser permission.
fn browser(cmd: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    crate::permissions::require("browser")?;
    crate::browser::request(cmd, args, std::time::Duration::from_secs(10))
}

fn element_count(selector: &str) -> Result<f64, String> {
    Ok(
        browser("count", serde_json::json!({ "selector": selector }))?["count"]
            .as_f64()
            .unwrap_or(0.0),
    )
}

static LATE_OPERATORS: &[ExtOperator] = &[ExtOperator {
    kind: "LastError",
    op: "LastError",
    arity: 0,
    default_args: Vec::new,
    eval: |_| Ok(Evaluated::Text(crate::macros::runner::last_error())),
}];

fn numbers(values: &[f64]) -> Vec<Value> {
    values.iter().map(|&v| Value::number(v)).collect()
}

fn one_text() -> Vec<Value> {
    vec![Value::text("")]
}

/// Teaches `blockstitch` about Blockwork's operators, in palette order.
/// Idempotent; called from [`crate::init`], which every entry point runs
/// before touching a macro.
pub fn register_blockwork_operators() {
    register_operators(OPERATORS);
    register_text_operators();
    register_operators(LATE_OPERATORS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_reporter_reads_list_items() {
        register_blockwork_operators();
        let item = r#"{"x":10,"y":20,"w":30,"h":40,"label":"player","score":0.5,"id":7}"#;
        let op = |field: &str, item: &str| {
            Value::op(
                Op::from_name("Detection"),
                vec![Value::text(field), Value::text(item)],
            )
            .eval()
        };
        assert_eq!(op("center x", item), Ok(Evaluated::Number(25.0)));
        assert_eq!(op("label", item), Ok(Evaluated::Text("player".into())));
        assert!(
            op("x", "").is_err(),
            "an empty item (list index out of range) is an error"
        );
    }

    #[test]
    fn battery_operators_evaluate_once_registered() {
        register_blockwork_operators();
        // Whether a battery exists depends on the machine, so this only
        // asserts the operator is wired up rather than erroring as unknown.
        let plugged_in = Value::op(Op::from_name("PluggedIn"), vec![]);
        assert!(plugged_in.eval().is_ok());
        let percentage = Value::op(Op::from_name("BatteryPercentage"), vec![]);
        match percentage.eval() {
            Ok(Evaluated::Number(n)) => assert!((0.0..=100.0).contains(&n)),
            Ok(other) => panic!("expected a number, got {other:?}"),
            // A desktop with no battery at all - a real answer, not a
            // missing-operator error.
            Err(e) => assert!(!e.contains("unknown operator"), "{e}"),
        }
    }
}
