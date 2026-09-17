//! Self-update via GitHub releases.
//!
//! Windows: downloads the latest release's setup exe and launches it; the
//! caller exits immediately after so the installer finds nothing locking the
//! install directory.
//!
//! macOS: downloads the latest release's `.app.zip` (produced by
//! `scripts/build-macos-bundle.sh` + `ditto -c -k --keepParent`), extracts it
//! with the system `ditto` (which restores the symlinks/resource forks inside
//! `Chromium Embedded Framework.framework` faithfully - a pure-Rust zip
//! extractor would not), swaps the new `Blockwork.app` over the running
//! bundle, and re-opens it. The caller exits immediately after.

use self_update::Download;
use self_update::backends::github::Update;
use std::fs;
use std::path::{Path, PathBuf};

const REPO_OWNER: &str = "Blockworked";
const REPO_NAME: &str = "blockwork";
// Must match the artifact_name values produced by .github/workflows/release.yml.
#[cfg(windows)]
const ASSET_NAME: &str = "blockwork-windows-x86_64.exe";
#[cfg(windows)]
const INSTALLER_ASSET_NAME: &str = "blockwork-windows-x86_64-setup.exe";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
}

/// Release asset holding the macOS app bundle for the current architecture,
/// e.g. `blockwork-macos-arm64.app.zip`. Note the release uses Go-style arch
/// names (`arm64`), not Rust's (`aarch64`), so the two are translated here.
#[cfg(any(target_os = "macos", test))]
fn macos_asset_name() -> String {
    format!(
        "blockwork-macos-{}.app.zip",
        release_arch_name(std::env::consts::ARCH)
    )
}

/// Release arch naming follows .github/workflows/release.yml, which uses
/// Go-style names: `std::env::consts::ARCH` is `aarch64` on Apple Silicon,
/// but that artifact is named `blockwork-macos-arm64`.
#[cfg(any(target_os = "macos", test))]
fn release_arch_name(rust_arch: &str) -> &str {
    match rust_arch {
        "aarch64" => "arm64",
        arch => arch,
    }
}

fn build_updater(current_version: &str) -> Result<Update, String> {
    let mut configure = Update::configure();
    configure
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("blockwork")
        .current_version(current_version)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true);
    #[cfg(windows)]
    configure.bin_path_in_archive(ASSET_NAME);
    configure.build().map_err(|err| err.to_string())
}

/// Blocking - call via `tokio::task::spawn_blocking`. `Ok(None)` means already up to date.
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

/// Blocking - call via `tokio::task::spawn_blocking`. Downloads the latest release's installer
/// and launches it; the caller is expected to kill the current process immediately after.
/// Deliberately doesn't touch the running exe - every in-process replacement attempt hit
/// "used by another process" - so it just runs the real installer, which finds nothing
/// locking the install once this process exits.
#[cfg(windows)]
pub fn apply_update(current_version: &str) -> Result<PathBuf, String> {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x0000_0008;

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
    download.request_header(http::header::ACCEPT, "application/octet-stream");
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

/// Blocking - call via `tokio::task::spawn_blocking`. Downloads the latest release's
/// `.app.zip`, swaps the new bundle over the running one, and re-opens it; the caller
/// is expected to exit immediately after so the old bundle isn't in use mid-swap.
#[cfg(target_os = "macos")]
pub fn apply_update(current_version: &str) -> Result<PathBuf, String> {
    let updater = build_updater(current_version)?;
    let releases = updater
        .get_latest_release()
        .map_err(|err| err.to_string())?;
    let release = releases
        .latest()
        .ok_or_else(|| "no releases found".to_string())?;
    let asset_name = macos_asset_name();
    let bundle_asset = release
        .assets()
        .iter()
        .find(|asset| asset.name() == asset_name)
        .ok_or_else(|| format!("no '{asset_name}' asset in the latest release"))?;

    let temp_dir = std::env::temp_dir().join("blockwork-update");
    fs::create_dir_all(&temp_dir).map_err(|err| err.to_string())?;
    let zip_path = temp_dir.join(&asset_name);
    let mut zip_file = fs::File::create(&zip_path).map_err(|err| err.to_string())?;

    // Same Accept-header gotcha as the Windows installer download above.
    let mut download = Download::from_url(bundle_asset.download_url());
    download.show_download_progress(false);
    download.request_header(http::header::ACCEPT, "application/octet-stream");
    download
        .download_to(&mut zip_file)
        .map_err(|err| err.to_string())?;
    drop(zip_file);

    // The release zip is created with `ditto -c -k --sequesterRsrc --keepParent`,
    // so extract with the system `ditto` to restore symlinks/resource forks exactly.
    let extract_dir = temp_dir.join("extracted");
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).map_err(|err| err.to_string())?;
    }
    fs::create_dir_all(&extract_dir).map_err(|err| err.to_string())?;
    let status = std::process::Command::new("ditto")
        .args(["-x", "-k"])
        .arg(&zip_path)
        .arg(&extract_dir)
        .status()
        .map_err(|err| format!("failed to extract update archive: {err}"))?;
    if !status.success() {
        return Err(format!(
            "failed to extract update archive: ditto exited with {status}"
        ));
    }

    let new_app = find_blockwork_app(&extract_dir)
        .ok_or_else(|| "update archive did not contain a Blockwork.app bundle".to_string())?;
    let current_bundle = current_app_bundle()?;

    let backup_bundle = backup_path(&current_bundle);
    if backup_bundle.exists() {
        fs::remove_dir_all(&backup_bundle).map_err(|err| err.to_string())?;
    }
    fs::rename(&current_bundle, &backup_bundle).map_err(|err| err.to_string())?;
    if let Err(err) = fs::rename(&new_app, &current_bundle) {
        // Best-effort rollback so the user isn't left without an app bundle.
        let _ = fs::rename(&backup_bundle, &current_bundle);
        return Err(err.to_string());
    }

    // A bundle moved in from a download carries a quarantine flag that makes
    // Gatekeeper refuse (or nag about) the fresh copy; the replaced bundle is
    // already ad-hoc signed by the release packaging, so just clear it.
    // Best-effort: a failure here shouldn't fail the update.
    let _ = std::process::Command::new("xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(&current_bundle)
        .status();

    std::process::Command::new("open")
        .args(["-n"])
        .arg(&current_bundle)
        .spawn()
        .map_err(|err| format!("update installed but failed to relaunch: {err}"))?;

    Ok(current_bundle)
}

