//! Blocks that reach outside the macro: signals, other macros, globals,
//! windows, files, notifications, web requests and shell commands.

use super::*;

/// Runs one of these blocks. A failure is soft (see `soft_fail`) unless the
/// strand is inside a `try`.
pub(super) fn run_system_block(ins: &Instruction, ctx: &mut ExecCtx, depth: u32) -> Result<(), String> {
    match &ins.kind {
        InstructionKind::SendSignal { name, wait } => {
            let name = match ctx.resolve(&name.value, depth).and_then(|v| v.eval_text()) {
                Ok(name) => name,
                Err(e) => {
                    soft_fail(ctx, &ins.id, "Send signal", e)?;
                    return Ok(());
                }
            };
            let mut threads = Vec::new();
            for body in signal_bodies(ctx, &name, depth) {
                let (source, emulator, stop_flag) = (Arc::clone(&ctx.source), Arc::clone(&ctx.emulator), ctx.stop_flag.clone());
                let (variables, lists, table) = (Arc::clone(&ctx.variables), Arc::clone(&ctx.lists), Arc::clone(&ctx.block_table));
                let (speed, loops) = (ctx.speed_multiplier, u32::try_from(ctx.max_loop_iterations).unwrap_or(u32::MAX));
                let spawned = std::thread::Builder::new().name(format!("signal_{name}")).stack_size(STRAND_STACK_SIZE).spawn(move || {
                    run_strand(source, body, emulator, stop_flag, speed, variables, lists, table, Duration::ZERO, loops)
                });
                match spawned {
                    Ok(thread) => threads.push(thread),
                    Err(e) => warn!("Failed to start a signal strand: {e}"),
                }
            }
            if *wait {
                for thread in threads {
                    let _ = thread.join();
                }
            } else {
                ctx.signal_threads.extend(threads);
            }
        }
        InstructionKind::WindowAction { action, pattern, by } => {
            let done = (|| {
                let pattern = ctx.resolve(pattern, depth)?.eval_text()?;
                crate::window::act(action.parse()?, &pattern, by.parse()?)
            })();
            if let Err(e) = done {
                soft_fail(ctx, &ins.id, "Window", e)?;
            }
        }
        InstructionKind::MoveWindow { pattern, by, resize, x, y } => {
            let done = (|| {
                let pattern = ctx.resolve(pattern, depth)?.eval_text()?;
                let x = ctx.resolve(x, depth)?.eval_number()?.round() as i32;
                let y = ctx.resolve(y, depth)?.eval_number()?.round() as i32;
                crate::window::place(&pattern, by.parse()?, *resize, x, y)
            })();
            if let Err(e) = done {
                soft_fail(ctx, &ins.id, if *resize { "Resize window" } else { "Move window" }, e)?;
            }
        }
        InstructionKind::RunMacro { target, wait } => {
            match ctx.resolve(target, depth).and_then(|v| v.eval_text()).and_then(|name| crate::macros::calls::start(&name)) {
                Ok((id, done)) if *wait => {
                    // Stopping this run stops the one it's waiting on too.
                    while !done.load(Ordering::Relaxed) {
                        if ctx.stop_flag.as_ref().is_some_and(stop_requested) {
                            crate::macros::run_registry::stop_macro(&id);
                            break;
                        }
                        std::thread::sleep(STOP_POLL_INTERVAL);
                    }
                }
                Ok(_) => {}
                Err(e) => soft_fail(ctx, &ins.id, "Run macro", e)?,
            }
        }
        InstructionKind::StopMacroNamed(target) => {
            if let Err(e) = ctx.resolve(target, depth).and_then(|v| v.eval_text()).and_then(|name| crate::macros::calls::stop(&name)) {
                soft_fail(ctx, &ins.id, "Stop macro", e)?;
            }
        }
        InstructionKind::SetGlobal { name, value } => {
            let set = ctx.resolve(name, depth).and_then(|n| n.eval_text()).and_then(|name| {
                crate::globals::set(&name, ctx.resolve(value, depth)?.eval()?)
            });
            if let Err(e) = set {
                soft_fail(ctx, &ins.id, "Set global", e)?;
            }
        }
        InstructionKind::ChangeGlobal { name, by } => {
            let changed = ctx.resolve(name, depth).and_then(|n| n.eval_text()).and_then(|name| {
                crate::globals::change(&name, ctx.resolve(by, depth)?.eval_number()?)
            });
            if let Err(e) = changed {
                soft_fail(ctx, &ins.id, "Change global", e)?;
            }
        }
        InstructionKind::Log(value) => match ctx.resolve(value, depth).and_then(|v| v.eval_text()) {
            Ok(text) => crate::run_log::info(&ctx.source, text),
            Err(e) => soft_fail(ctx, &ins.id, "Log", e)?,
        },
        InstructionKind::Notify { title, message } => {
            let title = ctx.resolve(title, depth).and_then(|v| v.eval_text());
            let message = ctx.resolve(message, depth).and_then(|v| v.eval_text());
            let shown = title.and_then(|t| message.and_then(|m| crate::notify::show(&t, &m)));
            if let Err(e) = shown {
                soft_fail(ctx, &ins.id, "Notify", e)?;
            }
        }
        InstructionKind::WriteFile { path, text, append } => {
            let path = ctx.resolve(path, depth).and_then(|v| v.eval_text());
            let text = ctx.resolve(text, depth).and_then(|v| v.eval_text());
            let written = path.and_then(|p| text.and_then(|t| crate::files::write_text(&p, &t, *append)));
            if let Err(e) = written {
                soft_fail(ctx, &ins.id, if *append { "Append to file" } else { "Write file" }, e)?;
            }
        }
        InstructionKind::HttpRequest {
            method,
            url,
            body,
            headers,
            variable,
        } => {
            let response = (|| {
                let url = ctx.resolve(url, depth)?.eval_text()?;
                let body = ctx.resolve(body, depth)?.eval_text()?;
                let headers = ctx.resolve(headers, depth)?.eval_text()?;
                crate::http::request(method, &url, &body, &headers)
            })();
            match response {
                Ok(text) => {
                    if !variable.is_empty()
                        && let Ok(mut vars) = ctx.variables.lock()
                    {
                        vars.insert(variable.clone(), Evaluated::Text(text));
                    }
                }
                Err(e) => soft_fail(ctx, &ins.id, "Web request", e)?,
            }
        }
        InstructionKind::Command(command) => {
            println!("Starting command: {command}");
            match shell_process(command).spawn() {
                // Reap it when it exits so it doesn't linger as a zombie.
                Ok(mut child) => {
                    std::thread::spawn(move || child.wait());
                }
                Err(e) => soft_fail(ctx, &ins.id, "Command", e)?,
            }
        }
        InstructionKind::CommandWait(command) => {
            println!("Running command: {command}");
            if let Err(e) = shell_process(command).status() {
                soft_fail(ctx, &ins.id, "Command", e)?;
            }
        }
        _ => unreachable!("run_block only sends system blocks here"),
    }
    Ok(())
}
