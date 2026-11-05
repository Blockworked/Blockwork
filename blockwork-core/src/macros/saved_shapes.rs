//! Older saved shapes of instructions, and how they load into the current ones.

use super::*;

/// `WhenTime` as saved now (recurrence + time value), or before the time
/// became a value (one object with a fixed `hour`/`minute`).
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum WhenTimeDe {
    Current(TimeSchedule, Value),
    Legacy(LegacyTimeSchedule),
}

#[derive(Deserialize)]
pub(super) enum InstructionKindDe {
    Token(InputToken),
    Wait(WaitDe),
    Command(String),
    CommandWait(String),
    Comment(String),
    WhenRan,
    WhenBatteryDischargedTo(Value),
    WhenBatteryChargedTo(Value),
    WhenTime(WhenTimeDe),
    WhenPowerPluggedIn,
    WhenPowerUnplugged,
    WhenClipboardChanged,
    WhenKey(Value, Direction),
    OpenApp {
        command: String,
        name: String,
        icon: Option<String>,
        #[serde(default)]
        launch: LaunchOptions,
    },
    CloseApp {
        command: String,
        name: String,
        icon: Option<String>,
    },
    SetVariable(String, Value),
    ChangeVariable(String, Value),
    SetClipboard(Value),
    AddToList {
        value: Value,
        name: String,
    },
    DeleteOfList {
        index: Value,
        name: String,
    },
    DeleteAllOfList {
        name: String,
    },
    ShiftList {
        name: String,
        amount: Value,
    },
    InsertIntoList {
        value: Value,
        index: Value,
        name: String,
    },
    ReplaceItemOfList {
        index: Value,
        name: String,
        value: Value,
    },
    ReverseList {
        name: String,
    },
    BlockHeader(String),
    CallBlock {
        block_id: String,
        args: Vec<Value>,
        #[serde(default)]
        branches: Vec<Vec<Instruction>>,
    },
    RunBranch(String),
    Return(Value),
    If {
        condition: Value,
        body: Vec<Instruction>,
    },
    IfElse {
        condition: Value,
        then_body: Vec<Instruction>,
        else_body: Vec<Instruction>,
    },
    Repeat {
        count: Value,
        body: Vec<Instruction>,
    },
    Forever {
        body: Vec<Instruction>,
    },
    While {
        condition: Value,
        body: Vec<Instruction>,
    },
    EscapeLoop,
    ContinueLoop,
    WriteFile {
        path: Value,
        text: Value,
        append: bool,
    },
    HttpRequest {
        method: String,
        url: Value,
        body: Value,
        headers: Value,
        variable: String,
    },
    Notify {
        title: Value,
        message: Value,
    },
    Log(Value),
    TryCatch {
        body: Vec<Instruction>,
        catch_body: Vec<Instruction>,
    },
    ThrowError(Value),
    StopMacro,
    WhenWindow {
        pattern: Value,
        focused: bool,
    },
    WhenFileChanged(Value),
    Retry {
        count: Value,
        delay: Value,
        body: Vec<Instruction>,
    },
    WindowAction {
        action: String,
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
    },
    MoveWindow {
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
        resize: bool,
        x: Value,
        y: Value,
    },
    RunMacro {
        target: Value,
        wait: bool,
    },
    StopMacroNamed(Value),
    SetGlobal {
        name: Value,
        value: Value,
    },
    ChangeGlobal {
        name: Value,
        by: Value,
    },
    WaitUntil {
        condition: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_timeout: Option<Value>,
    },
    FindImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        x_variable: String,
        y_variable: String,
        #[serde(default)]
        pick: Option<ImageMatch>,
        #[serde(default)]
        region: Option<String>,
        #[serde(default)]
        count_variable: Option<String>,
    },
    ClickImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        button: MacroButton,
        #[serde(default)]
        pick: Option<ImageMatch>,
        #[serde(default)]
        region: Option<String>,
    },
    WaitForImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        appear: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Value>,
        #[serde(default)]
        region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_timeout: Option<Value>,
    },
    WhenTyped {
        text: Value,
        erase: bool,
    },
    PasteText(Value),
    WhenApp {
        pattern: Value,
        started: bool,
    },
    WhenIdle(Value),
    WhenScreenLock {
        locked: bool,
    },
    WithLock {
        name: Value,
        body: Vec<Instruction>,
    },
    WhenCron(Value),
    SplitIntoList {
        text: Value,
        separator: Value,
        name: String,
    },
    WhenNetwork {
        connected: bool,
    },
    WhenUsb {
        pattern: Value,
        connected: bool,
    },
    WhenMouseButton {
        button: MacroButton,
        pressed: bool,
        #[serde(default)]
        modifiers: Option<Value>,
    },
    AskText {
        prompt: Value,
        default: Value,
        name: String,
    },
    AskYesNo {
        prompt: Value,
        name: String,
    },
    AskChoice {
        prompt: Value,
        list: String,
        name: String,
    },
    ShowMessage(Value),
    WhenAt(Value),
    WhenSignal(SignalName),
    SendSignal {
        name: SignalName,
        wait: bool,
    },
    WhenWebhook {
        path: Value,
        variable: String,
    },
    DetectObjectsInWindow {
        pattern: Value,
        by: String,
        detector: Value,
        label: Value,
        confidence: Value,
        sort: String,
        results_list: String,
        count_variable: String,
        #[serde(default)]
        region: String,
    },
    WaitForDetection {
        pattern: Value,
        by: String,
        detector: Value,
        label: Value,
        confidence: Value,
        appear: bool,
        timeout: Value,
        #[serde(default)]
        region: String,
    },
    AimStep {
        pattern: Value,
        by: String,
        target_x: Value,
        target_y: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
    },
    AimAtDetection {
        results_list: String,
        index: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
        timeout: Value,
        settle_ms: Value,
        status_variable: String,
    },
    BrowserOpen {
        url: Value,
        new_tab: bool,
    },
    BrowserClick {
        selector: Value,
        #[serde(default)]
        nth: Option<Value>,
    },
    BrowserType {
        text: Value,
        selector: Value,
        #[serde(default)]
        nth: Option<Value>,
    },
    BrowserWaitFor {
        selector: Value,
        appear: bool,
        timeout: Value,
    },
    BrowserScript {
        code: Value,
        variable: String,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum WaitDe {
    /// Oldest save shape: a bare duration, no randomness field existed yet.
    LegacyNumber(u64),
    /// Pre-`Op::Random` save shape: `[duration, randomness]`, migrated into
    /// `Op::Random` below so old macros keep the same spread of wait times.
    LegacyWithRandomness(Value, Value),
    /// Current shape: a single `Value` (a plain duration, or a duration
    /// wrapped in any operator including `Op::Random`).
    Current(Value),
}

/// Folds a legacy `[duration, randomness]` `Wait` into today's single-`Value`
/// shape: `duration` alone if randomness was zero, otherwise `duration`
/// wrapped in `Op::Random` spanning `[duration - randomness, duration + randomness]`.
pub(super) fn migrate_wait_duration(duration: Value, randomness: Value) -> Value {
    if randomness == Value::number(0.0) {
        return duration;
    }
    let zero = || Box::new(Value::number(0.0));
    Value::Op {
        op: Op::Random,
        args: vec![
            Value::Op {
                op: Op::Sub,
                args: vec![duration.clone(), randomness.clone()],
                saved: zero(),
            },
            Value::Op {
                op: Op::Add,
                args: vec![duration, randomness],
                saved: zero(),
            },
        ],
        saved: zero(),
    }
}

impl From<InstructionKindDe> for InstructionKind {
    fn from(de: InstructionKindDe) -> Self {
        match de {
            InstructionKindDe::Token(t) => InstructionKind::Token(t),
            InstructionKindDe::Wait(WaitDe::LegacyNumber(d)) => {
                InstructionKind::Wait(Value::number(d as f64))
            }
            InstructionKindDe::Wait(WaitDe::LegacyWithRandomness(d, r)) => {
                InstructionKind::Wait(migrate_wait_duration(d, r))
            }
            InstructionKindDe::Wait(WaitDe::Current(d)) => InstructionKind::Wait(d),
            InstructionKindDe::Command(s) => InstructionKind::Command(s),
            InstructionKindDe::CommandWait(s) => InstructionKind::CommandWait(s),
            InstructionKindDe::Comment(s) => InstructionKind::Comment(s),
            InstructionKindDe::WhenRan => InstructionKind::WhenRan,
            InstructionKindDe::WhenBatteryDischargedTo(v) => {
                InstructionKind::WhenBatteryDischargedTo(v)
            }
            InstructionKindDe::WhenBatteryChargedTo(v) => InstructionKind::WhenBatteryChargedTo(v),
            InstructionKindDe::WhenTime(WhenTimeDe::Current(s, t)) => InstructionKind::WhenTime(s, t),
            InstructionKindDe::WhenTime(WhenTimeDe::Legacy(old)) => {
                InstructionKind::WhenTime(old.schedule, Value::text(format!("{:02}:{:02}", old.hour, old.minute)))
            }
            InstructionKindDe::WhenPowerPluggedIn => InstructionKind::WhenPowerPluggedIn,
            InstructionKindDe::WhenPowerUnplugged => InstructionKind::WhenPowerUnplugged,
            InstructionKindDe::WhenClipboardChanged => InstructionKind::WhenClipboardChanged,
            InstructionKindDe::WhenKey(key, direction) => InstructionKind::WhenKey(key, direction),
            InstructionKindDe::OpenApp {
                command,
                name,
                icon,
                launch,
            } => InstructionKind::OpenApp {
                command,
                name,
                icon,
                launch,
            },
            InstructionKindDe::CloseApp {
                command,
                name,
                icon,
            } => InstructionKind::CloseApp {
                command,
                name,
                icon,
            },
            InstructionKindDe::SetVariable(n, v) => InstructionKind::SetVariable(n, v),
            InstructionKindDe::ChangeVariable(n, v) => InstructionKind::ChangeVariable(n, v),
            InstructionKindDe::SetClipboard(v) => InstructionKind::SetClipboard(v),
            InstructionKindDe::AddToList { value, name } => {
                InstructionKind::AddToList { value, name }
            }
            InstructionKindDe::DeleteOfList { index, name } => {
                InstructionKind::DeleteOfList { index, name }
            }
            InstructionKindDe::DeleteAllOfList { name } => {
                InstructionKind::DeleteAllOfList { name }
            }
            InstructionKindDe::ShiftList { name, amount } => {
                InstructionKind::ShiftList { name, amount }
            }
            InstructionKindDe::InsertIntoList { value, index, name } => {
                InstructionKind::InsertIntoList { value, index, name }
            }
            InstructionKindDe::ReplaceItemOfList { index, name, value } => {
                InstructionKind::ReplaceItemOfList { index, name, value }
            }
            InstructionKindDe::ReverseList { name } => InstructionKind::ReverseList { name },
            InstructionKindDe::BlockHeader(id) => InstructionKind::BlockHeader(id),
            InstructionKindDe::CallBlock {
                block_id,
                args,
                branches,
            } => InstructionKind::CallBlock {
                block_id,
                args,
                branches,
            },
            InstructionKindDe::RunBranch(name) => InstructionKind::RunBranch(name),
            InstructionKindDe::Return(v) => InstructionKind::Return(v),
            InstructionKindDe::If { condition, body } => InstructionKind::If { condition, body },
            InstructionKindDe::IfElse {
                condition,
                then_body,
                else_body,
            } => InstructionKind::IfElse {
                condition,
                then_body,
                else_body,
            },
            InstructionKindDe::Repeat { count, body } => InstructionKind::Repeat { count, body },
            InstructionKindDe::Forever { body } => InstructionKind::Forever { body },
            InstructionKindDe::While { condition, body } => {
                InstructionKind::While { condition, body }
            }
            InstructionKindDe::EscapeLoop => InstructionKind::EscapeLoop,
            InstructionKindDe::ContinueLoop => InstructionKind::ContinueLoop,
            InstructionKindDe::WriteFile { path, text, append } => {
                InstructionKind::WriteFile { path, text, append }
            }
            InstructionKindDe::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            } => InstructionKind::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            },
            InstructionKindDe::Notify { title, message } => InstructionKind::Notify { title, message },
            InstructionKindDe::Log(v) => InstructionKind::Log(v),
            InstructionKindDe::TryCatch { body, catch_body } => {
                InstructionKind::TryCatch { body, catch_body }
            }
            InstructionKindDe::ThrowError(v) => InstructionKind::ThrowError(v),
            InstructionKindDe::StopMacro => InstructionKind::StopMacro,
            InstructionKindDe::WhenWindow { pattern, focused } => {
                InstructionKind::WhenWindow { pattern, focused }
            }
            InstructionKindDe::WhenFileChanged(v) => InstructionKind::WhenFileChanged(v),
            InstructionKindDe::Retry { count, delay, body } => InstructionKind::Retry { count, delay, body },
            InstructionKindDe::WindowAction { action, pattern, by } => {
                InstructionKind::WindowAction { action, pattern, by }
            }
            InstructionKindDe::MoveWindow { pattern, by, resize, x, y } => {
                InstructionKind::MoveWindow { pattern, by, resize, x, y }
            }
            InstructionKindDe::RunMacro { target, wait } => InstructionKind::RunMacro { target, wait },
            InstructionKindDe::StopMacroNamed(v) => InstructionKind::StopMacroNamed(v),
            InstructionKindDe::SetGlobal { name, value } => InstructionKind::SetGlobal { name, value },
            InstructionKindDe::ChangeGlobal { name, by } => InstructionKind::ChangeGlobal { name, by },
            InstructionKindDe::WaitUntil { condition, timeout, error_timeout } => InstructionKind::WaitUntil { condition, timeout, error_timeout },
            InstructionKindDe::FindImage { image, tolerance, x_variable, y_variable, pick, region, count_variable } => {
                InstructionKind::FindImage { image, tolerance, x_variable, y_variable, pick, region, count_variable }
            }
            InstructionKindDe::ClickImage { image, tolerance, button, pick, region } => {
                InstructionKind::ClickImage { image, tolerance, button, pick, region }
            }
            InstructionKindDe::WaitForImage { image, tolerance, appear, timeout, region, error_timeout } => {
                InstructionKind::WaitForImage { image, tolerance, appear, timeout, region, error_timeout }
            }
            InstructionKindDe::WhenTyped { text, erase } => InstructionKind::WhenTyped { text, erase },
            InstructionKindDe::PasteText(v) => InstructionKind::PasteText(v),
            InstructionKindDe::WhenApp { pattern, started } => InstructionKind::WhenApp { pattern, started },
            InstructionKindDe::WhenIdle(v) => InstructionKind::WhenIdle(v),
            InstructionKindDe::WhenScreenLock { locked } => InstructionKind::WhenScreenLock { locked },
            InstructionKindDe::WithLock { name, body } => InstructionKind::WithLock { name, body },
            InstructionKindDe::WhenCron(v) => InstructionKind::WhenCron(v),
            InstructionKindDe::SplitIntoList { text, separator, name } => InstructionKind::SplitIntoList { text, separator, name },
            InstructionKindDe::WhenNetwork { connected } => InstructionKind::WhenNetwork { connected },
            InstructionKindDe::WhenUsb { pattern, connected } => InstructionKind::WhenUsb { pattern, connected },
            InstructionKindDe::WhenMouseButton { button, pressed, modifiers } => {
                InstructionKind::WhenMouseButton { button, pressed, modifiers }
            }
            InstructionKindDe::AskText { prompt, default, name } => InstructionKind::AskText { prompt, default, name },
            InstructionKindDe::AskYesNo { prompt, name } => InstructionKind::AskYesNo { prompt, name },
            InstructionKindDe::AskChoice { prompt, list, name } => InstructionKind::AskChoice { prompt, list, name },
            InstructionKindDe::ShowMessage(v) => InstructionKind::ShowMessage(v),
            InstructionKindDe::WhenAt(v) => InstructionKind::WhenAt(v),
            InstructionKindDe::WhenSignal(v) => InstructionKind::WhenSignal(v),
            InstructionKindDe::SendSignal { name, wait } => InstructionKind::SendSignal { name, wait },
            InstructionKindDe::WhenWebhook { path, variable } => InstructionKind::WhenWebhook { path, variable },
            InstructionKindDe::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
                InstructionKind::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region }
            }
            InstructionKindDe::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
                InstructionKind::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region }
            }
            InstructionKindDe::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => {
                InstructionKind::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone }
            }
            InstructionKindDe::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
                InstructionKind::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable }
            }
            InstructionKindDe::BrowserOpen { url, new_tab } => InstructionKind::BrowserOpen { url, new_tab },
            InstructionKindDe::BrowserClick { selector, nth } => InstructionKind::BrowserClick { selector, nth },
            InstructionKindDe::BrowserType { text, selector, nth } => InstructionKind::BrowserType { text, selector, nth },
            InstructionKindDe::BrowserWaitFor { selector, appear, timeout } => {
                InstructionKind::BrowserWaitFor { selector, appear, timeout }
            }
            InstructionKindDe::BrowserScript { code, variable } => InstructionKind::BrowserScript { code, variable },
        }
    }
}
