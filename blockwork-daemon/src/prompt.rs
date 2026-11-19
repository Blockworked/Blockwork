//! Shows macro dialogs by starting `blockwork --prompt`, which reads the
//! request on stdin and prints the answer, so they work with the editor closed.
//! A window prints a line per button click before its answer.

use blockwork_app::prompt::{Reply, Request};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::Duration;

pub(crate) fn install() {
    blockwork_app::prompt::set_handler(Arc::new(show));
}

fn show(
    request: &Request,
    stopped: &dyn Fn() -> bool,
    on_click: &dyn Fn(&Reply),
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
    let request = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(request.as_bytes());
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
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(reply) if reply.answer == "click" => on_click(&reply),
            Ok(reply) => {
                let _ = child.wait();
                return Ok(reply);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.wait();
                return Err("the dialog closed without an answer".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) if stopped() => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("stopped".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}
