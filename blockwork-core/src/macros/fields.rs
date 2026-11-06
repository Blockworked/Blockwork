//! Field ids: what the frontend calls each value slot on an instruction,
//! and the lookups blockstitch resolves a drag or a typed edit through.

use crate::input::types::InputToken;
use crate::input::value::Value;
use crate::macros::{BlockDef, InstructionKind};
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
    WaitErrorTimeout,
    ImageTolerance,
    ImageIndex,
    ImageNearX,
    ImageNearY,
    TypedText,
    PasteValue,
    BrowserUrl,
    BrowserSelector,
    BrowserNth,
    BrowserText,
    BrowserCode,
    AppPattern,
    IdleMinutes,
    LockName,
    CronExpression,
    ScheduleTime,
    SplitText,
    SplitSeparator,
    UsbPattern,
    MouseModifiers,
    AskPrompt,
    AskDefault,
    MessageText,
    AtTime,
    SignalName,
    WebhookPath,
    DetectorUrl,
    DetectLabel,
    DetectConfidence,
    AimTargetX,
    AimTargetY,
    AimGain,
    AimMaxStep,
    AimDeadzone,
    AimIndex,
    AimSettle,
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
                | FieldId::AimIndex
        )
    }
}

// A field id's string form is its variant name, or `CallArg:<n>`.
impl std::fmt::Display for FieldId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldId::CallArg(i) => write!(f, "CallArg:{i}"),
            named => write!(f, "{named:?}"),
        }
    }
}

