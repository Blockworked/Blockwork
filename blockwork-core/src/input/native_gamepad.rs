//! Optional Linux uinput gamepad. Other backends keep explicit unsupported methods.
use crate::input::gamepad::{BUTTONS, GamepadButton, GamepadState};
use evdev::{
    AbsInfo, AbsoluteAxisCode as Abs, AttributeSet, EventType, InputEvent, KeyCode, UinputAbsSetup,
    uinput::VirtualDevice,
};
use std::sync::Mutex;

static DEVICE: Mutex<Option<VirtualDevice>> = Mutex::new(None);

fn key(button: GamepadButton) -> Option<KeyCode> {
    Some(match button {
        GamepadButton::South => KeyCode::BTN_SOUTH,
        GamepadButton::East => KeyCode::BTN_EAST,
        GamepadButton::North => KeyCode::BTN_NORTH,
        GamepadButton::West => KeyCode::BTN_WEST,
        GamepadButton::LeftShoulder => KeyCode::BTN_TL,
        GamepadButton::RightShoulder => KeyCode::BTN_TR,
        GamepadButton::LeftStick => KeyCode::BTN_THUMBL,
        GamepadButton::RightStick => KeyCode::BTN_THUMBR,
        GamepadButton::Select => KeyCode::BTN_SELECT,
        GamepadButton::Start => KeyCode::BTN_START,
        GamepadButton::Mode => KeyCode::BTN_MODE,
        _ => return None,
    })
}

pub fn prepare() -> Result<(), String> {
    if !crate::config::load_settings()
        .virtual_gamepad_enabled
        .unwrap_or(false)
    {
        return Err("Virtual controller output is disabled in Settings".into());
    }
    let mut device = DEVICE.lock().map_err(|e| e.to_string())?;
    if device.is_some() {
        return Ok(());
    }
    let mut keys = AttributeSet::new();
    for button in BUTTONS {
        if let Some(code) = key(*button) {
            keys.insert(code);
        }
    }
    let mut builder = VirtualDevice::builder()
        .map_err(|e| format!("Virtual controller output needs /dev/uinput access: {e}"))?
        .name(crate::input::VIRTUAL_GAMEPAD_NAME)
        .with_keys(&keys)
        .map_err(|e| e.to_string())?;
    for (axis, min, max) in [
        (Abs::ABS_X, -32767, 32767),
        (Abs::ABS_Y, -32767, 32767),
        (Abs::ABS_RX, -32767, 32767),
        (Abs::ABS_RY, -32767, 32767),
        (Abs::ABS_Z, 0, 1023),
        (Abs::ABS_RZ, 0, 1023),
        (Abs::ABS_HAT0X, -1, 1),
        (Abs::ABS_HAT0Y, -1, 1),
    ] {
        builder = builder
            .with_absolute_axis(&UinputAbsSetup::new(
                axis,
                AbsInfo::new(0, min, max, 0, 0, 0),
            ))
            .map_err(|e| e.to_string())?;
    }
    *device = Some(builder.build().map_err(|e| e.to_string())?);
    std::thread::sleep(std::time::Duration::from_millis(100));
    Ok(())
}

pub fn state(state: &GamepadState) -> Result<(), String> {
    state.validate()?;
    let mut device = DEVICE.lock().map_err(|e| e.to_string())?;
    let device = device
        .as_mut()
        .ok_or("Virtual controller wasn't initialized")?;
    let mut events = vec![];
    for button in BUTTONS {
        if let Some(code) = key(*button) {
            events.push(InputEvent::new(
                EventType::KEY.0,
                code.0,
                i32::from(state.buttons.contains(button)),
            ));
        }
    }
    let pressed = |button| i32::from(state.buttons.contains(&button));
    for (axis, value) in [
        (Abs::ABS_X, state.left_x as i32),
        (Abs::ABS_Y, -(state.left_y as i32)),
        (Abs::ABS_RX, state.right_x as i32),
        (Abs::ABS_RY, -(state.right_y as i32)),
        (Abs::ABS_Z, state.left_trigger as i32),
        (Abs::ABS_RZ, state.right_trigger as i32),
        (
            Abs::ABS_HAT0X,
            pressed(GamepadButton::DPadRight) - pressed(GamepadButton::DPadLeft),
        ),
        (
            Abs::ABS_HAT0Y,
            pressed(GamepadButton::DPadDown) - pressed(GamepadButton::DPadUp),
        ),
    ] {
        events.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, value));
    }
    device.emit(&events).map_err(|e| e.to_string())
}

pub fn reset() -> Result<(), String> {
    if DEVICE.lock().unwrap_or_else(|p| p.into_inner()).is_none() {
        return Ok(());
    }
    if let Err(error) = state(&GamepadState::default()) {
        // Destroy a failed device as a final fallback instead of retaining held buttons.
        *DEVICE.lock().unwrap_or_else(|p| p.into_inner()) = None;
        return Err(error);
    }
    Ok(())
}
