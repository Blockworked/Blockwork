//! The editor's frontend shape: a macro as the QML canvas reads it and the
//! flat `{"type": ...}` instructions it sends back. The desktop app and the
//! website's in-browser editor both speak exactly this, so it lives with the
//! vocabulary rather than in either host.

use crate::block_packs;
use crate::input::schedule::TimeSchedule;
use crate::input::types::{Axis, Coordinate, Direction, InputToken};
use crate::input::value::Value;
use crate::input::{get_mouse_button_names, mouse_button_to_index};
use crate::macros::{
    BlockDef, Comment, FloatingValue, Instruction, InstructionKind, LaunchOptions, Macro, MacroSettings, Strand,
};
use blockstitch_core::editor::{InstrPath, ValueLocation};
use blockstitch_core::graph::{Fragment, ListDef, ListItem};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", content = "value")]
pub enum ListItemDto {
    Number(f64),
    Text(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListDto {
    pub name: String,
    pub items: Vec<ListItemDto>,
    pub editor_visible: bool,
    pub editor_x: i32,
    pub editor_y: i32,
}

pub fn list_to_dto(list: &ListDef) -> ListDto {
    ListDto {
        name: list.name.clone(),
        items: list
            .items
            .iter()
            .map(|item| match item {
                ListItem::Number(value) => ListItemDto::Number(*value),
                ListItem::Text(value) => ListItemDto::Text(value.clone()),
            })
            .collect(),
        editor_visible: list.editor_visible,
        editor_x: list.editor_x,
        editor_y: list.editor_y,
    }
}

pub fn dto_to_list_item(item: &ListItemDto) -> ListItem {
    match item {
        ListItemDto::Number(value) => ListItem::Number(*value),
        ListItemDto::Text(value) => ListItem::Text(value.clone()),
    }
}

#[derive(Serialize, Clone)]
pub struct MacroDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub strands: Vec<StrandDto>,
    pub recording_target_strand_id: Option<String>,
    pub speed_multiplier: f64,
    pub floating_values: Vec<FloatingValue>,
    /// Floating/attached notes - see `Comment`.
    pub comments: Vec<Comment>,
    /// Declared variable names only, for the sidebar/dropdowns - current
    /// values aren't surfaced to the frontend.
    pub variables: Vec<String>,
    /// Macro-scoped lists, including their literal items for the list editor.
    pub lists: Vec<ListDto>,
    pub signals: Vec<String>,
    /// User-defined custom blocks ("My Blocks").
    pub block_defs: Vec<BlockDefinitionDto>,
    /// Settings edited from the "Macro Settings" popup - see `MacroSettingsDto`.
    pub settings: MacroSettingsDto,
}

#[derive(Serialize, Clone)]
pub struct BlockDefinitionDto {
    #[serde(flatten)]
    pub definition: BlockDef,
    pub read_only: bool,
    pub pack_name: String,
    pub packs: Vec<BlockPackMembershipDto>,
}

#[derive(Serialize, Clone)]
pub struct BlockPackMembershipDto {
    pub id: String,
    pub name: String,
}

/// Wire shape for `MacroSettings` - see there for field meanings.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MacroSettingsDto {
    pub always_listen: bool,
    pub max_run_secs: u32,
    pub max_loop_iterations: u32,
    pub only_in_window: String,
    pub power: crate::macros::conditions::PowerCondition,
    /// `HH:MM`, both empty when there are no active hours.
    pub active_from: String,
    pub active_until: String,
    pub active_days: u8,
    pub folder: String,
    pub catch_up_missed: bool,
}

pub fn macro_settings_to_dto(settings: &MacroSettings) -> MacroSettingsDto {
    use crate::macros::conditions::clock;
    let (active_from, active_until) = settings.active_hours.map(|(a, b)| (clock(a), clock(b))).unwrap_or_default();
    MacroSettingsDto {
        always_listen: settings.always_listen,
        max_run_secs: settings.max_run_secs,
        max_loop_iterations: settings.max_loop_iterations,
        only_in_window: settings.only_in_window.clone(),
        power: settings.power,
        active_from,
        active_until,
        active_days: settings.active_days,
        folder: settings.folder.clone(),
        catch_up_missed: settings.catch_up_missed,
    }
}

#[derive(Serialize, Clone)]
pub struct StrandDto {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub instructions: Vec<InstructionDto>,
}

#[derive(Serialize, Clone)]
pub struct KeyCaptureDto {
    pub kind: String,
    pub strand_id: Option<String>,
    pub index: Option<InstrPath>,
}

/// Copied blocks and the notes pinned to them, as the editor's copy buffer
/// holds them.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FragmentDto {
    pub instructions: Vec<InstructionDto>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

pub fn fragment_to_dto(fragment: &Fragment<InstructionKind>) -> FragmentDto {
    FragmentDto {
        instructions: fragment.instructions.iter().map(instruction_to_dto).collect(),
        comments: fragment.comments.clone(),
    }
}

