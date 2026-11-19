//! The system's key repeat settings: how long a held key waits before it
//! repeats, and how often it repeats after that.

use std::time::Duration;

/// Used where the system can't be asked.
const DEFAULT: (Duration, Duration) = (Duration::from_millis(500), Duration::from_millis(33));

/// (delay before the first repeat, time between repeats).
pub fn settings() -> (Duration, Duration) {
    read().unwrap_or(DEFAULT)
}

#[cfg(windows)]
fn read() -> Option<(Duration, Duration)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETKEYBOARDDELAY, SPI_GETKEYBOARDSPEED, SystemParametersInfoW,
    };
    let (mut delay, mut speed) = (0u32, 0u32);
    // SAFETY: both calls write one u32 into the pointer given.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETKEYBOARDDELAY, 0, (&raw mut delay).cast(), 0) != 0
            && SystemParametersInfoW(SPI_GETKEYBOARDSPEED, 0, (&raw mut speed).cast(), 0) != 0
    };
    ok.then(|| from_windows(delay, speed))
}

/// Windows stores the delay as 0 to 3 (250 ms steps) and the speed as 0
/// (about 2.5 a second) to 31 (about 30 a second).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn from_windows(delay: u32, speed: u32) -> (Duration, Duration) {
    let per_second = 2.5 + f64::from(speed.min(31)) * 27.5 / 31.0;
    (
        Duration::from_millis(250 * (u64::from(delay.min(3)) + 1)),
        Duration::from_secs_f64(1.0 / per_second),
    )
}

#[cfg(target_os = "macos")]
fn read() -> Option<(Duration, Duration)> {
    // Both are in 15 ms units; unset means the system default.
    let read = |key: &str, default: f64| {
        std::process::Command::new("defaults")
            .args(["read", "-g", key])
            .output()
            .ok()
            .and_then(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse::<f64>()
                    .ok()
            })
            .unwrap_or(default)
    };
    let delay = read("InitialKeyRepeat", 15.0);
    let every = read("KeyRepeat", 2.0);
    Some((
        Duration::from_secs_f64(delay.max(1.0) * 0.015),
        Duration::from_secs_f64(every.max(1.0) * 0.015),
    ))
}

/// Linux repeats injected keys itself, so nothing reads this there.
#[cfg(not(any(windows, target_os = "macos")))]
fn read() -> Option<(Duration, Duration)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_values_cover_their_range() {
        assert_eq!(from_windows(0, 31).0, Duration::from_millis(250));
        assert_eq!(from_windows(3, 0).0, Duration::from_millis(1000));
        assert_eq!(from_windows(1, 31).1.as_millis(), 33);
        assert_eq!(from_windows(1, 0).1.as_millis(), 400);
    }
}
