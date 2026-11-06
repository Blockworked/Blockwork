//! The blocks that show a dialog and wait for an answer.

use super::*;

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
        _ => unreachable!("run_dialog_block got another block"),
    }
    Ok(None)
}
