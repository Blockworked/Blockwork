//! Native UI automation through Windows UI Automation. Elements are found
//! by walking each app window's control view, the same way the Linux
//! adapter walks AT-SPI, so selectors mean the same thing on both.
use super::common::{Found, contains, inspected, matches, picked, resolve, with_selector};
use super::*;
use crate::input::value::Evaluated;
use crate::window::ScreenWindow;
use std::cell::RefCell;
use std::collections::VecDeque;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Accessibility::*;
use windows::core::{BSTR, Interface};

const LIMIT_NODES: usize = 4000;
const LIMIT_DEPTH: usize = 64;

type Element = IUIAutomationElement;

thread_local! {
    static AUTOMATION: RefCell<Option<(IUIAutomation, IUIAutomationCacheRequest)>> =
        const { RefCell::new(None) };
}

/// This thread's UI Automation client, and a cache request that fetches
/// everything a selector looks at in one call per element.
fn automation() -> Result<(IUIAutomation, IUIAutomationCacheRequest), String> {
    AUTOMATION.with(|cell| {
        if let Some(made) = cell.borrow().as_ref() {
            return Ok(made.clone());
        }
        let fail = |e: windows::core::Error| format!("UI Automation unavailable: {e}");
        // SAFETY: COM is set up on this thread before any object is made;
        // the objects release themselves when dropped.
        let made = unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).map_err(fail)?;
            // A hung app shouldn't hang the macro for long.
            if let Ok(two) = automation.cast::<IUIAutomation2>() {
                let _ = two.SetConnectionTimeout(2000);
                let _ = two.SetTransactionTimeout(3000);
            }
            let request = automation.CreateCacheRequest().map_err(fail)?;
            for property in [
                UIA_NamePropertyId,
                UIA_ControlTypePropertyId,
                UIA_AutomationIdPropertyId,
                UIA_IsEnabledPropertyId,
                UIA_IsOffscreenPropertyId,
                UIA_IsPasswordPropertyId,
                UIA_BoundingRectanglePropertyId,
            ] {
                request.AddProperty(property).map_err(fail)?;
            }
            (automation, request)
        };
        *cell.borrow_mut() = Some(made.clone());
        Ok(made)
    })
}

/// Control types by name, from `UIA_ButtonControlTypeId` (50000) on.
const ROLES: [&str; 41] = [
    "button",
    "calendar",
    "check box",
    "combo box",
    "edit",
    "hyperlink",
    "image",
    "list item",
    "list",
    "menu",
    "menu bar",
    "menu item",
    "progress bar",
    "radio button",
    "scroll bar",
    "slider",
    "spinner",
    "status bar",
    "tab",
    "tab item",
    "text",
    "tool bar",
    "tool tip",
    "tree",
    "tree item",
    "custom",
    "group",
    "thumb",
    "data grid",
    "data item",
    "document",
    "split button",
    "window",
    "pane",
    "header",
    "header item",
    "table",
    "title bar",
    "separator",
    "semantic zoom",
    "app bar",
];

fn role_name(control_type: i32) -> &'static str {
    usize::try_from(control_type - UIA_ButtonControlTypeId.0)
        .ok()
        .and_then(|i| ROLES.get(i))
        .copied()
        .unwrap_or("")
}

/// What the cache holds for `element`: its node, whether it's on screen,
/// and whether it's a password field.
fn describe(element: &Element, app: &str, window: &str) -> (UiNode, bool, bool) {
    // SAFETY: cached reads on a live element; failures fall back to empty.
    unsafe {
        let text = |r: windows::core::Result<BSTR>| r.map(|b| b.to_string()).unwrap_or_default();
        let node = UiNode {
            app: app.into(),
            window: window.into(),
            name: text(element.CachedName()),
            role: role_name(element.CachedControlType().map(|c| c.0).unwrap_or(0)).into(),
            id: text(element.CachedAutomationId()),
            enabled: element.CachedIsEnabled().is_ok_and(|b| b.as_bool()),
            selector: UiSelector::default(),
            text: String::new(),
            matches: 0,
        };
        let showing = !element.CachedIsOffscreen().is_ok_and(|b| b.as_bool());
        let password = element.CachedIsPassword().is_ok_and(|b| b.as_bool());
        (node, showing, password)
    }
}

/// `element`'s children in the control view, with the cache filled.
fn children(
    automation: &IUIAutomation,
    request: &IUIAutomationCacheRequest,
    element: &Element,
) -> Vec<Element> {
    let mut out = vec![];
    // SAFETY: walker calls on live elements; each returns a new reference.
    unsafe {
        let Ok(walker) = automation.ControlViewWalker() else {
            return out;
        };
        let mut next = walker.GetFirstChildElementBuildCache(element, request);
        while let Ok(child) = next {
            next = walker.GetNextSiblingElementBuildCache(&child, request);
            out.push(child);
            if out.len() > LIMIT_NODES {
                break;
            }
        }
    }
    out
}

