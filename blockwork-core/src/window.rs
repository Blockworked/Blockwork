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

/// One top-level window for the agent `windows` list.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    pub title: String,
    pub app: String,
    pub rect: Rect,
    pub focused: bool,
}

/// Which part of a window a pattern is matched against, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchBy {
    /// The title or the app.
    #[default]
    Any,
    Title,
    App,
    /// The whole title, ignoring case.
    Exact,
    /// A regular expression on the title, ignoring case.
    Regex,
}

impl std::str::FromStr for MatchBy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "" | "Any" => Ok(MatchBy::Any),
            "Title" => Ok(MatchBy::Title),
            "App" => Ok(MatchBy::App),
            "Exact" => Ok(MatchBy::Exact),
            "Regex" => Ok(MatchBy::Regex),
            _ => Err(format!("unknown window match '{s}'")),
        }
    }
}

impl ActiveWindow {
    /// Case-insensitive substring match on the title and/or app; an empty
    /// pattern matches any window.
    pub fn matches(&self, pattern: &str, by: MatchBy) -> bool {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return true;
        }
        match by {
            MatchBy::Exact => self.title.trim().to_lowercase() == pattern.to_lowercase(),
            MatchBy::Regex => regex::RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .is_ok_and(|re| re.is_match(&self.title)),
            _ => {
                let pattern = pattern.to_lowercase();
                (by != MatchBy::App && self.title.to_lowercase().contains(&pattern))
                    || (by != MatchBy::Title && self.app.to_lowercase().contains(&pattern))
            }
        }
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
    /// Keeps the window above other windows.
    AlwaysOnTop,
    NotOnTop,
    /// Takes the window off the screen and the taskbar, until `Show`.
    Hide,
    Show,
}

pub const ACTIONS: [&str; 9] = [
    "Focus",
    "Minimize",
    "Maximize",
    "Restore",
    "Close",
    "AlwaysOnTop",
    "NotOnTop",
    "Hide",
    "Show",
];

impl std::str::FromStr for Action {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "Focus" => Ok(Action::Focus),
            "Minimize" => Ok(Action::Minimize),
            "Maximize" => Ok(Action::Maximize),
            "Restore" => Ok(Action::Restore),
            "Close" => Ok(Action::Close),
            "AlwaysOnTop" => Ok(Action::AlwaysOnTop),
            "NotOnTop" => Ok(Action::NotOnTop),
            "Hide" => Ok(Action::Hide),
            "Show" => Ok(Action::Show),
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

#[cfg(target_os = "linux")]
pub fn list_for_agent() -> Result<Vec<WindowInfo>, String> {
    linux::list_for_agent()
}

/// A window on screen, for finding what is under a point.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenWindow {
    pub title: String,
    pub app: String,
    /// 0 when the compositor doesn't say.
    pub pid: u32,
    /// The client area, in screen coordinates.
    pub bounds: Rect,
}

impl ScreenWindow {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let b = &self.bounds;
        x >= b.x && y >= b.y && x < b.x + b.width && y < b.y + b.height
    }
}

/// Every visible window on the current workspace(s), topmost first.
#[cfg(target_os = "linux")]
pub fn stacked() -> Result<Vec<ScreenWindow>, String> {
    linux::stacked()
}

/// Installs the GNOME Shell extension window control needs on GNOME
/// Wayland; says what to do next.
pub fn install_gnome_extension() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    return linux::gnome::install();
    #[cfg(not(target_os = "linux"))]
    Err("the GNOME extension is only for Linux".into())
}

