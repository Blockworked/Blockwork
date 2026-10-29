//! Cross-platform battery percentage/power-source reads, backing
//! `Op::BatteryPercentage`/`Op::PluggedIn` and the
//! `WhenBatteryDischargedTo`/`WhenBatteryChargedTo`/`WhenPowerPluggedIn`/
//! `WhenPowerUnplugged` instructions.

#[cfg(not(target_os = "android"))]
mod desktop {
    use starship_battery::State;
    use starship_battery::units::ratio::percent;

    /// Charge (0-100) of the system's first detected battery. Errs if the
    /// platform's battery API is unavailable or no battery is present (e.g. a
    /// desktop with no battery at all).
    pub fn percentage() -> Result<f64, String> {
        let manager = starship_battery::Manager::new()
            .map_err(|e| format!("battery info unavailable: {e}"))?;
        let battery = manager
            .batteries()
            .map_err(|e| format!("failed to enumerate batteries: {e}"))?
            .next()
            .ok_or_else(|| "no battery detected".to_string())?
            .map_err(|e| format!("failed to read battery: {e}"))?;
        checked_percentage(battery.state_of_charge().get::<percent>() as f64)
    }

    /// Rejects readings that aren't numbers. Out-of-range ones are clamped: a
    /// worn battery can report a little over its design capacity (101%).
    fn checked_percentage(value: f64) -> Result<f64, String> {
        if value.is_finite() {
            Ok(value.clamp(0.0, 100.0))
        } else {
            Err("battery charge unavailable: the system returned an invalid percentage".into())
        }
    }

    /// Whether the system is currently receiving external power - plugged into
    /// AC/USB, or (unlike `percentage`) simply has no battery or UPS at all,
    /// since a desktop with no battery is always "plugged in". `Discharging` is
    /// the only battery state that counts as unplugged; every other state
    /// (charging, full, an ambiguous/unknown reading) counts as plugged in, same
    /// as no battery being present.
    pub fn is_plugged_in() -> bool {
        let Ok(manager) = starship_battery::Manager::new() else {
            return true;
        };
        let Ok(mut batteries) = manager.batteries() else {
            return true;
        };
        let Some(Ok(battery)) = batteries.next() else {
            return true;
        };
        battery.state() != State::Discharging
    }
    #[cfg(test)]
    mod tests {
        use super::checked_percentage;

        #[test]
        fn unavailable_capacity_does_not_become_a_numeric_charge() {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(checked_percentage(value).is_err());
            }
            assert_eq!(checked_percentage(101.3), Ok(100.0));
            assert_eq!(checked_percentage(-1.0), Ok(0.0));
            for value in [0.0, 45.5, 100.0] {
                assert_eq!(checked_percentage(value), Ok(value));
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
pub use desktop::*;

#[cfg(target_os = "android")]
mod android {
    pub fn percentage() -> Result<f64, String> {
        Err("battery reporting is not connected to the Android host".into())
    }
    pub fn is_plugged_in() -> bool {
        false
    }
}

#[cfg(target_os = "android")]
pub use android::*;
