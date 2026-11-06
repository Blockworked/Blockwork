//! Blockwork's half of the block-editor contract: where instructions keep values and bodies, which are headers, and how field ids map onto their slots.

use super::*;

/// Every variable and list name a kind holds as text, for
/// [`BlockKind::visit_refs_mut`] and [`BlockKind::visit_refs`] alike.
macro_rules! for_each_ref {
    ($kind:expr, $f:expr) => {
        match $kind {
            InstructionKind::SetVariable(name, _)
            | InstructionKind::ChangeVariable(name, _)
            | InstructionKind::HttpRequest { variable: name, .. }
            | InstructionKind::BrowserScript { variable: name, .. }
            | InstructionKind::AskText { name, .. }
            | InstructionKind::AskYesNo { name, .. }
            | InstructionKind::WhenWebhook { variable: name, .. } => $f(RefKind::Variable, name),
            InstructionKind::FindImage {
                x_variable,
                y_variable,
                count_variable,
                ..
            } => {
                $f(RefKind::Variable, x_variable);
                $f(RefKind::Variable, y_variable);
                if let Some(count_variable) = count_variable {
                    $f(RefKind::Variable, count_variable);
                }
            }
            InstructionKind::AskChoice { list, name, .. } => {
                $f(RefKind::List, list);
                $f(RefKind::Variable, name);
            }
            InstructionKind::DetectObjectsInWindow {
                results_list,
                count_variable,
                ..
            } => {
                $f(RefKind::List, results_list);
                $f(RefKind::Variable, count_variable);
            }
            InstructionKind::AimAtDetection {
                results_list,
                status_variable,
                ..
            } => {
                $f(RefKind::List, results_list);
                $f(RefKind::Variable, status_variable);
            }
            InstructionKind::AddToList { name, .. }
            | InstructionKind::DeleteOfList { name, .. }
            | InstructionKind::DeleteAllOfList { name }
            | InstructionKind::ShiftList { name, .. }
            | InstructionKind::InsertIntoList { name, .. }
            | InstructionKind::ReplaceItemOfList { name, .. }
            | InstructionKind::ReverseList { name }
            | InstructionKind::SplitIntoList { name, .. } => $f(RefKind::List, name),
            _ => {}
        }
    };
}

impl BlockKind for InstructionKind {
    const HEADER_LABEL: &'static str = "When Ran/Block Definition";
    // CallBlock branch callbacks need one body slot per branch (up to 255,
    // see validate_block_pieces), so walks cover every slot and `body_mut`
    // decides per instruction. Other instructions answer `None` past their
    // own slots, so the wider loop costs them nothing.
    const BODY_SLOTS: u8 = u8::MAX;

