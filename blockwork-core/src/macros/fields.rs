//! Field ids: what the frontend calls each value slot on an instruction,
//! and the lookups blockstitch resolves a drag or a typed edit through.

use crate::input::types::InputToken;
use crate::input::value::Value;
use crate::macros::{BlockDef, InputValueType, InstructionKind};
use serde::{Deserialize, Serialize};

/// One addressable value slot on an [`InstructionKind`]. Travels as its
/// `Display`/`FromStr` string form, which is what the frontend sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldId {
    WaitDuration,
    MoveMouseX,
    MoveMouseY,
    ScrollAmount,
    TextValue,
    SetVariableValue,
    ChangeVariableValue,
    AddToListValue,
    DeleteOfListIndex,
    ShiftListAmount,
    InsertIntoListValue,
    InsertIntoListIndex,
    ReplaceItemOfListIndex,
    ReplaceItemOfListValue,
    ReturnValue,
    CallArg(usize),
    Condition,
    RepeatCount,
    BatteryDischargeThreshold,
    BatteryChargeThreshold,
}

impl FieldId {
    /// Mouse coordinates, scroll amounts and the battery-percentage
    /// thresholds are integer-only; everything else allows decimals.
    pub fn requires_integer(self) -> bool {
        matches!(
            self,
            FieldId::MoveMouseX
                | FieldId::MoveMouseY
                | FieldId::ScrollAmount
                | FieldId::BatteryDischargeThreshold
                | FieldId::BatteryChargeThreshold
                | FieldId::DeleteOfListIndex
                | FieldId::ShiftListAmount
                | FieldId::InsertIntoListIndex
                | FieldId::ReplaceItemOfListIndex
        )
    }
}

impl std::fmt::Display for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldId::WaitDuration => write!(f, "WaitDuration"),
            FieldId::MoveMouseX => write!(f, "MoveMouseX"),
            FieldId::MoveMouseY => write!(f, "MoveMouseY"),
            FieldId::ScrollAmount => write!(f, "ScrollAmount"),
            FieldId::TextValue => write!(f, "TextValue"),
            FieldId::SetVariableValue => write!(f, "SetVariableValue"),
            FieldId::ChangeVariableValue => write!(f, "ChangeVariableValue"),
            FieldId::AddToListValue => write!(f, "AddToListValue"),
            FieldId::DeleteOfListIndex => write!(f, "DeleteOfListIndex"),
            FieldId::ShiftListAmount => write!(f, "ShiftListAmount"),
            FieldId::InsertIntoListValue => write!(f, "InsertIntoListValue"),
            FieldId::InsertIntoListIndex => write!(f, "InsertIntoListIndex"),
            FieldId::ReplaceItemOfListIndex => write!(f, "ReplaceItemOfListIndex"),
            FieldId::ReplaceItemOfListValue => write!(f, "ReplaceItemOfListValue"),
            FieldId::ReturnValue => write!(f, "ReturnValue"),
            FieldId::CallArg(i) => write!(f, "CallArg:{i}"),
            FieldId::Condition => write!(f, "Condition"),
            FieldId::RepeatCount => write!(f, "RepeatCount"),
            FieldId::BatteryDischargeThreshold => write!(f, "BatteryDischargeThreshold"),
            FieldId::BatteryChargeThreshold => write!(f, "BatteryChargeThreshold"),
        }
    }
}

impl std::str::FromStr for FieldId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "WaitDuration" => Ok(FieldId::WaitDuration),
            "MoveMouseX" => Ok(FieldId::MoveMouseX),
            "MoveMouseY" => Ok(FieldId::MoveMouseY),
            "ScrollAmount" => Ok(FieldId::ScrollAmount),
            "TextValue" => Ok(FieldId::TextValue),
            "SetVariableValue" => Ok(FieldId::SetVariableValue),
            "ChangeVariableValue" => Ok(FieldId::ChangeVariableValue),
            "AddToListValue" => Ok(FieldId::AddToListValue),
            "DeleteOfListIndex" => Ok(FieldId::DeleteOfListIndex),
            "ShiftListAmount" => Ok(FieldId::ShiftListAmount),
            "InsertIntoListValue" => Ok(FieldId::InsertIntoListValue),
            "InsertIntoListIndex" => Ok(FieldId::InsertIntoListIndex),
            "ReplaceItemOfListIndex" => Ok(FieldId::ReplaceItemOfListIndex),
            "ReplaceItemOfListValue" => Ok(FieldId::ReplaceItemOfListValue),
            "ReturnValue" => Ok(FieldId::ReturnValue),
            "Condition" => Ok(FieldId::Condition),
            "RepeatCount" => Ok(FieldId::RepeatCount),
            "BatteryDischargeThreshold" => Ok(FieldId::BatteryDischargeThreshold),
            "BatteryChargeThreshold" => Ok(FieldId::BatteryChargeThreshold),
            _ if s.starts_with("CallArg:") => s["CallArg:".len()..]
                .parse::<usize>()
                .map(FieldId::CallArg)
                .map_err(|_| format!("Unknown FieldId: {s}")),
            _ => Err(format!("Unknown FieldId: {s}")),
        }
    }
}

