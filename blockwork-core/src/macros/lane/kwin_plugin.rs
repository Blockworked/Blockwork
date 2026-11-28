//! Installing Blockwork's KWin plugin for this user, without sudo: a build
//! for the running KWin goes in Blockwork's data folder, and a Plasma env
//! script adds that folder to `QT_PLUGIN_PATH` from the next login. KWin
//! only loads plugins built for its exact version, so CI publishes one per
//! KWin release; without a match the plugin is built on this computer.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const PLUGIN_ID: &str = "blockworkseathost";
const LIBRARY: &str = "blockworkseathost.so";
const RELEASES: &str = "https://github.com/Blockworked/Blockwork/releases/download";
const MAX_BYTES: u64 = 32 * 1024 * 1024;

const CMAKE_LISTS: &str = include_str!("../../../../blockwork-kwin/CMakeLists.txt");
/// The plugin's sources, for building it here: (path under the source root, text).
const SOURCES: &[(&str, &str)] = &[
    ("blockwork-kwin/CMakeLists.txt", CMAKE_LISTS),
    (
        "blockwork-kwin/main.cpp",
        include_str!("../../../../blockwork-kwin/main.cpp"),
    ),
    (
        "blockwork-kwin/seathost.cpp",
        include_str!("../../../../blockwork-kwin/seathost.cpp"),
    ),
    (
        "blockwork-kwin/seathost.h",
        include_str!("../../../../blockwork-kwin/seathost.h"),
    ),
    (
        "blockwork-kwin/metadata.json",
        include_str!("../../../../blockwork-kwin/metadata.json"),
    ),
    (
        "blockwork-kwin/pointer-constraints-unstable-v1.xml",
        include_str!("../../../../blockwork-kwin/pointer-constraints-unstable-v1.xml"),
    ),
    (
        "blockwork-core/protocols/blockwork-seat-host-v1.xml",
        include_str!("../../../protocols/blockwork-seat-host-v1.xml"),
    ),
];

/// The plugin's version, from `project(... VERSION x)` in its CMakeLists.
pub fn plugin_version() -> &'static str {
    CMAKE_LISTS
        .lines()
        .find_map(|line| line.trim().strip_prefix("project(blockwork_kwin VERSION "))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or("0.0.0")
}

/// The part of a KWin version a plugin is tied to: "6.3.2.1" is "6.3.2".
fn plugin_abi(kwin: &str) -> String {
    kwin.trim().split('.').take(3).collect::<Vec<_>>().join(".")
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME isn't set: Blockwork can't find your home folder".to_string())
}

/// The folder added to `QT_PLUGIN_PATH`. Real home paths, so a Flatpak
/// writes where the host's KWin looks.
fn plugin_root() -> Result<PathBuf, String> {
    Ok(home()?.join(".local/share/blockwork/kwin-plugin"))
}

fn library_path() -> Result<PathBuf, String> {
    Ok(plugin_root()?.join("kwin/plugins").join(LIBRARY))
}

fn installed_note() -> Result<PathBuf, String> {
    Ok(plugin_root()?.join("installed.json"))
}

fn env_script() -> Result<PathBuf, String> {
    let config = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !crate::flatpak::is_flatpak() && !dir.is_empty() => PathBuf::from(dir),
        _ => home()?.join(".config"),
    };
    Ok(config.join("plasma-workspace/env/blockwork-kwin-plugin.sh"))
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
struct Installed {
    kwin: String,
    plugin: String,
    how: String,
}

fn read_installed() -> Option<Installed> {
    let text = std::fs::read_to_string(installed_note().ok()?).ok()?;
    let note: Installed = serde_json::from_str(&text).ok()?;
    library_path().ok()?.exists().then_some(note)
}

