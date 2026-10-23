//! Registers the daemon to launch at login, in `--background` mode (tray only,
//! no editor window). One entry per OS: an XDG autostart file, a launchd agent,
//! or an HKCU Run value. Flatpak goes through the Background portal.

use std::path::{Path, PathBuf};

const ENTRY_NAME: &str = "blockwork";

/// Adds or removes the login entry for the running daemon binary.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if !enabled {
        return remove();
    }
    let exe = std::env::current_exe().map_err(|e| format!("Can't locate the daemon binary: {e}"))?;
    install(&exe)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install(exe: &Path) -> Result<(), String> {
    if flatpak() {
        return portal_request(true, exe);
    }
    let path = desktop_entry_path()?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Blockwork\nComment=Start Blockwork in the background\n\
         Exec={} --background\nIcon=blockwork\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        quote_exec(exe)
    );
    std::fs::write(&path, entry).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn remove() -> Result<(), String> {
    if flatpak() {
        return portal_request(false, Path::new(""));
    }
    match std::fs::remove_file(desktop_entry_path()?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_entry_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or("No config directory")?;
    Ok(base.join("autostart").join(format!("{ENTRY_NAME}.desktop")))
}

/// Quotes a path for a desktop-entry `Exec` line.
#[cfg(all(unix, not(target_os = "macos")))]
fn quote_exec(exe: &Path) -> String {
    let s = exe.to_string_lossy();
    let escaped = s
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some()
}

/// The sandbox can't write the host's autostart dir; the portal does it.
#[cfg(all(unix, not(target_os = "macos")))]
fn portal_request(autostart: bool, exe: &Path) -> Result<(), String> {
    // The wrapper script sets BLOCKWORK_UI_EXE so the tray can relaunch the UI.
    let script = format!(
        "BLOCKWORK_UI_EXE=/app/bin/blockwork exec '{}' --background",
        exe.to_string_lossy().replace('\'', "'\\''")
    );
    let script = script.replace('\\', "\\\\").replace('\'', "\\'");
    let options = format!(
        "{{'autostart': <{autostart}>, 'commandline': <['sh', '-c', '{script}']>, 'reason': <'Keep Blockwork hotkeys active after login'>}}"
    );
    let status = std::process::Command::new("gdbus")
        .args([
            "call", "--session",
            "--dest", "org.freedesktop.portal.Desktop",
            "--object-path", "/org/freedesktop/portal/desktop",
            "--method", "org.freedesktop.portal.Background.RequestBackground",
            "", &options,
        ])
        .output()
        .map_err(|e| format!("Background portal unavailable: {e}"))?;
    if status.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&status.stderr).trim().to_string())
    }
}

#[cfg(target_os = "macos")]
const LABEL: &str = "com.blockworked.Blockwork";

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    Ok(home.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn install(exe: &Path) -> Result<(), String> {
    let path = plist_path()?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().replace('&', "&amp;").replace('<', "&lt;");
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\n\
         <key>Label</key><string>{LABEL}</string>\n\
         <key>ProgramArguments</key><array><string>{exe}</string><string>--background</string></array>\n\
         <key>RunAtLoad</key><true/>\n\
         </dict></plist>\n"
    );
    std::fs::write(&path, plist).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

#[cfg(target_os = "macos")]
fn remove() -> Result<(), String> {
    match std::fs::remove_file(plist_path()?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
fn reg(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(windows)]
fn install(exe: &Path) -> Result<(), String> {
    let value = format!("\"{}\" --background", exe.display());
    reg(&["add", RUN_KEY, "/v", ENTRY_NAME, "/t", "REG_SZ", "/d", &value, "/f"])
}

#[cfg(windows)]
fn remove() -> Result<(), String> {
    // Deleting a missing value is not an error worth surfacing.
    let _ = reg(&["delete", RUN_KEY, "/v", ENTRY_NAME, "/f"]);
    Ok(())
}
