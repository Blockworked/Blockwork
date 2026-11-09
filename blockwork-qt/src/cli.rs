//! `blockwork list|run|stop`: deprecated shim to `blockwork-agent`.
//! The agent binary is the headless CLI now; this forwards to it so there is
//! exactly one headless implementation.

pub fn requested() -> Option<Vec<String>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first()?;
    matches!(
        first.as_str(),
        "list" | "run" | "stop" | "help" | "--help" | "-h"
    )
    .then_some(args)
}

pub fn main(args: Vec<String>) -> i32 {
    let agent = std::env::current_exe()
        .map(|exe| exe.with_file_name(format!("blockwork-agent{}", std::env::consts::EXE_SUFFIX)))
        .ok();
    let agent = match agent {
        Some(p) if p.exists() => p,
        _ => {
            eprintln!("blockwork: blockwork-agent isn't installed beside this binary (deprecated 'list/run/stop'; use blockwork-agent)");
            return 1;
        }
    };
    let mapped: Vec<String> = match args.first().map(String::as_str) {
        Some("list") => vec!["list-macros".to_string()],
        Some("run") => {
            let mut v = vec!["run-macro".to_string()];
            v.extend(args[1..].to_vec());
            v
        }
        Some("stop") => {
            let target = args.get(1).cloned().unwrap_or_default();
            vec!["stop-macro".to_string(), target]
        }
        _ => {
            println!("Usage:\n  blockwork                    Open the editor\n  blockwork list|run|stop   Deprecated; use blockwork-agent (list-macros|run-macro|stop-macro)");
            return 0;
        }
    };
    match std::process::Command::new(&agent).args(&mapped).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("blockwork: failed to start blockwork-agent: {e}");
            1
        }
    }
}
