//! Enumerates locally installed applications for the "Open App" instruction's
//! picker popup - desktop-app-only concern (the cross-platform
//! `InstructionKind::OpenApp` just launches whatever `command` string this
//! produced, never re-scans), so this lives here rather than in
//! `blockwork-core`.
//!
//! Icon support is currently Linux-only: freedesktop `.desktop` entries name
//! an icon theme lookup key, which resolves fairly reliably to a `.png`/
//! `.svg` file we can inline as a `data:` URI. Windows/macOS listings below
//! are best-effort (name + launch target only, no icon) - resolving a Start
//! Menu shortcut's icon or a `.icns` bundle icon needs real image decoding
//! this app has no other reason to depend on.

pub(crate) struct AppEntry {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) icon: Option<String>,
    /// What its windows report as their app, for window blocks matching by app.
    pub(crate) window: String,
    /// Its running program's name, for the app-running reporters.
    pub(crate) process: String,
}

#[cfg(target_os = "linux")]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    linux::list_apps()
}

#[cfg(target_os = "windows")]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    windows::list_apps()
}

#[cfg(target_os = "macos")]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    macos::list_apps()
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub(crate) fn list_apps() -> Vec<AppEntry> {
    Vec::new()
}

/// The file a Windows `.lnk` shortcut points at, from its LinkInfo's local
/// base path (MS-SHLLINK 2.3). None for advertised or network shortcuts.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn shortcut_target(bytes: &[u8]) -> Option<String> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let u32_at =
        |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize);
    if u32_at(0)? != 0x4c {
        return None;
    }
    let flags = u32_at(0x14)?;
    let mut at = 0x4c;
    if flags & 0x1 != 0 {
        at += 2 + u16_at(at)? as usize;
    }
    if flags & 0x2 == 0 {
        return None;
    }
    let info = at;
    let header_size = u32_at(info + 4)?;
    if u32_at(info + 8)? & 0x1 == 0 {
        return None;
    }
    let ansi = |offset: usize| {
        let start = info.checked_add(offset)?;
        let rest = bytes.get(start..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        Some(String::from_utf8_lossy(&rest[..end]).into_owned())
    };
    let wide = |offset: usize| {
        let start = info.checked_add(offset)?;
        let units: Vec<u16> = bytes
            .get(start..)?
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        Some(String::from_utf16_lossy(&units))
    };
    let (base, suffix) = if header_size >= 0x24 {
        (wide(u32_at(info + 0x1c)?)?, wide(u32_at(info + 0x20)?)?)
    } else {
        (ansi(u32_at(info + 0x10)?)?, ansi(u32_at(info + 0x18)?)?)
    };
    let target = base + &suffix;
    (!target.is_empty()).then_some(target)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::AppEntry;
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// Every `applications/` directory the freedesktop menu spec says to
    /// search. `seen_ids` just dedupes by app id (first `.desktop` file
    /// wins) - no override semantics needed here.
    fn application_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        match std::env::var("XDG_DATA_DIRS") {
            Ok(v) if !v.is_empty() => {
                dirs.extend(v.split(':').map(|d| PathBuf::from(d).join("applications")))
            }
            _ => {
                dirs.push(PathBuf::from("/usr/local/share/applications"));
                dirs.push(PathBuf::from("/usr/share/applications"));
            }
        }
        if let Some(data_home) = dirs::data_dir() {
            dirs.push(data_home.join("applications"));
        }
        dirs
    }

    /// A parsed `.desktop` entry whose `Icon=` value hasn't been resolved to
    /// a file yet - resolution differs between native and Flatpak listings.
    struct DesktopEntry {
        name: String,
        command: String,
        icon: Option<String>,
        window: String,
        process: String,
    }

    pub(crate) fn list_apps() -> Vec<AppEntry> {
        let mut apps = if blockwork_core::flatpak::is_flatpak() {
            host::list_apps()
        } else {
            list_local_apps()
        };
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps
    }

    fn list_local_apps() -> Vec<AppEntry> {
        let mut seen_ids = HashSet::new();
        let mut apps = Vec::new();
        for dir in application_dirs() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                let Some(id) = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                else {
                    continue;
                };
                if !seen_ids.insert(id.clone()) {
                    continue;
                }
                let Some(entry) = fs::read_to_string(&path)
                    .ok()
                    .and_then(|c| parse_desktop_entry(&id, &c))
                else {
                    continue;
                };
                apps.push(AppEntry {
                    name: entry.name,
                    command: entry.command,
                    icon: entry.icon.and_then(|i| resolve_icon(&i)),
                    window: entry.window,
                    process: entry.process,
                });
            }
        }
        apps
    }

    /// Reads the handful of keys we care about out of a `.desktop` file's
    /// `[Desktop Entry]` section - a purpose-built scan rather than a general
    /// INI parser, since that's all this needs.
    fn parse_desktop_entry(id: &str, content: &str) -> Option<DesktopEntry> {
        let mut in_main_section = false;
        let mut name = None;
        let mut exec = None;
        let mut icon = None;
        let mut wm_class = None;
        let mut no_display = false;
        let mut hidden = false;
        let mut is_application = true;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_main_section = line == "[Desktop Entry]";
                continue;
            }
            if !in_main_section {
                continue;
            }
            if let Some(v) = line.strip_prefix("Name=") {
                if name.is_none() {
                    name = Some(v.to_string());
                }
            } else if let Some(v) = line.strip_prefix("Exec=") {
                exec = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("Icon=") {
                icon = Some(v.to_string());
            } else if let Some(v) = line.strip_prefix("StartupWMClass=") {
                wm_class = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            } else if let Some(v) = line.strip_prefix("NoDisplay=") {
                no_display = v.eq_ignore_ascii_case("true");
            } else if let Some(v) = line.strip_prefix("Hidden=") {
                hidden = v.eq_ignore_ascii_case("true");
            } else if let Some(v) = line.strip_prefix("Type=") {
                is_application = v == "Application";
            }
        }

        if no_display || hidden || !is_application {
            return None;
        }
        let name = name?;
        let command = clean_exec_command(&exec?);
        if command.is_empty() {
            return None;
        }
        let id = id.strip_suffix(".desktop").unwrap_or(id);
        Some(DesktopEntry {
            name,
            // Wayland app IDs are the desktop file's name; X11 classes are
            // what StartupWMClass says when it differs.
            window: wm_class.unwrap_or_else(|| id.to_string()),
            process: program_name(&command, id),
            command,
            icon,
        })
    }

    #[cfg(test)]
    pub(super) fn parse_desktop_entry_for_test(
        id: &str,
        content: &str,
    ) -> Option<(String, String)> {
        parse_desktop_entry(id, content).map(|e| (e.window, e.process))
    }

    /// The program an `Exec=` line runs, as the process list names it:
    /// past `env VAR=value`, and the app inside `flatpak run`.
    fn program_name(command: &str, id: &str) -> String {
        let mut words = command
            .split_whitespace()
            .map(|w| w.trim_matches(|c| c == '"' || c == '\''));
        let mut program = words.next().unwrap_or_default();
        if program.rsplit('/').next() == Some("env") {
            program = words
                .by_ref()
                .find(|w| !w.contains('=') && !w.starts_with('-'))
                .unwrap_or_default();
        }
        let program = program.rsplit('/').next().unwrap_or_default();
        if program != "flatpak" {
            return program.to_string();
        }
        let rest: Vec<&str> = words.collect();
        if let Some(command) = rest.iter().find_map(|w| w.strip_prefix("--command=")) {
            return command.rsplit('/').next().unwrap_or_default().to_string();
        }
        let app = rest
            .iter()
            .skip_while(|w| **w != "run")
            .skip(1)
            .find(|w| !w.starts_with('-'))
            .copied()
            .unwrap_or(id);
        app.rsplit('.').next().unwrap_or(app).to_lowercase()
    }

    /// Strips freedesktop field codes (`%f`/`%F`/`%u`/`%U`/etc.) from an
    /// `Exec=` line - those stand for file/URL args a launch from this
    /// picker never has, so they're dropped rather than substituted.
    fn clean_exec_command(exec: &str) -> String {
        let mut result = String::new();
        let mut chars = exec.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                result.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => result.push('%'),
                Some(
                    'f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'i' | 'c' | 'k' | 'v' | 'm',
                )
                | None => {}
                Some(other) => {
                    result.push('%');
                    result.push(other);
                }
            }
        }
        result.trim().to_string()
    }

    /// Resolves a `.desktop` file's `Icon=` value (either an absolute path,
    /// or an icon-theme lookup key) to a `data:` URI, if a usable file was
    /// found for it.
    fn resolve_icon(icon_name: &str) -> Option<String> {
        let path = if Path::new(icon_name).is_absolute() {
            PathBuf::from(icon_name)
        } else {
            find_icon_file(icon_name)?
        };
        icon_file_to_data_uri(&path)
    }

    /// Best-effort icon-theme lookup: rather than parsing every theme's
    /// `index.theme` to know its real size/scale layout, just probes the
    /// common hicolor-style `<theme>/<size>/apps/<name>.<ext>` layout most
    /// installed themes follow, largest first, plus the older flat
    /// `pixmaps` directory as a fallback.
    fn find_icon_file(name: &str) -> Option<PathBuf> {
        let mut base_dirs: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            base_dirs.push(home.join(".local/share/icons"));
            base_dirs.push(home.join(".icons"));
        }
        base_dirs.push(PathBuf::from("/usr/share/icons"));
        base_dirs.push(PathBuf::from("/usr/local/share/icons"));

        const THEMES: &[&str] = &["hicolor", "Adwaita", "gnome", "breeze", "Papirus"];
        const SIZES: &[&str] = &[
            "scalable", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32",
        ];
        const EXTS: &[&str] = &["svg", "png"];

        for base in &base_dirs {
            for theme in THEMES {
                for size in SIZES {
                    for ext in EXTS {
                        let candidate = base
                            .join(theme)
                            .join(size)
                            .join("apps")
                            .join(format!("{name}.{ext}"));
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        for dir in ["/usr/share/pixmaps", "/usr/local/share/pixmaps"] {
            for ext in EXTS {
                let candidate = PathBuf::from(dir).join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn icon_file_to_data_uri(path: &Path) -> Option<String> {
        let mime = icon_mime(path.extension().and_then(|e| e.to_str())?)?;
        let bytes = fs::read(path).ok()?;
        use base64::Engine;
        Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }

    fn icon_mime(extension: &str) -> Option<&'static str> {
        match extension {
            "svg" => Some("image/svg+xml"),
            "png" => Some("image/png"),
            // .xpm and other legacy formats aren't natively displayable by
            // an <img> tag without decoding/re-encoding first.
            _ => None,
        }
    }

    /// Listing from inside a Flatpak sandbox, where the host's desktop files
    /// and icon themes aren't mounted. Everything is read on the host
    /// through `flatpak-spawn --host` instead, in two batched calls (all
    /// desktop files, then all icons) rather than one spawn per file. Using
    /// the host's own `XDG_DATA_DIRS` also picks up apps installed as
    /// Flatpaks that the native listing only sees if exported.
    mod host {
        use super::{AppEntry, icon_mime, parse_desktop_entry};
        use std::collections::{HashMap, HashSet};

        /// Separates records in the scripts' output: an ASCII record
        /// separator can't appear in a desktop file or base64.
        const RECORD: char = '\x1e';

        /// Prints every `applications/*.desktop` file in the host's data dirs
        /// as `RECORD <file name>\n<contents>`, in the same order as the
        /// native `application_dirs()`.
        const LIST_DESKTOP_FILES: &str = r#"
data_home=${XDG_DATA_HOME:-$HOME/.local/share}
IFS=:
for dir in ${XDG_DATA_DIRS:-/usr/local/share:/usr/share} $data_home; do
    for file in "$dir"/applications/*.desktop; do
        [ -f "$file" ] || continue
        printf '\036%s\n' "${file##*/}"
        cat "$file"
        echo
    done
done
"#;

        /// For each icon name argument, prints `RECORD <name>\n<ext>\n<base64>`
        /// for the first file found, probing the same theme/size layout as
        /// the native `find_icon_file`.
        const READ_ICONS: &str = r#"
nl='
'
data_home=${XDG_DATA_HOME:-$HOME/.local/share}
bases="$data_home/icons$nl$HOME/.icons"
IFS=:
for dir in ${XDG_DATA_DIRS:-/usr/local/share:/usr/share}; do
    bases="$bases$nl$dir/icons"
done
IFS=$nl

find_icon() {
    case $1 in
        /*) [ -f "$1" ] && printf '%s' "$1"; return ;;
    esac
    for base in $bases; do
        for theme in hicolor Adwaita gnome breeze Papirus; do
            for size in scalable 256x256 128x128 96x96 64x64 48x48 32x32; do
                for ext in svg png; do
                    if [ -f "$base/$theme/$size/apps/$1.$ext" ]; then
                        printf '%s' "$base/$theme/$size/apps/$1.$ext"
                        return
                    fi
                done
            done
        done
    done
    for dir in /usr/share/pixmaps /usr/local/share/pixmaps; do
        for ext in svg png; do
            if [ -f "$dir/$1.$ext" ]; then
                printf '%s' "$dir/$1.$ext"
                return
            fi
        done
    done
}

for name in "$@"; do
    file=$(find_icon "$name")
    [ -n "$file" ] || continue
    printf '\036%s\n%s\n' "$name" "${file##*.}"
    base64 "$file" | tr -d '\n'
done
"#;

        pub(super) fn list_apps() -> Vec<AppEntry> {
            let Some(output) = run_host_script(LIST_DESKTOP_FILES, &[]) else {
                return Vec::new();
            };

            let mut seen_ids = HashSet::new();
            let mut entries = Vec::new();
            for record in output.split(RECORD).skip(1) {
                let Some((id, content)) = record.split_once('\n') else {
                    continue;
                };
                if !seen_ids.insert(id) {
                    continue;
                }
                if let Some(entry) = parse_desktop_entry(id, content) {
                    entries.push(entry);
                }
            }

            let icon_names: Vec<&str> = entries
                .iter()
                .filter_map(|e| e.icon.as_deref())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let icons = read_icons(&icon_names);

            entries
                .into_iter()
                .map(|entry| AppEntry {
                    icon: entry.icon.and_then(|i| icons.get(&i).cloned()),
                    name: entry.name,
                    command: entry.command,
                    window: entry.window,
                    process: entry.process,
                })
                .collect()
        }

        /// Icon name → `data:` URI, for every name the host had a usable file for.
        fn read_icons(names: &[&str]) -> HashMap<String, String> {
            let mut icons = HashMap::new();
            if names.is_empty() {
                return icons;
            }
            let Some(output) = run_host_script(READ_ICONS, names) else {
                return icons;
            };
            for record in output.split(RECORD).skip(1) {
                let mut lines = record.splitn(3, '\n');
                let (Some(name), Some(ext), Some(data)) =
                    (lines.next(), lines.next(), lines.next())
                else {
                    continue;
                };
                if let Some(mime) = icon_mime(ext) {
                    icons.insert(name.to_string(), format!("data:{mime};base64,{data}"));
                }
            }
            icons
        }

        fn run_host_script(script: &str, args: &[&str]) -> Option<String> {
            let output = blockwork_core::flatpak::host_command("sh")
                .args(["-c", script, "sh"])
                .args(args)
                .output()
                .inspect_err(|e| tracing::warn!("Failed to list host apps: {e}"))
                .ok()?;
            if !output.status.success() {
                tracing::warn!(
                    "Listing host apps failed ({}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::AppEntry;
    use std::path::Path;

    /// Start Menu shortcuts, for the current user and "all users" - no icon
    /// (see module doc comment); `command` is the `.lnk` path itself, which
    /// `runner::open_app`'s `cmd /C start "" <path>` launches directly.
    pub(crate) fn list_apps() -> Vec<AppEntry> {
        let mut dirs = Vec::new();
        if let Ok(program_data) = std::env::var("ProgramData") {
            dirs.push(Path::new(&program_data).join("Microsoft/Windows/Start Menu/Programs"));
        }
        if let Ok(app_data) = std::env::var("APPDATA") {
            dirs.push(Path::new(&app_data).join("Microsoft/Windows/Start Menu/Programs"));
        }

        let mut apps = Vec::new();
        for dir in dirs {
            walk(&dir, &mut apps);
        }
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }

    fn walk(dir: &Path, out: &mut Vec<AppEntry>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
                continue;
            }
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("lnk"))
                != Some(true)
            {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Skip the noisy "Uninstall X"/"X Website" shortcuts most
            // installers also drop into the Start Menu next to the real app.
            let lower = stem.to_lowercase();
            if lower.starts_with("uninstall") || lower.contains("website") {
                continue;
            }
            // Windows name an app by its executable; a shortcut whose
            // target can't be read falls back to its own name.
            let program = std::fs::read(&path)
                .ok()
                .and_then(|bytes| super::shortcut_target(&bytes))
                .and_then(|target| {
                    Path::new(&target)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| stem.to_string());
            out.push(AppEntry {
                name: stem.to_string(),
                command: path.to_string_lossy().to_string(),
                icon: None,
                window: program.clone(),
                process: program,
            });
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::AppEntry;
    use std::path::Path;

    /// macOS splits applications between user, system and Cryptex locations.
    /// Walk category folders such as Utilities, but never enter an app bundle
    /// (which would expose its helper apps as independent launch targets).
    pub(crate) fn list_apps() -> Vec<AppEntry> {
        let mut roots = vec![
            Path::new("/Applications").to_path_buf(),
            Path::new("/System/Applications").to_path_buf(),
            Path::new("/System/Library/CoreServices/Applications").to_path_buf(),
            Path::new("/System/Volumes/Preboot/Cryptexes/App/System/Applications").to_path_buf(),
            Path::new("/System/Library/CoreServices/Finder.app").to_path_buf(),
        ];
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join("Applications"));
        }
        list_in(&roots)
    }

    fn list_in(roots: &[std::path::PathBuf]) -> Vec<AppEntry> {
        // Folders reached through a symlink can turn up again at a shallower
        // depth, so remember the shallowest visit; apps are listed once.
        let mut seen = std::collections::HashMap::new();
        let mut listed = std::collections::HashSet::new();
        // Category folders (Utilities, a vendor's suite) are shallow. The cap
        // keeps a large non-app tree under ~/Applications from being walked.
        const MAX_DEPTH: usize = 3;
        let mut pending: Vec<_> = roots.iter().map(|root| (root.clone(), 0)).collect();
        let mut apps = Vec::new();
        while let Some((path, depth)) = pending.pop() {
            if !path.is_dir() {
                continue;
            }
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            if seen
                .get(&canonical)
                .is_some_and(|&visited| visited <= depth)
            {
                continue;
            }
            seen.insert(canonical.clone(), depth);
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
            {
                if !listed.insert(canonical) {
                    continue;
                }
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                apps.push(AppEntry {
                    name: name.into(),
                    command: path.to_string_lossy().into_owned(),
                    icon: None,
                    window: name.into(),
                    process: name.into(),
                });
                continue;
            }
            // Renamed bundles (for example Editor.app.backup) still contain
            // helpers. They aren't category folders or independent apps.
            if depth >= MAX_DEPTH
                || path.join("Contents/Info.plist").is_file()
                || path.join("Contents/MacOS").is_dir()
            {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                if !entry.file_name().to_string_lossy().starts_with('.') {
                    pending.push((entry.path(), depth + 1));
                }
            }
        }
        apps.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.command.cmp(&b.command))
        });
        apps
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn includes_system_and_utility_apps_without_helpers_or_symlink_duplicates() {
            let root =
                std::env::temp_dir().join(format!("blockwork-app-picker-{}", uuid::Uuid::new_v4()));
            let user = root.join("Applications");
            let system = root.join("System/Applications");
            for path in [
                user.join("Editor.app/Contents/Helpers/Helper.app"),
                user.join("Editor.app.backup/Contents/Helpers/Old Helper.app"),
                system.join("Notes.app"),
                system.join("Utilities/Terminal.app"),
                user.join("a/b/c/Too Deep.app"),
            ] {
                std::fs::create_dir_all(path).unwrap();
            }
            std::fs::write(
                user.join("Editor.app.backup/Contents/Info.plist"),
                "<plist/>",
            )
            .unwrap();
            std::os::unix::fs::symlink(&user, root.join("Alias")).unwrap();
            std::os::unix::fs::symlink(&root, user.join("Loop")).unwrap();
            let apps = list_in(&[user, system, root.join("Alias")]);
            assert_eq!(
                apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
                ["Editor", "Notes", "Terminal"]
            );
            assert!(apps.iter().all(|a| Path::new(&a.command).is_dir()));
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shortcut_target;

    fn shortcut(base: &str, suffix: &str) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x4c];
        bytes[0] = 0x4c;
        bytes[0x14] = 0x2;
        let base_at = 0x1c;
        let suffix_at = base_at + base.len() + 1;
        let size = suffix_at + suffix.len() + 1;
        for value in [size, 0x1c, 1, 0, base_at, 0, suffix_at] {
            bytes.extend_from_slice(&(value as u32).to_le_bytes());
        }
        for text in [base, suffix] {
            bytes.extend_from_slice(text.as_bytes());
            bytes.push(0);
        }
        bytes
    }

    #[test]
    fn reads_a_shortcuts_local_target() {
        let lnk = shortcut(r"C:\Program Files\Google\Chrome\Application\chrome.exe", "");
        assert_eq!(
            shortcut_target(&lnk).as_deref(),
            Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe")
        );
        assert_eq!(shortcut_target(&lnk[..0x50]), None);
        let mut advertised = lnk.clone();
        advertised[0x14] = 0;
        assert_eq!(shortcut_target(&advertised), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_entries_name_their_window_and_program() {
        let entry = |id: &str, body: &str| {
            let text = format!("[Desktop Entry]\nType=Application\nName=App\n{body}");
            let e = super::linux::parse_desktop_entry_for_test(id, &text).unwrap();
            (e.0, e.1)
        };
        assert_eq!(
            entry("org.kde.kate.desktop", "Exec=kate -b %U"),
            ("org.kde.kate".into(), "kate".into())
        );
        assert_eq!(
            entry(
                "google-chrome.desktop",
                "Exec=/usr/bin/google-chrome-stable %U\nStartupWMClass=Google-chrome"
            ),
            ("Google-chrome".into(), "google-chrome-stable".into())
        );
        assert_eq!(
            entry("x.desktop", "Exec=env GDK_BACKEND=x11 /opt/tool/bin/tool"),
            ("x".into(), "tool".into())
        );
        assert_eq!(
            entry(
                "org.mozilla.firefox.desktop",
                "Exec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=firefox --file-forwarding org.mozilla.firefox @@u %u @@"
            ),
            ("org.mozilla.firefox".into(), "firefox".into())
        );
        assert_eq!(
            entry(
                "org.gnome.Calculator.desktop",
                "Exec=flatpak run org.gnome.Calculator"
            ),
            ("org.gnome.Calculator".into(), "calculator".into())
        );
    }
}
