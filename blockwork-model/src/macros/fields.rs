//! Field ids: what the frontend calls each value slot on an instruction,
//! and the lookups blockstitch resolves a drag or a typed edit through.

use crate::input::types::InputToken;
use crate::input::value::Value;
use crate::macros::{BlockDef, ImageMatch, InstructionKind};
use blockstitch_core::graph::InputValueType;
use serde::{Deserialize, Serialize};

/// One addressable value slot on an [`InstructionKind`]. Travels as its
/// `Display`/`FromStr` string form, which is what the frontend sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldId {
    AndroidPackage,
    ElementText,
    ElementTimeout,
    TouchX,
    TouchY,
    TouchEndX,
    TouchEndY,
    TouchDuration,

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

/// Every value slot on an instruction, with its field id and type, in the
/// order the canvas draws them. This is the one list of a block's slots:
/// `visit_values_mut` and [`value_slot_mut`] both walk it.
pub(crate) fn slots_mut<'a>(
    kind: &'a mut InstructionKind,
    f: &mut dyn FnMut(FieldId, &'a mut Value, InputValueType),
) {
    use FieldId as F;
    use InputValueType::{Any, Bool};
    match kind {
        InstructionKind::LaunchAndroidApp { package } => f(F::AndroidPackage, package, Any),
        InstructionKind::Element { text, timeout, .. } => {
            f(F::ElementText, text, Any);
            f(F::ElementTimeout, timeout, Any);
        }
        InstructionKind::Touch {
            x,
            y,
            end_x,
            end_y,
            duration,
            ..
        } => {
            f(F::TouchX, x, Any);
            f(F::TouchY, y, Any);
            f(F::TouchEndX, end_x, Any);
            f(F::TouchEndY, end_y, Any);
            f(F::TouchDuration, duration, Any);
        }
        InstructionKind::Token(token) => match token {
            InputToken::MoveMouse(x, y, _) => {
                f(F::MoveMouseX, x, Any);
                f(F::MoveMouseY, y, Any);
            }
            InputToken::Scroll(amount, _) => f(F::ScrollAmount, amount, Any),
            InputToken::Text(value) => f(F::TextValue, value, Any),
            InputToken::Key(key, _) => f(F::KeyValue, key, Any),
            InputToken::Button(..) | InputToken::Raw(..) => {}
        },
        InstructionKind::Wait(v) => f(F::WaitDuration, v, Any),
        InstructionKind::Return(v) => f(F::ReturnValue, v, Any),
        InstructionKind::WhenBatteryDischargedTo(v) => f(F::BatteryDischargeThreshold, v, Any),
        InstructionKind::WhenBatteryChargedTo(v) => f(F::BatteryChargeThreshold, v, Any),
        InstructionKind::WhenKey(v, _) => f(F::KeyValue, v, Any),
        InstructionKind::SetClipboard(v) => f(F::SetClipboardValue, v, Any),
        InstructionKind::SetVariable(_, v) => f(F::SetVariableValue, v, Any),
        InstructionKind::ChangeVariable(_, v) => f(F::ChangeVariableValue, v, Any),
        InstructionKind::Repeat { count, .. } => f(F::RepeatCount, count, Any),
        InstructionKind::Log(v) => f(F::LogValue, v, Any),
        InstructionKind::ThrowError(v) => f(F::ErrorMessage, v, Any),
        InstructionKind::WhenFileChanged(v) => f(F::WatchedFilePath, v, Any),
        InstructionKind::WhenWindow { pattern, .. } => f(F::WindowPattern, pattern, Any),
        InstructionKind::WhenTyped { text, .. } => f(F::TypedText, text, Any),
        InstructionKind::PasteText(v) => f(F::PasteValue, v, Any),
        InstructionKind::WhenApp { pattern, .. } => f(F::AppPattern, pattern, Any),
        InstructionKind::WhenIdle(v) => f(F::IdleMinutes, v, Any),
        InstructionKind::WhenCron(v) => f(F::CronExpression, v, Any),
        InstructionKind::WithLock { name, .. } => f(F::LockName, name, Any),
        InstructionKind::WhenUsb { pattern, .. } => f(F::UsbPattern, pattern, Any),
        InstructionKind::AskYesNo { prompt, .. } | InstructionKind::AskChoice { prompt, .. } => {
            f(F::AskPrompt, prompt, Any)
        }
        InstructionKind::ShowMessage(v) => f(F::MessageText, v, Any),
        InstructionKind::WhenAt(v) => f(F::AtTime, v, Any),
        InstructionKind::WhenWebhook { path, .. } => f(F::WebhookPath, path, Any),
        InstructionKind::BrowserOpen { url, .. } => f(F::BrowserUrl, url, Any),
        InstructionKind::BrowserScript { code, .. } => f(F::BrowserCode, code, Any),
        InstructionKind::BrowserClick { selector, nth } => {
            f(F::BrowserSelector, selector, Any);
            if let Some(nth) = nth {
                f(F::BrowserNth, nth, Any);
            }
        }
        InstructionKind::BrowserType {
            text,
            selector,
            nth,
        } => {
            f(F::BrowserText, text, Any);
            f(F::BrowserSelector, selector, Any);
            if let Some(nth) = nth {
                f(F::BrowserNth, nth, Any);
            }
        }
        InstructionKind::BrowserWaitFor {
            selector, timeout, ..
        } => {
            f(F::BrowserSelector, selector, Any);
            f(F::WaitTimeout, timeout, Any);
        }
        InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } => {
            f(F::SignalName, &mut name.value, Any)
        }
        InstructionKind::AskText {
            prompt, default, ..
        } => {
            f(F::AskPrompt, prompt, Any);
            f(F::AskDefault, default, Any);
        }
        InstructionKind::SplitIntoList {
            text, separator, ..
        } => {
            f(F::SplitText, text, Any);
            f(F::SplitSeparator, separator, Any);
        }
        InstructionKind::WriteFile { path, text, .. } => {
            f(F::WriteFilePath, path, Any);
            f(F::WriteFileText, text, Any);
        }
        InstructionKind::HttpRequest {
            url, body, headers, ..
        } => {
            f(F::HttpUrl, url, Any);
            f(F::HttpBody, body, Any);
            f(F::HttpHeaders, headers, Any);
        }
        InstructionKind::Notify { title, message } => {
            f(F::NotifyTitle, title, Any);
            f(F::NotifyMessage, message, Any);
        }
        InstructionKind::Retry { count, delay, .. } => {
            f(F::RetryCount, count, Any);
            f(F::RetryDelay, delay, Any);
        }
        InstructionKind::WindowAction { pattern, .. } => f(F::WindowPattern, pattern, Any),
        InstructionKind::MoveWindow { pattern, x, y, .. } => {
            f(F::WindowPattern, pattern, Any);
            f(F::WindowX, x, Any);
            f(F::WindowY, y, Any);
        }
        InstructionKind::RunMacro { target, .. } | InstructionKind::StopMacroNamed(target) => {
            f(F::MacroTarget, target, Any)
        }
        InstructionKind::SetGlobal { name, value: other }
        | InstructionKind::ChangeGlobal { name, by: other } => {
            f(F::GlobalName, name, Any);
            f(F::GlobalValue, other, Any);
        }
        InstructionKind::WhenMouseButton { modifiers, .. } => {
            if let Some(v) = modifiers {
                f(F::MouseModifiers, v, Any);
            }
        }
        InstructionKind::WaitUntil {
            condition,
            timeout,
            error_timeout,
        } => {
            f(F::Condition, condition, Bool);
            if let Some(v) = timeout {
                f(F::WaitTimeout, v, Any);
            }
            if let Some(v) = error_timeout {
                f(F::WaitErrorTimeout, v, Any);
            }
        }
        InstructionKind::FindImage {
            tolerance, pick, ..
        }
        | InstructionKind::ClickImage {
            tolerance, pick, ..
        } => {
            if let Some(v) = tolerance {
                f(F::ImageTolerance, v, Any);
            }
            if let Some(ImageMatch {
                index,
                near_x,
                near_y,
                ..
            }) = pick
            {
                f(F::ImageIndex, index, Any);
                f(F::ImageNearX, near_x, Any);
                f(F::ImageNearY, near_y, Any);
            }
        }
        InstructionKind::WaitForImage {
            tolerance,
            timeout,
            error_timeout,
            ..
        } => {
            if let Some(v) = tolerance {
                f(F::ImageTolerance, v, Any);
            }
            if let Some(v) = timeout {
                f(F::WaitTimeout, v, Any);
            }
            if let Some(v) = error_timeout {
                f(F::WaitErrorTimeout, v, Any);
            }
        }
        InstructionKind::DetectObjectsInWindow {
            pattern,
            detector,
            label,
            confidence,
            ..
        } => {
            f(F::WindowPattern, pattern, Any);
            f(F::DetectorUrl, detector, Any);
            f(F::DetectLabel, label, Any);
            f(F::DetectConfidence, confidence, Any);
        }
        InstructionKind::WaitForDetection {
            pattern,
            detector,
            label,
            confidence,
            timeout,
            ..
        } => {
            f(F::WindowPattern, pattern, Any);
            f(F::DetectorUrl, detector, Any);
            f(F::DetectLabel, label, Any);
            f(F::DetectConfidence, confidence, Any);
            f(F::WaitTimeout, timeout, Any);
        }
        InstructionKind::AimStep {
            pattern,
            target_x,
            target_y,
            gain,
            max_step,
            deadzone,
            ..
        } => {
            f(F::WindowPattern, pattern, Any);
            f(F::AimTargetX, target_x, Any);
            f(F::AimTargetY, target_y, Any);
            f(F::AimGain, gain, Any);
            f(F::AimMaxStep, max_step, Any);
            f(F::AimDeadzone, deadzone, Any);
        }
        InstructionKind::AimAtDetection {
            index,
            gain,
            max_step,
            deadzone,
            timeout,
            settle_ms,
            ..
        } => {
            f(F::AimIndex, index, Any);
            f(F::AimGain, gain, Any);
            f(F::AimMaxStep, max_step, Any);
            f(F::AimDeadzone, deadzone, Any);
            f(F::WaitTimeout, timeout, Any);
            f(F::AimSettle, settle_ms, Any);
        }
        // A call's declared Boolean inputs live on the `BlockDef`, not
        // here: `BlockGraph::migrate_bool_slots` handles those.
        InstructionKind::CallBlock { args, .. } => {
            for (i, arg) in args.iter_mut().enumerate() {
                f(F::CallArg(i), arg, Any);
            }
        }
        InstructionKind::If { condition, .. }
        | InstructionKind::IfElse { condition, .. }
        | InstructionKind::While { condition, .. } => f(F::Condition, condition, Bool),
        InstructionKind::AddToList { value, .. } => f(F::AddToListValue, value, Any),
        InstructionKind::InsertIntoList { value, index, .. } => {
            f(F::InsertIntoListValue, value, Any);
            f(F::InsertIntoListIndex, index, Any);
        }
        InstructionKind::WhenTime(_, time) => f(F::ScheduleTime, time, Any),
        InstructionKind::DeleteOfList { index, .. } => f(F::DeleteOfListIndex, index, Any),
        InstructionKind::ShiftList { amount, .. } => f(F::ShiftListAmount, amount, Any),
        InstructionKind::ReplaceItemOfList { index, value, .. } => {
            f(F::ReplaceItemOfListIndex, index, Any);
            f(F::ReplaceItemOfListValue, value, Any);
        }
        InstructionKind::Command(_)
        | InstructionKind::CommandWait(_)
        | InstructionKind::Comment(_)
        | InstructionKind::WhenRan
        | InstructionKind::BlockHeader(_)
        | InstructionKind::EscapeLoop
        | InstructionKind::ContinueLoop
        | InstructionKind::Stop { .. }
        | InstructionKind::TryCatch { .. }
        | InstructionKind::WhenPowerPluggedIn
        | InstructionKind::WhenPowerUnplugged
        | InstructionKind::WhenClipboardChanged
        | InstructionKind::WhenScreenLock { .. }
        | InstructionKind::WhenNetwork { .. }
        | InstructionKind::Forever { .. }
        | InstructionKind::OpenApp { .. }
        | InstructionKind::CloseApp { .. }
        | InstructionKind::DeleteAllOfList { .. }
        | InstructionKind::ReverseList { .. }
        | InstructionKind::WhenForegroundApp { .. }
        | InstructionKind::RunBranch(_) => {}
    }
}