fn window_element(
    automation: &IUIAutomation,
    request: &IUIAutomationCacheRequest,
    hwnd: isize,
) -> Option<Element> {
    // SAFETY: the handle came from EnumWindows; a closed window just fails.
    unsafe {
        automation
            .ElementFromHandle(HWND(hwnd as *mut _))
            .and_then(|e| e.BuildUpdatedCache(request))
            .ok()
    }
}

/// Every element matching `selector`, breadth first per window, at most
/// `limit`.
fn find(selector: &UiSelector, limit: usize) -> Result<Vec<Found<Element>>, String> {
    let (automation, request) = automation()?;
    let mut windows = crate::window::handles();
    let mut window_filter = selector.window.clone();
    if selector.scope == UiScope::Focused {
        let front = crate::window::foreground();
        windows.retain(|(hwnd, _)| *hwnd == front);
        if windows.is_empty() {
            return Err(
                "Couldn't tell which window has focus; pick the element so its app is saved".into(),
            );
        }
        window_filter.clear();
    }
    windows.retain(|(_, w)| contains(&w.app, &selector.app));
    let mut seen = 0usize;
    let mut result = vec![];
    for (hwnd, window) in windows {
        let Some(root) = window_element(&automation, &request, hwnd) else {
            continue;
        };
        let mut queue = VecDeque::from([(root, window.title.clone(), 0usize)]);
        while let Some((element, mut title, depth)) = queue.pop_front() {
            seen += 1;
            if seen > LIMIT_NODES {
                return Err(
                    "UI Automation search exceeded 4,000 elements; narrow the app or window filter"
                        .into(),
                );
            }
            if depth > LIMIT_DEPTH {
                return Err("UI Automation search exceeded 64 levels; narrow the selector".into());
            }
            let (mut node, showing, password) = describe(&element, &window.app, &title);
            if password {
                continue;
            }
            // A dialog inside the window names the elements under it.
            if depth > 0 && node.role == "window" {
                title = node.name.clone();
                node.window = title.clone();
            }
            if matches(selector, &window_filter, &node) {
                result.push(Found {
                    object: element.clone(),
                    node: with_selector(node),
                    showing,
                });
                if result.len() >= limit {
                    return Ok(result);
                }
            }
            for child in children(&automation, &request, &element) {
                queue.push_back((child, title.clone(), depth + 1));
            }
        }
    }
    Ok(result)
}

