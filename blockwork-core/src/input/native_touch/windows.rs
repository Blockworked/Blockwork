use crate::input::touch::TouchContact;
use std::sync::Mutex;
use windows_sys::Win32::UI::Input::Pointer::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    PT_TOUCH, TOUCH_MASK_CONTACTAREA, TOUCH_MASK_ORIENTATION, TOUCH_MASK_PRESSURE,
};
static STATE: Mutex<(bool, Vec<TouchContact>)> = Mutex::new((false, Vec::new()));
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
pub fn prepare() -> Result<(), String> {
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    if !state.0 {
        if unsafe { InitializeTouchInjection(10, TOUCH_FEEDBACK_DEFAULT) } == 0 {
            return Err(format!(
                "Windows native touch isn't available: {}",
                std::io::Error::last_os_error()
            ));
        }
        state.0 = true;
    }
    Ok(())
}
pub fn frame(contacts: &[TouchContact]) -> Result<(), String> {
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    let mut points = vec![];
    for old in &state.1 {
        if !contacts.iter().any(|p| p.id == old.id) {
            points.push(info(*old, POINTER_FLAG_UP));
        }
    }
    for point in contacts {
        let flag = if state.1.iter().any(|old| old.id == point.id) {
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
        state.1 = contacts.to_vec();
    } else {
        for point in contacts {
            if !state.1.iter().any(|old| old.id == point.id) {
                state.1.push(*point);
            }
        }
    }
    result
}
pub fn cancel() -> Result<(), String> {
    let mut state = STATE.lock().unwrap_or_else(|p| p.into_inner());
    let mut error = None;
    for point in state.1.drain(..) {
        if let Err(e) = inject(&[info(point, POINTER_FLAG_UP | POINTER_FLAG_CANCELED)]) {
            error = Some(e);
        }
    }
    error.map_or(Ok(()), Err)
}
