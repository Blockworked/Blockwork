//! Ask blocks and restricted macros.

use super::*;

#[test]
fn ask_blocks_put_answers_in_variables_and_cancel_stops() {
    install_test_dialogs();
    let vars = empty_vars();
    let lists: ListStore = Arc::new(Mutex::new(HashMap::from([(
        "colors".to_string(),
        vec![ListItem::Text("red".into()), ListItem::Text("blue".into())],
    )])));
    run_instructions_with_lists(
        vec![
            Instruction::new(InstructionKind::AskText {
                prompt: Value::text("Name?"),
                default: Value::text("Ada"),
                name: "name".into(),
            }),
            Instruction::new(InstructionKind::AskYesNo {
                prompt: Value::text("Sure?"),
                name: "sure".into(),
            }),
            Instruction::new(InstructionKind::AskChoice {
                prompt: Value::text("Pick"),
                list: "colors".into(),
                name: "pick".into(),
            }),
            Instruction::new(InstructionKind::ShowMessage(Value::text("Done"))),
            Instruction::new(InstructionKind::AskFile {
                prompt: Value::text("Which file?"),
                folder: false,
                name: "file".into(),
            }),
            Instruction::new(InstructionKind::AskText {
                prompt: Value::text("cancel"),
                default: Value::text(""),
                name: "never".into(),
            }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
        lists,
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("name"), Some(&Evaluated::Text("Ada!".into())));
    assert_eq!(vars.get("sure"), Some(&Evaluated::Bool(true)));
    assert_eq!(vars.get("pick"), Some(&Evaluated::Text("blue".into())));
    assert_eq!(
        vars.get("file"),
        Some(&Evaluated::Text("/tmp/picked.txt".into()))
    );
    assert_eq!(vars.get("never"), None);
    assert_eq!(vars.get("after"), None, "cancel stopped the run");
}

#[test]
fn restricted_macros_skip_blocks_they_may_not_run() {
    install_test_dialogs();
    let marker = std::env::temp_dir().join(format!("bw-perm-{}", std::process::id()));
    let mut mac = Macro::new(
        "Restricted".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::CommandWait(format!(
                "touch '{}'",
                marker.display()
            ))),
            set("after", Value::number(1.0)),
        ],
    );
    mac.id = "runner-restricted".into();
    crate::permissions::restrict(&mac.id, Vec::new());
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    crate::permissions::trust("runner-restricted");
    assert!(!marker.exists(), "the denied command didn't run");
    assert_eq!(
        vars.lock().unwrap().get("after"),
        Some(&Evaluated::Number(1.0))
    );
}

#[test]
fn restricted_macros_ask_before_globals_files_and_input() {
    install_test_dialogs();
    let read = |op: &str, arg: &str| Value::op(Op::from_name(op), vec![Value::text(arg)]);
    let mut mac = Macro::new(
        "Restricted reads".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::SetGlobal {
                name: Value::text("runner-restricted-global"),
                value: Value::number(1.0),
            }),
            set("file", read("ReadFile", "/etc/hostname")),
            set("global", read("GlobalVariable", "runner-restricted-global")),
            Instruction::new(InstructionKind::RunMacro {
                target: Value::text("anything"),
                wait: true,
            }),
            set("after", Value::number(1.0)),
        ],
    );
    mac.id = "runner-restricted-reads".into();
    crate::permissions::restrict(&mac.id, Vec::new());
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    crate::permissions::trust("runner-restricted-reads");
    assert_eq!(
        crate::globals::get("runner-restricted-global"),
        Evaluated::Number(0.0)
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("file"), None);
    assert_eq!(vars.get("global"), None);
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn window_saves_its_values_and_buttons_run_their_blocks() {
    install_test_dialogs();
    let button = |label: &str, close: bool, var: &str| {
        Instruction::new(InstructionKind::WindowButton {
            label: Value::text(label),
            close,
            body: vec![set(var, Value::Var { name: "who".into() })],
        })
    };
    let mac = Macro::new(
        "Window".into(),
        String::new(),
        vec![
            set("who", Value::text("Ann")),
            Instruction::new(InstructionKind::ShowWindow {
                title: Value::text("Settings"),
                body: vec![
                    Instruction::new(InstructionKind::WindowNote(Value::text("Hi"))),
                    Instruction::new(InstructionKind::WindowTextBox {
                        label: Value::text("Name"),
                        name: "who".into(),
                    }),
                    Instruction::new(InstructionKind::WindowCheckbox {
                        label: Value::text("Loud"),
                        name: "loud".into(),
                    }),
                    // Other blocks inside are skipped.
                    set("skipped", Value::number(1.0)),
                    button("Apply", false, "applied"),
                    button("Done", true, "done"),
                ],
            }),
            // A closing button's blocks run before the ones after the window.
            set(
                "after",
                Value::Var {
                    name: "done".into(),
                },
            ),
        ],
    );
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    let typed = Some(&Evaluated::Text("typed".into()));
    assert_eq!(vars.get("loud"), Some(&Evaluated::Bool(true)));
    assert_eq!(vars.get("applied"), typed);
    assert_eq!(vars.get("done"), typed);
    assert_eq!(vars.get("after"), typed);
    assert_eq!(vars.get("skipped"), None);
}
