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
}

#[test]
fn a_window_with_a_normal_block_inside_fails_and_parts_outside_do_nothing() {
    install_test_dialogs();
    let mac = Macro::new(
        "Window".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::WindowNote(Value::text("Loose"))),
            Instruction::new(InstructionKind::ShowWindow {
                title: Value::text("Settings"),
                body: vec![
                    set("inside", Value::number(1.0)),
                    Instruction::new(InstructionKind::WindowButton {
                        label: Value::text("Done"),
                        close: true,
                        body: vec![set("done", Value::number(1.0))],
                    }),
                ],
            }),
            set("after", Value::number(1.0)),
        ],
    );
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("inside"), None);
    assert_eq!(vars.get("done"), None, "the window never opened");
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn named_window_applies_lifecycle_and_binding_updates_while_running() {
    install_test_dialogs();
    let id = Value::text("named-form-lifecycle-test");
    let open = Value::op(Op::from_name("FormWindowOpen"), vec![id.clone()]);
    let window_part = |kind| Instruction::new(kind);
    let var = Value::Var {
        name: "input".into(),
    };
    let mac = Macro::new(
        "Named form".into(),
        String::new(),
        vec![
            set("input", Value::text("initial")),
            window_part(InstructionKind::OpenFormWindow {
                window: id.clone(),
                title: Value::text("Named form test"),
                width: Value::number(500.0),
                height: Value::number(300.0),
                body: vec![
                    window_part(InstructionKind::WindowLayout {
                        layout: "row".into(),
                        label: Value::text(""),
                        key: "details".into(),
                        columns: Value::number(2.0),
                        body: vec![
                            window_part(InstructionKind::WindowTextBox {
                                label: Value::text("Input"),
                                name: "input".into(),
                            }),
                            window_part(InstructionKind::WindowNote(var.clone())),
                        ],
                    }),
                    window_part(InstructionKind::WindowButton {
                        label: Value::text("Verified"),
                        close: false,
                        body: vec![set("verified", Value::number(1.0))],
                    }),
                    window_part(InstructionKind::WindowOnChange {
                        name: "input".into(),
                        body: vec![set("changed", var)],
                    }),
                ],
            }),
            set("open", open.clone()),
            window_part(InstructionKind::SetFormWindowTitle {
                window: id.clone(),
                title: Value::text("Updated"),
            }),
            window_part(InstructionKind::ResizeFormWindow {
                window: id.clone(),
                width: Value::number(600.0),
                height: Value::number(400.0),
            }),
            window_part(InstructionKind::SetFormComponent {
                window: id.clone(),
                component: Value::text("details"),
                property: "visible".into(),
                state: Value::number(0.0),
            }),
            window_part(InstructionKind::CloseFormWindow(id.clone())),
            window_part(InstructionKind::WaitForFormWindow(id)),
            set("after", open),
        ],
    );
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("open"), Some(&Evaluated::Bool(true)));
    assert_eq!(vars.get("after"), Some(&Evaluated::Bool(false)));
    assert_eq!(vars.get("changed"), Some(&Evaluated::Text("edited".into())));
    assert_eq!(vars.get("verified"), Some(&Evaluated::Number(1.0)));
}
