//! Runs a shell command and captures what it printed, for the
//! `ShellOutput`/`ShellSucceeded` reporters. Inside a Flatpak the command runs
//! on the host, like Open App does.

use std::process::{Command, Stdio};

pub struct Output {
    pub stdout: String,
    pub success: bool,
}

#[cfg(windows)]
fn shell(script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new("cmd");
    command.args(["/C", script]).creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn shell(script: &str) -> Command {
    let mut command = crate::flatpak::host_command("sh");
    command.args(["-c", script]);
    command
}

/// Runs `script` to completion. Only fails if it couldn't be started.
pub fn run(script: &str) -> Result<Output, String> {
    let output = shell(script)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("couldn't run command: {e}"))?;
    let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // Most commands end with a newline nobody wants in a variable.
    while stdout.ends_with('\n') || stdout.ends_with('\r') {
        stdout.pop();
    }
    Ok(Output {
        stdout,
        success: output.status.success(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn captures_output_and_exit_status() {
        let out = run("printf 'hello\\n'").unwrap();
        assert_eq!(out.stdout, "hello");
        assert!(out.success);
        assert!(!run("exit 3").unwrap().success);
    }
}
