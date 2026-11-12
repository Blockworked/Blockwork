//! Touch playback through Windows pointer injection.
use crate::input::touch::{MAX_CONTACTS, TouchContact};
use std::sync::Mutex;
use windows_sys::Win32::UI::Input::Pointer::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    PT_TOUCH, TOUCH_MASK_CONTACTAREA, TOUCH_MASK_ORIENTATION, TOUCH_MASK_PRESSURE,
};

struct State {
    /// `InitializeTouchInjection` succeeded; it only needs to run once.
    initialized: bool,
    /// Contacts Windows thinks are down, released on cancel.
    active: Vec<TouchContact>,
}

static STATE: Mutex<State> = Mutex::new(State {
    initialized: false,
    active: Vec::new(),
});

/// One injected pointer, with a small fixed contact area and pressure.
fn info(point: TouchContact, flags: u32) -> POINTER_TOUCH_INFO {
    let mut info = POINTER_TOUCH_INFO::default();
    info.pointerInfo.pointerType = PT_TOUCH;
    info.pointerInfo.pointerId = point.id as u32 + 1;
    info.pointerInfo.pointerFlags = flags;
    info.pointerInfo.ptPixelLocation.x = point.x;
    info.pointerInfo.ptPixelLocation.y = point.y;
    info.touchMask = TOUCH_MASK_CONTACTAREA | TOUCH_MASK_ORIENTATION | TOUCH_MASK_PRESSURE;
    info.rcContact.left = point.x.saturating_sub(2);
    info.rcContact.right = point.x.saturating_add(2);
    info.rcContact.top = point.y.saturating_sub(2);
    info.rcContact.bottom = point.y.saturating_add(2);
    info.orientation = 90;
    info.pressure = 512;
    info
}

fn inject(points: &[POINTER_TOUCH_INFO]) -> Result<(), String> {
    if points.is_empty() {
        return Ok(());
    }
    if unsafe { InjectTouchInput(points.len() as u32, points.as_ptr()) } == 0 {
        return Err(format!(
            "Windows touch injection failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Turns on touch injection for this process.
pub fn prepare() -> Result<(), String> {
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    if !state.initialized {
        if unsafe { InitializeTouchInjection(MAX_CONTACTS as u32, TOUCH_FEEDBACK_DEFAULT) } == 0 {
            return Err(format!(
                "Windows native touch isn't available: {}",
                std::io::Error::last_os_error()
            ));
        }
        state.initialized = true;
    }
    Ok(())
}

/// Sends one complete contact frame: fingers missing from `contacts` lift.
pub fn frame(contacts: &[TouchContact]) -> Result<(), String> {
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    let mut points = vec![];
    for old in &state.active {
        if !contacts.iter().any(|p| p.id == old.id) {
            points.push(info(*old, POINTER_FLAG_UP));
        }
    }
    for point in contacts {
        let flag = if state.active.iter().any(|old| old.id == point.id) {
            POINTER_FLAG_UPDATE
        } else {
            POINTER_FLAG_DOWN
        };
        points.push(info(
            *point,
            flag | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT,
        ));
    }
    let result = inject(&points);
    // Keep the union on a partial failure so cancellation can release both sets.
    if result.is_ok() {
        state.active = contacts.to_vec();
    } else {
        for point in contacts {
            if !state.active.iter().any(|old| old.id == point.id) {
                state.active.push(*point);
            }
        }
    }
    result
}

/// Cancels every contact still down.
pub fn cancel() -> Result<(), String> {
    let mut state = STATE.lock().unwrap_or_else(|p| p.into_inner());
    let mut error = None;
    for point in state.active.drain(..) {
        if let Err(e) = inject(&[info(point, POINTER_FLAG_UP | POINTER_FLAG_CANCELED)]) {
            error = Some(e);
        }
    }
    error.map_or(Ok(()), Err)
}
