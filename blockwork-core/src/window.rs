//! Window info and control: the active-window reporters, the `WhenWindow`
//! trigger and the window action blocks. Wayland has no common API for
//! this, so Linux asks the compositor directly where it can (Hyprland, Sway,
//! KDE via kdotool) and falls back to X11 off Wayland.

pub use crate::screen::Rect;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveWindow {
    pub title: String,
    /// App or process name (window class on Linux).
    pub app: String,
}

/// Which part of a window a pattern is matched against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchBy {
    /// The title or the app.
    #[default]
    Any,
    Title,
    App,
}

impl std::str::FromStr for MatchBy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "" | "Any" => Ok(MatchBy::Any),
            "Title" => Ok(MatchBy::Title),
            "App" => Ok(MatchBy::App),
            _ => Err(format!("unknown window match '{s}'")),
        }
    }
}

impl ActiveWindow {
    /// Case-insensitive substring match on the title and/or app; an empty
    /// pattern matches any window.
    pub fn matches(&self, pattern: &str, by: MatchBy) -> bool {
        let pattern = pattern.trim().to_lowercase();
        pattern.is_empty()
            || (by != MatchBy::App && self.title.to_lowercase().contains(&pattern))
            || (by != MatchBy::Title && self.app.to_lowercase().contains(&pattern))
    }
}

/// What the `WindowAction` block can do, by its saved name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Focus,
    Minimize,
    Maximize,
    Restore,
    Close,
}

pub const ACTIONS: [&str; 5] = ["Focus", "Minimize", "Maximize", "Restore", "Close"];

impl std::str::FromStr for Action {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "Focus" => Ok(Action::Focus),
            "Minimize" => Ok(Action::Minimize),
            "Maximize" => Ok(Action::Maximize),
            "Restore" => Ok(Action::Restore),
            "Close" => Ok(Action::Close),
            _ => Err(format!("unknown window action '{s}'")),
        }
    }
}

#[cfg(any(target_os = "linux", windows))]
fn no_match(pattern: &str) -> String {
    format!("no window matches '{}'", pattern.trim())
}

#[cfg(target_os = "linux")]
pub fn active() -> Result<ActiveWindow, String> {
    linux::active()
}

/// Runs `action` on the first window whose title or app (per `by`) contains
/// `pattern`, or on the active window when `pattern` is empty.
#[cfg(target_os = "linux")]
pub fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
    linux::act(action, pattern.trim(), by)
}

/// Moves a matching window to (`a`, `b`), or with `resize` sizes it to `a`
/// by `b` pixels.
#[cfg(target_os = "linux")]
pub fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
    linux::place(pattern.trim(), by, resize, a, b)
}

/// The client area of a matching window (the active one when `pattern` is
/// empty), in screen coordinates.
#[cfg(target_os = "linux")]
pub fn bounds(pattern: &str, by: MatchBy) -> Result<Rect, String> {
    linux::bounds(pattern.trim(), by)
}

/// Whether any window's title or app contains `pattern`.
#[cfg(target_os = "linux")]
pub fn exists(pattern: &str) -> Result<bool, String> {
    linux::exists(pattern.trim())
}

#[cfg(windows)]
pub use windows::{act, active, bounds, exists, place};

#[cfg(target_os = "macos")]
pub use macos::{act, active, bounds, exists, place};

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod unsupported {
    use super::{Action, ActiveWindow, MatchBy, Rect};
    const MSG: &str = "window info isn't supported on this platform";
    pub fn bounds(_: &str, _: MatchBy) -> Result<Rect, String> {
        Err(MSG.into())
    }
    pub fn active() -> Result<ActiveWindow, String> {
        Err(MSG.into())
    }
    pub fn act(_: Action, _: &str, _: MatchBy) -> Result<(), String> {
        Err(MSG.into())
    }
    pub fn place(_: &str, _: MatchBy, _: bool, _: i32, _: i32) -> Result<(), String> {
        Err(MSG.into())
    }
    pub fn exists(_: &str) -> Result<bool, String> {
        Err(MSG.into())
    }
}
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub use unsupported::{act, active, bounds, exists, place};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn sway_restore_brings_minimized_windows_back() {
        let hidden = serde_json::json!({ "workspace": "__i3_scratch" });
        let tiled = serde_json::json!({ "workspace": "1", "scratchpad_state": "none" });
        assert_eq!(
            linux::sway::act_command(
                Action::Restore,
                &serde_json::json!({ "scratchpad_state": "fresh" })
            ),
            "scratchpad show, floating disable"
        );
        assert_eq!(
            linux::sway::act_command(Action::Minimize, &tiled),
            "move scratchpad"
        );
        assert_eq!(
            linux::sway::act_command(Action::Restore, &hidden),
            "scratchpad show, floating disable"
        );
        assert_eq!(
            linux::sway::act_command(Action::Restore, &tiled),
            "fullscreen disable"
        );
    }

    #[test]
    fn matches_title_or_app_ignoring_case() {
        let w = ActiveWindow {
            title: "Inbox - Mail".into(),
            app: "Thunderbird".into(),
        };
        assert!(w.matches("", MatchBy::Title));
        assert!(w.matches("inbox", MatchBy::Any));
        assert!(w.matches("THUNDER", MatchBy::Any));
        assert!(!w.matches("firefox", MatchBy::Any));
        assert!(w.matches("inbox", MatchBy::Title) && !w.matches("inbox", MatchBy::App));
        assert!(w.matches("thunder", MatchBy::App) && !w.matches("thunder", MatchBy::Title));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kdotool_geometry_parses() {
        let out = "Window {5f1e}\n  Position: 120,-40\n  Geometry: 800x600\n";
        assert_eq!(
            linux::kde::parse_geometry(out),
            Ok(Rect {
                x: 120.0,
                y: -40.0,
                width: 800.0,
                height: 600.0
            })
        );
        assert!(linux::kde::parse_geometry("Position: 1,2\n  Geometry: 0x0").is_err());
        assert!(linux::kde::parse_geometry("nonsense").is_err());
        assert_eq!(
            linux::kde::parse_rect("2427,374,480,360"),
            Ok(Rect {
                x: 2427.0,
                y: 374.0,
                width: 480.0,
                height: 360.0
            })
        );
        assert_eq!(
            linux::kde::parse_rect("0,13,1707.5,1067").map(|r| r.width),
            Ok(1707.5)
        );
        assert!(linux::kde::parse_rect("").is_err());
        assert!(linux::kde::parse_rect("1,2,0,0").is_err());
    }

    #[test]
    fn action_names_round_trip() {
        for (i, name) in ACTIONS.iter().enumerate() {
            let action: Action = name.parse().unwrap();
            assert_eq!(action as usize, i);
        }
        assert!("Explode".parse::<Action>().is_err());
    }
}
