//! Flow blocks: conditions, loops, error handling, locks and custom block
//! calls.

use super::*;

/// What `run_block` does after a flow block.
pub(super) enum Next {
    /// Goes on to the next block.
    Go,
    /// Ends this strand (a stop block, or a stop while waiting).
    Stop,
    /// Unwinds to the caller with this flow.
    Exit(Flow),
}

/// Runs one flow block, recursing into `run_block` for its bodies.
pub(super) fn run_control_block(
    ins: &Instruction,
    ctx: &mut ExecCtx,
    depth: u32,
) -> Result<Next, String> {
    match &ins.kind {
        InstructionKind::WithLock { name, body } => {
            let name = match ctx.resolve(name, depth).and_then(|v| v.eval_text()) {
                Ok(name) => name,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Lock", e)?;
                    return Ok(Next::Go);
                }
            };
            let stop = ctx.stop_flag.clone();
            let Some(_guard) =
                crate::macros::locks::acquire(&name, Duration::from_millis(50), || {
                    stop.as_ref().is_some_and(stop_requested)
                })
            else {
                return Ok(Next::Stop);
            };
            let flow = run_block(body, ctx, depth, Instant::now())?;
            if flow != Flow::Normal {
                return Ok(Next::Exit(flow));
            }
        }
        InstructionKind::Return(value) => {
            let resolved = ctx.resolve(value, depth)?;
            return Ok(Next::Exit(Flow::Return(resolved.eval()?)));
        }
        InstructionKind::EscapeLoop => return Ok(Next::Exit(Flow::Break)),
        InstructionKind::ContinueLoop => return Ok(Next::Exit(Flow::Continue)),
        InstructionKind::StopMacro => {
            crate::run_log::info(&ctx.source, "Stopped by a stop macro block");
            if let Some(flag) = &ctx.stop_flag
                && let Ok(mut running) = flag.lock()
            {
                *running = false;
            }
            return Ok(Next::Stop);
        }
        InstructionKind::ThrowError(message) => {
            let message = ctx
                .resolve(message, depth)
                .and_then(|v| v.eval_text())
                .unwrap_or_else(|e| e);
            ctx.error_block = ins.id.clone();
            return Err(if message.is_empty() {
                "error".to_string()
            } else {
                message
            });
        }
        InstructionKind::TryCatch { body, catch_body } => {
            ctx.try_depth += 1;
            let result = run_block(body, ctx, depth, Instant::now());
            ctx.try_depth -= 1;
            match result {
                Ok(Flow::Normal) => {}
                Ok(flow) => return Ok(Next::Exit(flow)),
                Err(message) => {
                    crate::run_log::info(&ctx.source, format!("Handled error: {message}"));
                    ctx.error_block.clear();
                    LAST_ERROR.with(|e| *e.borrow_mut() = message);
                    let flow = run_block(catch_body, ctx, depth, Instant::now())?;
                    if flow != Flow::Normal {
                        return Ok(Next::Exit(flow));
                    }
                }
            }
        }
        InstructionKind::Retry { count, delay, body } => {
            let attempts = ctx.resolve(count, depth).and_then(|v| v.eval_number());
            let delay_ms = ctx.resolve(delay, depth).and_then(|v| v.eval_number());
            let (attempts, delay_ms) = match (attempts, delay_ms) {
                (Ok(a), Ok(d)) => (a.max(1.0).round() as u64, d),
                (Err(e), _) | (_, Err(e)) => {
                    soft_fail(ctx, &ins.id, "Retry", e)?;
                    return Ok(Next::Go);
                }
            };
            for attempt in 1..=attempts {
                ctx.try_depth += 1;
                let result = run_block(body, ctx, depth, Instant::now());
                ctx.try_depth -= 1;
                let message = match result {
                    Ok(Flow::Normal) => break,
                    Ok(flow) => return Ok(Next::Exit(flow)),
                    Err(message) => message,
                };
                ctx.error_block.clear();
                LAST_ERROR.with(|e| *e.borrow_mut() = message.clone());
                if attempt == attempts {
                    soft_fail(
                        ctx,
                        &ins.id,
                        "Retry",
                        format!("gave up after {attempts} attempts: {message}"),
                    )?;
                    break;
                }
                crate::run_log::info(
                    &ctx.source,
                    format!("Attempt {attempt} of {attempts} failed: {message}"),
                );
                if sleep_unless_stopped(ctx, delay_ms) {
                    break;
                }
            }
        }
        InstructionKind::WaitUntil {
            condition,
            timeout,
            error_timeout,
        } => {
            let (timeout, error_timeout) = match (|| {
                Ok::<_, String>((
                    optional_number(ctx, depth, timeout.as_ref(), 0.0)?,
                    optional_number(ctx, depth, error_timeout.as_ref(), 0.0)?,
                ))
            })() {
                Ok(limits) => limits,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Wait until", e)?;
                    return Ok(Next::Go);
                }
            };
            let started = Instant::now();
            loop {
                match ctx
                    .resolve(condition, depth)
                    .and_then(|v| v.eval())
                    .map(|v| v.as_bool())
                {
                    Ok(true) => break,
                    Ok(false) => {}
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "Wait until", e)?;
                        break;
                    }
                }
                if wait_limit_reached(
                    ctx,
                    &ins.id,
                    started,
                    timeout,
                    error_timeout,
                    "Wait until: still false",
                )? {
                    break;
                }
                if sleep_unless_stopped(ctx, CONDITION_POLL_MS * ctx.speed_multiplier) {
                    break;
                }
            }
        }
        InstructionKind::CallBlock {
            block_id,
            args,
            branches,
        } => {
            let mut evaluated_args = Vec::with_capacity(args.len());
            for a in args {
                match ctx.resolve(a, depth).and_then(|v| v.eval()) {
                    Ok(v) => evaluated_args.push(v),
                    Err(e) => {
                        soft_fail(ctx, &ins.id, &format!("Call to '{block_id}'"), e)?;
                        continue;
                    }
                }
            }
            // A non-reporter (`Normal`/`Ending`) block's body normally
            // has no `Return`; if one sneaks in, it just ends the call
            // early.
            let _ = call_block(block_id, evaluated_args, branches.clone(), ctx, depth + 1)?;
        }
        InstructionKind::RunBranch(name) => {
            if let Some(body) = ctx.branch_env.get(name).cloned() {
                let flow = run_block(&body, ctx, depth, Instant::now())?;
                if flow != Flow::Normal {
                    return Ok(Next::Exit(flow));
                }
            }
        }
        InstructionKind::If { condition, body } => {
            match ctx.resolve(condition, depth).and_then(|v| v.eval()) {
                Ok(cond) => {
                    if cond.as_bool() {
                        let flow = run_block(body, ctx, depth, Instant::now())?;
                        if flow != Flow::Normal {
                            return Ok(Next::Exit(flow));
                        }
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "If", e)?,
            }
        }
        InstructionKind::IfElse {
            condition,
            then_body,
            else_body,
        } => match ctx.resolve(condition, depth).and_then(|v| v.eval()) {
            Ok(cond) => {
                let branch = if cond.as_bool() { then_body } else { else_body };
                let flow = run_block(branch, ctx, depth, Instant::now())?;
                if flow != Flow::Normal {
                    return Ok(Next::Exit(flow));
                }
            }
            Err(e) => soft_fail(ctx, &ins.id, "If else", e)?,
        },
        InstructionKind::Repeat { count, body } => {
            let n = match ctx.resolve(count, depth).and_then(|v| v.eval_number()) {
                Ok(v) => v,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Repeat", e)?;
                    return Ok(Next::Go);
                }
            };
            let n = n.max(0.0).round() as u64;
            let mut iterations = 0;
            for _ in 0..n {
                if let Some(flag) = &ctx.stop_flag
                    && stop_requested(flag)
                {
                    break;
                }
                if loop_limit_hit(ctx, &ins.id, &mut iterations)? {
                    break;
                }
                match run_block(body, ctx, depth, Instant::now())? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Break => break,
                    flow @ Flow::Return(_) => return Ok(Next::Exit(flow)),
                }
            }
        }
        InstructionKind::Forever { body } => {
            let mut iterations = 0;
            loop {
                if let Some(flag) = &ctx.stop_flag
                    && stop_requested(flag)
                {
                    break;
                }
                if loop_limit_hit(ctx, &ins.id, &mut iterations)? {
                    break;
                }
                match run_block(body, ctx, depth, Instant::now())? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Break => break,
                    flow @ Flow::Return(_) => return Ok(Next::Exit(flow)),
                }
            }
        }
        InstructionKind::While { condition, body } => {
            let mut iterations = 0;
            loop {
                if let Some(flag) = &ctx.stop_flag
                    && stop_requested(flag)
                {
                    break;
                }
                match ctx.resolve(condition, depth).and_then(|v| v.eval()) {
                    Ok(cond) => {
                        if !cond.as_bool() {
                            break;
                        }
                    }
                    Err(e) => {
                        soft_fail(ctx, &ins.id, "While", e)?;
                        break;
                    }
                }
                if loop_limit_hit(ctx, &ins.id, &mut iterations)? {
                    break;
                }
                match run_block(body, ctx, depth, Instant::now())? {
                    Flow::Normal | Flow::Continue => {}
                    Flow::Break => break,
                    flow @ Flow::Return(_) => return Ok(Next::Exit(flow)),
                }
            }
        }
        _ => unreachable!("run_block only sends flow blocks here"),
    }
    Ok(Next::Go)
}
