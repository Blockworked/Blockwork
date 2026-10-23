//! Blockwork's view of the value system. The tree and its evaluation live
//! in `blockstitch-core`; the reporter blocks Blockwork adds are here.

pub use blockstitch_core::value::*;

/// Blockwork's own operators, in blockstitch's extension shape. They read
/// live system state, so none takes operands - see [`crate::battery`] and
/// [`crate::clipboard`]. The wire names are what they serialized as before,
/// so saves keep loading.
#[cfg(feature = "system")]
static OPERATORS: &[ExtOperator] = &[
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
            Ok(Evaluated::Text(crate::files::read_text(&args[0].as_text())?))
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
            Ok(Evaluated::Text(crate::http::json_get(&args[1].as_text(), &args[0].as_text())?))
        },
    },
    ExtOperator {
        kind: "ShellOutput",
        op: "ShellOutput",
        arity: 1,
        default_args: || vec![Value::text("echo hello")],
        eval: |args| {
            crate::permissions::require("commands")?;
            Ok(Evaluated::Text(crate::shell::run(&args[0].as_text())?.stdout))
        },
    },
    ExtOperator {
        kind: "ShellSucceeded",
        op: "ShellSucceeded",
        arity: 1,
        default_args: || vec![Value::text("true")],
        eval: |args| {
            crate::permissions::require("commands")?;
            Ok(Evaluated::Bool(crate::shell::run(&args[0].as_text())?.success))
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
            let item = Value::op(Op::from_name("ListItem"), vec![Value::number(1.0), Value::text("detections")]);
            vec![Value::text("center x"), item]
        },
        eval: |args| crate::detect::Detection::from_item(&args[1].as_text())?.field(&args[0].as_text()),
    },
    ExtOperator {
        kind: "MacroRunning",
        op: "MacroRunning",
        arity: 1,
        default_args: one_text,
        eval: |args| Ok(Evaluated::Bool(crate::macros::calls::is_running(&args[0].as_text())?)),
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
            Ok(Evaluated::Text(crate::screen::pixel_color(args[0].as_number()?, args[1].as_number()?)?))
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
            Ok(Evaluated::Text(crate::screen::average_color(n(0)?, n(1)?, n(2)?, n(3)?)?))
        },
    },
    ExtOperator {
        kind: "ColorsMatch",
        op: "ColorsMatch",
        arity: 3,
        default_args: || vec![Value::text("#ffffff"), Value::text("#fafafa"), Value::number(10.0)],
        eval: |args| {
            let a = crate::screen::parse_color(&args[0].as_text())?;
            let b = crate::screen::parse_color(&args[1].as_text())?;
            Ok(Evaluated::Bool(crate::screen::color_distance(a, b) <= args[2].as_number()?))
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
            Ok(Evaluated::Text(crate::screen::read_text(n(0)?, n(1)?, n(2)?, n(3)?)?))
        },
    },
    ExtOperator {
        kind: "MousePosition",
        op: "MousePosition",
        arity: 1,
        default_args: || vec![Value::text("x")],
        eval: |args| {
            let (x, y) = crate::screen::cursor()?;
            Ok(Evaluated::Number(if args[0].as_text() == "y" { y } else { x } as f64))
        },
    },
    ExtOperator {
        kind: "ScreenSize",
        op: "ScreenSize",
        arity: 1,
        default_args: || vec![Value::text("width")],
        eval: |args| {
            let desktop = crate::screen::desktop()?;
            Ok(Evaluated::Number(if args[0].as_text() == "height" { desktop.height } else { desktop.width }))
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
        eval: |args| Ok(Evaluated::Bool(crate::system::app_running(&args[0].as_text())?)),
    },
    ExtOperator {
        kind: "IdleSeconds",
        op: "IdleSeconds",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Number(crate::system::idle_time()?.as_secs_f64().floor())),
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
            Ok(Evaluated::Bool(crate::system::usb_devices()?.iter().any(|d| d.matches(&pattern))))
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
];

