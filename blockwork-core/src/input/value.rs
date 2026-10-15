//! Blockwork's view of the value system. The tree and its evaluation live
//! in `blockstitch-core`; the reporter blocks Blockwork adds are here.

pub use blockstitch_core::value::*;

/// Blockwork's own operators, in blockstitch's extension shape. They read
/// live system state, so none takes operands - see [`crate::battery`] and
/// [`crate::clipboard`]. The wire names are what they serialized as before,
/// so saves keep loading.
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
        eval: |_| Ok(Evaluated::Text(crate::clipboard::get_text()?)),
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
        default_args: one_text,
        eval: |args| Ok(Evaluated::Text(crate::files::read_text(&args[0].as_text())?)),
    },
    ExtOperator {
        kind: "FileExists",
        op: "FileExists",
        arity: 1,
        default_args: one_text,
        eval: |args| Ok(Evaluated::Bool(crate::files::exists(&args[0].as_text()))),
    },
    ExtOperator {
        kind: "ListDirectory",
        op: "ListDirectory",
        arity: 1,
        default_args: one_text,
        eval: |args| Ok(Evaluated::Text(crate::files::list_dir(&args[0].as_text())?)),
    },
    ExtOperator {
        kind: "HttpGet",
        op: "HttpGet",
        arity: 1,
        default_args: one_text,
        eval: |args| Ok(Evaluated::Text(crate::http::get(&args[0].as_text())?)),
    },
    ExtOperator {
        kind: "JsonGet",
        op: "JsonGet",
        arity: 2,
        default_args: two_texts,
        eval: |args| {
            Ok(Evaluated::Text(crate::http::json_get(&args[1].as_text(), &args[0].as_text())?))
        },
    },
    ExtOperator {
        kind: "ShellOutput",
        op: "ShellOutput",
        arity: 1,
        default_args: one_text,
        eval: |args| Ok(Evaluated::Text(crate::shell::run(&args[0].as_text())?.stdout)),
    },
    ExtOperator {
        kind: "ShellSucceeded",
        op: "ShellSucceeded",
        arity: 1,
        default_args: one_text,
        eval: |args| Ok(Evaluated::Bool(crate::shell::run(&args[0].as_text())?.success)),
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
        default_args: one_text,
        eval: |args| Ok(Evaluated::Bool(crate::window::exists(&args[0].as_text())?)),
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
        eval: |args| Ok(crate::globals::get(&args[0].as_text())),
    },
    ExtOperator {
        kind: "PixelColor",
        op: "PixelColor",
        arity: 2,
        default_args: || numbers(&[100.0, 100.0]),
        eval: |args| Ok(Evaluated::Text(crate::screen::pixel_color(args[0].as_number()?, args[1].as_number()?)?)),
    },
    ExtOperator {
        kind: "AverageColor",
        op: "AverageColor",
        arity: 4,
        default_args: || numbers(&[0.0, 0.0, 100.0, 100.0]),
        eval: |args| {
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

fn one_text() -> Vec<Value> {
    vec![Value::text("")]
}

fn two_texts() -> Vec<Value> {
    vec![Value::text(""), Value::text("")]
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

/// Teaches `blockstitch` about [`OPERATORS`]. Idempotent; called from
/// [`crate::init`], which every entry point runs before touching a macro.
pub fn register_blockwork_operators() {
    register_operators(OPERATORS);
}

#[cfg(test)]
mod tests {
    use super::*;

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
