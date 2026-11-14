//! Stamps `BLOCKWORK_BUILD`: the package version, plus the git commit when
//! there is one (or `BLOCKWORK_BUILD_ID` from a packager).
use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn main() {
    println!("cargo:rerun-if-env-changed=BLOCKWORK_BUILD_ID");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let id = std::env::var("BLOCKWORK_BUILD_ID")
        .ok()
        .filter(|id| !id.is_empty())
        .or_else(|| {
            // Rebuild when the commit moves. Only existing files: a missing
            // one would rebuild every time.
            let mut watched = vec!["HEAD".to_owned(), "packed-refs".to_owned()];
            watched.extend(git(&["symbolic-ref", "-q", "HEAD"]));
            for name in watched {
                if let Some(path) =
                    git(&["rev-parse", "--path-format=absolute", "--git-path", &name])
                    && Path::new(&path).exists()
                {
                    println!("cargo:rerun-if-changed={path}");
                }
            }
            git(&["rev-parse", "HEAD"])
        });
    let build = match id {
        Some(id) => format!("{version}+{id}"),
        None => version,
    };
    println!("cargo:rustc-env=BLOCKWORK_BUILD={build}");
}