/// Pure text and date reporters: no machine state, so the website's
/// editor (built without `system`) registers them too.
static TEXT_OPERATORS: &[ExtOperator] = &[
    ExtOperator {
        kind: "TextCheck",
        op: "TextCheck",
        arity: 3,
        default_args: || vec![Value::text("hello world"), Value::text("contains"), Value::text("world")],
        eval: |args| Ok(Evaluated::Bool(crate::text::check(&args[0].as_text(), &args[1].as_text(), &args[2].as_text())?)),
    },
    ExtOperator {
        kind: "TextReplace",
        op: "TextReplace",
        arity: 3,
        default_args: || vec![Value::text("world"), Value::text("hello world"), Value::text("there")],
        eval: |args| Ok(Evaluated::Text(args[1].as_text().replace(&args[0].as_text(), &args[2].as_text()))),
    },
    ExtOperator {
        kind: "RegexReplace",
        op: "RegexReplace",
        arity: 3,
        default_args: || vec![Value::text("[0-9]+"), Value::text("order 1234"), Value::text("#")],
        eval: |args| {
            Ok(Evaluated::Text(crate::text::regex_replace(&args[0].as_text(), &args[1].as_text(), &args[2].as_text())?))
        },
    },
    ExtOperator {
        kind: "RegexFind",
        op: "RegexFind",
        arity: 2,
        default_args: || vec![Value::text("[0-9]+"), Value::text("order 1234")],
        eval: |args| Ok(Evaluated::Text(crate::text::regex_find(&args[0].as_text(), &args[1].as_text())?)),
    },
    ExtOperator {
        kind: "SplitPart",
        op: "SplitPart",
        arity: 3,
        default_args: || vec![Value::number(1.0), Value::text("a,b,c"), Value::text(",")],
        eval: |args| {
            Ok(Evaluated::Text(crate::text::split_part(&args[1].as_text(), &args[2].as_text(), args[0].as_number()?)?))
        },
    },
    ExtOperator {
        kind: "SplitCount",
        op: "SplitCount",
        arity: 2,
        default_args: || vec![Value::text("a,b,c"), Value::text(",")],
        eval: |args| Ok(Evaluated::Number(crate::text::split(&args[0].as_text(), &args[1].as_text()).len() as f64)),
    },
    ExtOperator {
        kind: "Substring",
        op: "Substring",
        arity: 3,
        default_args: || vec![Value::number(1.0), Value::number(3.0), Value::text("hello")],
        eval: |args| {
            Ok(Evaluated::Text(crate::text::substring(&args[2].as_text(), args[0].as_number()?, args[1].as_number()?)))
        },
    },
    ExtOperator {
        kind: "Trim",
        op: "Trim",
        arity: 1,
        default_args: || vec![Value::text("  hello  ")],
        eval: |args| Ok(Evaluated::Text(args[0].as_text().trim().to_string())),
    },
    ExtOperator {
        kind: "FormatNumber",
        op: "FormatNumber",
        arity: 2,
        default_args: || numbers(&[12.3456, 2.0]),
        eval: |args| Ok(Evaluated::Text(crate::text::format_number(args[0].as_number()?, args[1].as_number()?))),
    },
    ExtOperator {
        kind: "Timestamp",
        op: "Timestamp",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Number(crate::text::now_timestamp())),
    },
    ExtOperator {
        kind: "FormatDate",
        op: "FormatDate",
        arity: 2,
        default_args: || vec![Value::op(Op::from_name("Timestamp"), vec![]), Value::text("%Y-%m-%d %H:%M")],
        eval: |args| Ok(Evaluated::Text(crate::text::format_date(args[0].as_number()?, &args[1].as_text())?)),
    },
    ExtOperator {
        kind: "ParseDate",
        op: "ParseDate",
        arity: 2,
        default_args: || vec![Value::text("2026-01-31 09:00"), Value::text("")],
        eval: |args| Ok(Evaluated::Number(crate::text::parse_date(&args[0].as_text(), &args[1].as_text())?)),
    },
    ExtOperator {
        kind: "DatePart",
        op: "DatePart",
        arity: 2,
        default_args: || vec![Value::text("day"), Value::op(Op::from_name("Timestamp"), vec![])],
        eval: |args| Ok(Evaluated::Number(crate::text::date_part(args[1].as_number()?, &args[0].as_text())?)),
    },
];

