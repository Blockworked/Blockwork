//! Shows macro dialogs by starting `blockwork --prompt`, which reads the
//! request on stdin and prints the answer, so they work with the editor closed.
//! A window prints a line per button click before its answer.

use blockwork_app::prompt::{Reply, Request, Updates};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::Duration;

pub(crate) fn install() {
    blockwork_app::prompt::set_live_handler(Arc::new(show));
}

fn show(
    request: &Request,
    stopped: &dyn Fn() -> bool,
    on_event: &dyn Fn(&Reply),
    updates: &Updates<'_>,
) -> Result<Reply, String> {
    let program = crate::server::ui_program().ok_or("can't find the Blockwork window program")?;
    let mut child = Command::new(&program)
        .arg("--prompt")
        .env(
            "BLOCKWORK_PROMPT_PARENT_PID",
            std::process::id().to_string(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("couldn't open the dialog: {e}"))?;
    let mut request = request.clone();
    request.language = blockwork_app::ui_language();
    request.animation_speed = Some(blockwork_app::animation_speed());
    let live = request.live;
    let (commands, pending) = mpsc::sync_channel::<serde_json::Value>(1);
    if let Some(mut stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            if writeln!(stdin, "{}", serde_json::json!(request)).is_ok() && live {
                for patch in pending {
                    if writeln!(stdin, "{patch}").is_err() {
                        break;
                    }
                }
            }
        });
    }
    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(reply) = serde_json::from_str::<Reply>(line.trim()) {
                    let _ = tx.send(reply);
                }
            }
        });
    }
    let mut unsent = None;
    let mut last_poll = std::time::Instant::now();
    loop {
        if stopped() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("stopped".into());
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(reply) if matches!(reply.answer.as_str(), "click" | "change") => on_event(&reply),
            Ok(reply) => {
                drop(commands);
                let _ = child.wait();
                return Ok(reply);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.wait();
                return Err("the dialog closed without an answer".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !live || last_poll.elapsed() < Duration::from_millis(40) {
            continue;
        }
        last_poll = std::time::Instant::now();
        if let Some(patch) = unsent.take().or_else(updates)
            && let Err(mpsc::TrySendError::Full(patch)) = commands.try_send(patch)
        {
            unsent = Some(patch);
        }
    }
}