impl std::str::FromStr for FieldId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let unknown = || format!("Unknown FieldId: {s}");
        if let Some(index) = s.strip_prefix("CallArg:") {
            return index.parse().map(FieldId::CallArg).map_err(|_| unknown());
        }
        // Unit variants deserialize from their name.
        serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(|_| unknown())
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
        (InstructionKind::WaitUntil { error_timeout, .. } | InstructionKind::WaitForImage { error_timeout, .. }, FieldId::WaitErrorTimeout) => error_timeout.as_mut(),
        (InstructionKind::WaitUntil { condition, .. }, FieldId::Condition) => Some(condition),
        (InstructionKind::WaitUntil { timeout, .. }, FieldId::WaitTimeout) => timeout.as_mut(),
        (InstructionKind::WaitForImage { timeout, .. }, FieldId::WaitTimeout) => timeout.as_mut(),
        (InstructionKind::FindImage { tolerance, .. }, FieldId::ImageTolerance) => tolerance.as_mut(),
        (InstructionKind::ClickImage { tolerance, .. }, FieldId::ImageTolerance) => tolerance.as_mut(),
        (InstructionKind::WaitForImage { tolerance, .. }, FieldId::ImageTolerance) => tolerance.as_mut(),
        (InstructionKind::FindImage { pick: Some(m), .. } | InstructionKind::ClickImage { pick: Some(m), .. }, FieldId::ImageIndex) => Some(&mut m.index),
        (InstructionKind::FindImage { pick: Some(m), .. } | InstructionKind::ClickImage { pick: Some(m), .. }, FieldId::ImageNearX) => Some(&mut m.near_x),
        (InstructionKind::FindImage { pick: Some(m), .. } | InstructionKind::ClickImage { pick: Some(m), .. }, FieldId::ImageNearY) => Some(&mut m.near_y),
        (InstructionKind::WhenTyped { text, .. }, FieldId::TypedText) => Some(text),
        (InstructionKind::PasteText(v), FieldId::PasteValue) => Some(v),
        (InstructionKind::BrowserOpen { url, .. }, FieldId::BrowserUrl) => Some(url),
        (
            InstructionKind::BrowserClick { selector, .. }
            | InstructionKind::BrowserType { selector, .. }
            | InstructionKind::BrowserWaitFor { selector, .. },
            FieldId::BrowserSelector,
        ) => Some(selector),
        (InstructionKind::BrowserClick { nth: Some(nth), .. } | InstructionKind::BrowserType { nth: Some(nth), .. }, FieldId::BrowserNth) => Some(nth),
        (InstructionKind::BrowserType { text, .. }, FieldId::BrowserText) => Some(text),
        (InstructionKind::BrowserWaitFor { timeout, .. }, FieldId::WaitTimeout) => Some(timeout),
        (InstructionKind::BrowserScript { code, .. }, FieldId::BrowserCode) => Some(code),
        (InstructionKind::WhenApp { pattern, .. }, FieldId::AppPattern) => Some(pattern),
        (InstructionKind::WhenIdle(v), FieldId::IdleMinutes) => Some(v),
        (InstructionKind::WithLock { name, .. }, FieldId::LockName) => Some(name),
        (InstructionKind::WhenCron(v), FieldId::CronExpression) => Some(v),
        (InstructionKind::WhenTime(_, v), FieldId::ScheduleTime) => Some(v),
        (InstructionKind::SplitIntoList { text, .. }, FieldId::SplitText) => Some(text),
        (InstructionKind::SplitIntoList { separator, .. }, FieldId::SplitSeparator) => Some(separator),
        (InstructionKind::WhenUsb { pattern, .. }, FieldId::UsbPattern) => Some(pattern),
        (InstructionKind::WhenMouseButton { modifiers, .. }, FieldId::MouseModifiers) => modifiers.as_mut(),
        (InstructionKind::AskText { prompt, .. } | InstructionKind::AskYesNo { prompt, .. } | InstructionKind::AskChoice { prompt, .. }, FieldId::AskPrompt) => Some(prompt),
        (InstructionKind::AskText { default, .. }, FieldId::AskDefault) => Some(default),
        (InstructionKind::ShowMessage(v), FieldId::MessageText) => Some(v),
        (InstructionKind::WhenAt(v), FieldId::AtTime) => Some(v),
        (InstructionKind::WhenSignal(v) | InstructionKind::SendSignal { name: v, .. }, FieldId::SignalName) => Some(&mut v.value),
        (InstructionKind::WhenWebhook { path, .. }, FieldId::WebhookPath) => Some(path),
        (
            InstructionKind::DetectObjectsInWindow { pattern, .. }
            | InstructionKind::WaitForDetection { pattern, .. }
            | InstructionKind::AimStep { pattern, .. },
            FieldId::WindowPattern,
        ) => Some(pattern),
        (
            InstructionKind::DetectObjectsInWindow { detector, .. } | InstructionKind::WaitForDetection { detector, .. },
            FieldId::DetectorUrl,
        ) => Some(detector),
        (
            InstructionKind::DetectObjectsInWindow { label, .. } | InstructionKind::WaitForDetection { label, .. },
            FieldId::DetectLabel,
        ) => Some(label),
        (
            InstructionKind::DetectObjectsInWindow { confidence, .. } | InstructionKind::WaitForDetection { confidence, .. },
            FieldId::DetectConfidence,
        ) => Some(confidence),
        (
            InstructionKind::WaitForDetection { timeout, .. } | InstructionKind::AimAtDetection { timeout, .. },
            FieldId::WaitTimeout,
        ) => Some(timeout),
        (InstructionKind::AimStep { target_x, .. }, FieldId::AimTargetX) => Some(target_x),
        (InstructionKind::AimStep { target_y, .. }, FieldId::AimTargetY) => Some(target_y),
        (InstructionKind::AimStep { gain, .. } | InstructionKind::AimAtDetection { gain, .. }, FieldId::AimGain) => Some(gain),
        (InstructionKind::AimStep { max_step, .. } | InstructionKind::AimAtDetection { max_step, .. }, FieldId::AimMaxStep) => {
            Some(max_step)
        }
        (InstructionKind::AimStep { deadzone, .. } | InstructionKind::AimAtDetection { deadzone, .. }, FieldId::AimDeadzone) => {
            Some(deadzone)
        }
        (InstructionKind::AimAtDetection { index, .. }, FieldId::AimIndex) => Some(index),
        (InstructionKind::AimAtDetection { settle_ms, .. }, FieldId::AimSettle) => Some(settle_ms),
        _ => None,
    }
}

/// The blank a field restores to when its value is dragged out; `None`
/// means a plain zero. Custom inputs use the definition's default.
pub(crate) fn blank_field_value(
    kind: &InstructionKind,
    field: FieldId,
    blocks: &[BlockDef],
) -> Option<Value> {
    match (kind, field) {
        (_, FieldId::Condition) => Some(Value::Bool),
        (_, FieldId::KeyValue) => Some(Value::Text { value: String::new() }),
        (InstructionKind::CallBlock { block_id, .. }, FieldId::CallArg(index)) => {
            blocks.iter().find(|def| def.id == *block_id)?.input_defaults().nth(index)
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
            FieldId::DetectorUrl,
            FieldId::AimSettle,
        ] {
            assert_eq!(field.to_string().parse(), Ok(field));
        }
        assert!("Nonsense".parse::<FieldId>().is_err());
        assert!("CallArg:x".parse::<FieldId>().is_err());
    }
}
