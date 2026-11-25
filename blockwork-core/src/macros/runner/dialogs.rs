//! The blocks that show a dialog and wait for an answer.

use super::*;
use crate::macros::TooltipPlace;

/// How long after opening a menu to look for its window to place it.
const MENU_SETTLE: Duration = Duration::from_millis(250);
/// Space between the mouse, or the screen's edge, and a box.
const BOX_MARGIN: f64 = 16.0;

/// Just below and right of the mouse, where a tooltip or menu opens.
fn near_mouse() -> (f64, f64) {
    crate::screen::cursor()
        .map(|(x, y)| (x as f64 + BOX_MARGIN, y as f64 + BOX_MARGIN))
        .unwrap_or((BOX_MARGIN, BOX_MARGIN))
}

/// Moves the box window titled `title` to `place` on the monitor under the
/// mouse. The window places itself where the system lets it (X11, Windows,
/// macOS); Wayland desktops leave that to the compositor, so this asks it.
fn move_box(title: &str, place: TooltipPlace, at: (f64, f64)) {
    use crate::window::MatchBy;
    let Ok(bounds) = crate::window::bounds(title, MatchBy::Exact) else {
        return;
    };
    let monitors = crate::screen::monitors();
    let Some(m) = monitors
        .iter()
        .find(|m| at.0 >= m.x && at.1 >= m.y && at.0 < m.x + m.width && at.1 < m.y + m.height)
        .or(monitors.first())
    else {
        return;
    };
    let (w, h) = (bounds.width, bounds.height);
    let (x, y) = match place {
        TooltipPlace::Mouse => (at.0, at.1),
        TooltipPlace::TopLeft => (m.x + BOX_MARGIN, m.y + BOX_MARGIN),
        TooltipPlace::TopRight => (m.x + m.width - w - BOX_MARGIN, m.y + BOX_MARGIN),
        TooltipPlace::BottomLeft => (m.x + BOX_MARGIN, m.y + m.height - h - BOX_MARGIN),
        TooltipPlace::BottomRight => (
            m.x + m.width - w - BOX_MARGIN,
            m.y + m.height - h - BOX_MARGIN,
        ),
        TooltipPlace::Center => (m.x + (m.width - w) / 2.0, m.y + (m.height - h) / 2.0),
    };
    // Keep it on the monitor.
    let x = x.min(m.x + m.width - w).max(m.x);
    let y = y.min(m.y + m.height - h).max(m.y);
    if (x - bounds.x).abs() > 1.0 || (y - bounds.y).abs() > 1.0 {
        let _ = crate::window::place(
            title,
            MatchBy::Exact,
            false,
            x.round() as i32,
            y.round() as i32,
        );
    }
}