/// The running KWin's version, from its support information.
pub fn kwin_version() -> Result<String, String> {
    let _runtime = crate::dbus::enter();
    let connection =
        zbus::blocking::Connection::session().map_err(|e| format!("no session bus: {e}"))?;
    let reply = connection
        .call_method(
            Some("org.kde.KWin"),
            "/KWin",
            Some("org.kde.KWin"),
            "supportInformation",
            &(),
        )
        .map_err(|e| format!("KWin didn't answer: {e}"))?;
    let text: String = reply
        .body()
        .deserialize()
        .map_err(|e| format!("KWin's answer didn't make sense: {e}"))?;
    text.lines()
        .find_map(|line| line.trim().strip_prefix("KWin version:"))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "KWin didn't say its version".to_string())
}

/// Asks the running KWin to load the plugin; true once it's loaded.
fn load_now() -> bool {
    let _runtime = crate::dbus::enter();
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    connection
        .call_method(
            Some("org.kde.KWin"),
            "/Plugins",
            Some("org.kde.KWin.Plugins"),
            "LoadPlugin",
            &(PLUGIN_ID,),
        )
        .ok()
        .and_then(|reply| reply.body().deserialize::<bool>().ok())
        .unwrap_or(false)
}

/// Where the plugin stands, for Settings and the install dialog.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Status {
    /// "ready", "missing", "outdated" (built for another KWin), "relogin"
    /// (installed, loads at the next login) or "unsupported".
    pub state: &'static str,
    pub ready: bool,
    pub detail: String,
    pub kwin: String,
    /// The KWin version this user's copy was built for.
    pub installed_for: String,
}

pub fn status() -> Status {
    status_with(super::availability())
}

/// Why the plugin isn't running, for when connecting to it failed.
pub(super) fn missing() -> Status {
    status_with(Err(String::new()))
}

fn status_with(ready: Result<String, String>) -> Status {
    let kde = crate::window::desktop_kind() == Some(crate::window::DesktopKind::Kde);
    let kwin = if kde {
        kwin_version().unwrap_or_default()
    } else {
        String::new()
    };
    let installed = read_installed();
    let installed_for = installed
        .as_ref()
        .map(|i| i.kwin.clone())
        .unwrap_or_default();
    let (state, detail) = match ready {
        Ok(detail) => ("ready", detail),
        Err(why) if !kde => ("unsupported", why),
        Err(_) => match installed {
            Some(note) if plugin_abi(&note.kwin) != plugin_abi(&kwin) => (
                "outdated",
                format!(
                    "the KWin plugin was built for KWin {} but this is KWin {kwin}: update it",
                    note.kwin
                ),
            ),
            Some(_) => (
                "relogin",
                "the KWin plugin is installed: log out and back in once to start it".to_string(),
            ),
            None => (
                "missing",
                "background input needs Blockwork's KWin plugin: install it here".to_string(),
            ),
        },
    };
    Status {
        state,
        ready: state == "ready",
        detail,
        kwin,
        installed_for,
    }
}

/// What `install` did.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Outcome {
    /// "downloaded" or "built".
    pub how: String,
    pub kwin: String,
    /// KWin loaded it straight away; otherwise it starts at the next login.
    pub loaded: bool,
    pub message: String,
}

#[derive(serde::Deserialize)]
struct Index {
    builds: std::collections::HashMap<String, Build>,
}

#[derive(serde::Deserialize)]
struct Build {
    file: String,
    sha256: String,
}

fn release_url(file: &str) -> String {
    format!("{RELEASES}/kwin-plugin-{}/{file}", plugin_version())
}

/// The published build for this KWin, checked against the release's index.
/// `Ok(None)` when there's no build for it.
fn download(kwin: &str) -> Result<Option<Vec<u8>>, String> {
    let index = crate::http::get_bytes(&release_url("index.json"), 1024 * 1024);
    let Ok(index) = index else {
        return Ok(None);
    };
    let index: Index = serde_json::from_slice(&index)
        .map_err(|e| format!("the plugin download list didn't make sense: {e}"))?;
    let Some(build) = index.builds.get(&plugin_abi(kwin)) else {
        return Ok(None);
    };
    let bytes = crate::http::get_bytes(&release_url(&build.file), MAX_BYTES)?;
    let got: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if !got.eq_ignore_ascii_case(build.sha256.trim()) {
        return Err(format!(
            "the downloaded KWin plugin doesn't match its published hash (got {got}): try again later"
        ));
    }
    Ok(Some(bytes))
}

