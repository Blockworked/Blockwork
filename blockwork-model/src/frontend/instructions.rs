//! The instruction wire shape the editor reads and writes (`InstructionDto`)
//! and its conversions to and from `InstructionKind`.
//!
//! A block whose wire fields are its `InstructionKind` fields, copied, is one
//! line in the `wire_blocks!` table below. A block that needs a real
//! conversion goes under `custom` with a hand-written arm in each direction.
//! `tests/wire_blocks.rs` holds every block's round trip.

use super::*;
use crate::input::index_to_mouse_button;
use crate::macros::StopScope;

/// One field's wire form: the same value for most types, the DTO list for a
/// nested body.
trait Wire<W>: Sized {
    fn to_wire(&self) -> W;
    fn from_wire(wire: &W) -> Option<Self>;
}

impl<T: Clone> Wire<T> for T {
    fn to_wire(&self) -> T {
        self.clone()
    }
    fn from_wire(wire: &T) -> Option<T> {
        Some(wire.clone())
    }
}

/// A body is dropped whole if any block in it doesn't convert.
impl Wire<Vec<InstructionDto>> for Vec<Instruction> {
    fn to_wire(&self) -> Vec<InstructionDto> {
        self.iter().map(instruction_to_dto).collect()
    }
    fn from_wire(wire: &Vec<InstructionDto>) -> Option<Self> {
        wire.iter().map(dto_to_instruction).collect()
    }
}

