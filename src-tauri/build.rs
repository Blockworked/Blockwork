fn main() {
    build_frontend();
    // Also embeds the Windows icon/version resource; don't duplicate that
    // here or CVTRES fails with a duplicate VERSION resource.
    tauri_build::build();
}

// `frontendDist` (`../ui/dist`) doesn't exist until Vite runs, and plain
// `cargo build`/`cargo run` never go through the Tauri CLI to trigger that
// — so build it here instead.
fn build_frontend() {
    let ui_dir = std::path::Path::new("..").join("ui");

    println!("cargo:rerun-if-changed={}", ui_dir.join("index.html").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("src").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("package.json").display());
    println!("cargo:rerun-if-changed={}", ui_dir.join("vite.config.js").display());

    // pnpm may be a `.cmd` shim (npm global) or `pnpm.exe` (WinGet/scoop) on
    // Windows; try both, then fall back to PATH resolution.
    let pnpm = if cfg!(windows) {
        ["pnpm.cmd", "pnpm.exe", "pnpm"]
            .iter()
            .find(|name| std::process::Command::new("where").arg(name).output().map_or(false, |o| o.status.success()))
            .copied()
            .unwrap_or("pnpm")
    } else {
        "pnpm"
    };

    let run = |args: &[&str]| {
        let mut command = std::process::Command::new(pnpm);
        command.args(args).current_dir(&ui_dir);
        // BLOCKWORK_PNPM_OFFLINE=1 (set by packaging/flatpak — deliberately
        // not FLATPAK, which cef-dll-sys's own build script already keys off
        // for its CEF lookup) needs pnpm to trust the lockfile offline; pnpm
        // 11 only honors that via CLI flags, not the env-var config.
        // --ignore-scripts avoids pnpm 11.27+ falling back to an online
        // download for the git-hosted blockstitch store entry (nothing in
        // ui/ needs install scripts anyway).
        if args[0] == "install" && std::env::var_os("BLOCKWORK_PNPM_OFFLINE").is_some() {
            command.args(["--offline", "--frozen-lockfile", "--trust-lockfile", "--ignore-scripts"]);
        }
        let status = command
            .status()
            .unwrap_or_else(|e| panic!("failed to run `pnpm {}` in {ui_dir:?}: {e}", args.join(" ")));
        if !status.success() {
            panic!("`pnpm {}` in {ui_dir:?} failed with {status}", args.join(" "));
        }
    };

    run(&["install"]);
    run(&["run", "build"]);
}