/// The command that installs what building the plugin needs, for this distro.
fn build_needs() -> String {
    let release = std::fs::read_to_string("/run/host/os-release")
        .or_else(|_| std::fs::read_to_string("/etc/os-release"))
        .unwrap_or_default();
    let field = |key: &str| {
        release
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim_matches('"').to_lowercase())
            .unwrap_or_default()
    };
    let ids = format!("{} {}", field("ID="), field("ID_LIKE="));
    let has = |name: &str| ids.split_whitespace().any(|id| id == name);
    if has("arch") || has("cachyos") || has("manjaro") {
        "sudo pacman -S --needed base-devel cmake extra-cmake-modules kwin wayland libxkbcommon vulkan-headers".into()
    } else if has("fedora") || has("rhel") {
        "sudo dnf install gcc-c++ cmake extra-cmake-modules kwin-devel wayland-devel libxkbcommon-devel vulkan-headers".into()
    } else if has("opensuse") || has("suse") || has("opensuse-tumbleweed") {
        "sudo zypper install gcc-c++ cmake extra-cmake-modules kwin6-devel wayland-devel libxkbcommon-devel vulkan-headers".into()
    } else if has("debian") || has("ubuntu") || has("neon") {
        "sudo apt install g++ cmake extra-cmake-modules kwin-dev libwayland-dev libxkbcommon-dev libvulkan-dev".into()
    } else {
        "a C++ compiler, CMake, extra-cmake-modules, KWin's development files, wayland, libxkbcommon and the Vulkan headers".into()
    }
}

/// Builds the plugin on this computer (the host's, from a Flatpak).
fn build_here() -> Result<Vec<u8>, String> {
    let work = home()?.join(".local/share/blockwork/kwin-plugin-build");
    let _ = std::fs::remove_dir_all(&work);
    for (path, text) in SOURCES {
        let file = work.join("src").join(path);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
        }
        std::fs::write(&file, text)
            .map_err(|e| format!("couldn't write {}: {e}", file.display()))?;
    }
    let source = work.join("src/blockwork-kwin");
    let build = work.join("build");
    let run = |args: &[&str]| -> Result<(), String> {
        let output = crate::flatpak::host_command("cmake")
            .args(args)
            .output()
            .map_err(|e| {
                format!(
                    "couldn't run cmake ({e}): building the KWin plugin needs {}",
                    build_needs()
                )
            })?;
        if output.status.success() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&output.stderr).into_owned()
            + &String::from_utf8_lossy(&output.stdout);
        let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let tail = tail[tail.len().saturating_sub(6)..].join("\n");
        Err(format!(
            "building the KWin plugin failed: install what it needs with `{}` and try again\n{tail}",
            build_needs()
        ))
    };
    let (source, build) = (source.to_string_lossy(), build.to_string_lossy());
    run(&["-S", &source, "-B", &build, "-DCMAKE_BUILD_TYPE=Release"])?;
    run(&["--build", &build])?;
    let built = Path::new(build.as_ref()).join(LIBRARY);
    let bytes = std::fs::read(&built).map_err(|e| format!("the build left no plugin: {e}"))?;
    let _ = std::fs::remove_dir_all(&work);
    Ok(bytes)
}

fn script_text(root: &Path) -> String {
    let root = root.to_string_lossy().replace('"', "\\\"");
    format!(
        "# Added by Blockwork: lets KWin find Blockwork's background input plugin.\n\
         # Remove it in Blockwork's Settings > Integrations, or delete this file.\n\
         export QT_PLUGIN_PATH=\"{root}${{QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}}\"\n"
    )
}

