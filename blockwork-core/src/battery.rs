//! Cross-platform battery percentage/power-source reads, backing
//! `Op::BatteryPercentage`/`Op::PluggedIn` and the
//! `WhenBatteryDischargedTo`/`WhenBatteryChargedTo`/`WhenPowerPluggedIn`/
//! `WhenPowerUnplugged` instructions.

use starship_battery::units::ratio::percent;
use starship_battery::State;

/// Charge (0-100) of the system's first detected battery. Errs if the
/// platform's battery API is unavailable or no battery is present (e.g. a
/// desktop with no battery at all).
pub fn percentage() -> Result<f64, String> {
    let manager = starship_battery::Manager::new().map_err(|e| format!("battery info unavailable: {e}"))?;
    let battery = manager
        .batteries()
        .map_err(|e| format!("failed to enumerate batteries: {e}"))?
        .next()
        .ok_or_else(|| "no battery detected".to_string())?
        .map_err(|e| format!("failed to read battery: {e}"))?;
    Ok(battery.state_of_charge().get::<percent>() as f64)
}

/// Whether the system is currently receiving external power - plugged into
/// AC/USB, or (unlike `percentage`) simply has no battery or UPS at all,
/// since a desktop with no battery is always "plugged in". `Discharging` is
/// the only battery state that counts as unplugged; every other state
/// (charging, full, an ambiguous/unknown reading) counts as plugged in, same
/// as no battery being present.
pub fn is_plugged_in() -> bool {
    let Ok(manager) = starship_battery::Manager::new() else { return true };
    let Ok(mut batteries) = manager.batteries() else { return true };
    let Some(Ok(battery)) = batteries.next() else { return true };
    battery.state() != State::Discharging
}
