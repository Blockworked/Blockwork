//! Opt-in end-to-end updater fixture. Run a copy of this executable from
//! `<owned fixture>/Blockwork.app/Contents/MacOS/`, passing the fixture root.
//! The production updater downloads the latest public release, replaces only
//! that disposable bundle, and schedules its GUI to reopen after this exits.
#[cfg(all(target_os = "macos", feature = "updater"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, io, path::PathBuf};
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("pass the owned fixture root")?,
    )
    .canonicalize()?;
    let app = root.join("Blockwork.app");
    let executable = std::env::current_exe()?.canonicalize()?;
    if !root.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .starts_with("blockwork-network-update-")
    }) || !executable.starts_with(app.join("Contents/MacOS"))
        || fs::read_to_string(root.join("fixture-owned"))?
            != "Blockwork updater integration fixture\n"
    {
        return Err("refusing to update outside the explicitly marked disposable app".into());
    }
    let available = blockwork_core::updater::check_for_update("0.0.0")
        .map_err(io::Error::other)?
        .ok_or("expected a published release newer than the fixture")?;
    let old_pid = std::process::id();
    let installed = blockwork_core::updater::apply_update("0.0.0").map_err(io::Error::other)?;
    if installed != app {
        return Err("updater returned an unexpected installation path".into());
    }
    fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "lookup_version": available.version,
            "old_pid": old_pid,
            "installed_bundle": installed,
            "download_cache": std::env::temp_dir().join("blockwork-update"),
            "replacement_and_relaunch_scheduled": true
        }))?,
    )?;
    // Let the orchestrator verify that no GUI reopened while the old
    // process remains alive, then explicitly release this exit barrier.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !root.join("allow-old-exit").exists() {
        if std::time::Instant::now() >= deadline {
            return Err("orchestrator did not release the old-process exit barrier".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(())
}

#[cfg(not(all(target_os = "macos", feature = "updater")))]
fn main() {
    eprintln!("This opt-in fixture requires macOS and the updater feature.");
}
