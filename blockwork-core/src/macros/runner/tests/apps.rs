//! Opening apps and splitting command lines.

use super::*;

#[cfg(target_os = "linux")]
#[test]
fn open_app_passes_arguments_folder_and_environment() {
    if crate::flatpak::is_flatpak() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("blockwork-open-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.txt");
    let launch = LaunchOptions {
        args: "'two words' \"$GREETING\"".into(),
        working_dir: dir.display().to_string(),
        env: vec![crate::macros::EnvVar {
            name: "GREETING".into(),
            value: "hi".into(),
        }],
        new_instance: false,
    };
    let command = format!(
        "sh -c 'printf \"%s|\" \"$PWD\" \"$@\" > {}' app",
        out.display()
    );
    open_app(&command, &launch).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !out.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(50));
    let text = std::fs::read_to_string(&out).unwrap();
    assert_eq!(text, format!("{}|two words|hi|", dir.display()));
    let missing = LaunchOptions {
        working_dir: dir.join("nope").display().to_string(),
        ..Default::default()
    };
    assert!(open_app("true", &missing).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn split_args_reads_quotes_and_escapes() {
    assert_eq!(split_args(""), Vec::<String>::new());
    assert_eq!(
        split_args("  --incognito  --new-window "),
        ["--incognito", "--new-window"]
    );
    assert_eq!(
        split_args(r#"--name "My Profile" 'a b' c\ d"#),
        ["--name", "My Profile", "a b", "c d"]
    );
    assert_eq!(split_args(r#"--x="" end"#), ["--x=", "end"]);
    assert_eq!(split_args(r#""say \"hi\"""#), [r#"say "hi""#]);
}