pub fn applications() -> Result<Vec<String>, String> {
    let mut names: Vec<String> = crate::window::handles()
        .into_iter()
        .map(|(_, w)| w.app)
        .filter(|a| !a.is_empty())
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

pub fn inspect(selector: &UiSelector) -> Result<Vec<UiNode>, String> {
    let everything = UiSelector {
        scope: UiScope::App,
        app: selector.app.clone(),
        ..UiSelector::default()
    };
    let all = find(&everything, LIMIT_NODES)?;
    Ok(inspected(&all, selector, ""))
}

fn pattern<T: Interface>(element: &Element, id: UIA_PATTERN_ID) -> Option<T> {
    // SAFETY: asks a live element for a pattern; unsupported ones fail.
    unsafe { element.GetCurrentPatternAs::<T>(id) }.ok()
}

pub fn query(selector: &UiSelector, query: Query) -> Result<Evaluated, String> {
    let target = resolve(find(selector, 20)?)?;
    if matches!(query, Query::Exists) {
        return Ok(Evaluated::Bool(target.is_some()));
    }
    let Some(Found { object, node, .. }) = target else {
        return if matches!(query, Query::Enabled) {
            Ok(Evaluated::Bool(false))
        } else {
            Err("No UI element matches this selector".into())
        };
    };
    match query {
        Query::Exists => unreachable!(),
        Query::Enabled => Ok(Evaluated::Bool(node.enabled)),
        Query::Text => {
            // SAFETY: pattern calls on a live element.
            unsafe {
                if let Some(text) = pattern::<IUIAutomationTextPattern>(&object, UIA_TextPatternId)
                    && let Ok(read) = text.DocumentRange().and_then(|r| r.GetText(65536))
                {
                    return Ok(Evaluated::Text(read.to_string()));
                }
                if let Some(value) =
                    pattern::<IUIAutomationValuePattern>(&object, UIA_ValuePatternId)
                    && let Ok(read) = value.CurrentValue()
                {
                    return Ok(Evaluated::Text(read.to_string()));
                }
            }
            Err("Element has no readable text".into())
        }
        Query::Value => {
            let range = pattern::<IUIAutomationRangeValuePattern>(&object, UIA_RangeValuePatternId)
                .ok_or("Element has no number value")?;
            // SAFETY: a pattern call on a live element.
            let value = unsafe { range.CurrentValue() }.map_err(|e| e.to_string())?;
            if !value.is_finite() {
                return Err("UI element returned a non-finite value".into());
            }
            Ok(Evaluated::Number(value))
        }
    }
}

/// Clicks, toggles, selects or opens `element`, whichever it supports.
fn invoke(element: &Element) -> Result<(), String> {
    // SAFETY: pattern calls on a live element.
    unsafe {
        if let Some(p) = pattern::<IUIAutomationInvokePattern>(element, UIA_InvokePatternId) {
            return p.Invoke().map_err(|e| e.to_string());
        }
        if let Some(p) = pattern::<IUIAutomationTogglePattern>(element, UIA_TogglePatternId) {
            return p.Toggle().map_err(|e| e.to_string());
        }
        if let Some(p) =
            pattern::<IUIAutomationSelectionItemPattern>(element, UIA_SelectionItemPatternId)
        {
            return p.Select().map_err(|e| e.to_string());
        }
        if let Some(p) =
            pattern::<IUIAutomationExpandCollapsePattern>(element, UIA_ExpandCollapsePatternId)
        {
            let collapsed = p
                .CurrentExpandCollapseState()
                .is_ok_and(|s| s == ExpandCollapseState_Collapsed);
            return if collapsed { p.Expand() } else { p.Collapse() }.map_err(|e| e.to_string());
        }
        if let Some(p) = pattern::<IUIAutomationLegacyIAccessiblePattern>(
            element,
            UIA_LegacyIAccessiblePatternId,
        ) {
            return p.DoDefaultAction().map_err(|e| e.to_string());
        }
    }
    Err("Element has no primary action".into())
}

pub fn action(selector: &UiSelector, action: UiAction, value: &str) -> Result<(), String> {
    let Found { object, node, .. } =
        resolve(find(selector, 20)?)?.ok_or("No UI element matches this selector")?;
    if !node.enabled {
        return Err("The matching UI element is disabled".into());
    }
    match action {
        UiAction::Invoke => invoke(&object),
        UiAction::SetText => {
            let p = pattern::<IUIAutomationValuePattern>(&object, UIA_ValuePatternId)
                .ok_or("Element isn't editable")?;
            // SAFETY: pattern calls on a live element.
            unsafe {
                if p.CurrentIsReadOnly().is_ok_and(|r| r.as_bool()) {
                    return Err("Element isn't editable".into());
                }
                p.SetValue(&BSTR::from(value)).map_err(|e| e.to_string())
            }
        }
        UiAction::SetValue => {
            let number: f64 = value
                .trim()
                .parse()
                .map_err(|_| "UI value must be a finite number")?;
            let p = pattern::<IUIAutomationRangeValuePattern>(&object, UIA_RangeValuePatternId)
                .ok_or("Element has no number value")?;
            // SAFETY: pattern calls on a live element.
            unsafe {
                let min = p.CurrentMinimum().map_err(|e| e.to_string())?;
                let max = p.CurrentMaximum().map_err(|e| e.to_string())?;
                if !number.is_finite() || number < min || number > max {
                    return Err("UI value is outside the element's numeric bounds".into());
                }
                p.SetValue(number).map_err(|e| e.to_string())
            }
        }
        // SAFETY: a plain call on a live element.
        UiAction::Focus => unsafe { object.SetFocus() }.map_err(|e| e.to_string()),
    }
}

// Picking from the screen.

fn rect_of(element: &Element) -> Option<(f64, f64, f64, f64)> {
    // SAFETY: a cached read on a live element.
    let r = unsafe { element.CachedBoundingRectangle() }.ok()?;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    (w > 0 && h > 0).then_some((r.left as f64, r.top as f64, w as f64, h as f64))
}

fn holds(rect: (f64, f64, f64, f64), x: f64, y: f64) -> bool {
    x >= rect.0 && y >= rect.1 && x < rect.0 + rect.2 && y < rect.1 + rect.3
}

/// The window under (`x`, `y`) and the deepest element there, going down
/// through whichever child holds the point (the smallest, when several do).
fn hit_at(x: f64, y: f64, ignore: &str) -> Result<Option<(UiHit, Option<Element>)>, String> {
    let (automation, request) = automation()?;
    let Some((hwnd, window)) = crate::window::handles()
        .into_iter()
        .find(|(_, w)| w.contains(x, y) && (ignore.is_empty() || !w.title.starts_with(ignore)))
    else {
        return Ok(None);
    };
    let ScreenWindow {
        title, app, bounds, ..
    } = window;
    let window_hit = UiHit {
        app: app.clone(),
        window: title.clone(),
        name: String::new(),
        role: String::new(),
        enabled: true,
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        window_only: true,
    };
    let Some(root) = window_element(&automation, &request, hwnd) else {
        return Ok(Some((window_hit, None)));
    };
    let mut current = root;
    let mut depth = 0;
    let mut window_title = title;
    loop {
        let deeper = children(&automation, &request, &current)
            .into_iter()
            .filter_map(|c| {
                rect_of(&c)
                    .filter(|r| holds(*r, x, y))
                    .map(|r| (c, r.2 * r.3))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((child, _)) = deeper else { break };
        current = child;
        depth += 1;
        let (node, _, _) = describe(&current, &app, &window_title);
        if node.role == "window" {
            window_title = node.name;
        }
        if depth >= LIMIT_DEPTH {
            break;
        }
    }
    if depth == 0 {
        return Ok(Some((window_hit, None)));
    }
    let (node, _, password) = describe(&current, &app, &window_title);
    let Some(rect) = rect_of(&current) else {
        return Ok(Some((window_hit, None)));
    };
    Ok(Some((
        UiHit {
            app,
            window: window_title,
            name: if password { String::new() } else { node.name },
            role: if password {
                "password text".into()
            } else {
                node.role
            },
            enabled: node.enabled,
            x: rect.0,
            y: rect.1,
            width: rect.2,
            height: rect.3,
            window_only: false,
        },
        Some(current),
    )))
}

pub fn element_at(x: f64, y: f64, ignore: &str) -> Result<Option<UiHit>, String> {
    Ok(hit_at(x, y, ignore)?.map(|(hit, _)| hit))
}

pub fn pick(x: f64, y: f64, ignore: &str) -> Result<Option<UiPick>, String> {
    let Some((hit, element)) = hit_at(x, y, ignore)? else {
        return Ok(None);
    };
    let Some(element) = element else {
        return Ok(Some(UiPick { hit, node: None }));
    };
    if hit.role == "password text" {
        return Err("Password fields can't be picked".into());
    }
    let everything = UiSelector {
        scope: UiScope::App,
        app: hit.app.clone(),
        ..UiSelector::default()
    };
    let all = find(&everything, LIMIT_NODES)?;
    let (automation, _) = automation()?;
    // SAFETY: compares two live elements by their runtime IDs.
    let same = |other: &Element| unsafe {
        automation
            .CompareElements(&element, other)
            .is_ok_and(|b| b.as_bool())
    };
    let Some(target) = all.iter().position(|f| same(&f.object)) else {
        return Ok(Some(UiPick { hit, node: None }));
    };
    Ok(Some(UiPick {
        hit,
        node: Some(picked(&all, target)),
    }))
}

// Change events, so watched UI conditions are checked again only when
// something changed.

static REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static STARTED: std::sync::Once = std::sync::Once::new();
static START_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn revision() -> Result<u64, String> {
    use std::sync::atomic::Ordering;
    use windows::Win32::UI::WindowsAndMessaging::{
        EVENT_OBJECT_CREATE, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_VALUECHANGE,
        EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG, OBJID_CARET, OBJID_CURSOR,
        WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    };
    unsafe extern "system" fn changed(
        _: HWINEVENTHOOK,
        event: u32,
        _: HWND,
        object: i32,
        _: i32,
        _: u32,
        _: u32,
    ) {
        // The caret and pointer move all the time and change nothing.
        if event == EVENT_OBJECT_LOCATIONCHANGE
            || object == OBJID_CURSOR.0
            || object == OBJID_CARET.0
        {
            return;
        }
        REVISION.fetch_add(1, Ordering::Release);
    }
    STARTED.call_once(|| {
        let started = std::thread::Builder::new()
            .name("uia-events".into())
            .spawn(|| {
                let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
                // SAFETY: out-of-context hooks call `changed` on this thread
                // while it pumps messages below, for the life of the process.
                unsafe {
                    SetWinEventHook(
                        EVENT_OBJECT_CREATE,
                        EVENT_OBJECT_VALUECHANGE,
                        None,
                        Some(changed),
                        0,
                        0,
                        flags,
                    );
                    SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        None,
                        Some(changed),
                        0,
                        0,
                        flags,
                    );
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
                }
            });
        if let Err(e) = started {
            *START_ERROR.lock().unwrap_or_else(|p| p.into_inner()) =
                Some(format!("Couldn't start UI Automation events: {e}"));
        }
    });
    if let Some(error) = START_ERROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
    {
        return Err(error);
    }
    Ok(REVISION.load(Ordering::Acquire))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_types_have_names() {
        assert_eq!(role_name(UIA_ButtonControlTypeId.0), "button");
        assert_eq!(role_name(UIA_AppBarControlTypeId.0), "app bar");
        assert_eq!(role_name(UIA_WindowControlTypeId.0), "window");
        assert_eq!(role_name(0), "");
    }
}