#[cfg(feature = "system")]
static LATE_OPERATORS: &[ExtOperator] = &[
    ExtOperator {
        kind: "LastError",
        op: "LastError",
        arity: 0,
        default_args: Vec::new,
        eval: |_| Ok(Evaluated::Text(crate::macros::runner::last_error())),
    },
];

fn numbers(values: &[f64]) -> Vec<Value> {
    values.iter().map(|&v| Value::number(v)).collect()
}

#[cfg(feature = "system")]
fn one_text() -> Vec<Value> {
    vec![Value::text("")]
}

/// Operators that run commands or go online, so evaluating one has side
/// effects. Click-to-preview refuses these, and importing a macro that uses
/// one asks first.
pub const SIDE_EFFECT_OPERATORS: [&str; 3] = ["ShellOutput", "ShellSucceeded", "HttpGet"];

/// Whether `value` contains any of [`SIDE_EFFECT_OPERATORS`].
pub fn has_side_effect_operator(value: &Value) -> bool {
    match value {
        Value::Op { op, args, .. } => {
            SIDE_EFFECT_OPERATORS.contains(&op.name()) || args.iter().any(has_side_effect_operator)
        }
        Value::Call { args, .. } => args.iter().any(has_side_effect_operator),
        _ => false,
    }
}

/// Teaches `blockstitch` about Blockwork's operators, in palette order.
/// Idempotent; called from [`crate::init`], which every entry point runs
/// before touching a macro. Without `system` only the pure ones exist.
pub fn register_blockwork_operators() {
    #[cfg(feature = "system")]
    register_operators(OPERATORS);
    register_operators(TEXT_OPERATORS);
    #[cfg(feature = "system")]
    register_operators(LATE_OPERATORS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_operators_evaluate() {
        register_blockwork_operators();
        let op = |name: &str, args: Vec<Value>| Value::op(Op::from_name(name), args).eval();
        let text = Value::text;
        assert_eq!(op("SplitPart", vec![Value::number(2.0), text("a-b-c"), text("-")]), Ok(Evaluated::Text("b".into())));
        assert_eq!(op("TextCheck", vec![text("hello"), text("ends with"), text("llo")]), Ok(Evaluated::Bool(true)));
        assert_eq!(op("RegexReplace", vec![text("o"), text("foo"), text("0")]), Ok(Evaluated::Text("f00".into())));
        assert_eq!(op("Substring", vec![Value::number(2.0), Value::number(3.0), text("abcd")]), Ok(Evaluated::Text("bc".into())));
        assert_eq!(op("FormatNumber", vec![Value::number(2.5), Value::number(2.0)]), Ok(Evaluated::Text("2.50".into())));
        let year = op("DatePart", vec![text("year"), Value::op(Op::from_name("Timestamp"), vec![])]);
        assert!(matches!(year, Ok(Evaluated::Number(y)) if y >= 2026.0));
    }

    #[cfg(feature = "system")]
    #[test]
    fn detection_reporter_reads_list_items() {
        register_blockwork_operators();
        let item = r#"{"x":10,"y":20,"w":30,"h":40,"label":"player","score":0.5,"id":7}"#;
        let op = |field: &str, item: &str| Value::op(Op::from_name("Detection"), vec![Value::text(field), Value::text(item)]).eval();
        assert_eq!(op("center x", item), Ok(Evaluated::Number(25.0)));
        assert_eq!(op("label", item), Ok(Evaluated::Text("player".into())));
        assert!(op("x", "").is_err(), "an empty item (list index out of range) is an error");
    }

    #[cfg(feature = "system")]
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