    fn visit_values_mut(&mut self, f: &mut dyn FnMut(&mut Value, InputValueType)) {
        match self {
            InstructionKind::Token(token) => token.visit_values_mut(f),
            InstructionKind::Wait(value)
            | InstructionKind::Return(value)
            | InstructionKind::WhenBatteryDischargedTo(value)
            | InstructionKind::WhenBatteryChargedTo(value)
            | InstructionKind::WhenKey(value, _)
            | InstructionKind::SetClipboard(value)
            | InstructionKind::SetVariable(_, value)
            | InstructionKind::ChangeVariable(_, value)
            | InstructionKind::Repeat { count: value, .. }
            | InstructionKind::Log(value)
            | InstructionKind::ThrowError(value)
            | InstructionKind::WhenFileChanged(value)
            | InstructionKind::WhenWindow { pattern: value, .. }
            | InstructionKind::WhenTyped { text: value, .. }
            | InstructionKind::PasteText(value)
            | InstructionKind::WhenApp { pattern: value, .. }
            | InstructionKind::WhenIdle(value)
            | InstructionKind::WhenCron(value)
            | InstructionKind::WithLock { name: value, .. }
            | InstructionKind::WhenUsb { pattern: value, .. }
            | InstructionKind::AskYesNo { prompt: value, .. }
            | InstructionKind::AskChoice { prompt: value, .. }
            | InstructionKind::ShowMessage(value)
            | InstructionKind::WhenAt(value)
            | InstructionKind::WhenWebhook { path: value, .. }
            | InstructionKind::BrowserOpen { url: value, .. }
            | InstructionKind::BrowserScript { code: value, .. } => f(value, InputValueType::Any),
            InstructionKind::BrowserClick { selector, nth } => {
                f(selector, InputValueType::Any);
                if let Some(nth) = nth {
                    f(nth, InputValueType::Any);
                }
            }
            InstructionKind::BrowserType {
                text,
                selector,
                nth,
            } => {
                f(text, InputValueType::Any);
                f(selector, InputValueType::Any);
                if let Some(nth) = nth {
                    f(nth, InputValueType::Any);
                }
            }
            InstructionKind::BrowserWaitFor {
                selector, timeout, ..
            } => {
                f(selector, InputValueType::Any);
                f(timeout, InputValueType::Any);
            }
            InstructionKind::WhenSignal(name) | InstructionKind::SendSignal { name, .. } => {
                f(&mut name.value, InputValueType::Any)
            }
            InstructionKind::AskText {
                prompt, default, ..
            } => {
                f(prompt, InputValueType::Any);
                f(default, InputValueType::Any);
            }
            InstructionKind::SplitIntoList {
                text, separator, ..
            } => {
                f(text, InputValueType::Any);
                f(separator, InputValueType::Any);
            }
            InstructionKind::WriteFile { path, text, .. } => {
                f(path, InputValueType::Any);
                f(text, InputValueType::Any);
            }
            InstructionKind::HttpRequest {
                url, body, headers, ..
            } => {
                f(url, InputValueType::Any);
                f(body, InputValueType::Any);
                f(headers, InputValueType::Any);
            }
            InstructionKind::Notify { title, message } => {
                f(title, InputValueType::Any);
                f(message, InputValueType::Any);
            }
            InstructionKind::Retry { count, delay, .. } => {
                f(count, InputValueType::Any);
                f(delay, InputValueType::Any);
            }
            InstructionKind::WindowAction { pattern, .. } => f(pattern, InputValueType::Any),
            InstructionKind::MoveWindow { pattern, x, y, .. } => {
                f(pattern, InputValueType::Any);
                f(x, InputValueType::Any);
                f(y, InputValueType::Any);
            }
            InstructionKind::RunMacro { target, .. } | InstructionKind::StopMacroNamed(target) => {
                f(target, InputValueType::Any)
            }
            InstructionKind::SetGlobal { name, value: other }
            | InstructionKind::ChangeGlobal { name, by: other } => {
                f(name, InputValueType::Any);
                f(other, InputValueType::Any);
            }
            InstructionKind::WhenMouseButton { modifiers, .. } => {
                if let Some(value) = modifiers {
                    f(value, InputValueType::Any);
                }
            }
            InstructionKind::WaitUntil {
                condition,
                timeout,
                error_timeout,
            } => {
                f(condition, InputValueType::Bool);
                for v in timeout.iter_mut().chain(error_timeout.iter_mut()) {
                    f(v, InputValueType::Any);
                }
            }
            InstructionKind::FindImage {
                tolerance, pick, ..
            }
            | InstructionKind::ClickImage {
                tolerance, pick, ..
            } => {
                if let Some(v) = tolerance {
                    f(v, InputValueType::Any);
                }
                if let Some(ImageMatch {
                    index,
                    near_x,
                    near_y,
                    ..
                }) = pick
                {
                    for v in [index, near_x, near_y] {
                        f(v, InputValueType::Any);
                    }
                }
            }
            InstructionKind::WaitForImage {
                tolerance,
                timeout,
                error_timeout,
                ..
            } => {
                if let Some(v) = tolerance {
                    f(v, InputValueType::Any);
                }
                for v in timeout.iter_mut().chain(error_timeout.iter_mut()) {
                    f(v, InputValueType::Any);
                }
            }
            InstructionKind::DetectObjectsInWindow {
                pattern,
                detector,
                label,
                confidence,
                ..
            } => {
                for v in [pattern, detector, label, confidence] {
                    f(v, InputValueType::Any);
                }
            }
            InstructionKind::WaitForDetection {
                pattern,
                detector,
                label,
                confidence,
                timeout,
                ..
            } => {
                for v in [pattern, detector, label, confidence, timeout] {
                    f(v, InputValueType::Any);
                }
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
                for v in [pattern, target_x, target_y, gain, max_step, deadzone] {
                    f(v, InputValueType::Any);
                }
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
                for v in [index, gain, max_step, deadzone, timeout, settle_ms] {
                    f(v, InputValueType::Any);
                }
            }
            // A call site's declared Boolean inputs live on the `BlockDef`,
            // not here - `BlockGraph::migrate_bool_slots` handles those.
            InstructionKind::CallBlock { args, .. } => {
                for arg in args {
                    f(arg, InputValueType::Any);
                }
            }
            InstructionKind::If { condition, .. }
            | InstructionKind::IfElse { condition, .. }
            | InstructionKind::While { condition, .. } => f(condition, InputValueType::Bool),
            InstructionKind::AddToList { value, .. } => f(value, InputValueType::Any),
            InstructionKind::InsertIntoList { value, index, .. } => {
                f(value, InputValueType::Any);
                f(index, InputValueType::Any);
            }
            InstructionKind::WhenTime(_, time) => f(time, InputValueType::Any),
            InstructionKind::DeleteOfList { index, .. } => f(index, InputValueType::Any),
            InstructionKind::ShiftList { amount, .. } => f(amount, InputValueType::Any),
            InstructionKind::ReplaceItemOfList { index, value, .. } => {
                f(index, InputValueType::Any);
                f(value, InputValueType::Any);
            }
            InstructionKind::Command(_)
            | InstructionKind::CommandWait(_)
            | InstructionKind::Comment(_)
            | InstructionKind::WhenRan
            | InstructionKind::BlockHeader(_)
            | InstructionKind::EscapeLoop
            | InstructionKind::ContinueLoop
            | InstructionKind::StopMacro
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
            | InstructionKind::RunBranch(_) => {}
        }
    }

