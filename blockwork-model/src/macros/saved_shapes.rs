//! Older saved shapes of `Wait` and `WhenTime`, and how they load into the
//! current ones. Every other block loads with its own derive.

use super::*;

/// `WhenTime` as saved now (recurrence + time value), or before the time
/// became a value (one object with a fixed `hour`/`minute`).
#[derive(Deserialize)]
#[serde(untagged)]
enum WhenTimeDe {
    Current(TimeSchedule, Value),
    Legacy(LegacyTimeSchedule),
}

pub(super) fn when_time<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<(TimeSchedule, Value), D::Error> {
    Ok(match WhenTimeDe::deserialize(d)? {
        WhenTimeDe::Current(schedule, time) => (schedule, time),
        WhenTimeDe::Legacy(old) => (
            old.schedule,
            Value::text(format!("{:02}:{:02}", old.hour, old.minute)),
        ),
    })
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WaitDe {
    /// Oldest save shape: a bare duration, no randomness field existed yet.
    LegacyNumber(u64),
    /// Pre-`Op::Random` save shape: `[duration, randomness]`, migrated into
    /// `Op::Random` below so old macros keep the same spread of wait times.
    LegacyWithRandomness(Value, Value),
    /// Current shape: a single `Value` (a plain duration, or a duration
    /// wrapped in any operator including `Op::Random`).
    Current(Value),
}

pub(super) fn wait<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Value, D::Error> {
    Ok(match WaitDe::deserialize(d)? {
        WaitDe::LegacyNumber(d) => Value::number(d as f64),
        WaitDe::LegacyWithRandomness(d, r) => migrate_wait_duration(d, r),
        WaitDe::Current(d) => d,
    })
}

/// Folds a legacy `[duration, randomness]` `Wait` into today's single-`Value`
/// shape: `duration` alone if randomness was zero, otherwise `duration`
/// wrapped in `Op::Random` spanning `[duration - randomness, duration + randomness]`.
pub(super) fn migrate_wait_duration(duration: Value, randomness: Value) -> Value {
    if randomness == Value::number(0.0) {
        return duration;
    }
    let zero = || Box::new(Value::number(0.0));
    Value::Op {
        op: Op::Random,
        args: vec![
            Value::Op {
                op: Op::Sub,
                args: vec![duration.clone(), randomness.clone()],
                saved: zero(),
            },
            Value::Op {
                op: Op::Add,
                args: vec![duration, randomness],
                saved: zero(),
            },
        ],
        saved: zero(),
    }
}
