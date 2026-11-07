//! Blockwork's view of the value system. The tree and its evaluation live
//! in `blockstitch-core`; Blockwork's pure reporters are here, and
//! `blockwork-core` adds the ones that read the machine.

pub use blockstitch_core::value::*;

/// Pure text and date reporters: no machine state, so the website's
/// editor registers them too.
static TEXT_OPERATORS: &[ExtOperator] = &[
    ExtOperator {
        kind: "TextCheck",
        op: "TextCheck",
        arity: 3,
        default_args: || {
            vec![
                Value::text("hello world"),
                Value::text("contains"),
                Value::text("world"),
            ]
        },
        eval: |args| {
            Ok(Evaluated::Bool(crate::text::check(
                &args[0].as_text(),
                &args[1].as_text(),
                &args[2].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "TextReplace",
        op: "TextReplace",
        arity: 3,
        default_args: || {
            vec![
                Value::text("world"),
                Value::text("hello world"),
                Value::text("there"),
            ]
        },
        eval: |args| {
            Ok(Evaluated::Text(
                args[1]
                    .as_text()
                    .replace(&args[0].as_text(), &args[2].as_text()),
            ))
        },
    },
    ExtOperator {
        kind: "RegexReplace",
        op: "RegexReplace",
        arity: 3,
        default_args: || {
            vec![
                Value::text("[0-9]+"),
                Value::text("order 1234"),
                Value::text("#"),
            ]
        },
        eval: |args| {
            Ok(Evaluated::Text(crate::text::regex_replace(
                &args[0].as_text(),
                &args[1].as_text(),
                &args[2].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "RegexFind",
        op: "RegexFind",
        arity: 2,
        default_args: || vec![Value::text("[0-9]+"), Value::text("order 1234")],
        eval: |args| {
            Ok(Evaluated::Text(crate::text::regex_find(
                &args[0].as_text(),
                &args[1].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "SplitPart",
        op: "SplitPart",
        arity: 3,
        default_args: || vec![Value::number(1.0), Value::text("a,b,c"), Value::text(",")],
        eval: |args| {
            Ok(Evaluated::Text(crate::text::split_part(
                &args[1].as_text(),
                &args[2].as_text(),
                args[0].as_number()?,
            )?))
        },
    },
    ExtOperator {
        kind: "SplitCount",
        op: "SplitCount",
        arity: 2,
        default_args: || vec![Value::text("a,b,c"), Value::text(",")],
        eval: |args| {
            Ok(Evaluated::Number(
                crate::text::split(&args[0].as_text(), &args[1].as_text()).len() as f64,
            ))
        },
    },
    ExtOperator {
        kind: "Substring",
        op: "Substring",
        arity: 3,
        default_args: || vec![Value::number(1.0), Value::number(3.0), Value::text("hello")],
        eval: |args| {
            Ok(Evaluated::Text(crate::text::substring(
                &args[2].as_text(),
                args[0].as_number()?,
                args[1].as_number()?,
            )))
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
        eval: |args| {
            Ok(Evaluated::Text(crate::text::format_number(
                args[0].as_number()?,
                args[1].as_number()?,
            )))
        },
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
        default_args: || {
            vec![
                Value::op(Op::from_name("Timestamp"), vec![]),
                Value::text("%Y-%m-%d %H:%M"),
            ]
        },
        eval: |args| {
            Ok(Evaluated::Text(crate::text::format_date(
                args[0].as_number()?,
                &args[1].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "ParseDate",
        op: "ParseDate",
        arity: 2,
        default_args: || vec![Value::text("2026-01-31 09:00"), Value::text("")],
        eval: |args| {
            Ok(Evaluated::Number(crate::text::parse_date(
                &args[0].as_text(),
                &args[1].as_text(),
            )?))
        },
    },
    ExtOperator {
        kind: "DatePart",
        op: "DatePart",
        arity: 2,
        default_args: || {
            vec![
                Value::text("day"),
                Value::op(Op::from_name("Timestamp"), vec![]),
            ]
        },
        eval: |args| {
            Ok(Evaluated::Number(crate::text::date_part(
                args[1].as_number()?,
                &args[0].as_text(),
            )?))
        },
    },
];

fn numbers(values: &[f64]) -> Vec<Value> {
    values.iter().map(|&v| Value::number(v)).collect()
}

/// Operators that run commands or go online, so evaluating one has side
/// effects. Click-to-preview refuses these, and importing a macro that uses
/// one asks first.
pub const SIDE_EFFECT_OPERATORS: &[&str] = &["ShellOutput", "ShellSucceeded", "HttpGet"];

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

/// Whether `name` is one of the pure reporters every build registers, so a
/// page without the app can evaluate it. True of nothing that reads the
/// machine.
pub fn is_pure_operator(name: &str) -> bool {
    TEXT_OPERATORS.iter().any(|o| o.op == name)
}

/// Teaches `blockstitch` the pure reporters. Idempotent; called from
/// [`crate::init`]. `blockwork-core` registers the ones that read the machine
/// around these, in palette order.
pub fn register_text_operators() {
    register_operators(TEXT_OPERATORS);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_operators_evaluate() {
        register_text_operators();
        let op = |name: &str, args: Vec<Value>| Value::op(Op::from_name(name), args).eval();
        let text = Value::text;
        assert_eq!(
            op(
                "SplitPart",
                vec![Value::number(2.0), text("a-b-c"), text("-")]
            ),
            Ok(Evaluated::Text("b".into()))
        );
        assert_eq!(
            op(
                "TextCheck",
                vec![text("hello"), text("ends with"), text("llo")]
            ),
            Ok(Evaluated::Bool(true))
        );
        assert_eq!(
            op("RegexReplace", vec![text("o"), text("foo"), text("0")]),
            Ok(Evaluated::Text("f00".into()))
        );
        assert_eq!(
            op(
                "Substring",
                vec![Value::number(2.0), Value::number(3.0), text("abcd")]
            ),
            Ok(Evaluated::Text("bc".into()))
        );
        assert_eq!(
            op("FormatNumber", vec![Value::number(2.5), Value::number(2.0)]),
            Ok(Evaluated::Text("2.50".into()))
        );
        let year = op(
            "DatePart",
            vec![text("year"), Value::op(Op::from_name("Timestamp"), vec![])],
        );
        assert!(matches!(year, Ok(Evaluated::Number(y)) if y >= 2026.0));
    }
}
