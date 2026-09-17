//! Checks whether an installed `openrazer` daemon is reachable by the current
//! user - Linux-only concern (like `installed_apps.rs`'s per-platform split),
//! surfaced as a `WarningBanners.vue` banner the same way `grab_available`
//! surfaces evdev grab failures.

#[cfg(target_os = "linux")]
pub(crate) fn permission_warning() -> bool {
    linux::permission_warning()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn permission_warning() -> bool {
    false
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::sync::OnceLock;

    /// Distro packaging disagrees on which group grants openrazer device
    /// access - Arch/Fedora's package uses `openrazer`, Debian/Ubuntu's
    /// instead piggybacks on `plugdev` - so pass if the user is in either.
    const CANDIDATE_GROUPS: [&str; 2] = ["openrazer", "plugdev"];

    /// Installed-ness and group membership don't change over a running
    /// session, so cache the one-time `/etc/group` + `/proc/self/status`
    /// read instead of redoing it on every state broadcast.
    pub(crate) fn permission_warning() -> bool {
        static WARNING: OnceLock<bool> = OnceLock::new();
        *WARNING.get_or_init(|| daemon_installed() && !in_any_group(&CANDIDATE_GROUPS))
    }

    fn daemon_installed() -> bool {
        let Some(path) = std::env::var_os("PATH") else { return false };
        std::env::split_paths(&path).any(|dir| dir.join("openrazer-daemon").is_file())
    }

    /// Resolves the candidate group names to gids via `/etc/group`, then
    /// checks those against this process's actual supplementary gids from
    /// `/proc/self/status`'s `Groups:` line - the list the kernel enforces,
    /// rather than re-deriving it from `/etc/group` alone.
    fn in_any_group(names: &[&str]) -> bool {
        let target_gids = gids_for_names(names);
        if target_gids.is_empty() {
            return false;
        }
        let Ok(status) = fs::read_to_string("/proc/self/status") else { return false };
        let Some(line) = status.lines().find(|l| l.starts_with("Groups:")) else { return false };
        line.split_whitespace().skip(1).filter_map(|g| g.parse::<u32>().ok()).any(|gid| target_gids.contains(&gid))
    }

    fn gids_for_names(names: &[&str]) -> Vec<u32> {
        let Ok(contents) = fs::read_to_string("/etc/group") else { return Vec::new() };
        contents
            .lines()
            .filter_map(|line| {
                let mut fields = line.split(':');
                let name = fields.next()?;
                if !names.contains(&name) {
                    return None;
                }
                fields.nth(1)?.parse::<u32>().ok()
            })
            .collect()
    }
}
