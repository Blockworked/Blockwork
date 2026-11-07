use crate::input::types::MacroButton;

pub fn get_mouse_button_names() -> &'static [&'static str] {
    &[
        "Left",
        "Right",
        "Middle",
        "Back",
        "Forward",
        "ScrollUp",
        "ScrollDown",
        "ScrollLeft",
        "ScrollRight",
    ]
}

pub fn mouse_button_to_index(button: &MacroButton) -> usize {
    match button {
        MacroButton::Left => 0,
        MacroButton::Right => 1,
        MacroButton::Middle => 2,
        MacroButton::Back => 3,
        MacroButton::Forward => 4,
        MacroButton::ScrollUp => 5,
        MacroButton::ScrollDown => 6,
        MacroButton::ScrollLeft => 7,
        MacroButton::ScrollRight => 8,
        MacroButton::Other(_) => 0,
    }
}

pub fn index_to_mouse_button(index: usize) -> MacroButton {
    match index {
        0 => MacroButton::Left,
        1 => MacroButton::Right,
        2 => MacroButton::Middle,
        3 => MacroButton::Back,
        4 => MacroButton::Forward,
        5 => MacroButton::ScrollUp,
        6 => MacroButton::ScrollDown,
        7 => MacroButton::ScrollLeft,
        8 => MacroButton::ScrollRight,
        _ => MacroButton::Left,
    }
}