/// The running `Blockwork.app` bundle, derived from the current executable's
/// path (`…/Blockwork.app/Contents/MacOS/Blockwork`). Errors when running
/// outside a bundle (e.g. `cargo run` during development).
#[cfg(any(target_os = "macos", test))]
fn current_app_bundle() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    exe.ancestors()
        .find(|ancestor| {
            ancestor.extension().is_some_and(|ext| ext == "app")
                && ancestor.join("Contents").is_dir()
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot self-update: running outside of a Blockwork.app bundle".to_string())
}

/// Sibling path for the displaced bundle, e.g. `…/Blockwork.app.backup`.
#[cfg(any(target_os = "macos", test))]
fn backup_path(bundle: &Path) -> PathBuf {
    let file_name = bundle
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Blockwork.app".to_string());
    bundle.with_file_name(format!("{file_name}.backup"))
}

/// Locate the freshly extracted bundle: `Blockwork.app` at the archive root
/// first (the layout `ditto --keepParent` produces), then any `*.app` found
/// by a shallow recursive walk.
#[cfg(any(target_os = "macos", test))]
fn find_blockwork_app(extract_dir: &Path) -> Option<PathBuf> {
    let direct = extract_dir.join("Blockwork.app");
    if direct.is_dir() {
        return Some(direct);
    }
    let mut stack = vec![extract_dir.to_path_buf()];
    let mut fallback: Option<PathBuf> = None;
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "app") {
                if path.file_name().is_some_and(|name| name == "Blockwork.app") {
                    return Some(path);
                }
                fallback.get_or_insert(path);
            } else {
                stack.push(path);
            }
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_asset_name_matches_release_artifact() {
        // Must match the artifact_name values produced by
        // .github/workflows/release.yml for the macOS matrix entries, which
        // use `arm64` (not Rust's `aarch64`).
        assert_eq!(
            macos_asset_name(),
            format!(
                "blockwork-macos-{}.app.zip",
                release_arch_name(std::env::consts::ARCH)
            )
        );
    }

    #[test]
    fn release_arch_name_translates_aarch64_to_arm64() {
        // The actual Apple Silicon release artifact is
        // `blockwork-macos-arm64.app.zip`, while `std::env::consts::ARCH` on
        // that machine is `aarch64` - the updater must emit the former.
        assert_eq!(release_arch_name("aarch64"), "arm64");
        assert_eq!(release_arch_name("x86_64"), "x86_64");
    }

    #[test]
    fn backup_path_appends_backup_suffix() {
        let bundle = Path::new("/Applications/Blockwork.app");
        assert_eq!(
            backup_path(bundle),
            Path::new("/Applications/Blockwork.app.backup")
        );
    }

    #[test]
    fn current_app_bundle_errors_outside_a_bundle() {
        // `cargo test` never runs inside a `*.app/Contents/…` path, so this
        // must take the graceful "dev mode" error rather than finding a bundle.
        assert!(current_app_bundle().is_err());
    }

    #[test]
    fn find_blockwork_app_prefers_blockwork_bundle() {
        let root = std::env::temp_dir().join(format!(
            "blockwork-updater-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let blockwork = root.join("nested").join("Blockwork.app");
        let other = root.join("Other.app");
        fs::create_dir_all(&blockwork).unwrap();
        fs::create_dir_all(&other).unwrap();
        assert_eq!(find_blockwork_app(&root), Some(blockwork));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn find_blockwork_app_falls_back_to_any_bundle() {
        let root = std::env::temp_dir().join(format!(
            "blockwork-updater-fallback-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let other = root.join("Other.app");
        fs::create_dir_all(&other).unwrap();
        assert_eq!(find_blockwork_app(&root), Some(other));
        fs::remove_dir_all(&root).unwrap();
    }
}