/// Declares `InstructionDto` and both conversions. `unit` blocks carry only an
/// id, `tuple` blocks map `InstructionKind::Name(a, b)` onto named wire fields
/// in that order, `named` blocks share their field names with the kind, and
/// `custom` variants get the hand-written arms in `to_wire`/`from_wire`.
macro_rules! wire_blocks {
    (
        unit { $($unit:ident),* $(,)? }
        tuple { $($tup:ident ( $($(#[$tm:meta])* $tf:ident : $tt:ty),+ )),* $(,)? }
        named { $($(#[$nv:meta])* $nam:ident { $($(#[$nm:meta])* $nf:ident : $nt:ty),+ }),* $(,)? }
        custom { $($cus:ident { $($(#[$cm:meta])* $cf:ident : $ct:ty),* }),* $(,)? }
        to_wire($ins:ident, $id:ident) { $($tp:pat $(if $tg:expr)? => $te:expr),* $(,)? }
        from_wire($dto:ident) { $($fp:pat => $fe:expr),* $(,)? }
    ) => {
        #[derive(Serialize, Deserialize, Clone, Debug)]
        #[serde(tag = "type")]
        pub enum InstructionDto {
            $($unit { id: String },)*
            $($tup { id: String, $($(#[$tm])* $tf: $tt,)+ },)*
            $($(#[$nv])* $nam { id: String, $($(#[$nm])* $nf: $nt,)+ },)*
            $($cus { id: String, $($(#[$cm])* $cf: $ct,)* },)*
        }

        pub fn instruction_to_dto($ins: &Instruction) -> InstructionDto {
            let $id = $ins.id.clone();
            match &$ins.kind {
                $(InstructionKind::$unit => InstructionDto::$unit { id: $id },)*
                $(InstructionKind::$tup($($tf),+) => InstructionDto::$tup {
                    id: $id,
                    $($tf: <_ as Wire<$tt>>::to_wire($tf),)+
                },)*
                $(InstructionKind::$nam { $($nf),+ } => InstructionDto::$nam {
                    id: $id,
                    $($nf: <_ as Wire<$nt>>::to_wire($nf),)+
                },)*
                $($tp $(if $tg)? => $te,)*
            }
        }

        pub fn dto_to_instruction($dto: &InstructionDto) -> Option<Instruction> {
            let (id, kind) = match $dto {
                $(InstructionDto::$unit { id } => (id, InstructionKind::$unit),)*
                $(InstructionDto::$tup { id, $($tf),+ } => {
                    (id, InstructionKind::$tup($(<_ as Wire<$tt>>::from_wire($tf)?),+))
                })*
                $(InstructionDto::$nam { id, $($nf),+ } => (id, InstructionKind::$nam {
                    $($nf: <_ as Wire<$nt>>::from_wire($nf)?,)+
                }),)*
                $($fp => $fe,)*
            };
            Some(Instruction { id: id.clone(), kind })
        }
    };
}

wire_blocks! {
    unit {
        WhenRan,
        WhenPowerPluggedIn,
        WhenPowerUnplugged,
        WhenClipboardChanged,
        EscapeLoop,
        ContinueLoop,
    }
    tuple {
        Wait(duration: Value),
        Command(command: String),
        CommandWait(command: String),
        Comment(comment: String),
        WhenBatteryDischargedTo(threshold: Value),
        WhenBatteryChargedTo(threshold: Value),
        WhenTime(schedule: TimeSchedule, #[serde(default = "default_schedule_time")] time: Value),
        SetVariable(name: String, value: Value),
        ChangeVariable(name: String, value: Value),
        SetClipboard(value: Value),
        BlockHeader(block_id: String),
        RunBranch(name: String),
        Return(value: Value),
        Log(value: Value),
        ThrowError(message: Value),
        WhenFileChanged(path: Value),
        StopMacroNamed(target: Value),
        PasteText(text: Value),
        WhenIdle(minutes: Value),
        WhenCron(expression: Value),
        ShowMessage(message: Value),
        WhenAt(at: Value),
    }
    named {
        #[serde(alias = "StopMacro")] Stop { #[serde(default)] scope: StopScope },
        OpenApp { command: String, name: String, icon: Option<String>, #[serde(default)] launch: LaunchOptions },
        CloseApp { command: String, name: String, icon: Option<String> },
        AddToList { value: Value, name: String },
        DeleteOfList { index: Value, name: String },
        DeleteAllOfList { name: String },
        ShiftList { name: String, amount: Value },
        InsertIntoList { value: Value, index: Value, name: String },
        ReplaceItemOfList { index: Value, name: String, value: Value },
        ReverseList { name: String },
        If { condition: Value, body: Vec<InstructionDto> },
        IfElse { condition: Value, then_body: Vec<InstructionDto>, else_body: Vec<InstructionDto> },
        Repeat { count: Value, body: Vec<InstructionDto> },
        Forever { body: Vec<InstructionDto> },
        While { condition: Value, body: Vec<InstructionDto> },
        WriteFile { path: Value, text: Value, append: bool },
        HttpRequest { method: String, url: Value, body: Value, headers: Value, variable: String },
        Notify { title: Value, message: Value },
        TryCatch { body: Vec<InstructionDto>, catch_body: Vec<InstructionDto> },
        WhenWindow { pattern: Value, focused: bool },
        Retry { count: Value, delay: Value, body: Vec<InstructionDto> },
        WindowAction { action: String, pattern: Value, #[serde(default = "match_by_title")] by: String },
        MoveWindow { pattern: Value, #[serde(default = "match_by_title")] by: String, resize: bool, x: Value, y: Value },
        RunMacro { target: Value, wait: bool },
        SetGlobal { name: Value, value: Value },
        ChangeGlobal { name: Value, by: Value },
        WaitUntil { condition: Value, #[serde(default, skip_serializing_if = "Option::is_none")] timeout: Option<Value>, #[serde(default, skip_serializing_if = "Option::is_none")] error_timeout: Option<Value> },
        FindImage { image: String, #[serde(default, skip_serializing_if = "Option::is_none")] tolerance: Option<Value>, x_variable: String, y_variable: String, #[serde(default, skip_serializing_if = "Option::is_none")] pick: Option<crate::macros::ImageMatch>, #[serde(default, skip_serializing_if = "Option::is_none")] region: Option<String>, #[serde(default, skip_serializing_if = "Option::is_none")] count_variable: Option<String> },
        ClickImage { image: String, #[serde(default, skip_serializing_if = "Option::is_none")] tolerance: Option<Value>, button: crate::input::types::MacroButton, #[serde(default, skip_serializing_if = "Option::is_none")] pick: Option<crate::macros::ImageMatch>, #[serde(default, skip_serializing_if = "Option::is_none")] region: Option<String> },
        WaitForImage { image: String, #[serde(default, skip_serializing_if = "Option::is_none")] tolerance: Option<Value>, appear: bool, #[serde(default, skip_serializing_if = "Option::is_none")] timeout: Option<Value>, #[serde(default, skip_serializing_if = "Option::is_none")] region: Option<String>, #[serde(default, skip_serializing_if = "Option::is_none")] error_timeout: Option<Value> },
        WhenTyped { text: Value, erase: bool },
        WhenApp { pattern: Value, started: bool },
        WhenScreenLock { locked: bool },
        WithLock { name: Value, body: Vec<InstructionDto> },
        SplitIntoList { text: Value, separator: Value, name: String },
        WhenNetwork { connected: bool },
        WhenUsb { pattern: Value, connected: bool },
        AskText { prompt: Value, default: Value, name: String },
        AskYesNo { prompt: Value, name: String },
        AskChoice { prompt: Value, list: String, name: String },
        WhenWebhook { path: Value, variable: String },
        DetectObjectsInWindow { pattern: Value, by: String, detector: Value, label: Value, confidence: Value, sort: String, results_list: String, count_variable: String, #[serde(default)] region: String },
        WaitForDetection { pattern: Value, by: String, detector: Value, label: Value, confidence: Value, appear: bool, timeout: Value, #[serde(default)] region: String },
        AimStep { pattern: Value, by: String, target_x: Value, target_y: Value, gain: Value, max_step: Value, deadzone: Value },
        AimAtDetection { results_list: String, index: Value, gain: Value, max_step: Value, deadzone: Value, timeout: Value, settle_ms: Value, status_variable: String },
        BrowserOpen { url: Value, new_tab: bool },
        BrowserClick { selector: Value, #[serde(default, skip_serializing_if = "Option::is_none")] nth: Option<Value> },
        BrowserType { text: Value, selector: Value, #[serde(default, skip_serializing_if = "Option::is_none")] nth: Option<Value> },
        BrowserWaitFor { selector: Value, appear: bool, timeout: Value },
        BrowserScript { code: Value, variable: String },
    }
    custom {
        Text { text: Value },
        Key { key: Value, direction: String },
        WhenKey { key: Value, direction: String },
        Button { button: String, direction: String },
        MoveMouse { x: Value, y: Value, coordinate: String },
        Scroll { amount: Value, axis: String },
        CallBlock { block_id: String, args: Vec<Value> },
        BranchCallBlock { block_id: String, args: Vec<Value>, branches: Vec<Vec<InstructionDto>> },
        WhenMouseButton { button: String, pressed: bool, #[serde(default, skip_serializing_if = "Option::is_none")] modifiers: Option<Value> },
        WhenSignal { #[serde(default)] expression: bool, name: Value },
        SendSignal { #[serde(default)] expression: bool, name: Value, wait: bool },
    }
    to_wire(ins, id) {
        InstructionKind::WhenKey(key, direction) => InstructionDto::WhenKey {
            id,
            key: key.clone(),
            direction: direction_to_str(direction).to_string(),
        },
        InstructionKind::CallBlock {
            block_id,
            args,
            branches,
        } if branches.is_empty() => InstructionDto::CallBlock {
            id,
            block_id: block_id.clone(),
            args: args.clone(),
        },
        InstructionKind::CallBlock {
            block_id,
            args,
            branches,
        } => InstructionDto::BranchCallBlock {
            id,
            block_id: block_id.clone(),
            args: args.clone(),
            branches: branches
                .iter()
                .map(|branch| branch.iter().map(instruction_to_dto).collect())
                .collect(),
        },
        InstructionKind::WhenMouseButton { button, pressed, modifiers } => InstructionDto::WhenMouseButton {
            id,
            button: get_mouse_button_names()[mouse_button_to_index(button)].to_string(),
            pressed: *pressed,
            modifiers: modifiers.clone(),
        },
        InstructionKind::WhenSignal(name) => InstructionDto::WhenSignal { id, name: name.value.clone(), expression: name.expression },
        InstructionKind::SendSignal { name, wait } => InstructionDto::SendSignal { id, name: name.value.clone(), expression: name.expression, wait: *wait },
        InstructionKind::Token(token) => match token {
            InputToken::Text(t) => InstructionDto::Text {
                id,
                text: t.clone(),
            },
            InputToken::Key(k, d) => InstructionDto::Key {
                id,
                key: k.clone(),
                direction: direction_to_str(d).to_string(),
            },
            InputToken::Button(b, d) => InstructionDto::Button {
                id,
                button: get_mouse_button_names()[mouse_button_to_index(b)].to_string(),
                direction: direction_to_str(d).to_string(),
            },
            InputToken::MoveMouse(x, y, coord) => InstructionDto::MoveMouse {
                id,
                x: x.clone(),
                y: y.clone(),
                coordinate: coordinate_to_str(coord).to_string(),
            },
            InputToken::Scroll(amt, axis) => InstructionDto::Scroll {
                id,
                amount: amt.clone(),
                axis: axis_to_str(axis).to_string(),
            },
            InputToken::Raw(_, _) => InstructionDto::Comment {
                id,
                comment: "(raw keycode)".to_string(),
            },
        },
    }
    from_wire(dto) {
        InstructionDto::Text { id, text } => (
            id,
            InstructionKind::Token(InputToken::Text(text.clone())),
        ),
        InstructionDto::Key { id, key, direction } => (
            id,
            InstructionKind::Token(InputToken::Key(key.clone(), str_to_direction(direction))),
        ),
        InstructionDto::WhenKey { id, key, direction } => (
            id,
            InstructionKind::WhenKey(key.clone(), str_to_direction(direction)),
        ),
        InstructionDto::Button {
            id,
            button,
            direction,
        } => {
            let names = get_mouse_button_names();
            let idx = names
                .iter()
                .position(|&n| n == button.as_str())
                .unwrap_or(0);
            (
                id,
                InstructionKind::Token(InputToken::Button(
                    index_to_mouse_button(idx),
                    str_to_direction(direction),
                )),
            )
        },
        InstructionDto::MoveMouse {
            id,
            x,
            y,
            coordinate,
        } => (
            id,
            InstructionKind::Token(InputToken::MoveMouse(
                x.clone(),
                y.clone(),
                str_to_coordinate(coordinate),
            )),
        ),
        InstructionDto::Scroll { id, amount, axis } => (
            id,
            InstructionKind::Token(InputToken::Scroll(amount.clone(), str_to_axis(axis))),
        ),
        InstructionDto::CallBlock { id, block_id, args } => (
            id,
            InstructionKind::CallBlock {
                block_id: block_id.clone(),
                args: args.clone(),
                branches: Vec::new(),
            },
        ),
        InstructionDto::BranchCallBlock {
            id,
            block_id,
            args,
            branches,
        } => (
            id,
            InstructionKind::CallBlock {
                block_id: block_id.clone(),
                args: args.clone(),
                branches: branches
                    .iter()
                    .map(|branch| branch.iter().filter_map(dto_to_instruction).collect())
                    .collect(),
            },
        ),
        InstructionDto::WhenMouseButton { id, button, pressed, modifiers } => {
            let index = get_mouse_button_names().iter().position(|&n| n == button.as_str())?;
            (
                id,
                InstructionKind::WhenMouseButton {
                    button: index_to_mouse_button(index),
                    pressed: *pressed,
                    modifiers: modifiers.clone(),
                },
            )
        },
        InstructionDto::WhenSignal { id, name, expression } => (id, InstructionKind::WhenSignal(crate::macros::SignalName::new(name.clone(), *expression))),
        InstructionDto::SendSignal { id, name, expression, wait } => (id, InstructionKind::SendSignal { name: crate::macros::SignalName::new(name.clone(), *expression), wait: *wait }),
    }
}

#[derive(Serialize, Clone)]
pub struct InvalidFieldDto {
    pub location: ValueLocation,
    pub text: String,
}

pub fn direction_to_str(d: &Direction) -> &'static str {
    match d {
        Direction::Click => "Click",
        Direction::Press => "Press",
        Direction::Release => "Release",
    }
}

pub fn str_to_direction(s: &str) -> Direction {
    match s {
        "Press" => Direction::Press,
        "Release" => Direction::Release,
        _ => Direction::Click,
    }
}

pub fn coordinate_to_str(c: &Coordinate) -> &'static str {
    match c {
        Coordinate::Abs => "Absolute",
        Coordinate::Rel => "Relative",
    }
}

pub fn str_to_coordinate(s: &str) -> Coordinate {
    match s {
        "Relative" => Coordinate::Rel,
        _ => Coordinate::Abs,
    }
}

pub fn axis_to_str(a: &Axis) -> &'static str {
    match a {
        Axis::Vertical => "Vertical",
        Axis::Horizontal => "Horizontal",
    }
}

pub fn str_to_axis(s: &str) -> Axis {
    match s {
        "Horizontal" => Axis::Horizontal,
        _ => Axis::Vertical,
    }
}

pub(super) fn default_schedule_time() -> Value {
    Value::text("09:00")
}

pub(super) fn match_by_title() -> String {
    "Title".into()
}
