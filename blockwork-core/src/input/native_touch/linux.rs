use crate::input::touch::TouchContact;
use crate::screen::Rect;
use evdev::{
    AbsInfo, AbsoluteAxisCode as Abs, AttributeSet, EventType, InputEvent, KeyCode, PropType,
    UinputAbsSetup, uinput::VirtualDevice,
};
use std::sync::Mutex;
static STATE: Mutex<Option<Device>> = Mutex::new(None);
struct Device {
    device: VirtualDevice,
    bounds: Rect,
    active: Vec<TouchContact>,
}
fn geometry() -> Result<Rect, String> {
    let monitors = crate::screen::monitors();
    if monitors.len() != 1 {
        return Err("Linux native touch currently requires one monitor; desktop device mapping is experimental".into());
    }
    let bounds = monitors[0];
    if bounds.width < 1.0 || bounds.height < 1.0 {
        return Err("Touch monitor geometry isn't available".into());
    }
    Ok(bounds)
}
fn event(axis: Abs, value: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE.0, axis.0, value)
}
fn same(a: Rect, b: Rect) -> bool {
    a.x == b.x && a.y == b.y && a.width == b.width && a.height == b.height
}
pub fn prepare() -> Result<(), String> {
    let bounds = geometry()?;
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    if state.as_ref().is_some_and(|s| same(s.bounds, bounds)) {
        return Ok(());
    }
    *state = None;
    let mut keys = AttributeSet::new();
    keys.insert(KeyCode::BTN_TOUCH);
    keys.insert(KeyCode::BTN_TOOL_FINGER);
    let mut props = AttributeSet::new();
    props.insert(PropType::DIRECT);
    let mut builder = VirtualDevice::builder()
        .map_err(|e| format!("Native touch needs /dev/uinput access: {e}"))?
        .name("Blockwork Virtual Touch")
        .with_keys(&keys)
        .map_err(|e| e.to_string())?
        .with_properties(&props)
        .map_err(|e| e.to_string())?;
    for (axis, maximum) in [
        (Abs::ABS_X, bounds.width as i32 - 1),
        (Abs::ABS_Y, bounds.height as i32 - 1),
        (Abs::ABS_MT_SLOT, 9),
        (Abs::ABS_MT_TRACKING_ID, 65535),
        (Abs::ABS_MT_POSITION_X, bounds.width as i32 - 1),
        (Abs::ABS_MT_POSITION_Y, bounds.height as i32 - 1),
    ] {
        builder = builder
            .with_absolute_axis(&UinputAbsSetup::new(
                axis,
                AbsInfo::new(0, 0, maximum, 0, 0, 0),
            ))
            .map_err(|e| e.to_string())?;
    }
    let device = builder
        .build()
        .map_err(|e| format!("Couldn't create native touch device: {e}"))?;
    *state = Some(Device {
        device,
        bounds,
        active: vec![],
    });
    // Allow the desktop's device discovery to register the new uinput device.
    std::thread::sleep(std::time::Duration::from_millis(100));
    Ok(())
}
pub fn frame(contacts: &[TouchContact]) -> Result<(), String> {
    let mut state = STATE.lock().map_err(|e| e.to_string())?;
    let state = state.as_mut().ok_or("Touch device wasn't initialized")?;
    let mut events = vec![];
    for old in &state.active {
        if !contacts.iter().any(|c| c.id == old.id) {
            events.extend([
                event(Abs::ABS_MT_SLOT, old.id as i32),
                event(Abs::ABS_MT_TRACKING_ID, -1),
            ]);
        }
    }
    for contact in contacts {
        events.push(event(Abs::ABS_MT_SLOT, contact.id as i32));
        if !state.active.iter().any(|old| old.id == contact.id) {
            events.push(event(Abs::ABS_MT_TRACKING_ID, contact.id as i32 + 1));
        }
        events.push(event(
            Abs::ABS_MT_POSITION_X,
            contact.x - state.bounds.x as i32,
        ));
        events.push(event(
            Abs::ABS_MT_POSITION_Y,
            contact.y - state.bounds.y as i32,
        ));
    }
    if let Some(first) = contacts.first() {
        events.push(event(Abs::ABS_X, first.x - state.bounds.x as i32));
        events.push(event(Abs::ABS_Y, first.y - state.bounds.y as i32));
    }
    events.push(InputEvent::new(
        EventType::KEY.0,
        KeyCode::BTN_TOUCH.0,
        i32::from(!contacts.is_empty()),
    ));
    events.push(InputEvent::new(
        EventType::KEY.0,
        KeyCode::BTN_TOOL_FINGER.0,
        i32::from(!contacts.is_empty()),
    ));
    state.active = contacts.to_vec();
    state
        .device
        .emit(&events)
        .map_err(|e| format!("Native touch failed: {e}"))
}
pub fn cancel() -> Result<(), String> {
    let mut state = STATE.lock().unwrap_or_else(|p| p.into_inner());
    let Some(state) = state.as_mut() else {
        return Ok(());
    };
    let mut events = vec![];
    // Release every slot even after a partial kernel write.
    for slot in 0..10 {
        events.extend([
            event(Abs::ABS_MT_SLOT, slot),
            event(Abs::ABS_MT_TRACKING_ID, -1),
        ]);
    }
    events.push(InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 0));
    events.push(InputEvent::new(
        EventType::KEY.0,
        KeyCode::BTN_TOOL_FINGER.0,
        0,
    ));
    state.active.clear();
    state.device.emit(&events).map_err(|e| e.to_string())
}
impl Drop for Device {
    fn drop(&mut self) {
        let mut events = vec![];
        for slot in 0..10 {
            events.extend([
                event(Abs::ABS_MT_SLOT, slot),
                event(Abs::ABS_MT_TRACKING_ID, -1),
            ]);
        }
        events.push(InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 0));
        let _ = self.device.emit(&events);
    }
}