    /// True for "header" blocks (`WhenRan`, `BlockHeader`, and the event
    /// entry points) - must be first in their strand, nothing may stack
    /// above them, and they render with a flat top edge.
    fn is_header(&self) -> bool {
        matches!(
            self,
            InstructionKind::WhenRan
                | InstructionKind::BlockHeader(_)
                | InstructionKind::WhenBatteryDischargedTo(_)
                | InstructionKind::WhenBatteryChargedTo(_)
                | InstructionKind::WhenTime(..)
                | InstructionKind::WhenPowerPluggedIn
                | InstructionKind::WhenPowerUnplugged
                | InstructionKind::WhenClipboardChanged
                | InstructionKind::WhenKey(..)
                | InstructionKind::WhenWindow { .. }
                | InstructionKind::WhenFileChanged(_)
                | InstructionKind::WhenTyped { .. }
                | InstructionKind::WhenApp { .. }
                | InstructionKind::WhenIdle(_)
                | InstructionKind::WhenScreenLock { .. }
                | InstructionKind::WhenCron(_)
                | InstructionKind::WhenNetwork { .. }
                | InstructionKind::WhenUsb { .. }
                | InstructionKind::WhenMouseButton { .. }
                | InstructionKind::WhenAt(_)
                | InstructionKind::WhenSignal(_)
                | InstructionKind::WhenWebhook { .. }
        )
    }

    fn body(&self, slot: u8) -> Option<&Vec<Instruction>> {
        match (self, slot) {
            (InstructionKind::If { body, .. }, 0) => Some(body),
            (InstructionKind::IfElse { then_body, .. }, 0) => Some(then_body),
            (InstructionKind::IfElse { else_body, .. }, 1) => Some(else_body),
            (InstructionKind::Repeat { body, .. }, 0) => Some(body),
            (InstructionKind::Forever { body }, 0) => Some(body),
            (InstructionKind::While { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { catch_body, .. }, 1) => Some(catch_body),
            (InstructionKind::Retry { body, .. }, 0) => Some(body),
            (InstructionKind::WithLock { body, .. }, 0) => Some(body),
            (InstructionKind::CallBlock { branches, .. }, slot) => branches.get(slot as usize),
            _ => None,
        }
    }

    fn body_mut(&mut self, slot: u8) -> Option<&mut Vec<Instruction>> {
        match (self, slot) {
            (InstructionKind::If { body, .. }, 0) => Some(body),
            (InstructionKind::IfElse { then_body, .. }, 0) => Some(then_body),
            (InstructionKind::IfElse { else_body, .. }, 1) => Some(else_body),
            (InstructionKind::Repeat { body, .. }, 0) => Some(body),
            (InstructionKind::Forever { body }, 0) => Some(body),
            (InstructionKind::While { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { body, .. }, 0) => Some(body),
            (InstructionKind::TryCatch { catch_body, .. }, 1) => Some(catch_body),
            (InstructionKind::Retry { body, .. }, 0) => Some(body),
            (InstructionKind::WithLock { body, .. }, 0) => Some(body),
            (InstructionKind::CallBlock { branches, .. }, slot) => branches.get_mut(slot as usize),
            _ => None,
        }
    }

    fn visit_refs_mut(&mut self, f: &mut dyn FnMut(RefKind, &mut String)) {
        for_each_ref!(self, f);
    }

    fn visit_refs(&self, f: &mut dyn FnMut(RefKind, &str)) {
        for_each_ref!(self, f);
    }

    fn calls_block(&self, block_id: &str) -> bool {
        matches!(self, InstructionKind::CallBlock { block_id: id, .. } if id == block_id)
    }

    fn called_block(&self) -> Option<&str> {
        match self {
            InstructionKind::CallBlock { block_id, .. } => Some(block_id),
            _ => None,
        }
    }

    fn call_args_mut(&mut self, block_id: &str) -> Option<&mut Vec<Value>> {
        match self {
            InstructionKind::CallBlock {
                block_id: id, args, ..
            } if id == block_id => Some(args),
            _ => None,
        }
    }

    fn call_branches_mut(&mut self, block_id: &str) -> Option<&mut Vec<Vec<Instruction>>> {
        match self {
            InstructionKind::CallBlock {
                block_id: id,
                branches,
                ..
            } if id == block_id => Some(branches),
            _ => None,
        }
    }

    fn block_header_id(&self) -> Option<&str> {
        match self {
            InstructionKind::BlockHeader(id) => Some(id),
            _ => None,
        }
    }

    fn branch_ref_mut(&mut self) -> Option<&mut String> {
        match self {
            InstructionKind::RunBranch(name) => Some(name),
            _ => None,
        }
    }

    fn value_slot_mut(&mut self, field: &str) -> Option<&mut Value> {
        fields::value_slot_mut(self, field.parse().ok()?)
    }

    fn blank_field_value(&self, field: &str, blocks: &[BlockDef]) -> Option<Value> {
        fields::blank_field_value(self, field.parse().ok()?, blocks)
    }

    fn field_requires_integer(&self, field: &str) -> bool {
        field
            .parse()
            .is_ok_and(|field: FieldId| field.requires_integer())
    }
}
