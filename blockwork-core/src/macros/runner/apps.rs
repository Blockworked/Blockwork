//! Opening and closing apps for the Open app and Close app blocks, per platform.

use super::*;

/// Each platform needs a different launcher: Windows' `start` handles a
/// `.lnk` path directly; macOS' `open` handles an `.app` bundle path; a plain
/// `sh -c` covers Linux's cleaned `Exec=` line (may still carry shell
/// syntax), run on the host when sandboxed since the app lives outside the
/// Flatpak.
#[cfg(target_os = "windows")]
pub(super) fn open_app(command: &str, launch: &LaunchOptions) -> std::io::Result<()> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", ""]);
    if let Some(dir) = working_dir(launch)? {
        cmd.arg("/D").arg(dir);
    }
    cmd.arg(command);
    // Raw, so the arguments reach the app as typed, quotes and all.
    if !launch.args.trim().is_empty() {
        cmd.raw_arg(launch.args.trim());
    }
    cmd.envs(env_vars(launch))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
pub(super) fn open_app(command: &str, launch: &LaunchOptions) -> std::io::Result<()> {
    let mut cmd = Command::new("/usr/bin/open");
    for (name, value) in env_vars(launch) {
        cmd.arg("--env").arg(format!("{name}={value}"));
    }
    cmd.arg(command);
    let args = split_args(&launch.args);
    if !args.is_empty() {
        cmd.arg("--args").args(args);
    }
    let output = cmd.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "couldn't open the application: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(target_os = "linux")]
pub(super) fn open_app(command: &str, launch: &LaunchOptions) -> std::io::Result<()> {
    let line = match launch.args.trim() {
        "" => command.to_string(),
        args => format!("{command} {args}"),
    };
    let dir = working_dir(launch)?;
    let env = env_vars(launch);
    let mut cmd = if crate::flatpak::is_flatpak() {
        // A host process gets its folder and variables from flatpak-spawn.
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host");
        if let Some(dir) = &dir {
            cmd.arg(format!("--directory={}", dir.display()));
        }
        for (name, value) in &env {
            cmd.arg(format!("--env={name}={value}"));
        }
        cmd.arg("sh");
        cmd
    } else {
        let mut cmd = Command::new("sh");
        if let Some(dir) = &dir {
            cmd.current_dir(dir);
        }
        cmd.envs(env);
        cmd
    };
    cmd.arg("-c").arg(line).spawn().map(|_| ())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub(super) fn open_app(_command: &str, _launch: &LaunchOptions) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening apps is not supported on this platform",
    ))
}

/// The launch's working folder with `~` expanded, or `None` to keep ours.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(super) fn working_dir(launch: &LaunchOptions) -> std::io::Result<Option<std::path::PathBuf>> {
    if launch.working_dir.trim().is_empty() {
        return Ok(None);
    }
    let dir = crate::files::expand(&launch.working_dir);
    // Most host folders are hidden from a Flatpak, so the host checks there.
    let sandboxed = cfg!(target_os = "linux") && crate::flatpak::is_flatpak();
    if !sandboxed && !dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("working folder {} doesn't exist", dir.display()),
        ));
    }
    Ok(Some(dir))
}

/// The launch's environment variables, skipping rows with no name.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub(super) fn env_vars(launch: &LaunchOptions) -> Vec<(String, String)> {
    launch
        .env
        .iter()
        .filter(|v| !v.name.trim().is_empty())
        .map(|v| (v.name.trim().to_string(), v.value.clone()))
        .collect()
}

/// Splits a typed argument line the way a shell would: whitespace between
/// words, single or double quotes around spaces, backslash escapes.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn split_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"') | None, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                in_word = true;
            }
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    args.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            (None, c) => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        args.push(current);
    }
    args
}

/// Terminates the app an `InstructionKind::CloseApp` names. No cross-platform
/// "close this launch string" API exists, so each platform derives a
/// best-effort process matcher from whatever `OpenApp`'s picker captured:
/// the executable basename from a Linux `Exec=` line or a Windows `.lnk`
/// path, or the exact macOS bundle path with a legacy display-name fallback.
#[cfg(target_os = "linux")]
pub(super) fn close_app(command: &str, _name: &str) -> std::io::Result<()> {
    let proc_name = command
        .split_whitespace()
        .next()
        .and_then(|c| c.rsplit('/').next())
        .unwrap_or("");
    if proc_name.is_empty() {
        return Ok(());
    }
    crate::flatpak::host_command("killall")
        .arg(proc_name)
        .status()
        .map(|_| ())
}

#[cfg(target_os = "windows")]
pub(super) fn close_app(command: &str, _name: &str) -> std::io::Result<()> {
    let Some(stem) = std::path::Path::new(command)
        .file_stem()
        .and_then(|s| s.to_str())
    else {
        return Ok(());
    };
    if stem.is_empty() {
        return Ok(());
    }
    Command::new("taskkill")
        .args(["/IM", &format!("{stem}.exe"), "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|_| ())
}

#[cfg(target_os = "macos")]
pub(super) fn close_app(command: &str, name: &str) -> std::io::Result<()> {
    crate::system::close_application(command, name)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub(super) fn close_app(_command: &str, _name: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "closing apps is not supported on this platform",
    ))
}
