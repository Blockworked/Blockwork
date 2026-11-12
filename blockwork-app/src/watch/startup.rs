//! One startup notification per login session, separate from opening the editor.
use super::Target;
use crate::{Backend, scheduled_run};
use blockwork_core::{config, macros::InstructionKind, run_log};
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;

/// Claim before checking enable/paused state: skipped events never fire later.
fn claim(root: &Path, session: &str) -> Result<bool, String> {
    if session.trim().is_empty() || session.len() > 4096 {
        return Err("Can't identify this startup session".into());
    }
    // Stable FNV-1a keeps OS session identifiers out of filenames.
    let key = session.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let dir = root.join("startup-claims");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{key:016x}")))
    {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(format!("Can't record startup session: {e}")),
    }
}

#[cfg(target_os = "linux")]
fn session() -> Result<String, String> {
    let boot =
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|e| e.to_string())?;
    let login = std::env::var("XDG_SESSION_ID")
        .or_else(|_| std::env::var("DISPLAY"))
        .unwrap_or_default();
    Ok(format!("{}:{login}", boot.trim()))
}

#[cfg(any(windows, target_os = "macos"))]
fn helper(program: &str, args: &[&str]) -> Result<String, String> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                if !status.success() || output.stdout.len() > 4096 {
                    return Err("Startup session query failed".into());
                }
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Startup session query timed out".into());
            }
        }
    }
}

#[cfg(windows)]
fn session() -> Result<String, String> {
    helper("whoami", &["/logonid"])
}
#[cfg(target_os = "macos")]
fn session() -> Result<String, String> {
    Ok(format!(
        "{}:{}",
        helper("/usr/sbin/sysctl", &["-n", "kern.boottime"])?,
        helper("/bin/launchctl", &["managerasid"])?
    ))
}
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
fn session() -> Result<String, String> {
    Err("Startup triggers aren't supported on this platform".into())
}

impl Backend {
    /// Called only by the OS startup entry, including when another daemon owns IPC.
    pub fn notify_system_startup(&self) {
        let event = || -> Result<(), String> {
            if !claim(&config::config_root()?, &session()?)? {
                return Ok(());
            }
            let (targets, emulator, speed) = {
                let state = self.state.lock().map_err(|e| e.to_string())?;
                if !state.start_at_startup || state.triggers_paused {
                    return Ok(());
                }
                let Some(emulator) = state.emulator.as_ref().map(Arc::clone) else {
                    return Ok(());
                };
                let targets: Vec<_> = scheduled_run::live_macros(&state)
                    .into_iter()
                    .flat_map(|mac| {
                        mac.strands
                            .iter()
                            .filter(|strand| {
                                matches!(
                                    strand.instructions.first().map(|i| &i.kind),
                                    Some(InstructionKind::WhenSystemStarts)
                                )
                            })
                            .map(|strand| Target::new(mac, strand))
                    })
                    .collect();
                (targets, emulator, state.global_speed_multiplier)
            };
            for target in targets {
                target.fire(&emulator, speed, &self.state, &self.app);
            }
            Ok(())
        };
        if let Err(error) = event() {
            run_log::error(&run_log::Source::app("System startup"), error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_is_claimed_once_per_session_even_after_restart() {
        let dir = std::env::temp_dir().join(format!("blockwork-startup-{}", uuid::Uuid::new_v4()));
        assert!(claim(&dir, "boot:login").unwrap());
        assert!(!claim(&dir, "boot:login").unwrap());
        assert!(claim(&dir, "boot:next-login").unwrap());
        assert!(claim(&dir, "next-boot:login").unwrap());
        assert!(claim(&dir, "").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