/// Runs one dialog block. `Some(flow)` ends the strand with it; `None` goes on to the next block.
pub(super) fn run_dialog_block(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Option<Flow>, String> {
    match &ins.kind {
        InstructionKind::AskText {
            prompt,
            default,
            name,
        } => {
            let request = (|| {
                let message = ctx.resolve(prompt, depth)?.eval_text()?;
                let mut request = crate::prompt::Request::new(
                    crate::prompt::Kind::Text,
                    &ctx.source.name,
                    message,
                );
                request.default = ctx.resolve(default, depth)?.eval_text()?;
                Ok::<_, String>(request)
            })();
            match request {
                Ok(request) => {
                    if let Some(reply) = ask(ctx, &ins.id, "Ask", request)? {
                        set_variable(ctx, name, Evaluated::Text(reply.text));
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Ask", e)?,
            }
        }
        InstructionKind::AskYesNo { prompt, name } => {
            match ctx.resolve(prompt, depth).and_then(|v| v.eval_text()) {
                Ok(message) => {
                    let request = crate::prompt::Request::new(
                        crate::prompt::Kind::Confirm,
                        &ctx.source.name,
                        message,
                    );
                    if let Some(reply) = ask(ctx, &ins.id, "Ask yes or no", request)? {
                        set_variable(ctx, name, Evaluated::Bool(reply.answer == "yes"));
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Ask yes or no", e)?,
            }
        }
        InstructionKind::AskChoice { prompt, list, name } => {
            let options: Vec<String> = ctx
                .lists
                .lock()
                .ok()
                .and_then(|lists| {
                    lists
                        .get(list)
                        .map(|items| items.iter().map(|i| i.evaluated().as_text()).collect())
                })
                .unwrap_or_default();
            let message = ctx.resolve(prompt, depth).and_then(|v| v.eval_text());
            match message {
                Ok(_) if options.is_empty() => soft_fail(
                    ctx,
                    &ins.id,
                    "Choose",
                    format!("the list \"{list}\" is empty"),
                )?,
                Ok(message) => {
                    let mut request = crate::prompt::Request::new(
                        crate::prompt::Kind::Choice,
                        &ctx.source.name,
                        message,
                    );
                    request.options = options;
                    if let Some(reply) = ask(ctx, &ins.id, "Choose", request)? {
                        set_variable(ctx, name, Evaluated::Text(reply.text));
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Choose", e)?,
            }
        }
        InstructionKind::AskFile {
            prompt,
            folder,
            name,
        } => match ctx.resolve(prompt, depth).and_then(|v| v.eval_text()) {
            Ok(message) => {
                let kind = if *folder {
                    crate::prompt::Kind::Folder
                } else {
                    crate::prompt::Kind::File
                };
                let request = crate::prompt::Request::new(kind, &ctx.source.name, message);
                if let Some(reply) = ask(ctx, &ins.id, "Ask for file", request)? {
                    set_variable(ctx, name, Evaluated::Text(reply.text));
                }
            }
            Err(e) => soft_fail(ctx, &ins.id, "Ask for file", e)?,
        },
        InstructionKind::ShowTooltip {
            text,
            place,
            duration,
        } => {
            let shown = (|| {
                let message = ctx.resolve(text, depth)?.eval_text()?;
                let duration = ctx.resolve(duration, depth)?.eval_number()?;
                let mut request =
                    crate::prompt::Request::new(crate::prompt::Kind::Tooltip, "", message);
                let at = near_mouse();
                (request.x, request.y) = (Some(at.0), Some(at.1));
                request.duration_ms = Some(duration.max(0.0));
                request.default = format!("{place:?}");
                let place = *place;
                crate::prompt::show_tooltip(request, move |title| move_box(title, place, at))
            })();
            if let Err(e) = shown {
                soft_fail(ctx, &ins.id, "Show tooltip", e)?;
            }
        }
        InstructionKind::ShowMenu { list, name } => {
            let options: Vec<String> = ctx
                .lists
                .lock()
                .ok()
                .and_then(|lists| {
                    lists
                        .get(list)
                        .map(|items| items.iter().map(|i| i.evaluated().as_text()).collect())
                })
                .unwrap_or_default();
            if options.is_empty() {
                soft_fail(
                    ctx,
                    &ins.id,
                    "Menu",
                    format!("the list \"{list}\" is empty"),
                )?;
                return Ok(None);
            }
            let mut request = crate::prompt::Request::new(
                crate::prompt::Kind::Menu,
                &format!("Blockwork menu {}", std::process::id()),
                "",
            );
            request.options = options;
            let at = near_mouse();
            (request.x, request.y) = (Some(at.0), Some(at.1));
            let title = request.title.clone();
            std::thread::spawn(move || {
                std::thread::sleep(MENU_SETTLE);
                move_box(&title, TooltipPlace::Mouse, at);
            });
            if let Some(reply) = ask(ctx, &ins.id, "Menu", request)? {
                set_variable(ctx, name, Evaluated::Text(reply.text));
            }
        }
        InstructionKind::ShowMessage(message) => {
            match ctx.resolve(message, depth).and_then(|v| v.eval_text()) {
                Ok(message) => {
                    let request = crate::prompt::Request::new(
                        crate::prompt::Kind::Message,
                        &ctx.source.name,
                        message,
                    );
                    // Closing a message is the same as OK.
                    let flag = ctx.stop_flag.clone();
                    let stopped = || flag.as_ref().is_some_and(stop_requested);
                    if let Err(e) = crate::prompt::ask(&request, &stopped)
                        && !stopped()
                    {
                        soft_fail(ctx, &ins.id, "Show message", e)?;
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Show message", e)?,
            }
        }
        InstructionKind::ShowWindow { .. }
        | InstructionKind::OpenFormWindow { .. }
        | InstructionKind::CloseFormWindow(_)
        | InstructionKind::WaitForFormWindow(_)
        | InstructionKind::SetFormWindowTitle { .. }
        | InstructionKind::ResizeFormWindow { .. }
        | InstructionKind::SetFormComponent { .. } => {
            return super::forms::run_form_block(ins, ctx, depth);
        }
        _ => unreachable!("run_dialog_block got another block"),
    }
    Ok(None)
}
