//! "Export as runnable": a folder with one macro and a launcher that runs it
//! through `blockwork run <file>`, without opening the editor. Where it can,
//! the folder also carries a copy of Blockwork, so it runs on a computer
//! without Blockwork installed.

use super::*;
use std::path::{Path, PathBuf};

/// What `export_runnable` made, for the UI to tell the user.
#[derive(serde::Serialize)]
pub(crate) struct Runnable {
    folder: String,
    launcher: String,
    /// Whether the folder has its own copy of Blockwork.
    portable: bool,
}

pub(crate) fn export_runnable(macro_id: String, parent: String) -> Result<Runnable, String> {
    let mac = config::get_macros_from_config()
        .into_iter()
        .find(|m| m.id == macro_id)
        .ok_or("Macro not found")?;
    let name = sanitize_filename(&mac.name);
    let folder = Path::new(&parent).join(&name);
    if folder.exists() {
        return Err(format!("{} already exists", folder.display()));
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let file = format!("{name}.macro");
    config::write_macro_file(&folder.join(&file), &mac)?;
    let ui = ui_program().ok_or("Can't tell where Blockwork is installed")?;
    let (launcher, portable) = write_launcher(&folder, &name, &file, &ui)?;
    Ok(Runnable {
        folder: folder.to_string_lossy().into_owned(),
        launcher: launcher.to_string_lossy().into_owned(),
        portable,
    })
}

/// The editor binary, which also takes `run`. Same rule as the daemon's.
fn ui_program() -> Option<PathBuf> {
    std::env::var_os("BLOCKWORK_UI_EXE")
        .map(PathBuf::from)
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            Some(exe.with_file_name(format!("blockwork{}", std::env::consts::EXE_SUFFIX)))
        })
}

#[cfg(windows)]
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Windows: copies the install folder next to the macro (Store installs
/// can't be copied, so their launcher points at the installed app).
#[cfg(windows)]
fn write_launcher(
    folder: &Path,
    name: &str,
    file: &str,
    ui: &Path,
) -> Result<(PathBuf, bool), String> {
    let exe_name = ui
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "blockwork.exe".into());
    // Never copy a folder into itself.
    let copied = ui.parent().is_some_and(|dir| {
        !folder.starts_with(dir) && copy_dir(dir, &folder.join("Blockwork")).is_ok()
    });
    let program = if copied {
        format!("%~dp0Blockwork\\{exe_name}")
    } else {
        let _ = std::fs::remove_dir_all(folder.join("Blockwork"));
        ui.to_string_lossy().into_owned()
    };
    let launcher = folder.join(format!("Run {name}.cmd"));
    let script = format!("@echo off\r\n\"{program}\" run \"%~dp0{file}\" --wait\r\n");
    std::fs::write(&launcher, script).map_err(|e| e.to_string())?;
    Ok((launcher, copied))
}

/// macOS: copies the app bundle next to the macro.
#[cfg(target_os = "macos")]
fn write_launcher(
    folder: &Path,
    name: &str,
    file: &str,
    ui: &Path,
) -> Result<(PathBuf, bool), String> {
    // .../Blockwork.app/Contents/MacOS/<binary>
    let bundle = ui
        .ancestors()
        .nth(3)
        .filter(|p| p.extension().is_some_and(|e| e == "app"));
    let copied = bundle.and_then(|app| {
        let app_name = app.file_name()?;
        copy_bundle(app, &folder.join(app_name)).ok()?;
        let inner = ui.strip_prefix(app).ok()?;
        Some(Path::new(app_name).join(inner))
    });
    let program = match &copied {
        Some(inner) => format!("\"$here/{}\"", inner.to_string_lossy()),
        None => shell_quote(&ui.to_string_lossy()),
    };
    let launcher = folder.join(format!("Run {name}.command"));
    write_shell_launcher(&launcher, &program, file)?;
    Ok((launcher, copied.is_some()))
}

/// `cp -R` keeps the bundle's symlinks and signature intact.
#[cfg(target_os = "macos")]
fn copy_bundle(from: &Path, to: &Path) -> Result<(), String> {
    let status = std::process::Command::new("/bin/cp")
        .arg("-R")
        .arg(from)
        .arg(to)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("copy failed".into())
    }
}

/// Linux: an AppImage is copied next to the macro; Flatpak and package
/// installs are run where they are.
#[cfg(not(any(windows, target_os = "macos")))]
fn write_launcher(
    folder: &Path,
    name: &str,
    file: &str,
    ui: &Path,
) -> Result<(PathBuf, bool), String> {
    let mut portable = false;
    let program = if let Some(image) = std::env::var_os("APPIMAGE").map(PathBuf::from) {
        let target = folder.join("Blockwork.AppImage");
        std::fs::copy(&image, &target).map_err(|e| e.to_string())?;
        portable = true;
        "\"$here/Blockwork.AppImage\"".to_string()
    } else if let Ok(id) = std::env::var("FLATPAK_ID") {
        format!("flatpak run {}", shell_quote(&id))
    } else {
        shell_quote(&ui.to_string_lossy())
    };
    let launcher = folder.join(format!("Run {name}.sh"));
    write_shell_launcher(&launcher, &program, file)?;
    Ok((launcher, portable))
}

#[cfg(not(windows))]
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(not(windows))]
fn write_shell_launcher(launcher: &Path, program: &str, file: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let script = format!(
        "#!/bin/sh\nhere=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\nexec {program} run \"$here/\"{} --wait\n",
        shell_quote(file)
    );
    std::fs::write(launcher, script).map_err(|e| e.to_string())?;
    std::fs::set_permissions(launcher, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[cfg(not(any(windows, target_os = "macos")))]
    #[test]
    fn launcher_runs_the_macro_next_to_it() {
        let dir = std::env::temp_dir().join(format!("blockwork-runnable-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (launcher, _) = super::write_launcher(
            &dir,
            "It's mine",
            "It's mine.macro",
            std::path::Path::new("/opt/Block work/blockwork"),
        )
        .unwrap();
        let script = std::fs::read_to_string(&launcher).unwrap();
        if std::env::var_os("APPIMAGE").is_none() && std::env::var_os("FLATPAK_ID").is_none() {
            assert!(script.contains(
                "exec '/opt/Block work/blockwork' run \"$here/\"'It'\\''s mine.macro' --wait"
            ));
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