/// The value tree a [`FieldId`] names on an instruction.
pub(crate) fn value_slot_mut(kind: &mut InstructionKind, field: FieldId) -> Option<&mut Value> {
    let mut found = None;
    slots_mut(kind, &mut |id, value, _| {
        if id == field && found.is_none() {
            found = Some(value);
        }
    });
    found
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
        (_, FieldId::KeyValue) => Some(Value::Text {
            value: String::new(),
        }),
        (InstructionKind::CallBlock { block_id, .. }, FieldId::CallArg(index)) => blocks
            .iter()
            .find(|def| def.id == *block_id)?
            .input_defaults()
            .nth(index),
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
    /// A block names each slot once, or `value_slot_mut` reaches only the
    /// first of two.
    #[test]
    fn no_block_names_a_slot_twice() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/wire_blocks.json")).unwrap();
        for block in fixtures.as_array().unwrap() {
            let dto: crate::frontend::InstructionDto =
                serde_json::from_value(block.clone()).unwrap();
            let mut kind = crate::frontend::dto_to_instruction(&dto).unwrap().kind;
            let mut ids = Vec::new();
            slots_mut(&mut kind, &mut |id, _, _| ids.push(id));
            let count = ids.len();
            ids.sort_by_key(|id| id.to_string());
            ids.dedup();
            assert_eq!(ids.len(), count, "{} repeats a field id", block["type"]);
        }
    }
}
