//! Shows macro dialogs by starting `blockwork --prompt`, which reads the
//! request on stdin and prints the answer, so they work with the editor closed.

use blockwork_app::prompt::{Reply, Request};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub(crate) fn install() {
    blockwork_app::prompt::set_handler(Arc::new(show));
}

fn show(request: &Request, stopped: &dyn Fn() -> bool) -> Result<Reply, String> {
    let program = crate::server::ui_program().ok_or("can't find the Blockwork window program")?;
    let mut child = Command::new(&program)
        .arg("--prompt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("couldn't open the dialog: {e}"))?;
    let request = serde_json::to_string(request).map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(request.as_bytes());
    }
    let (tx, rx) = mpsc::channel();
    if let Some(mut stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout.read_to_string(&mut text);
            let _ = tx.send(text);
        });
    }
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(text) => {
                let _ = child.wait();
                let line = text.lines().rev().find(|l| l.trim_start().starts_with('{')).unwrap_or_default();
                return serde_json::from_str(line).map_err(|_| "the dialog closed without an answer".to_string());
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