/// Locates the value tree a [`FieldId`] names on a given instruction.
pub(crate) fn value_slot_mut(kind: &mut InstructionKind, field: FieldId) -> Option<&mut Value> {
    match (kind, field) {
        (InstructionKind::Wait(d), FieldId::WaitDuration) => Some(d),
        (InstructionKind::Token(InputToken::MoveMouse(x, _, _)), FieldId::MoveMouseX) => Some(x),
        (InstructionKind::Token(InputToken::MoveMouse(_, y, _)), FieldId::MoveMouseY) => Some(y),
        (InstructionKind::Token(InputToken::Scroll(a, _)), FieldId::ScrollAmount) => Some(a),
        (InstructionKind::Token(InputToken::Text(t)), FieldId::TextValue) => Some(t),
        (InstructionKind::SetVariable(_, v), FieldId::SetVariableValue) => Some(v),
        (InstructionKind::Return(v), FieldId::ReturnValue) => Some(v),
        (InstructionKind::CallBlock { args, .. }, FieldId::CallArg(i)) => args.get_mut(i),
        (InstructionKind::ChangeVariable(_, v), FieldId::ChangeVariableValue) => Some(v),
        (InstructionKind::AddToList { value, .. }, FieldId::AddToListValue) => Some(value),
        (InstructionKind::DeleteOfList { index, .. }, FieldId::DeleteOfListIndex) => Some(index),
        (InstructionKind::ShiftList { amount, .. }, FieldId::ShiftListAmount) => Some(amount),
        (InstructionKind::InsertIntoList { value, .. }, FieldId::InsertIntoListValue) => {
            Some(value)
        }
        (InstructionKind::InsertIntoList { index, .. }, FieldId::InsertIntoListIndex) => {
            Some(index)
        }
        (InstructionKind::ReplaceItemOfList { index, .. }, FieldId::ReplaceItemOfListIndex) => {
            Some(index)
        }
        (InstructionKind::ReplaceItemOfList { value, .. }, FieldId::ReplaceItemOfListValue) => {
            Some(value)
        }
        (InstructionKind::If { condition, .. }, FieldId::Condition) => Some(condition),
        (InstructionKind::IfElse { condition, .. }, FieldId::Condition) => Some(condition),
        (InstructionKind::While { condition, .. }, FieldId::Condition) => Some(condition),
        (InstructionKind::Repeat { count, .. }, FieldId::RepeatCount) => Some(count),
        (InstructionKind::WhenBatteryDischargedTo(v), FieldId::BatteryDischargeThreshold) => {
            Some(v)
        }
        (InstructionKind::WhenBatteryChargedTo(v), FieldId::BatteryChargeThreshold) => Some(v),
        _ => None,
    }
}

/// The blank a field restores to when its value is dragged out; `None`
/// means a plain zero. Only boolean slots differ, and a call site's
/// declared types live on the [`BlockDef`] rather than on the call.
pub(crate) fn blank_field_value(
    kind: &InstructionKind,
    field: FieldId,
    blocks: &[BlockDef],
) -> Option<Value> {
    match (kind, field) {
        (_, FieldId::Condition) => Some(Value::Bool),
        (InstructionKind::CallBlock { block_id, .. }, FieldId::CallArg(index)) => {
            let declared = blocks
                .iter()
                .find(|def| def.id == *block_id)?
                .input_types()
                .nth(index)?;
            (declared == InputValueType::Bool).then_some(Value::Bool)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_ids_round_trip_through_their_string_form() {
        for field in [
            FieldId::WaitDuration,
            FieldId::MoveMouseX,
            FieldId::CallArg(3),
            FieldId::BatteryChargeThreshold,
        ] {
            assert_eq!(field.to_string().parse(), Ok(field));
        }
        assert!("Nonsense".parse::<FieldId>().is_err());
        assert!("CallArg:x".parse::<FieldId>().is_err());
    }
}
