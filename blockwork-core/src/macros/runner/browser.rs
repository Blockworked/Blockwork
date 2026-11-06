//! The browser blocks, which drive a page through the Blockwork extension.

use super::*;

/// Runs one browser block. `Some(flow)` ends the strand with it; `None` goes on to the next block.
pub(super) fn run_browser_block(ins: &Instruction, ctx: &mut ExecCtx, depth: u32) -> Result<Option<Flow>, String> {
    match &ins.kind {
        InstructionKind::BrowserOpen { url, new_tab } => {
            let opened = ctx
                .resolve(url, depth)
                .and_then(|u| u.eval_text())
                .and_then(|url| crate::browser::request("open", serde_json::json!({ "url": url, "newTab": new_tab }), BROWSER_LOAD_LIMIT));
            if let Err(e) = opened {
                soft_fail(ctx, &ins.id, "Open in browser", e)?;
            }
        }
        InstructionKind::BrowserClick { selector, nth } => {
            let clicked = (|| {
                let selector = ctx.resolve(selector, depth)?.eval_text()?;
                let nth = match nth {
                    Some(nth) => ctx.resolve(nth, depth)?.eval_number()?,
                    None => 1.0,
                };
                crate::browser::request("click", serde_json::json!({ "selector": selector, "nth": nth }), BROWSER_LIMIT)
            })();
            if let Err(e) = clicked {
                soft_fail(ctx, &ins.id, "Click in browser", e)?;
            }
        }
        InstructionKind::BrowserType { text, selector, nth } => {
            let typed = (|| {
                let text = ctx.resolve(text, depth)?.eval_text()?;
                let selector = ctx.resolve(selector, depth)?.eval_text()?;
                let nth = match nth {
                    Some(nth) => ctx.resolve(nth, depth)?.eval_number()?,
                    None => 1.0,
                };
                crate::browser::request("type", serde_json::json!({ "selector": selector, "nth": nth, "text": text }), BROWSER_LIMIT)
            })();
            if let Err(e) = typed {
                soft_fail(ctx, &ins.id, "Type in browser", e)?;
            }
        }
        InstructionKind::BrowserWaitFor { selector, appear, timeout } => {
            let setup = (|| {
                let selector = ctx.resolve(selector, depth)?.eval_text()?;
                let timeout = ctx.resolve(timeout, depth)?.eval_number()?.max(0.0);
                Ok::<_, String>((selector, timeout))
            })();
            let (selector, timeout) = match setup {
                Ok(setup) => setup,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Wait for element", e)?;
                    return Ok(Some(Flow::Normal));
                }
            };
            let started = Instant::now();
            loop {
                match crate::browser::request("count", serde_json::json!({ "selector": selector }), BROWSER_LIMIT) {
                    Ok(found) if (found["count"].as_u64().unwrap_or(0) > 0) == *appear => break,
                    Ok(_) => {}
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait for element", e)?;
                        break;
                    }
                }
                if timeout > 0.0 && started.elapsed().as_secs_f64() * 1000.0 >= timeout {
                    let what = if *appear { "didn't appear" } else { "didn't go away" };
                    soft_fail(ctx, &ins.id, "Wait for element", format!("{selector} {what} within {timeout} ms"))?;
                    break;
                }
                if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                    break;
                }
            }
        }
        InstructionKind::BrowserScript { code, variable } => {
            let ran = ctx
                .resolve(code, depth)
                .and_then(|c| c.eval_text())
                .and_then(|code| {
                    let stopped = || ctx.stop_flag.as_ref().is_some_and(stop_requested);
                    crate::browser::request_until("script", serde_json::json!({ "code": code }), BROWSER_SCRIPT_LIMIT, stopped)
                });
            match ran {
                Ok(result) => set_variable(ctx, variable, Evaluated::Text(crate::browser::text_field(&result, "value"))),
                Err(e) => soft_fail(ctx, &ins.id, "Run JavaScript", e)?,
            }
        }
        _ => unreachable!("run_browser_block got another block"),
    }
    Ok(None)
}