/// Downloads the plugin built for the running KWin, or builds it here when
/// there's none (or `build` asks to), and installs it for this user.
pub fn install(build: bool) -> Result<Outcome, String> {
    if crate::window::desktop_kind() != Some(crate::window::DesktopKind::Kde) {
        return Err("the KWin plugin is only for KDE Plasma: log in to a Plasma (Wayland) session to install it".into());
    }
    let kwin = kwin_version()?;
    let downloaded = if build { None } else { download(&kwin)? };
    let (bytes, how) = match downloaded {
        Some(bytes) => (bytes, "downloaded"),
        None => (build_here()?, "built"),
    };
    let library = library_path()?;
    let dir = library.parent().expect("library has a folder");
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    // A new file, not overwritten in place: KWin may have the old one mapped.
    let partial = library.with_extension("so.part");
    std::fs::write(&partial, &bytes).map_err(|e| format!("couldn't save the plugin: {e}"))?;
    std::fs::rename(&partial, &library).map_err(|e| format!("couldn't save the plugin: {e}"))?;
    let note = Installed {
        kwin: kwin.clone(),
        plugin: plugin_version().to_string(),
        how: how.to_string(),
    };
    std::fs::write(
        installed_note()?,
        serde_json::to_string_pretty(&note).unwrap_or_default(),
    )
    .map_err(|e| format!("couldn't save the plugin: {e}"))?;
    let script = env_script()?;
    if let Some(dir) = script.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    }
    std::fs::write(&script, script_text(&plugin_root()?))
        .map_err(|e| format!("couldn't write {}: {e}", script.display()))?;
    // KWin can't swap a plugin it's running, only start one it isn't.
    let was_running = super::availability().is_ok();
    let loaded = !was_running && load_now() && super::availability().is_ok();
    let message = if loaded {
        "the KWin plugin is installed and running".to_string()
    } else if was_running {
        "the KWin plugin is updated: log out and back in to start the new one".to_string()
    } else {
        "the KWin plugin is installed: log out and back in once to start it".to_string()
    };
    Ok(Outcome {
        how: how.to_string(),
        kwin,
        loaded,
        message,
    })
}

/// Removes this user's copy and its env script.
pub fn remove() -> Result<(), String> {
    for path in [env_script()?, installed_note()?, library_path()?] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("couldn't remove {}: {e}", path.display())),
        }
    }
    Ok(())
}

/// Offers to install the plugin when an In window block needs it. True
/// when it's running now.
pub fn offer() -> Result<bool, String> {
    let status = missing();
    if !matches!(status.state, "missing" | "outdated") {
        return Ok(false);
    }
    if crate::permissions::require("network").is_err() {
        return Ok(false);
    }
    let action = if status.state == "outdated" {
        "Update"
    } else {
        "Install"
    };
    let message = format!(
        "Background input needs Blockwork's KWin plugin. {action} it for KWin {} now? It's downloaded from Blockwork's GitHub releases (or built on this computer when there's no download for your KWin) and installed for your user only, no password needed.",
        status.kwin
    );
    let request = crate::prompt::Request::new(crate::prompt::Kind::Confirm, "Blockwork", message);
    if !crate::prompt::ask(&request, &|| false).is_ok_and(|r| r.answer == "yes") {
        return Ok(false);
    }
    let note = crate::prompt::Request::new(
        crate::prompt::Kind::Tooltip,
        "Blockwork",
        "Installing Blockwork's KWin plugin...",
    );
    let _ = crate::prompt::ask(&note, &|| false);
    let outcome = install(false)?;
    if outcome.loaded {
        return Ok(true);
    }
    Err(outcome.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_read_from_the_plugin_sources() {
        assert!(
            plugin_version().split('.').count() == 3,
            "{}",
            plugin_version()
        );
        assert_eq!(plugin_abi("6.3.2.1"), "6.3.2");
        assert_eq!(plugin_abi(" 6.8.3\n"), "6.8.3");
    }

    #[test]
    fn the_env_script_prepends_the_folder() {
        let text = script_text(Path::new("/home/me/.local/share/blockwork/kwin-plugin"));
        assert!(text.contains(
            "export QT_PLUGIN_PATH=\"/home/me/.local/share/blockwork/kwin-plugin${QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}\""
        ));
    }
}
