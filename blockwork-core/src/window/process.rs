//! Cross-platform process-running check via `sysinfo` — backs
//! `window::is_app_running`. No window-manager API involved (a tray-only or
//! headless process still counts as "running"), so unlike the rest of
//! `window`, this needs no per-platform branching at all.

use sysinfo::System;

/// Whether any running process's name or executable path contains `app`
/// (case-insensitive), so `"firefox"` matches a process literally named
/// `firefox` as well as one launched from `/usr/lib/firefox/firefox`. A
/// blank `app` matches nothing (rather than trivially matching every
/// process), since that's almost certainly an unfilled field, not "is
/// anything running?".
pub fn is_app_running(app: &str) -> bool {
    let needle = app.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.processes().values().any(|process| {
        let name_matches = process.name().to_string_lossy().to_lowercase().contains(&needle);
        let exe_matches = process
            .exe()
            .map(|path| path.to_string_lossy().to_lowercase().contains(&needle))
            .unwrap_or(false);
        name_matches || exe_matches
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_needle_never_matches() {
        assert!(!is_app_running(""));
        assert!(!is_app_running("   "));
    }

    // `sleep` is a POSIX utility, not available as a bare command on Windows.
    #[test]
    #[cfg(unix)]
    fn detects_a_process_actually_running_right_now() {
        let mut child = std::process::Command::new("sleep").arg("2").spawn().expect("failed to spawn test process");
        // sysinfo needs a moment to see a just-spawned process on some platforms.
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(is_app_running("sleep"), "expected the freshly spawned `sleep` process to be detected as running");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn does_not_match_an_unrelated_name() {
        assert!(!is_app_running("blockwork-window-sensing-test-process-that-does-not-exist"));
    }
}
