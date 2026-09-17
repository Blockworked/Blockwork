//! Running inside a Flatpak sandbox, where host programs (other installed
//! apps, `killall`, the user's own shell environment) aren't visible and have
//! to be reached through `flatpak-spawn --host` instead. That needs the
//! `--talk-name=org.freedesktop.Flatpak` permission in the Flatpak manifest.

use std::process::Command;
use std::sync::OnceLock;

/// Whether this process is running inside a Flatpak sandbox. Flatpak always
/// bind-mounts `/.flatpak-info` into the sandbox root, and nothing else does.
pub fn is_flatpak() -> bool {
    static IS_FLATPAK: OnceLock<bool> = OnceLock::new();
    *IS_FLATPAK.get_or_init(|| std::path::Path::new("/.flatpak-info").exists())
}

/// A `Command` that runs `program` on the host when sandboxed, or directly
/// otherwise — so callers can add args and spawn it the same way either way.
pub fn host_command(program: &str) -> Command {
    if is_flatpak() {
        let mut command = Command::new("flatpak-spawn");
        command.args(["--host", program]);
        command
    } else {
        Command::new(program)
    }
}
