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
