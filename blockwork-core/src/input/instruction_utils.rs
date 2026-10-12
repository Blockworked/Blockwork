use crate::input::types::{Axis, Coordinate, Direction, InputToken, MacroButton};
use crate::input::value::Value;
use crate::macros::{Instruction, InstructionKind};

pub fn create_default_instruction(instruction_type: usize) -> Option<Instruction> {
    let kind = match instruction_type {
        0 => InstructionKind::Wait(Value::number(1000.0)),
        1 => InstructionKind::Token(InputToken::Text(Value::Text { value: "text".into() })),
        2 => InstructionKind::Token(InputToken::Key(Value::Text { value: "a".into() }, Direction::Click)),
        3 => InstructionKind::Token(InputToken::Button(MacroButton::Left, Direction::Click)),
        4 => InstructionKind::Token(InputToken::MoveMouse(Value::number(0.0), Value::number(0.0), Coordinate::Rel)),
        5 => InstructionKind::Token(InputToken::Scroll(Value::number(4.0), Axis::Vertical)),
        6 => InstructionKind::Command("script".into()),
        _ => return None,
    };
    Some(Instruction::new(kind))
}

pub fn get_instruction_type_names() -> &'static [&'static str] {
    &[
        "Wait",
        "Text",
        "Key",
        "Mouse Button",
        "Move Mouse",
        "Scroll",
        "Run Script",
    ]
}