/// Changes a matching window's opacity (0 to 100), title, or monitor
/// (counting from 1, in the order of the monitor reporters).
pub fn set(
    setting: blockwork_model::macros::WindowSetting,
    pattern: &str,
    by: MatchBy,
    value: &str,
) -> Result<(), String> {
    use blockwork_model::macros::WindowSetting;
    let number = || {
        value
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("'{value}' isn't a number"))
    };
    match setting {
        WindowSetting::Opacity => {
            let percent = number()?;
            if !(0.0..=100.0).contains(&percent) {
                return Err(format!("{percent} isn't an opacity from 0 to 100"));
            }
            set_opacity(pattern.trim(), by, percent / 100.0)
        }
        WindowSetting::Title => set_title(pattern.trim(), by, value),
        WindowSetting::Monitor => to_monitor(pattern, by, number()?),
    }
}

/// Moves a window to the same spot on another monitor.
fn to_monitor(pattern: &str, by: MatchBy, n: f64) -> Result<(), String> {
    let monitors = crate::screen::monitors();
    let target = monitors
        .get((n.round() as usize).wrapping_sub(1))
        .ok_or_else(|| format!("there's no monitor {n} ({} found)", monitors.len()))?;
    let window = bounds(pattern, by)?;
    let (cx, cy) = (
        window.x + window.width / 2.0,
        window.y + window.height / 2.0,
    );
    let from = monitors
        .iter()
        .find(|m| cx >= m.x && cy >= m.y && cx < m.x + m.width && cy < m.y + m.height)
        .unwrap_or(target);
    // Keep the same offset, but stay on the target monitor.
    let x = (target.x + (window.x - from.x))
        .clamp(target.x, (target.x + target.width - 50.0).max(target.x));
    let y = (target.y + (window.y - from.y))
        .clamp(target.y, (target.y + target.height - 50.0).max(target.y));
    place(pattern, by, false, x.round() as i32, y.round() as i32)
}

/// Every window, topmost first where the system says.
#[cfg(target_os = "linux")]
pub fn list() -> Result<Vec<ScreenWindow>, String> {
    stacked()
}

#[cfg(target_os = "linux")]
fn set_opacity(pattern: &str, by: MatchBy, opacity: f64) -> Result<(), String> {
    linux::set_opacity(pattern, by, opacity)
}

#[cfg(target_os = "linux")]
fn set_title(pattern: &str, by: MatchBy, title: &str) -> Result<(), String> {
    linux::set_title(pattern, by, title)
}

#[cfg(windows)]
pub use windows::{act, active, bounds, exists, list, list_for_agent, place};
#[cfg(windows)]
pub(crate) use windows::{foreground, handles};
#[cfg(windows)]
use windows::{set_opacity, set_title};

#[cfg(target_os = "macos")]
pub use macos::{act, active, bounds, exists, list_for_agent, place};

#[cfg(not(any(target_os = "linux", windows)))]
pub fn list() -> Result<Vec<ScreenWindow>, String> {
    Err("listing windows isn't supported on this system yet".into())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn set_opacity(_: &str, _: MatchBy, _: f64) -> Result<(), String> {
    Err("changing a window's opacity isn't supported on this system".into())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn set_title(_: &str, _: MatchBy, _: &str) -> Result<(), String> {
    Err("renaming windows isn't supported on this system".into())
}

/// The process ID of the first window matching `pattern`.
pub fn pid(pattern: &str, by: MatchBy) -> Result<u32, String> {
    list()?
        .into_iter()
        .find(|w| {
            ActiveWindow {
                title: w.title.clone(),
                app: w.app.clone(),
            }
            .matches(pattern, by)
        })
        .map(|w| w.pid)
        .ok_or_else(|| format!("no window matches '{}'", pattern.trim()))
}

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
    pub fn list_for_agent() -> Result<Vec<super::WindowInfo>, String> {
        Err(MSG.into())
    }
}
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub use unsupported::{act, active, bounds, exists, list_for_agent, place};

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
        assert!(w.matches("inbox - MAIL", MatchBy::Exact) && !w.matches("inbox", MatchBy::Exact));
        assert!(w.matches("^inbox.*mail$", MatchBy::Regex) && !w.matches("^mail", MatchBy::Regex));
        assert!(!w.matches("(", MatchBy::Regex));
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
