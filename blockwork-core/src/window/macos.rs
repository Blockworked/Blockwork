//! Native window queries and controls. Avoid System Events/Apple Events:
//! those need a second Automation grant and can stall waiting for its UI.
use super::{Action, ActiveWindow, MatchBy, Rect};
use core_foundation::{
    array::CFArray,
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use core_graphics::{
    geometry::{CGPoint, CGSize},
    window::{
        copy_window_info, kCGNullWindowID, kCGWindowListOptionAll, kCGWindowListOptionOnScreenOnly,
    },
};
use std::collections::HashSet;
use std::ffi::c_void;

type Element = CFTypeRef;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateSystemWide() -> Element;
    fn AXUIElementCreateApplication(pid: i32) -> Element;
    fn AXUIElementGetPid(element: Element, pid: *mut i32) -> i32;
    fn AXUIElementCopyAttributeValue(
        element: Element,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: Element,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXUIElementPerformAction(element: Element, action: CFStringRef) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Element, seconds: f32) -> i32;
    fn AXValueCreate(kind: u32, value: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, kind: u32, output: *mut c_void) -> bool;
}

fn error(code: i32) -> String {
    match code {
        -25211 => {
            "Blockwork needs Accessibility access under Privacy & Security > Accessibility".into()
        }
        -25204 => {
            "the window's application did not respond to Accessibility within one second".into()
        }
        -25205 | -25206 | -25212 => "the window does not support this operation".into(),
        _ => format!("macOS window operation failed (Accessibility error {code})"),
    }
}

fn check(code: i32) -> Result<(), String> {
    if code == 0 { Ok(()) } else { Err(error(code)) }
}

fn trusted() -> Result<(), String> {
    if unsafe { AXIsProcessTrusted() } {
        Ok(())
    } else {
        Err(error(-25211))
    }
}

