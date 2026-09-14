#![cfg(windows)]

use self_update::backends::github::Update;
use self_update::Download;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;

const REPO_OWNER: &str = "EthanRStokes";
const REPO_NAME: &str = "blockwork";
// Must match the artifact_name values produced by .github/workflows/release.yml.
const ASSET_NAME: &str = "blockwork-windows-x86_64.exe";
const INSTALLER_ASSET_NAME: &str = "blockwork-windows-x86_64-setup.exe";

const DETACHED_PROCESS: u32 = 0x0000_0008;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
}

fn build_updater(current_version: &str) -> Result<Update, String> {
    Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("blockwork")
        .bin_path_in_archive(ASSET_NAME)
        .current_version(current_version)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .build()
        .map_err(|err| err.to_string())
}

/// Blocking — call via `tokio::task::spawn_blocking`. `Ok(None)` means already up to date.
pub fn check_for_update(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let updater = build_updater(current_version)?;
    let releases = updater
        .get_latest_release()
        .map_err(|err| err.to_string())?;
    let release = releases
        .latest()
        .ok_or_else(|| "no releases found".to_string())?;

    let is_newer = self_update::version::bump_is_greater(current_version, release.version())
        .map_err(|err| err.to_string())?;

    Ok(is_newer.then(|| UpdateInfo {
        version: release.version().to_owned(),
    }))
}

/// Blocking — call via `tokio::task::spawn_blocking`. Downloads the latest release's installer
/// and launches it; the caller is expected to kill the current process immediately after.
/// Deliberately doesn't touch the running exe — every in-process replacement attempt hit
/// "used by another process" — so it just runs the real installer, which finds nothing
/// locking the install once this process exits.
pub fn apply_update(current_version: &str) -> Result<PathBuf, String> {
    let updater = build_updater(current_version)?;
    let releases = updater
        .get_latest_release()
        .map_err(|err| err.to_string())?;
    let release = releases
        .latest()
        .ok_or_else(|| "no releases found".to_string())?;
    let installer_asset = release
        .assets()
        .iter()
        .find(|asset| asset.name() == INSTALLER_ASSET_NAME)
        .ok_or_else(|| format!("no '{INSTALLER_ASSET_NAME}' asset in the latest release"))?;

    let temp_dir = std::env::temp_dir().join("blockwork-update");
    fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;
    let installer_path = temp_dir.join(INSTALLER_ASSET_NAME);
    let mut installer_file = fs::File::create(&installer_path).map_err(|err| err.to_string())?;

    // GitHub's API asset endpoint returns a JSON description instead of binary content
    // without this header, producing a garbage "exe" Windows can't recognize as a valid PE.
    let mut download = Download::from_url(installer_asset.download_url());
    download.show_download_progress(false);
    download.request_header(
        http::header::ACCEPT,
        "application/octet-stream",
    );
    download
        .download_to(&mut installer_file)
        .map_err(|err| err.to_string())?;
    drop(installer_file);

    std::process::Command::new(&installer_path)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|err| format!("failed to launch installer: {err}"))?;

    Ok(installer_path)
}