pub fn dto_to_fragment(dto: &FragmentDto) -> Option<Fragment<InstructionKind>> {
    Some(Fragment {
        instructions: dto
            .instructions
            .iter()
            .map(dto_to_instruction)
            .collect::<Option<_>>()?,
        comments: dto.comments.clone(),
    })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum InstructionDto {
    Wait {
        id: String,
        duration: Value,
    },
    Text {
        id: String,
        text: Value,
    },
    Key {
        id: String,
        key: Value,
        direction: String,
    },
    WhenKey {
        id: String,
        key: Value,
        direction: String,
    },
    Button {
        id: String,
        button: String,
        direction: String,
    },
    MoveMouse {
        id: String,
        x: Value,
        y: Value,
        coordinate: String,
    },
    Scroll {
        id: String,
        amount: Value,
        axis: String,
    },
    Command {
        id: String,
        command: String,
    },
    CommandWait {
        id: String,
        command: String,
    },
    Comment {
        id: String,
        comment: String,
    },
    WhenRan {
        id: String,
    },
    WhenBatteryDischargedTo {
        id: String,
        threshold: Value,
    },
    WhenBatteryChargedTo {
        id: String,
        threshold: Value,
    },
    WhenTime {
        id: String,
        schedule: TimeSchedule,
        #[serde(default = "default_schedule_time")]
        time: Value,
    },
    WhenPowerPluggedIn {
        id: String,
    },
    WhenPowerUnplugged {
        id: String,
    },
    WhenClipboardChanged {
        id: String,
    },
    OpenApp {
        id: String,
        command: String,
        name: String,
        icon: Option<String>,
        #[serde(default)]
        launch: LaunchOptions,
    },
    CloseApp {
        id: String,
        command: String,
        name: String,
        icon: Option<String>,
    },
    SetVariable {
        id: String,
        name: String,
        value: Value,
    },
    ChangeVariable {
        id: String,
        name: String,
        value: Value,
    },
    SetClipboard {
        id: String,
        value: Value,
    },
    AddToList { id: String, value: Value, name: String },
    DeleteOfList { id: String, index: Value, name: String },
    DeleteAllOfList { id: String, name: String },
    ShiftList { id: String, name: String, amount: Value },
    InsertIntoList { id: String, value: Value, index: Value, name: String },
    ReplaceItemOfList { id: String, index: Value, name: String, value: Value },
    ReverseList { id: String, name: String },
    BlockHeader {
        id: String,
        block_id: String,
    },
    CallBlock {
        id: String,
        block_id: String,
        args: Vec<Value>,
    },
    BranchCallBlock {
        id: String,
        block_id: String,
        args: Vec<Value>,
        branches: Vec<Vec<InstructionDto>>,
    },
    RunBranch {
        id: String,
        name: String,
    },
    Return {
        id: String,
        value: Value,
    },
    If {
        id: String,
        condition: Value,
        body: Vec<InstructionDto>,
    },
    IfElse {
        id: String,
        condition: Value,
        then_body: Vec<InstructionDto>,
        else_body: Vec<InstructionDto>,
    },
    Repeat {
        id: String,
        count: Value,
        body: Vec<InstructionDto>,
    },
    Forever {
        id: String,
        body: Vec<InstructionDto>,
    },
    While {
        id: String,
        condition: Value,
        body: Vec<InstructionDto>,
    },
    EscapeLoop {
        id: String,
    },
    ContinueLoop {
        id: String,
    },
    WriteFile {
        id: String,
        path: Value,
        text: Value,
        append: bool,
    },
    HttpRequest {
        id: String,
        method: String,
        url: Value,
        body: Value,
        headers: Value,
        variable: String,
    },
    Notify {
        id: String,
        title: Value,
        message: Value,
    },
    Log {
        id: String,
        value: Value,
    },
    TryCatch {
        id: String,
        body: Vec<InstructionDto>,
        catch_body: Vec<InstructionDto>,
    },
    ThrowError {
        id: String,
        message: Value,
    },
    StopMacro {
        id: String,
    },
    WhenWindow {
        id: String,
        pattern: Value,
        focused: bool,
    },
    WhenFileChanged {
        id: String,
        path: Value,
    },
    Retry {
        id: String,
        count: Value,
        delay: Value,
        body: Vec<InstructionDto>,
    },
    WindowAction {
        id: String,
        action: String,
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
    },
    MoveWindow {
        id: String,
        pattern: Value,
        #[serde(default = "match_by_title")]
        by: String,
        resize: bool,
        x: Value,
        y: Value,
    },
    RunMacro {
        id: String,
        target: Value,
        wait: bool,
    },
    StopMacroNamed {
        id: String,
        target: Value,
    },
    SetGlobal {
        id: String,
        name: Value,
        value: Value,
    },
    ChangeGlobal {
        id: String,
        name: Value,
        by: Value,
    },
    WaitUntil {
        id: String,
        condition: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_timeout: Option<Value>,
    },
    FindImage {
        id: String,
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        x_variable: String,
        y_variable: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pick: Option<crate::macros::ImageMatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count_variable: Option<String>,
    },
    ClickImage {
        id: String,
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        button: crate::input::types::MacroButton,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pick: Option<crate::macros::ImageMatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
    },
    WaitForImage {
        id: String,
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tolerance: Option<Value>,
        appear: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_timeout: Option<Value>,
    },
    WhenTyped {
        id: String,
        text: Value,
        erase: bool,
    },
    PasteText {
        id: String,
        text: Value,
    },
    WhenApp {
        id: String,
        pattern: Value,
        started: bool,
    },
    WhenIdle {
        id: String,
        minutes: Value,
    },
    WhenScreenLock {
        id: String,
        locked: bool,
    },
    WithLock {
        id: String,
        name: Value,
        body: Vec<InstructionDto>,
    },
    WhenCron {
        id: String,
        expression: Value,
    },
    SplitIntoList {
        id: String,
        text: Value,
        separator: Value,
        name: String,
    },
    WhenNetwork {
        id: String,
        connected: bool,
    },
    WhenUsb {
        id: String,
        pattern: Value,
        connected: bool,
    },
    WhenMouseButton {
        id: String,
        button: String,
        pressed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modifiers: Option<Value>,
    },
    AskText {
        id: String,
        prompt: Value,
        default: Value,
        name: String,
    },
    AskYesNo {
        id: String,
        prompt: Value,
        name: String,
    },
    AskChoice {
        id: String,
        prompt: Value,
        list: String,
        name: String,
    },
    ShowMessage {
        id: String,
        message: Value,
    },
    WhenAt {
        id: String,
        at: Value,
    },
    WhenSignal {
        id: String,
        #[serde(default)]
        expression: bool,
        name: Value,
    },
    SendSignal {
        id: String,
        #[serde(default)]
        expression: bool,
        name: Value,
        wait: bool,
    },
    WhenWebhook {
        id: String,
        path: Value,
        variable: String,
    },
    DetectObjectsInWindow {
        id: String,
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
        id: String,
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
        id: String,
        pattern: Value,
        by: String,
        target_x: Value,
        target_y: Value,
        gain: Value,
        max_step: Value,
        deadzone: Value,
    },
    AimAtDetection {
        id: String,
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
        id: String,
        url: Value,
        new_tab: bool,
    },
    BrowserClick {
        id: String,
        selector: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<Value>,
    },
    BrowserType {
        id: String,
        text: Value,
        selector: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nth: Option<Value>,
    },
    BrowserWaitFor {
        id: String,
        selector: Value,
        appear: bool,
        timeout: Value,
    },
    BrowserScript {
        id: String,
        code: Value,
        variable: String,
    },
}

#[derive(Serialize, Clone)]
pub struct InvalidFieldDto {
    pub location: ValueLocation,
    pub text: String,
}

// ─── Conversions ───────────────────────────────────────────────────────────

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

fn default_schedule_time() -> Value {
    Value::text("09:00")
}

fn match_by_title() -> String {
    "Title".into()
}

pub fn instruction_to_dto(ins: &Instruction) -> InstructionDto {
    let id = ins.id.clone();
    match &ins.kind {
        InstructionKind::Wait(dur) => InstructionDto::Wait {
            id,
            duration: dur.clone(),
        },
        InstructionKind::Command(cmd) => InstructionDto::Command {
            id,
            command: cmd.clone(),
        },
        InstructionKind::CommandWait(cmd) => InstructionDto::CommandWait {
            id,
            command: cmd.clone(),
        },
        InstructionKind::Comment(c) => InstructionDto::Comment {
            id,
            comment: c.clone(),
        },
        InstructionKind::WhenRan => InstructionDto::WhenRan { id },
        InstructionKind::WhenBatteryDischargedTo(threshold) => {
            InstructionDto::WhenBatteryDischargedTo {
                id,
                threshold: threshold.clone(),
            }
        }
        InstructionKind::WhenBatteryChargedTo(threshold) => InstructionDto::WhenBatteryChargedTo {
            id,
            threshold: threshold.clone(),
        },
        InstructionKind::WhenTime(schedule, time) => InstructionDto::WhenTime {
            id,
            schedule: *schedule,
            time: time.clone(),
        },
        InstructionKind::WhenPowerPluggedIn => InstructionDto::WhenPowerPluggedIn { id },
        InstructionKind::WhenPowerUnplugged => InstructionDto::WhenPowerUnplugged { id },
        InstructionKind::WhenClipboardChanged => InstructionDto::WhenClipboardChanged { id },
        InstructionKind::WhenKey(key, direction) => InstructionDto::WhenKey {
            id,
            key: key.clone(),
            direction: direction_to_str(direction).to_string(),
        },
        InstructionKind::OpenApp {
            command,
            name,
            icon,
            launch,
        } => InstructionDto::OpenApp {
            id,
            command: command.clone(),
            name: name.clone(),
            icon: icon.clone(),
            launch: launch.clone(),
        },
        InstructionKind::CloseApp {
            command,
            name,
            icon,
        } => InstructionDto::CloseApp {
            id,
            command: command.clone(),
            name: name.clone(),
            icon: icon.clone(),
        },
        InstructionKind::SetVariable(name, value) => InstructionDto::SetVariable {
            id,
            name: name.clone(),
            value: value.clone(),
        },
        InstructionKind::ChangeVariable(name, value) => InstructionDto::ChangeVariable {
            id,
            name: name.clone(),
            value: value.clone(),
        },
        InstructionKind::SetClipboard(value) => InstructionDto::SetClipboard {
            id,
            value: value.clone(),
        },
        InstructionKind::AddToList { value, name } => InstructionDto::AddToList { id, value: value.clone(), name: name.clone() },
        InstructionKind::DeleteOfList { index, name } => InstructionDto::DeleteOfList { id, index: index.clone(), name: name.clone() },
        InstructionKind::DeleteAllOfList { name } => InstructionDto::DeleteAllOfList { id, name: name.clone() },
        InstructionKind::ShiftList { name, amount } => InstructionDto::ShiftList { id, name: name.clone(), amount: amount.clone() },
        InstructionKind::InsertIntoList { value, index, name } => InstructionDto::InsertIntoList { id, value: value.clone(), index: index.clone(), name: name.clone() },
        InstructionKind::ReplaceItemOfList { index, name, value } => InstructionDto::ReplaceItemOfList { id, index: index.clone(), name: name.clone(), value: value.clone() },
        InstructionKind::ReverseList { name } => InstructionDto::ReverseList { id, name: name.clone() },
        InstructionKind::BlockHeader(block_id) => InstructionDto::BlockHeader {
            id,
            block_id: block_id.clone(),
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
        InstructionKind::RunBranch(name) => InstructionDto::RunBranch {
            id,
            name: name.clone(),
        },
        InstructionKind::Return(value) => InstructionDto::Return {
            id,
            value: value.clone(),
        },
        InstructionKind::If { condition, body } => InstructionDto::If {
            id,
            condition: condition.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::IfElse {
            condition,
            then_body,
            else_body,
        } => InstructionDto::IfElse {
            id,
            condition: condition.clone(),
            then_body: then_body.iter().map(instruction_to_dto).collect(),
            else_body: else_body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::Repeat { count, body } => InstructionDto::Repeat {
            id,
            count: count.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::Forever { body } => InstructionDto::Forever {
            id,
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::While { condition, body } => InstructionDto::While {
            id,
            condition: condition.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::EscapeLoop => InstructionDto::EscapeLoop { id },
        InstructionKind::ContinueLoop => InstructionDto::ContinueLoop { id },
        InstructionKind::WriteFile { path, text, append } => InstructionDto::WriteFile {
            id,
            path: path.clone(),
            text: text.clone(),
            append: *append,
        },
        InstructionKind::HttpRequest {
            method,
            url,
            body,
            headers,
            variable,
        } => InstructionDto::HttpRequest {
            id,
            method: method.clone(),
            url: url.clone(),
            body: body.clone(),
            headers: headers.clone(),
            variable: variable.clone(),
        },
        InstructionKind::Notify { title, message } => InstructionDto::Notify {
            id,
            title: title.clone(),
            message: message.clone(),
        },
        InstructionKind::Log(value) => InstructionDto::Log {
            id,
            value: value.clone(),
        },
        InstructionKind::TryCatch { body, catch_body } => InstructionDto::TryCatch {
            id,
            body: body.iter().map(instruction_to_dto).collect(),
            catch_body: catch_body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::ThrowError(message) => InstructionDto::ThrowError {
            id,
            message: message.clone(),
        },
        InstructionKind::StopMacro => InstructionDto::StopMacro { id },
        InstructionKind::WhenWindow { pattern, focused } => InstructionDto::WhenWindow {
            id,
            pattern: pattern.clone(),
            focused: *focused,
        },
        InstructionKind::WhenFileChanged(path) => InstructionDto::WhenFileChanged {
            id,
            path: path.clone(),
        },
        InstructionKind::Retry { count, delay, body } => InstructionDto::Retry {
            id,
            count: count.clone(),
            delay: delay.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::WindowAction { action, pattern, by } => InstructionDto::WindowAction {
            id,
            action: action.clone(),
            pattern: pattern.clone(),
            by: by.clone(),
        },
        InstructionKind::MoveWindow { pattern, by, resize, x, y } => InstructionDto::MoveWindow {
            id,
            pattern: pattern.clone(),
            by: by.clone(),
            resize: *resize,
            x: x.clone(),
            y: y.clone(),
        },
        InstructionKind::RunMacro { target, wait } => InstructionDto::RunMacro {
            id,
            target: target.clone(),
            wait: *wait,
        },
        InstructionKind::StopMacroNamed(target) => InstructionDto::StopMacroNamed { id, target: target.clone() },
        InstructionKind::SetGlobal { name, value } => InstructionDto::SetGlobal {
            id,
            name: name.clone(),
            value: value.clone(),
        },
        InstructionKind::ChangeGlobal { name, by } => InstructionDto::ChangeGlobal {
            id,
            name: name.clone(),
            by: by.clone(),
        },
        InstructionKind::WaitUntil { condition, timeout, error_timeout } => InstructionDto::WaitUntil {
            id,
            condition: condition.clone(),
            timeout: timeout.clone(),
            error_timeout: error_timeout.clone(),
        },
        InstructionKind::FindImage { image, tolerance, x_variable, y_variable, pick, region, count_variable } => {
            InstructionDto::FindImage {
                id,
                image: image.clone(),
                tolerance: tolerance.clone(),
                x_variable: x_variable.clone(),
                y_variable: y_variable.clone(),
                pick: pick.clone(),
                region: region.clone(),
                count_variable: count_variable.clone(),
            }
        }
        InstructionKind::ClickImage { image, tolerance, button, pick, region } => InstructionDto::ClickImage {
            id,
            image: image.clone(),
            tolerance: tolerance.clone(),
            button: button.clone(),
            pick: pick.clone(),
            region: region.clone(),
        },
        InstructionKind::WaitForImage { image, tolerance, appear, timeout, region, error_timeout } => InstructionDto::WaitForImage {
            id,
            image: image.clone(),
            tolerance: tolerance.clone(),
            appear: *appear,
            timeout: timeout.clone(),
            error_timeout: error_timeout.clone(),
            region: region.clone(),
        },
        InstructionKind::WhenTyped { text, erase } => InstructionDto::WhenTyped { id, text: text.clone(), erase: *erase },
        InstructionKind::PasteText(text) => InstructionDto::PasteText { id, text: text.clone() },
        InstructionKind::WhenApp { pattern, started } => {
            InstructionDto::WhenApp { id, pattern: pattern.clone(), started: *started }
        }
        InstructionKind::WhenIdle(minutes) => InstructionDto::WhenIdle { id, minutes: minutes.clone() },
        InstructionKind::WhenScreenLock { locked } => InstructionDto::WhenScreenLock { id, locked: *locked },
        InstructionKind::WithLock { name, body } => InstructionDto::WithLock {
            id,
            name: name.clone(),
            body: body.iter().map(instruction_to_dto).collect(),
        },
        InstructionKind::WhenCron(expression) => InstructionDto::WhenCron { id, expression: expression.clone() },
        InstructionKind::SplitIntoList { text, separator, name } => InstructionDto::SplitIntoList {
            id,
            text: text.clone(),
            separator: separator.clone(),
            name: name.clone(),
        },
        InstructionKind::WhenNetwork { connected } => InstructionDto::WhenNetwork { id, connected: *connected },
        InstructionKind::WhenUsb { pattern, connected } => {
            InstructionDto::WhenUsb { id, pattern: pattern.clone(), connected: *connected }
        }
        InstructionKind::WhenMouseButton { button, pressed, modifiers } => InstructionDto::WhenMouseButton {
            id,
            button: get_mouse_button_names()[mouse_button_to_index(button)].to_string(),
            pressed: *pressed,
            modifiers: modifiers.clone(),
        },
        InstructionKind::AskText { prompt, default, name } => {
            InstructionDto::AskText { id, prompt: prompt.clone(), default: default.clone(), name: name.clone() }
        }
        InstructionKind::AskYesNo { prompt, name } => InstructionDto::AskYesNo { id, prompt: prompt.clone(), name: name.clone() },
        InstructionKind::AskChoice { prompt, list, name } => {
            InstructionDto::AskChoice { id, prompt: prompt.clone(), list: list.clone(), name: name.clone() }
        }
        InstructionKind::ShowMessage(message) => InstructionDto::ShowMessage { id, message: message.clone() },
        InstructionKind::WhenAt(at) => InstructionDto::WhenAt { id, at: at.clone() },
        InstructionKind::WhenSignal(name) => InstructionDto::WhenSignal { id, name: name.value.clone(), expression: name.expression },
        InstructionKind::SendSignal { name, wait } => InstructionDto::SendSignal { id, name: name.value.clone(), expression: name.expression, wait: *wait },
        InstructionKind::WhenWebhook { path, variable } => {
            InstructionDto::WhenWebhook { id, path: path.clone(), variable: variable.clone() }
        }
        InstructionKind::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
            InstructionDto::DetectObjectsInWindow {
                region: region.clone(),
                id,
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                sort: sort.clone(),
                results_list: results_list.clone(),
                count_variable: count_variable.clone(),
            }
        }
        InstructionKind::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
            InstructionDto::WaitForDetection {
                region: region.clone(),
                id,
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                appear: *appear,
                timeout: timeout.clone(),
            }
        }
        InstructionKind::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => InstructionDto::AimStep {
            id,
            pattern: pattern.clone(),
            by: by.clone(),
            target_x: target_x.clone(),
            target_y: target_y.clone(),
            gain: gain.clone(),
            max_step: max_step.clone(),
            deadzone: deadzone.clone(),
        },
        InstructionKind::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
            InstructionDto::AimAtDetection {
                id,
                results_list: results_list.clone(),
                index: index.clone(),
                gain: gain.clone(),
                max_step: max_step.clone(),
                deadzone: deadzone.clone(),
                timeout: timeout.clone(),
                settle_ms: settle_ms.clone(),
                status_variable: status_variable.clone(),
            }
        }
        InstructionKind::BrowserOpen { url, new_tab } => InstructionDto::BrowserOpen { id, url: url.clone(), new_tab: *new_tab },
        InstructionKind::BrowserClick { selector, nth } => {
            InstructionDto::BrowserClick { id, selector: selector.clone(), nth: nth.clone() }
        }
        InstructionKind::BrowserType { text, selector, nth } => {
            InstructionDto::BrowserType { id, text: text.clone(), selector: selector.clone(), nth: nth.clone() }
        }
        InstructionKind::BrowserWaitFor { selector, appear, timeout } => {
            InstructionDto::BrowserWaitFor { id, selector: selector.clone(), appear: *appear, timeout: timeout.clone() }
        }
        InstructionKind::BrowserScript { code, variable } => {
            InstructionDto::BrowserScript { id, code: code.clone(), variable: variable.clone() }
        }
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
}

pub fn dto_to_instruction(dto: &InstructionDto) -> Option<Instruction> {
    use crate::input::index_to_mouse_button;
    let (id, kind) = match dto {
        InstructionDto::Wait { id, duration } => {
            (id, InstructionKind::Wait(duration.clone()))
        }
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
        }
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
        InstructionDto::Command { id, command } => (id, InstructionKind::Command(command.clone())),
        InstructionDto::CommandWait { id, command } => (id, InstructionKind::CommandWait(command.clone())),
        InstructionDto::Comment { id, comment } => (id, InstructionKind::Comment(comment.clone())),
        InstructionDto::WhenRan { id } => (id, InstructionKind::WhenRan),
        InstructionDto::WhenBatteryDischargedTo { id, threshold } => (
            id,
            InstructionKind::WhenBatteryDischargedTo(threshold.clone()),
        ),
        InstructionDto::WhenBatteryChargedTo { id, threshold } => (
            id,
            InstructionKind::WhenBatteryChargedTo(threshold.clone()),
        ),
        InstructionDto::WhenTime { id, schedule, time } => (id, InstructionKind::WhenTime(*schedule, time.clone())),
        InstructionDto::WhenPowerPluggedIn { id } => (id, InstructionKind::WhenPowerPluggedIn),
        InstructionDto::WhenPowerUnplugged { id } => (id, InstructionKind::WhenPowerUnplugged),
        InstructionDto::WhenClipboardChanged { id } => (id, InstructionKind::WhenClipboardChanged),
        InstructionDto::OpenApp {
            id,
            command,
            name,
            icon,
            launch,
        } => (
            id,
            InstructionKind::OpenApp {
                command: command.clone(),
                name: name.clone(),
                icon: icon.clone(),
                launch: launch.clone(),
            },
        ),
        InstructionDto::CloseApp {
            id,
            command,
            name,
            icon,
        } => (
            id,
            InstructionKind::CloseApp {
                command: command.clone(),
                name: name.clone(),
                icon: icon.clone(),
            },
        ),
        InstructionDto::SetVariable { id, name, value } => (
            id,
            InstructionKind::SetVariable(name.clone(), value.clone()),
        ),
        InstructionDto::ChangeVariable { id, name, value } => (
            id,
            InstructionKind::ChangeVariable(name.clone(), value.clone()),
        ),
        InstructionDto::SetClipboard { id, value } => (
            id,
            InstructionKind::SetClipboard(value.clone()),
        ),
        InstructionDto::AddToList { id, value, name } => (id, InstructionKind::AddToList { value: value.clone(), name: name.clone() }),
        InstructionDto::DeleteOfList { id, index, name } => (id, InstructionKind::DeleteOfList { index: index.clone(), name: name.clone() }),
        InstructionDto::DeleteAllOfList { id, name } => (id, InstructionKind::DeleteAllOfList { name: name.clone() }),
        InstructionDto::ShiftList { id, name, amount } => (id, InstructionKind::ShiftList { name: name.clone(), amount: amount.clone() }),
        InstructionDto::InsertIntoList { id, value, index, name } => (id, InstructionKind::InsertIntoList { value: value.clone(), index: index.clone(), name: name.clone() }),
        InstructionDto::ReplaceItemOfList { id, index, name, value } => (id, InstructionKind::ReplaceItemOfList { index: index.clone(), name: name.clone(), value: value.clone() }),
        InstructionDto::ReverseList { id, name } => (id, InstructionKind::ReverseList { name: name.clone() }),
        InstructionDto::BlockHeader { id, block_id } => {
            (id, InstructionKind::BlockHeader(block_id.clone()))
        }
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
        InstructionDto::RunBranch { id, name } => (id, InstructionKind::RunBranch(name.clone())),
        InstructionDto::Return { id, value } => (id, InstructionKind::Return(value.clone())),
        InstructionDto::If {
            id,
            condition,
            body,
        } => (
            id,
            InstructionKind::If {
                condition: condition.clone(),
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::IfElse {
            id,
            condition,
            then_body,
            else_body,
        } => (
            id,
            InstructionKind::IfElse {
                condition: condition.clone(),
                then_body: then_body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
                else_body: else_body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::Repeat { id, count, body } => (
            id,
            InstructionKind::Repeat {
                count: count.clone(),
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::Forever { id, body } => (
            id,
            InstructionKind::Forever {
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::While {
            id,
            condition,
            body,
        } => (
            id,
            InstructionKind::While {
                condition: condition.clone(),
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::EscapeLoop { id } => (id, InstructionKind::EscapeLoop),
        InstructionDto::ContinueLoop { id } => (id, InstructionKind::ContinueLoop),
        InstructionDto::WriteFile {
            id,
            path,
            text,
            append,
        } => (
            id,
            InstructionKind::WriteFile {
                path: path.clone(),
                text: text.clone(),
                append: *append,
            },
        ),
        InstructionDto::HttpRequest {
            id,
            method,
            url,
            body,
            headers,
            variable,
        } => (
            id,
            InstructionKind::HttpRequest {
                method: method.clone(),
                url: url.clone(),
                body: body.clone(),
                headers: headers.clone(),
                variable: variable.clone(),
            },
        ),
        InstructionDto::Notify { id, title, message } => (
            id,
            InstructionKind::Notify {
                title: title.clone(),
                message: message.clone(),
            },
        ),
        InstructionDto::Log { id, value } => (id, InstructionKind::Log(value.clone())),
        InstructionDto::TryCatch {
            id,
            body,
            catch_body,
        } => (
            id,
            InstructionKind::TryCatch {
                body: body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
                catch_body: catch_body
                    .iter()
                    .map(dto_to_instruction)
                    .collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::ThrowError { id, message } => {
            (id, InstructionKind::ThrowError(message.clone()))
        }
        InstructionDto::StopMacro { id } => (id, InstructionKind::StopMacro),
        InstructionDto::WhenWindow {
            id,
            pattern,
            focused,
        } => (
            id,
            InstructionKind::WhenWindow {
                pattern: pattern.clone(),
                focused: *focused,
            },
        ),
        InstructionDto::WhenFileChanged { id, path } => {
            (id, InstructionKind::WhenFileChanged(path.clone()))
        }
        InstructionDto::Retry { id, count, delay, body } => (
            id,
            InstructionKind::Retry {
                count: count.clone(),
                delay: delay.clone(),
                body: body.iter().map(dto_to_instruction).collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::WindowAction { id, action, pattern, by } => (
            id,
            InstructionKind::WindowAction { action: action.clone(), pattern: pattern.clone(), by: by.clone() },
        ),
        InstructionDto::MoveWindow { id, pattern, by, resize, x, y } => (
            id,
            InstructionKind::MoveWindow {
                pattern: pattern.clone(),
                by: by.clone(),
                resize: *resize,
                x: x.clone(),
                y: y.clone(),
            },
        ),
        InstructionDto::RunMacro { id, target, wait } => {
            (id, InstructionKind::RunMacro { target: target.clone(), wait: *wait })
        }
        InstructionDto::StopMacroNamed { id, target } => (id, InstructionKind::StopMacroNamed(target.clone())),
        InstructionDto::SetGlobal { id, name, value } => {
            (id, InstructionKind::SetGlobal { name: name.clone(), value: value.clone() })
        }
        InstructionDto::ChangeGlobal { id, name, by } => {
            (id, InstructionKind::ChangeGlobal { name: name.clone(), by: by.clone() })
        }
        InstructionDto::WaitUntil { id, condition, timeout, error_timeout } => {
            (id, InstructionKind::WaitUntil { condition: condition.clone(), timeout: timeout.clone(), error_timeout: error_timeout.clone() })
        }
        InstructionDto::FindImage { id, image, tolerance, x_variable, y_variable, pick, region, count_variable } => (
            id,
            InstructionKind::FindImage {
                image: image.clone(),
                tolerance: tolerance.clone(),
                x_variable: x_variable.clone(),
                y_variable: y_variable.clone(),
                pick: pick.clone(),
                region: region.clone(),
                count_variable: count_variable.clone(),
            },
        ),
        InstructionDto::ClickImage { id, image, tolerance, button, pick, region } => (
            id,
            InstructionKind::ClickImage {
                image: image.clone(),
                tolerance: tolerance.clone(),
                button: button.clone(),
                pick: pick.clone(),
                region: region.clone(),
            },
        ),
        InstructionDto::WaitForImage { id, image, tolerance, appear, timeout, region, error_timeout } => (
            id,
            InstructionKind::WaitForImage {
                image: image.clone(),
                tolerance: tolerance.clone(),
                appear: *appear,
                timeout: timeout.clone(),
                error_timeout: error_timeout.clone(),
                region: region.clone(),
            },
        ),
        InstructionDto::WhenTyped { id, text, erase } => (id, InstructionKind::WhenTyped { text: text.clone(), erase: *erase }),
        InstructionDto::PasteText { id, text } => (id, InstructionKind::PasteText(text.clone())),
        InstructionDto::WhenApp { id, pattern, started } => {
            (id, InstructionKind::WhenApp { pattern: pattern.clone(), started: *started })
        }
        InstructionDto::WhenIdle { id, minutes } => (id, InstructionKind::WhenIdle(minutes.clone())),
        InstructionDto::WhenScreenLock { id, locked } => (id, InstructionKind::WhenScreenLock { locked: *locked }),
        InstructionDto::WithLock { id, name, body } => (
            id,
            InstructionKind::WithLock {
                name: name.clone(),
                body: body.iter().map(dto_to_instruction).collect::<Option<Vec<_>>>()?,
            },
        ),
        InstructionDto::WhenCron { id, expression } => (id, InstructionKind::WhenCron(expression.clone())),
        InstructionDto::SplitIntoList { id, text, separator, name } => (
            id,
            InstructionKind::SplitIntoList { text: text.clone(), separator: separator.clone(), name: name.clone() },
        ),
        InstructionDto::WhenNetwork { id, connected } => (id, InstructionKind::WhenNetwork { connected: *connected }),
        InstructionDto::WhenUsb { id, pattern, connected } => {
            (id, InstructionKind::WhenUsb { pattern: pattern.clone(), connected: *connected })
        }
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
        }
        InstructionDto::AskText { id, prompt, default, name } => {
            (id, InstructionKind::AskText { prompt: prompt.clone(), default: default.clone(), name: name.clone() })
        }
        InstructionDto::AskYesNo { id, prompt, name } => (id, InstructionKind::AskYesNo { prompt: prompt.clone(), name: name.clone() }),
        InstructionDto::AskChoice { id, prompt, list, name } => {
            (id, InstructionKind::AskChoice { prompt: prompt.clone(), list: list.clone(), name: name.clone() })
        }
        InstructionDto::ShowMessage { id, message } => (id, InstructionKind::ShowMessage(message.clone())),
        InstructionDto::WhenAt { id, at } => (id, InstructionKind::WhenAt(at.clone())),
        InstructionDto::WhenSignal { id, name, expression } => (id, InstructionKind::WhenSignal(crate::macros::SignalName::new(name.clone(), *expression))),
        InstructionDto::SendSignal { id, name, expression, wait } => (id, InstructionKind::SendSignal { name: crate::macros::SignalName::new(name.clone(), *expression), wait: *wait }),
        InstructionDto::WhenWebhook { id, path, variable } => {
            (id, InstructionKind::WhenWebhook { path: path.clone(), variable: variable.clone() })
        }
        InstructionDto::DetectObjectsInWindow { id, pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => (
            id,
            InstructionKind::DetectObjectsInWindow {
                region: region.clone(),
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                sort: sort.clone(),
                results_list: results_list.clone(),
                count_variable: count_variable.clone(),
            },
        ),
        InstructionDto::WaitForDetection { id, pattern, by, detector, label, confidence, appear, timeout, region } => (
            id,
            InstructionKind::WaitForDetection {
                region: region.clone(),
                pattern: pattern.clone(),
                by: by.clone(),
                detector: detector.clone(),
                label: label.clone(),
                confidence: confidence.clone(),
                appear: *appear,
                timeout: timeout.clone(),
            },
        ),
        InstructionDto::AimStep { id, pattern, by, target_x, target_y, gain, max_step, deadzone } => (
            id,
            InstructionKind::AimStep {
                pattern: pattern.clone(),
                by: by.clone(),
                target_x: target_x.clone(),
                target_y: target_y.clone(),
                gain: gain.clone(),
                max_step: max_step.clone(),
                deadzone: deadzone.clone(),
            },
        ),
        InstructionDto::AimAtDetection { id, results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => (
            id,
            InstructionKind::AimAtDetection {
                results_list: results_list.clone(),
                index: index.clone(),
                gain: gain.clone(),
                max_step: max_step.clone(),
                deadzone: deadzone.clone(),
                timeout: timeout.clone(),
                settle_ms: settle_ms.clone(),
                status_variable: status_variable.clone(),
            },
        ),
        InstructionDto::BrowserOpen { id, url, new_tab } => (id, InstructionKind::BrowserOpen { url: url.clone(), new_tab: *new_tab }),
        InstructionDto::BrowserClick { id, selector, nth } => {
            (id, InstructionKind::BrowserClick { selector: selector.clone(), nth: nth.clone() })
        }
        InstructionDto::BrowserType { id, text, selector, nth } => {
            (id, InstructionKind::BrowserType { text: text.clone(), selector: selector.clone(), nth: nth.clone() })
        }
        InstructionDto::BrowserWaitFor { id, selector, appear, timeout } => (
            id,
            InstructionKind::BrowserWaitFor { selector: selector.clone(), appear: *appear, timeout: timeout.clone() },
        ),
        InstructionDto::BrowserScript { id, code, variable } => {
            (id, InstructionKind::BrowserScript { code: code.clone(), variable: variable.clone() })
        }
    };
    Some(Instruction {
        id: id.clone(),
        kind,
    })
}

pub fn strand_to_dto(strand: &Strand) -> StrandDto {
    StrandDto {
        id: strand.id.clone(),
        x: strand.x,
        y: strand.y,
        instructions: strand.instructions.iter().map(instruction_to_dto).collect(),
    }
}

pub fn macro_to_dto(mac: &Macro) -> MacroDto {
    let mut hidden_instruction_ids = HashSet::new();
    for strand in mac.strands.iter().filter(|strand| block_packs::is_pack_strand(mac, strand)) {
        for ins in &strand.instructions {
            ins.walk(&mut |ins| { hidden_instruction_ids.insert(ins.id.clone()); });
        }
    }
    MacroDto {
        id: mac.id.clone(),
        name: mac.name.clone(),
        description: mac.description.clone(),
        strands: mac.editor_strands()
            .map(strand_to_dto).collect(),
        recording_target_strand_id: mac.recording_target_id(),
        speed_multiplier: mac.speed_multiplier,
        floating_values: mac.floating_values.clone(),
        comments: mac.comments.iter().filter(|comment| {
            comment.attached_to.as_ref().is_none_or(|id| !hidden_instruction_ids.contains(id))
        }).cloned().collect(),
        variables: mac.variables.iter().map(|v| v.name.clone()).collect(),
        lists: mac.lists.iter().map(list_to_dto).collect(),
        signals: mac.signals.clone(),
        block_defs: mac.block_defs.iter().map(|def| {
            let pack = block_packs::owner(mac, &def.id);
            let mut definition = def.clone();
            if let Some(pack) = pack {
                let name = if definition.identifier.is_empty() {
                    block_packs::block_name(def)
                } else { definition.identifier.clone() };
                definition.identifier = block_packs::details_identifier(&pack.author, &name);
                if definition.description.is_empty() { definition.description = pack.description.clone(); }
            }
            BlockDefinitionDto {
                definition,
                read_only: pack.is_some(),
                pack_name: pack.map(|p| p.name.clone()).unwrap_or_default(),
                packs: mac.settings.block_packs.iter().filter(|pack| pack.blocks.contains(&def.id))
                    .map(|pack| BlockPackMembershipDto { id: pack.id.clone(), name: pack.name.clone() }).collect(),
            }
        }).collect(),
        settings: macro_settings_to_dto(&mac.settings),
    }
}

/// Decodes reporter bodies stored as frontend instructions or older native records.
pub fn reporter_branch_to_instructions(body: &[serde_json::Value]) -> Result<Vec<Instruction>, String> {
    body.iter().map(|value| {
        serde_json::from_value::<InstructionDto>(value.clone()).ok()
            .and_then(|dto| dto_to_instruction(&dto))
            .or_else(|| serde_json::from_value::<Instruction>(value.clone()).ok())
            .ok_or_else(|| "Invalid instruction in a custom reporter branch".to_string())
    }).collect()
}