fn attribute(element: &CFType, name: &str) -> Result<CFType, String> {
    let name = CFString::new(name);
    let mut value = std::ptr::null();
    check(unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    })?;
    if value.is_null() {
        return Err("macOS returned an empty window attribute".into());
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

fn text_attribute(element: &CFType, name: &str) -> String {
    attribute(element, name)
        .ok()
        .and_then(|v| v.downcast::<CFString>())
        .map(|v| v.to_string())
        .unwrap_or_default()
}

fn set(element: &CFType, name: &str, value: &CFType) -> Result<(), String> {
    check(unsafe {
        AXUIElementSetAttributeValue(
            element.as_CFTypeRef(),
            CFString::new(name).as_concrete_TypeRef(),
            value.as_CFTypeRef(),
        )
    })
}

fn perform(element: &CFType, action: &str) -> Result<(), String> {
    check(unsafe {
        AXUIElementPerformAction(
            element.as_CFTypeRef(),
            CFString::new(action).as_concrete_TypeRef(),
        )
    })
}

#[derive(Clone)]
struct Window {
    pid: i32,
    info: ActiveWindow,
    rect: Rect,
}

type Dictionary = CFDictionary;
fn value(dict: &Dictionary, name: &str) -> Option<CFType> {
    dict.find(CFString::new(name).as_CFTypeRef())
        .map(|v| unsafe { CFType::wrap_under_get_rule(*v) })
}
fn number(dict: &Dictionary, name: &str) -> Option<f64> {
    value(dict, name)?.downcast::<CFNumber>()?.to_f64()
}
fn string(dict: &Dictionary, name: &str) -> String {
    value(dict, name)
        .and_then(|v| v.downcast::<CFString>())
        .map(|v| v.to_string())
        .unwrap_or_default()
}

fn windows(on_screen: bool) -> Result<Vec<Window>, String> {
    let list = copy_window_info(
        if on_screen {
            kCGWindowListOptionOnScreenOnly
        } else {
            kCGWindowListOptionAll
        },
        kCGNullWindowID,
    )
    .ok_or("couldn't enumerate macOS windows")?;
    let mut result = Vec::new();
    for item in list.iter() {
        let item = unsafe { CFType::wrap_under_get_rule(*item) };
        let Some(dict) = item.downcast::<Dictionary>() else {
            continue;
        };
        if number(&dict, "kCGWindowLayer") != Some(0.0) {
            continue;
        }
        let Some(pid) = number(&dict, "kCGWindowOwnerPID") else {
            continue;
        };
        let Some(bounds) = value(&dict, "kCGWindowBounds").and_then(|v| v.downcast::<Dictionary>())
        else {
            continue;
        };
        let rect = Rect {
            x: number(&bounds, "X").unwrap_or(0.0),
            y: number(&bounds, "Y").unwrap_or(0.0),
            width: number(&bounds, "Width").unwrap_or(0.0),
            height: number(&bounds, "Height").unwrap_or(0.0),
        };
        if rect.width < 1.0 || rect.height < 1.0 {
            continue;
        }
        result.push(Window {
            pid: pid as i32,
            rect,
            info: ActiveWindow {
                app: string(&dict, "kCGWindowOwnerName"),
                title: string(&dict, "kCGWindowName"),
            },
        });
    }
    Ok(result)
}

fn application(pid: i32) -> Result<CFType, String> {
    trusted()?;
    let raw = unsafe { AXUIElementCreateApplication(pid) };
    if raw.is_null() {
        return Err("the window's application has exited".into());
    }
    let app = unsafe { CFType::wrap_under_create_rule(raw) };
    check(unsafe { AXUIElementSetMessagingTimeout(app.as_CFTypeRef(), 1.0) })?;
    Ok(app)
}

fn focused() -> Result<(CFType, Option<CFType>, i32), String> {
    trusted()?;
    let system = unsafe { CFType::wrap_under_create_rule(AXUIElementCreateSystemWide()) };
    check(unsafe { AXUIElementSetMessagingTimeout(system.as_CFTypeRef(), 1.0) })?;
    let app = attribute(&system, "AXFocusedApplication")?;
    let mut pid = 0;
    check(unsafe { AXUIElementGetPid(app.as_CFTypeRef(), &mut pid) })?;
    let window = attribute(&app, "AXFocusedWindow").ok();
    Ok((app, window, pid))
}

pub fn active() -> Result<ActiveWindow, String> {
    let (_, window, pid) = focused()?;
    let app = windows(false)?
        .into_iter()
        .find(|w| w.pid == pid)
        .map(|w| w.info.app)
        .unwrap_or_default();
    Ok(ActiveWindow {
        app,
        title: window
            .map(|w| text_attribute(&w, "AXTitle"))
            .unwrap_or_default(),
    })
}

fn ax_bounds(window: &CFType) -> Result<Rect, String> {
    let position = attribute(window, "AXPosition")?;
    let size = attribute(window, "AXSize")?;
    let mut point = CGPoint::new(0.0, 0.0);
    let mut dimensions = CGSize::new(0.0, 0.0);
    if !unsafe {
        AXValueGetValue(
            position.as_CFTypeRef(),
            1,
            &mut point as *mut _ as *mut c_void,
        )
    } || !unsafe {
        AXValueGetValue(
            size.as_CFTypeRef(),
            2,
            &mut dimensions as *mut _ as *mut c_void,
        )
    } {
        return Err("macOS returned invalid window bounds".into());
    }
    Ok(Rect {
        x: point.x,
        y: point.y,
        width: dimensions.width,
        height: dimensions.height,
    })
}

fn find(pattern: &str, by: MatchBy) -> Result<(CFType, CFType), String> {
    if pattern.trim().is_empty() {
        let (app, window, _) = focused()?;
        return Ok((app, window.ok_or("the active application has no window")?));
    }
    trusted()?;
    // List windows front-to-back, and ask each owning app only once. AX titles
    // remain available when minimized and without a Screen Recording grant.
    let mut seen = HashSet::new();
    let mut candidates: Vec<_> = windows(false)?
        .into_iter()
        .map(|w| (w.pid, w.info.app))
        .collect();
    // During a fullscreen Space transition WindowServer can temporarily omit
    // every window owned by the frontmost app. Its Accessibility window tree
    // is still available, so don't make CG enumeration the only way to find it.
    if let Ok((app, _, pid)) = focused() {
        if !candidates.iter().any(|(candidate, _)| *candidate == pid) {
            candidates.insert(0, (pid, text_attribute(&app, "AXTitle")));
        }
    }
    for (pid, name) in candidates {
        if !seen.insert(pid) {
            continue;
        }
        let app = application(pid)?;
        let Ok(list) = attribute(&app, "AXWindows") else {
            continue;
        };
        let Some(list) = list.downcast::<CFArray>() else {
            continue;
        };
        for window in list.iter() {
            let window = unsafe { CFType::wrap_under_get_rule(*window) };
            let info = ActiveWindow {
                app: name.clone(),
                title: text_attribute(&window, "AXTitle"),
            };
            if info.matches(pattern, by) {
                return Ok((app, window));
            }
        }
    }
    Err(format!("no window matches '{pattern}'"))
}

pub fn bounds(pattern: &str, by: MatchBy) -> Result<Rect, String> {
    // Capture only visible windows; WindowServer provides geometry directly,
    // without launching a script or making an Accessibility round trip.
    if let Some(window) = windows(true)?
        .into_iter()
        .find(|w| !pattern.trim().is_empty() && w.info.matches(pattern, by))
    {
        return Ok(window.rect);
    }
    let (_, window) = find(pattern, by)?;
    if attribute(&window, "AXMinimized")
        .ok()
        .and_then(|v| v.downcast::<CFBoolean>())
        .is_some_and(|v| v.into())
    {
        return Err("the window is minimized or hidden".into());
    }
    ax_bounds(&window)
}

pub fn exists(pattern: &str) -> Result<bool, String> {
    if pattern.trim().is_empty() {
        return Ok(true);
    }
    // WindowServer can retain a closed fullscreen window (including its title)
    // while AppKit no longer lists it as an actionable window. Prefer the AX
    // tree when available so "window exists" agrees with the action blocks.
    // Keep read-only discovery working when Accessibility hasn't been granted.
    if !unsafe { AXIsProcessTrusted() }
        && windows(false)?
            .iter()
            .any(|w| w.info.matches(pattern, MatchBy::Any))
    {
        return Ok(true);
    }
    match find(pattern, MatchBy::Any) {
        Ok(_) => Ok(true),
        Err(e) if e.starts_with("no window matches") => Ok(false),
        Err(e) => Err(e),
    }
}

fn fullscreen(window: &CFType, enabled: bool, pattern: &str, by: MatchBy) -> Result<(), String> {
    let value = if enabled {
        CFBoolean::true_value()
    } else {
        CFBoolean::false_value()
    };
    set(window, "AXFullScreen", &value.as_CFType())?;
    let title = text_attribute(window, "AXTitle");
    let mut pid = 0;
    check(unsafe { AXUIElementGetPid(window.as_CFTypeRef(), &mut pid) })?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let applied = attribute(window, "AXFullScreen")
            .ok()
            .and_then(|v| v.downcast::<CFBoolean>())
            .is_some_and(|v| bool::from(v) == enabled);
        // A successful setter only starts the Space animation. Wait until the
        // target is discoverable again before allowing the next window block.
        if applied {
            if let Ok((_, current)) = find(pattern, by) {
                let mut current_pid = 0;
                check(unsafe { AXUIElementGetPid(current.as_CFTypeRef(), &mut current_pid) })?;
                if current_pid == pid && text_attribute(&current, "AXTitle") == title {
                    return Ok(());
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(
                "the application did not finish its fullscreen transition within five seconds"
                    .into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
    let (app, window) = find(pattern, by)?;
    let yes = CFBoolean::true_value().as_CFType();
    let no = CFBoolean::false_value().as_CFType();
    match action {
        Action::Focus => {
            set(&window, "AXMinimized", &no)?;
            set(&app, "AXFrontmost", &yes)?;
            perform(&window, "AXRaise")?;
            let mut target_pid = 0;
            check(unsafe { AXUIElementGetPid(app.as_CFTypeRef(), &mut target_pid) })?;
            let title = text_attribute(&window, "AXTitle");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            loop {
                if let Ok((_, Some(current), pid)) = focused() {
                    if pid == target_pid && text_attribute(&current, "AXTitle") == title {
                        return Ok(());
                    }
                }
                if std::time::Instant::now() >= deadline {
                    return Err(
                        "the application did not focus the requested window within two seconds"
                            .into(),
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Action::Minimize => set(&window, "AXMinimized", &yes),
        Action::Maximize => fullscreen(&window, true, pattern, by),
        Action::Restore => {
            set(&window, "AXMinimized", &no)?;
            match fullscreen(&window, false, pattern, by) {
                Err(e) if e == error(-25205) => Ok(()),
                result => result,
            }
        }
        Action::Close => perform(&attribute(&window, "AXCloseButton")?, "AXPress"),
    }
}

pub fn place(pattern: &str, by: MatchBy, resize: bool, a: i32, b: i32) -> Result<(), String> {
    let (_, window) = find(pattern, by)?;
    let before = ax_bounds(&window)?;
    if resize && (a < 1 || b < 1) {
        return Err("window dimensions must be positive".into());
    }
    let point = CGPoint::new(a as f64, b as f64);
    let size = CGSize::new(a as f64, b as f64);
    let raw = unsafe {
        if resize {
            AXValueCreate(2, &size as *const _ as *const c_void)
        } else {
            AXValueCreate(1, &point as *const _ as *const c_void)
        }
    };
    if raw.is_null() {
        return Err("couldn't create macOS window coordinates".into());
    }
    let value = unsafe { CFType::wrap_under_create_rule(raw) };
    set(
        &window,
        if resize { "AXSize" } else { "AXPosition" },
        &value,
    )?;
    if attribute(&window, "AXMinimized")
        .ok()
        .and_then(|v| v.downcast::<CFBoolean>())
        .is_some_and(bool::from)
    {
        return Ok(());
    }
    // AX setters return before AppKit commits its frame to WindowServer. A
    // subsequent size setter can otherwise reuse the old frame and undo a move.
    let mut pid = 0;
    check(unsafe { AXUIElementGetPid(window.as_CFTypeRef(), &mut pid) })?;
    let title = text_attribute(&window, "AXTitle");
    let expected = if resize {
        Rect {
            width: a as f64,
            height: b as f64,
            ..before
        }
    } else {
        Rect {
            x: a as f64,
            y: b as f64,
            ..before
        }
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut restored_origin = false;
    loop {
        let live_ax = ax_bounds(&window)?;
        if let Some(current) = windows(false)?.into_iter().find(|w| {
            w.pid == pid
                && (w.info.title == title ||
                    // macOS redacts other apps' CG titles without Screen
                    // Recording, but AX control and CG bounds remain usable.
                    (w.info.title.is_empty() &&
                        (w.rect.x - live_ax.x).abs() < 1.0 &&
                        (w.rect.y - live_ax.y).abs() < 1.0 &&
                        (w.rect.width - live_ax.width).abs() < 1.0 &&
                        (w.rect.height - live_ax.height).abs() < 1.0))
        }) {
            let r = current.rect;
            let position_matches = (r.x - expected.x).abs() < 1.0 && (r.y - expected.y).abs() < 1.0;
            let size_matches =
                (r.width - expected.width).abs() < 1.0 && (r.height - expected.height).abs() < 1.0;
            if position_matches && size_matches {
                return Ok(());
            }
            if resize && size_matches && !position_matches && !restored_origin {
                let origin = CGPoint::new(before.x, before.y);
                let raw = unsafe { AXValueCreate(1, &origin as *const _ as *const c_void) };
                if raw.is_null() {
                    return Err("couldn't preserve the window position".into());
                }
                let origin = unsafe { CFType::wrap_under_create_rule(raw) };
                set(&window, "AXPosition", &origin)?;
                restored_origin = true;
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(
                "the application did not apply the requested window geometry within two seconds"
                    .into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "moves, resizes and minimizes the dedicated native fixture"]
    fn live_window_controls_restore_the_fixture() {
        let title = std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW");
        assert_eq!(
            title, "Blockwork macOS Test Window",
            "only manipulate the dedicated fixture"
        );
        let original = bounds(&title, MatchBy::Title).unwrap();
        struct Restore(String, Rect);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = act(Action::Focus, &self.0, MatchBy::Title);
                let _ = place(
                    &self.0,
                    MatchBy::Title,
                    true,
                    self.1.width as i32,
                    self.1.height as i32,
                );
                let _ = place(
                    &self.0,
                    MatchBy::Title,
                    false,
                    self.1.x as i32,
                    self.1.y as i32,
                );
            }
        }
        let _restore = Restore(title.clone(), original);
        act(Action::Focus, &title, MatchBy::Title).unwrap();
        assert!(active().unwrap().matches(&title, MatchBy::Title));
        assert!(exists(&title).unwrap());
        place(
            &title,
            MatchBy::Title,
            false,
            original.x as i32 + 30,
            original.y as i32 - 20,
        )
        .unwrap();
        place(&title, MatchBy::Title, true, 680, 480).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let now = bounds(&title, MatchBy::Title).unwrap();
            if (now.x - original.x - 30.0).abs() < 1.0
                && (now.y - original.y + 20.0).abs() < 1.0
                && (now.width - 680.0).abs() < 1.0
                && (now.height - 480.0).abs() < 1.0
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "window geometry did not update: {now:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        act(Action::Minimize, &title, MatchBy::Title).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while bounds(&title, MatchBy::Title).is_ok() {
            assert!(
                std::time::Instant::now() < deadline,
                "window did not minimize"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        act(Action::Focus, &title, MatchBy::Title).unwrap();
        assert!(bounds(&title, MatchBy::Title).is_ok());
    }

    #[test]
    #[ignore = "enters fullscreen, restores and closes only the dedicated native fixture"]
    fn live_window_maximize_restore_and_close_fixture() {
        let title = std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW");
        assert_eq!(title, "Blockwork macOS Test Window");
        struct Restore(String);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = act(Action::Restore, &self.0, MatchBy::Title);
            }
        }
        let _restore = Restore(title.clone());
        let wait_fullscreen = |expected: bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let (_, window) = find(&title, MatchBy::Title).unwrap();
                let actual: bool = attribute(&window, "AXFullScreen")
                    .unwrap()
                    .downcast::<CFBoolean>()
                    .unwrap()
                    .into();
                if actual == expected {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "fullscreen did not become {expected}"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        act(Action::Focus, &title, MatchBy::Title).unwrap();
        act(Action::Maximize, &title, MatchBy::Title).unwrap();
        wait_fullscreen(true);
        act(Action::Restore, &title, MatchBy::Title).unwrap();
        wait_fullscreen(false);
        act(Action::Close, &title, MatchBy::Title).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while exists(&title).unwrap() {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture window did not close: AX exists={}, visible={}",
                find(&title, MatchBy::Title).is_ok(),
                windows(true).unwrap().iter().any(|w| w.info.title == title)
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}
