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
    KeyValue,
    SetVariableValue,
    ChangeVariableValue,
    SetClipboardValue,
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
    WriteFilePath,
    WriteFileText,
    HttpUrl,
    HttpBody,
    HttpHeaders,
    NotifyTitle,
    NotifyMessage,
    LogValue,
    ErrorMessage,
    WindowPattern,
    WatchedFilePath,
    RetryCount,
    RetryDelay,
    WindowX,
    WindowY,
    MacroTarget,
    GlobalName,
    GlobalValue,
    WaitTimeout,
    ImageTolerance,
    TypedText,
    PasteValue,
    AppPattern,
    IdleMinutes,
    LockName,
    CronExpression,
    SplitText,
    SplitSeparator,
    UsbPattern,
    MouseModifiers,
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
                | FieldId::RetryCount
                | FieldId::WindowX
                | FieldId::WindowY
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
            FieldId::KeyValue => write!(f, "KeyValue"),
            FieldId::SetVariableValue => write!(f, "SetVariableValue"),
            FieldId::ChangeVariableValue => write!(f, "ChangeVariableValue"),
            FieldId::SetClipboardValue => write!(f, "SetClipboardValue"),
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
            FieldId::WriteFilePath => write!(f, "WriteFilePath"),
            FieldId::WriteFileText => write!(f, "WriteFileText"),
            FieldId::HttpUrl => write!(f, "HttpUrl"),
            FieldId::HttpBody => write!(f, "HttpBody"),
            FieldId::HttpHeaders => write!(f, "HttpHeaders"),
            FieldId::NotifyTitle => write!(f, "NotifyTitle"),
            FieldId::NotifyMessage => write!(f, "NotifyMessage"),
            FieldId::LogValue => write!(f, "LogValue"),
            FieldId::ErrorMessage => write!(f, "ErrorMessage"),
            FieldId::WindowPattern => write!(f, "WindowPattern"),
            FieldId::WatchedFilePath => write!(f, "WatchedFilePath"),
            FieldId::RetryCount => write!(f, "RetryCount"),
            FieldId::RetryDelay => write!(f, "RetryDelay"),
            FieldId::WindowX => write!(f, "WindowX"),
            FieldId::WindowY => write!(f, "WindowY"),
            FieldId::MacroTarget => write!(f, "MacroTarget"),
            FieldId::GlobalName => write!(f, "GlobalName"),
            FieldId::GlobalValue => write!(f, "GlobalValue"),
            FieldId::WaitTimeout => write!(f, "WaitTimeout"),
            FieldId::ImageTolerance => write!(f, "ImageTolerance"),
            FieldId::TypedText => write!(f, "TypedText"),
            FieldId::PasteValue => write!(f, "PasteValue"),
            FieldId::AppPattern => write!(f, "AppPattern"),
            FieldId::IdleMinutes => write!(f, "IdleMinutes"),
            FieldId::LockName => write!(f, "LockName"),
            FieldId::CronExpression => write!(f, "CronExpression"),
            FieldId::SplitText => write!(f, "SplitText"),
            FieldId::SplitSeparator => write!(f, "SplitSeparator"),
            FieldId::UsbPattern => write!(f, "UsbPattern"),
            FieldId::MouseModifiers => write!(f, "MouseModifiers"),
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
            "KeyValue" => Ok(FieldId::KeyValue),
            "SetVariableValue" => Ok(FieldId::SetVariableValue),
            "ChangeVariableValue" => Ok(FieldId::ChangeVariableValue),
            "SetClipboardValue" => Ok(FieldId::SetClipboardValue),
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
            "WriteFilePath" => Ok(FieldId::WriteFilePath),
            "WriteFileText" => Ok(FieldId::WriteFileText),
            "HttpUrl" => Ok(FieldId::HttpUrl),
            "HttpBody" => Ok(FieldId::HttpBody),
            "HttpHeaders" => Ok(FieldId::HttpHeaders),
            "NotifyTitle" => Ok(FieldId::NotifyTitle),
            "NotifyMessage" => Ok(FieldId::NotifyMessage),
            "LogValue" => Ok(FieldId::LogValue),
            "ErrorMessage" => Ok(FieldId::ErrorMessage),
            "WindowPattern" => Ok(FieldId::WindowPattern),
            "WatchedFilePath" => Ok(FieldId::WatchedFilePath),
            "RetryCount" => Ok(FieldId::RetryCount),
            "RetryDelay" => Ok(FieldId::RetryDelay),
            "WindowX" => Ok(FieldId::WindowX),
            "WindowY" => Ok(FieldId::WindowY),
            "MacroTarget" => Ok(FieldId::MacroTarget),
            "GlobalName" => Ok(FieldId::GlobalName),
            "GlobalValue" => Ok(FieldId::GlobalValue),
            "WaitTimeout" => Ok(FieldId::WaitTimeout),
            "ImageTolerance" => Ok(FieldId::ImageTolerance),
            "TypedText" => Ok(FieldId::TypedText),
            "PasteValue" => Ok(FieldId::PasteValue),
            "AppPattern" => Ok(FieldId::AppPattern),
            "IdleMinutes" => Ok(FieldId::IdleMinutes),
            "LockName" => Ok(FieldId::LockName),
            "CronExpression" => Ok(FieldId::CronExpression),
            "SplitText" => Ok(FieldId::SplitText),
            "SplitSeparator" => Ok(FieldId::SplitSeparator),
            "UsbPattern" => Ok(FieldId::UsbPattern),
            "MouseModifiers" => Ok(FieldId::MouseModifiers),
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
        (InstructionKind::Token(InputToken::Key(k, _)), FieldId::KeyValue) => Some(k),
        (InstructionKind::WhenKey(k, _), FieldId::KeyValue) => Some(k),
        (InstructionKind::SetVariable(_, v), FieldId::SetVariableValue) => Some(v),
        (InstructionKind::Return(v), FieldId::ReturnValue) => Some(v),
        (InstructionKind::CallBlock { args, .. }, FieldId::CallArg(i)) => args.get_mut(i),
        (InstructionKind::ChangeVariable(_, v), FieldId::ChangeVariableValue) => Some(v),
        (InstructionKind::SetClipboard(v), FieldId::SetClipboardValue) => Some(v),
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
        (InstructionKind::WriteFile { path, .. }, FieldId::WriteFilePath) => Some(path),
        (InstructionKind::WriteFile { text, .. }, FieldId::WriteFileText) => Some(text),
        (InstructionKind::HttpRequest { url, .. }, FieldId::HttpUrl) => Some(url),
        (InstructionKind::HttpRequest { body, .. }, FieldId::HttpBody) => Some(body),
        (InstructionKind::HttpRequest { headers, .. }, FieldId::HttpHeaders) => Some(headers),
        (InstructionKind::Notify { title, .. }, FieldId::NotifyTitle) => Some(title),
        (InstructionKind::Notify { message, .. }, FieldId::NotifyMessage) => Some(message),
        (InstructionKind::Log(v), FieldId::LogValue) => Some(v),
        (InstructionKind::ThrowError(v), FieldId::ErrorMessage) => Some(v),
        (InstructionKind::WhenWindow { pattern, .. }, FieldId::WindowPattern) => Some(pattern),
        (InstructionKind::WhenFileChanged(v), FieldId::WatchedFilePath) => Some(v),
        (InstructionKind::Retry { count, .. }, FieldId::RetryCount) => Some(count),
        (InstructionKind::Retry { delay, .. }, FieldId::RetryDelay) => Some(delay),
        (InstructionKind::WindowAction { pattern, .. }, FieldId::WindowPattern) => Some(pattern),
        (InstructionKind::MoveWindow { pattern, .. }, FieldId::WindowPattern) => Some(pattern),
        (InstructionKind::MoveWindow { x, .. }, FieldId::WindowX) => Some(x),
        (InstructionKind::MoveWindow { y, .. }, FieldId::WindowY) => Some(y),
        (InstructionKind::RunMacro { target, .. }, FieldId::MacroTarget) => Some(target),
        (InstructionKind::StopMacroNamed(v), FieldId::MacroTarget) => Some(v),
        (InstructionKind::SetGlobal { name, .. }, FieldId::GlobalName) => Some(name),
        (InstructionKind::SetGlobal { value, .. }, FieldId::GlobalValue) => Some(value),
        (InstructionKind::ChangeGlobal { name, .. }, FieldId::GlobalName) => Some(name),
        (InstructionKind::ChangeGlobal { by, .. }, FieldId::GlobalValue) => Some(by),
        (InstructionKind::WaitUntil { condition, .. }, FieldId::Condition) => Some(condition),
        (InstructionKind::WaitUntil { timeout, .. }, FieldId::WaitTimeout) => Some(timeout),
        (InstructionKind::WaitForImage { timeout, .. }, FieldId::WaitTimeout) => Some(timeout),
        (InstructionKind::FindImage { tolerance, .. }, FieldId::ImageTolerance) => Some(tolerance),
        (InstructionKind::ClickImage { tolerance, .. }, FieldId::ImageTolerance) => Some(tolerance),
        (InstructionKind::WaitForImage { tolerance, .. }, FieldId::ImageTolerance) => Some(tolerance),
        (InstructionKind::WhenTyped { text, .. }, FieldId::TypedText) => Some(text),
        (InstructionKind::PasteText(v), FieldId::PasteValue) => Some(v),
        (InstructionKind::WhenApp { pattern, .. }, FieldId::AppPattern) => Some(pattern),
        (InstructionKind::WhenIdle(v), FieldId::IdleMinutes) => Some(v),
        (InstructionKind::WithLock { name, .. }, FieldId::LockName) => Some(name),
        (InstructionKind::WhenCron(v), FieldId::CronExpression) => Some(v),
        (InstructionKind::SplitIntoList { text, .. }, FieldId::SplitText) => Some(text),
        (InstructionKind::SplitIntoList { separator, .. }, FieldId::SplitSeparator) => Some(separator),
        (InstructionKind::WhenUsb { pattern, .. }, FieldId::UsbPattern) => Some(pattern),
        (InstructionKind::WhenMouseButton { modifiers, .. }, FieldId::MouseModifiers) => Some(modifiers),
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
        (_, FieldId::KeyValue) => Some(Value::Text { value: String::new() }),
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
