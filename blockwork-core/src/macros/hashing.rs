//! Hashing an instruction by its saved content.

use super::*;

impl std::hash::Hash for InstructionKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Token(t) => {
                0u8.hash(state);
                t.hash(state);
            }
            Self::Wait(d) => {
                1u8.hash(state);
                d.hash(state);
            }
            Self::Command(s) => {
                2u8.hash(state);
                s.hash(state);
            }
            Self::CommandWait(s) => {
                77u8.hash(state);
                s.hash(state);
            }
            Self::Comment(s) => {
                3u8.hash(state);
                s.hash(state);
            }
            Self::WhenRan => {
                4u8.hash(state);
            }
            Self::SetVariable(n, v) => {
                5u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Self::ChangeVariable(n, v) => {
                6u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Self::BlockHeader(id) => {
                7u8.hash(state);
                id.hash(state);
            }
            Self::CallBlock {
                block_id,
                args,
                branches,
            } => {
                8u8.hash(state);
                block_id.hash(state);
                args.hash(state);
                branches.hash(state);
            }
            Self::RunBranch(name) => {
                26u8.hash(state);
                name.hash(state);
            }
            Self::Return(v) => {
                9u8.hash(state);
                v.hash(state);
            }
            Self::If { condition, body } => {
                10u8.hash(state);
                condition.hash(state);
                body.hash(state);
            }
            Self::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                11u8.hash(state);
                condition.hash(state);
                then_body.hash(state);
                else_body.hash(state);
            }
            Self::Repeat { count, body } => {
                12u8.hash(state);
                count.hash(state);
                body.hash(state);
            }
            Self::Forever { body } => {
                13u8.hash(state);
                body.hash(state);
            }
            Self::While { condition, body } => {
                14u8.hash(state);
                condition.hash(state);
                body.hash(state);
            }
            Self::EscapeLoop => {
                15u8.hash(state);
            }
            Self::ContinueLoop => {
                16u8.hash(state);
            }
            Self::WhenBatteryDischargedTo(v) => {
                17u8.hash(state);
                v.hash(state);
            }
            Self::WhenBatteryChargedTo(v) => {
                18u8.hash(state);
                v.hash(state);
            }
            Self::WhenTime(s, t) => {
                19u8.hash(state);
                s.hash(state);
                t.hash(state);
            }
            Self::WhenPowerPluggedIn => {
                20u8.hash(state);
            }
            Self::WhenPowerUnplugged => {
                21u8.hash(state);
            }
            Self::OpenApp {
                command,
                name,
                icon,
                launch,
            } => {
                22u8.hash(state);
                command.hash(state);
                name.hash(state);
                icon.hash(state);
                launch.hash(state);
            }
            Self::CloseApp {
                command,
                name,
                icon,
            } => {
                23u8.hash(state);
                command.hash(state);
                name.hash(state);
                icon.hash(state);
            }
            Self::AddToList { value, name } => {
                24u8.hash(state);
                value.hash(state);
                name.hash(state);
            }
            Self::DeleteOfList { index, name } => {
                25u8.hash(state);
                index.hash(state);
                name.hash(state);
            }
            Self::DeleteAllOfList { name } => {
                26u8.hash(state);
                name.hash(state);
            }
            Self::ShiftList { name, amount } => {
                27u8.hash(state);
                name.hash(state);
                amount.hash(state);
            }
            Self::InsertIntoList { value, index, name } => {
                28u8.hash(state);
                value.hash(state);
                index.hash(state);
                name.hash(state);
            }
            Self::ReplaceItemOfList { index, name, value } => {
                29u8.hash(state);
                index.hash(state);
                name.hash(state);
                value.hash(state);
            }
            Self::ReverseList { name } => {
                30u8.hash(state);
                name.hash(state);
            }
            Self::SetClipboard(v) => {
                31u8.hash(state);
                v.hash(state);
            }
            Self::WhenClipboardChanged => {
                32u8.hash(state);
            }
            Self::WhenKey(key, direction) => {
                33u8.hash(state);
                key.hash(state);
                direction.hash(state);
            }
            Self::WriteFile { path, text, append } => {
                34u8.hash(state);
                path.hash(state);
                text.hash(state);
                append.hash(state);
            }
            Self::HttpRequest {
                method,
                url,
                body,
                headers,
                variable,
            } => {
                35u8.hash(state);
                method.hash(state);
                url.hash(state);
                body.hash(state);
                headers.hash(state);
                variable.hash(state);
            }
            Self::Notify { title, message } => {
                36u8.hash(state);
                title.hash(state);
                message.hash(state);
            }
            Self::Log(v) => {
                37u8.hash(state);
                v.hash(state);
            }
            Self::TryCatch { body, catch_body } => {
                38u8.hash(state);
                body.hash(state);
                catch_body.hash(state);
            }
            Self::ThrowError(v) => {
                39u8.hash(state);
                v.hash(state);
            }
            Self::StopMacro => {
                40u8.hash(state);
            }
            Self::WhenWindow { pattern, focused } => {
                41u8.hash(state);
                pattern.hash(state);
                focused.hash(state);
            }
            Self::WhenFileChanged(path) => {
                42u8.hash(state);
                path.hash(state);
            }
            Self::Retry { count, delay, body } => {
                43u8.hash(state);
                count.hash(state);
                delay.hash(state);
                body.hash(state);
            }
            Self::WindowAction { action, pattern, by } => {
                44u8.hash(state);
                action.hash(state);
                pattern.hash(state);
                by.hash(state);
            }
            Self::MoveWindow { pattern, by, resize, x, y } => {
                45u8.hash(state);
                pattern.hash(state);
                by.hash(state);
                resize.hash(state);
                x.hash(state);
                y.hash(state);
            }
            Self::RunMacro { target, wait } => {
                46u8.hash(state);
                target.hash(state);
                wait.hash(state);
            }
            Self::StopMacroNamed(target) => {
                47u8.hash(state);
                target.hash(state);
            }
            Self::SetGlobal { name, value } => {
                48u8.hash(state);
                name.hash(state);
                value.hash(state);
            }
            Self::ChangeGlobal { name, by } => {
                49u8.hash(state);
                name.hash(state);
                by.hash(state);
            }
            Self::WaitUntil { condition, timeout, error_timeout } => {
                50u8.hash(state);
                condition.hash(state);
                timeout.hash(state);
                error_timeout.hash(state);
            }
            Self::FindImage { image, tolerance, x_variable, y_variable, pick, region, count_variable } => {
                51u8.hash(state);
                image.hash(state);
                tolerance.hash(state);
                x_variable.hash(state);
                y_variable.hash(state);
                (pick, region, count_variable).hash(state);
            }
            Self::ClickImage { image, tolerance, button, pick, region } => {
                52u8.hash(state);
                image.hash(state);
                tolerance.hash(state);
                button.hash(state);
                (pick, region).hash(state);
            }
            Self::WaitForImage { image, tolerance, appear, timeout, region, error_timeout } => {
                53u8.hash(state);
                image.hash(state);
                tolerance.hash(state);
                appear.hash(state);
                timeout.hash(state);
                error_timeout.hash(state);
                region.hash(state);
            }
            Self::WhenTyped { text, erase } => {
                54u8.hash(state);
                text.hash(state);
                erase.hash(state);
            }
            Self::PasteText(v) => {
                55u8.hash(state);
                v.hash(state);
            }
            Self::WhenApp { pattern, started } => {
                56u8.hash(state);
                pattern.hash(state);
                started.hash(state);
            }
            Self::WhenIdle(v) => {
                57u8.hash(state);
                v.hash(state);
            }
            Self::WhenScreenLock { locked } => {
                58u8.hash(state);
                locked.hash(state);
            }
            Self::WithLock { name, body } => {
                59u8.hash(state);
                name.hash(state);
                body.hash(state);
            }
            Self::WhenCron(v) => {
                60u8.hash(state);
                v.hash(state);
            }
            Self::SplitIntoList { text, separator, name } => {
                61u8.hash(state);
                text.hash(state);
                separator.hash(state);
                name.hash(state);
            }
            Self::WhenNetwork { connected } => {
                62u8.hash(state);
                connected.hash(state);
            }
            Self::WhenUsb { pattern, connected } => {
                63u8.hash(state);
                pattern.hash(state);
                connected.hash(state);
            }
            Self::WhenMouseButton { button, pressed, modifiers } => {
                64u8.hash(state);
                button.hash(state);
                pressed.hash(state);
                modifiers.hash(state);
            }
            Self::AskText { prompt, default, name } => {
                65u8.hash(state);
                prompt.hash(state);
                default.hash(state);
                name.hash(state);
            }
            Self::AskYesNo { prompt, name } => {
                66u8.hash(state);
                prompt.hash(state);
                name.hash(state);
            }
            Self::AskChoice { prompt, list, name } => {
                67u8.hash(state);
                prompt.hash(state);
                list.hash(state);
                name.hash(state);
            }
            Self::ShowMessage(v) => {
                68u8.hash(state);
                v.hash(state);
            }
            Self::WhenAt(v) => {
                69u8.hash(state);
                v.hash(state);
            }
            Self::WhenSignal(v) => {
                70u8.hash(state);
                v.hash(state);
            }
            Self::SendSignal { name, wait } => {
                71u8.hash(state);
                name.hash(state);
                wait.hash(state);
            }
            Self::WhenWebhook { path, variable } => {
                72u8.hash(state);
                path.hash(state);
                variable.hash(state);
            }
            Self::DetectObjectsInWindow { pattern, by, detector, label, confidence, sort, results_list, count_variable, region } => {
                73u8.hash(state);
                (pattern, by, detector, label, confidence).hash(state);
                (sort, results_list, count_variable, region).hash(state);
            }
            Self::WaitForDetection { pattern, by, detector, label, confidence, appear, timeout, region } => {
                74u8.hash(state);
                (pattern, by, detector, label, confidence, appear, timeout, region).hash(state);
            }
            Self::AimStep { pattern, by, target_x, target_y, gain, max_step, deadzone } => {
                75u8.hash(state);
                (pattern, by, target_x, target_y, gain, max_step, deadzone).hash(state);
            }
            Self::AimAtDetection { results_list, index, gain, max_step, deadzone, timeout, settle_ms, status_variable } => {
                76u8.hash(state);
                (results_list, index, gain, max_step, deadzone).hash(state);
                (timeout, settle_ms, status_variable).hash(state);
            }
            Self::BrowserOpen { url, new_tab } => {
                77u8.hash(state);
                (url, new_tab).hash(state);
            }
            Self::BrowserClick { selector, nth } => {
                78u8.hash(state);
                (selector, nth).hash(state);
            }
            Self::BrowserType { text, selector, nth } => {
                79u8.hash(state);
                (text, selector, nth).hash(state);
            }
            Self::BrowserWaitFor { selector, appear, timeout } => {
                80u8.hash(state);
                (selector, appear, timeout).hash(state);
            }
            Self::BrowserScript { code, variable } => {
                81u8.hash(state);
                (code, variable).hash(state);
            }
        }
    }
}
