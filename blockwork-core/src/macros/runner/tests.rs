use super::*;
use super::vision::*;
use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::{
    BlockDef, BlockPiece, BlockShape, InputValueType, MacroGraph, Strand,
    default_block_color,
};
use blockstitch_core::graph::resolve_list_reporters;

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
        env: vec![crate::macros::EnvVar { name: "GREETING".into(), value: "hi".into() }],
        new_instance: false,
    };
    let command = format!("sh -c 'printf \"%s|\" \"$PWD\" \"$@\" > {}' app", out.display());
    open_app(&command, &launch).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !out.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(50));
    let text = std::fs::read_to_string(&out).unwrap();
    assert_eq!(text, format!("{}|two words|hi|", dir.display()));
    let missing = LaunchOptions { working_dir: dir.join("nope").display().to_string(), ..Default::default() };
    assert!(open_app("true", &missing).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn split_args_reads_quotes_and_escapes() {
    assert_eq!(split_args(""), Vec::<String>::new());
    assert_eq!(split_args("  --incognito  --new-window "), ["--incognito", "--new-window"]);
    assert_eq!(split_args(r#"--name "My Profile" 'a b' c\ d"#), ["--name", "My Profile", "a b", "c d"]);
    assert_eq!(split_args(r#"--x="" end"#), ["--x=", "end"]);
    assert_eq!(split_args(r#""say \"hi\"""#), [r#"say "hi""#]);
}

struct NoopBackend;
impl InputBackend for NoopBackend {
    fn key(&mut self, _key: MacroKey, _dir: Direction) -> Result<(), String> {
        Ok(())
    }
    fn raw_keycode(&mut self, _keycode: u16, _dir: Direction) -> Result<(), String> {
        Ok(())
    }
    fn button(&mut self, _button: MacroButton, _dir: Direction) -> Result<(), String> {
        Ok(())
    }
    fn move_mouse_rel(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }
    fn move_mouse_abs(&mut self, _x: i32, _y: i32) -> Result<(), String> {
        Ok(())
    }
    fn scroll(&mut self, _amount: i32, _axis: Axis) -> Result<(), String> {
        Ok(())
    }
    fn text(&mut self, _s: &str) -> Result<(), String> {
        Ok(())
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
}

fn when_ran_strand(id: &str, wait_ms: f64) -> Strand {
    Strand {
        id: id.to_string(),
        x: 0,
        y: 0,
        instructions: vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::Wait(Value::number(wait_ms))),
        ],
    }
}

fn empty_vars() -> VariableStore {
    Arc::new(Mutex::new(HashMap::new()))
}

fn noop_emulator() -> Arc<Mutex<dyn InputBackend>> {
    Arc::new(Mutex::new(NoopBackend))
}

#[test]
fn list_commands_and_reporters_share_the_same_literal_store() {
    use crate::input::value::Op;

    let text = |value: &str| Value::Text {
        value: value.to_string(),
    };
    let reporter = |op, args| Value::Op {
        op,
        args,
        saved: Box::new(Value::number(0.0)),
    };
    let variables: VariableStore = Arc::new(Mutex::new(HashMap::new()));
    let lists: ListStore = Arc::new(Mutex::new(HashMap::from([(
        "items".to_string(),
        vec![ListItem::Text("one".into()), ListItem::Number(2.0)],
    )])));

    run_instructions_with_lists(
        vec![
            Instruction::new(InstructionKind::AddToList {
                value: Value::number(3.0),
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::InsertIntoList {
                value: text("z"),
                index: Value::number(2.0),
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::ReplaceItemOfList {
                index: Value::number(3.0),
                name: "items".into(),
                value: text("y"),
            }),
            Instruction::new(InstructionKind::ShiftList {
                name: "items".into(),
                amount: Value::number(1.0),
            }),
            Instruction::new(InstructionKind::DeleteOfList {
                index: Value::number(2.0),
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::ReverseList {
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::SetVariable(
                "item".into(),
                reporter(Op::from_name("ListItem"), vec![Value::number(1.0), text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "position".into(),
                reporter(Op::from_name("ListItemNumber"), vec![text("z"), text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "amount".into(),
                reporter(Op::from_name("ListAmount"), vec![text("y"), text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "length".into(),
                reporter(Op::from_name("ListLength"), vec![text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "contains".into(),
                reporter(Op::from_name("ListContains"), vec![text("items"), text("z")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "exists".into(),
                reporter(Op::from_name("ListItemExists"), vec![Value::number(3.0), text("items")]),
            )),
            Instruction::new(InstructionKind::SetVariable(
                "empty".into(),
                reporter(Op::from_name("ListIsEmpty"), vec![text("items")]),
            )),
            Instruction::new(InstructionKind::DeleteAllOfList {
                name: "items".into(),
            }),
            Instruction::new(InstructionKind::SetVariable(
                "now_empty".into(),
                reporter(Op::from_name("ListIsEmpty"), vec![text("items")]),
            )),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&variables),
        Arc::clone(&lists),
    );

    assert!(lists.lock().unwrap().get("items").unwrap().is_empty());
    let variables = variables.lock().unwrap();
    assert_eq!(variables.get("item"), Some(&Evaluated::Text("y".into())));
    assert_eq!(variables.get("position"), Some(&Evaluated::Number(2.0)));
    assert_eq!(variables.get("amount"), Some(&Evaluated::Number(1.0)));
    assert_eq!(variables.get("length"), Some(&Evaluated::Number(3.0)));
    assert_eq!(variables.get("contains"), Some(&Evaluated::Bool(true)));
    assert_eq!(variables.get("exists"), Some(&Evaluated::Bool(true)));
    assert_eq!(variables.get("empty"), Some(&Evaluated::Bool(false)));
    assert_eq!(variables.get("now_empty"), Some(&Evaluated::Bool(true)));
}

#[test]
fn resolve_list_reporters_supports_canvas_preview_without_running_a_macro() {
    use crate::input::value::Op;

    let lists = HashMap::from([(
        "items".to_string(),
        vec![ListItem::Text("first".into()), ListItem::Number(7.0)],
    )]);
    let list_item = Value::Op {
        op: Op::from_name("ListItem"),
        args: vec![
            Value::number(2.0),
            Value::Text {
                value: "items".into(),
            },
        ],
        saved: Box::new(Value::number(0.0)),
    };
    let preview =
        resolve_list_reporters(&list_item, &lists).and_then(|value| value.eval_text());
    assert_eq!(preview, Ok("7".to_string()));

    let nested_length = Value::Op {
        op: Op::Add,
        args: vec![
            Value::Op {
                op: Op::from_name("ListLength"),
                args: vec![Value::Text {
                    value: "items".into(),
                }],
                saved: Box::new(Value::number(0.0)),
            },
            Value::number(1.0),
        ],
        saved: Box::new(Value::number(0.0)),
    };
    let preview =
        resolve_list_reporters(&nested_length, &lists).and_then(|value| value.eval_text());
    assert_eq!(preview, Ok("3".to_string()));
}

/// Three entry strands each waiting 150ms should finish in well under
/// 3×150ms if they're actually running concurrently rather than one
/// after another.
#[test]
fn run_executes_all_when_ran_strands_concurrently() {
    let mac = Macro {
        id: "m".into(),
        name: "Concurrent".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                when_ran_strand("a", 150.0),
                when_ran_strand("b", 150.0),
                when_ran_strand("c", 150.0),
                Strand {
                    id: "inert".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![Instruction::new(InstructionKind::Wait(Value::number(
                        150.0,
                    )))],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run(noop_emulator(), None, 1.0, empty_vars());
    assert!(
        start.elapsed() < Duration::from_millis(400),
        "entry strands ran sequentially instead of concurrently"
    );
}

/// `run_with_offset` should backdate the first `Wait`'s deadline anchor
/// by `initial_offset` - a macro whose only strand waits 2000ms should
/// return in roughly (2000ms - offset) wall-clock time, proving the
/// anchor is `Instant::now() - initial_offset`, not just `Instant::now()`
/// with the offset silently ignored.
// A 1500ms separation distinguishes an ignored offset while allowing
// scheduler jitter on shared CI runners; the expected wait is 500ms.
#[test]
fn run_with_offset_backdates_the_first_wait_deadline() {
    let mac = Macro {
        id: "m".into(),
        name: "Offset".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![when_ran_strand("a", 2000.0)],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run_with_offset(
        noop_emulator(),
        None,
        1.0,
        empty_vars(),
        Duration::from_millis(1500),
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(1200),
        "offset didn't shorten the wait as expected: {elapsed:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(250),
        "returned suspiciously fast, offset may have overshot: {elapsed:?}"
    );
}

/// A stop flag flipped false mid-run should cut every concurrently
/// running entry strand short, not just one of them.
#[test]
fn stop_flag_stops_every_concurrent_strand() {
    let long_wait = 5_000.0;
    let mac = Macro {
        id: "m".into(),
        name: "Stoppable".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                when_ran_strand("a", long_wait),
                when_ran_strand("b", long_wait),
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let stop_flag = Arc::new(Mutex::new(true));
    let flag_clone = Arc::clone(&stop_flag);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        *flag_clone.lock().unwrap() = false;
    });
    let start = Instant::now();
    mac.run(noop_emulator(), Some(stop_flag), 1.0, empty_vars());
    assert!(
        start.elapsed() < Duration::from_millis(1000),
        "stop flag didn't stop both concurrent strands promptly"
    );
}

#[test]
fn set_variable_writes_evaluated_value() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::number(5.0),
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn change_variable_adds_to_existing_numeric_value() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(10.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::ChangeVariable(
            "x".to_string(),
            Value::number(5.0),
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(15.0))
    );
}

#[test]
fn change_variable_coerces_non_numeric_current_value_to_zero() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Text("hello".to_string()));
    run_instructions(
        vec![Instruction::new(InstructionKind::ChangeVariable(
            "x".to_string(),
            Value::number(5.0),
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn change_variable_with_text_delta_is_a_no_op() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(10.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::ChangeVariable(
            "x".to_string(),
            Value::Text {
                value: "abc".to_string(),
            },
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(10.0))
    );
}

#[test]
fn set_variable_value_can_read_other_variables() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("y".to_string(), Evaluated::Number(7.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::SetVariable(
            "x".to_string(),
            Value::Var {
                name: "y".to_string(),
            },
        ))],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
}

/// A macro with one custom "double" block (`Return param*2`) called via
/// `Value::Call` should write the doubled result.
fn macro_with_double_block(caller_instructions: Vec<Instruction>) -> Macro {
    let block_id = "double".to_string();
    Macro {
        id: "m".into(),
        name: "Double".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: {
                        let mut ins = vec![Instruction::new(InstructionKind::WhenRan)];
                        ins.extend(caller_instructions);
                        ins
                    },
                },
                Strand {
                    id: "double_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Op {
                            op: crate::input::value::Op::Mul,
                            args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![
                    BlockPiece::Label {
                        id: "p1".into(),
                        text: "double".into(),
                    },
                    BlockPiece::Input {
                        component: false,
                        id: "p2".into(),
                        name: "n".into(),
                        value_type: InputValueType::Any,
                        default_value: None,
                        multiline: false,
                    },
                ],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    }
}

#[test]
fn custom_block_runs_the_callback_supplied_to_its_branch() {
    let vars = empty_vars();
    let block_id = "with_branch".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Branches".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::CallBlock {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![vec![Instruction::new(
                                InstructionKind::SetVariable(
                                    "ran".into(),
                                    Value::number(1.0),
                                ),
                            )]],
                        }),
                    ],
                },
                Strand {
                    id: "body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::RunBranch("callback".into())),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![BlockPiece::Branch {
                    id: "b1".into(),
                    name: "callback".into(),
                }],
                shape: BlockShape::Normal,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("ran"),
        Some(&Evaluated::Number(1.0))
    );
}

#[test]
fn pack_state_is_isolated_persistent_and_available_to_reporters_and_triggers() {
    use crate::block_packs::{self, PackInfo, VariableInterface, VariableRole};
    use crate::macros::{BlockPiece, BlockShape, VariableDef};
    let mut mac = Macro::new("user".into(), String::new(), vec![]);
    mac.graph.variables.push(VariableDef { name: "counter".into(), value: Evaluated::Number(99.0) });
    let mut calls = Vec::new();
    for name in ["first", "second"] {
        let mut author = Macro::new("author".into(), String::new(), vec![]);
        author.graph.variables.push(VariableDef { name: "counter".into(), value: Evaluated::Number(5.0) });
        let block = author.create_block(vec![BlockPiece::Label { id: "label".into(), text: name.into() }], BlockShape::Normal, "#4C97FF".into(), 0, 0);
        author.graph.strands.iter_mut().find(|s| s.block_header_id() == Some(block.as_str())).unwrap().instructions.push(Instruction::new(InstructionKind::ChangeVariable("counter".into(), Value::number(1.0))));
        let mut pack = block_packs::pack_from_macro(&author, &[block.clone()], PackInfo { id: name.into(), name: name.into(), ..Default::default() }).unwrap();
        pack.variable_roles.push(VariableInterface { name: "counter".into(), role: VariableRole::Output });
        block_packs::merge_into(&mut mac, &pack, "");
        calls.push(Instruction::new(InstructionKind::CallBlock { block_id: block, args: vec![], branches: vec![] }));
    }
    let first = mac.settings.block_packs[0].clone();
    let second = mac.settings.block_packs[1].clone();
    let reporter = Value::Call { block_id: first.output_id("counter"), args: vec![], branches: vec![], saved: Box::new(Value::number(0.0)) };
    let mut body = calls.clone();
    body.push(Instruction::new(InstructionKind::SetVariable("result".into(), reporter)));
    mac.graph.strands.push(Strand::with_instructions(0, 0, [vec![Instruction::new(InstructionKind::WhenRan)], body.clone()].concat()));
    let vars = Arc::new(Mutex::new(mac.variable_values()));
    mac.clone().run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    {
        let values = vars.lock().unwrap();
        assert_eq!(values.get("counter"), Some(&Evaluated::Number(99.0)));
        assert_eq!(values.get("result"), Some(&Evaluated::Number(6.0)));
        assert_eq!(values.get(&first.variable_key("counter")), Some(&Evaluated::Number(6.0)));
        assert_eq!(values.get(&second.variable_key("counter")), Some(&Evaluated::Number(6.0)));
        mac.sync_variables_from(&values);
    }
    let (loaded, _) = crate::macros::migrate::from_value(crate::macros::migrate::to_value(&mac).unwrap()).unwrap();
    let triggered = empty_vars();
    run_fired_strand(crate::run_log::Source::default(), body, noop_emulator(), None, 1.0, Arc::clone(&triggered),
        Arc::new(Mutex::new(HashMap::new())), loaded.block_table(), 0);
    assert_eq!(triggered.lock().unwrap().get("result"), Some(&Evaluated::Number(7.0)));
}

#[test]
fn shipped_text_helpers_pack_runs() {
    crate::input::value::register_blockwork_operators();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../blockwork-core/tests/fixtures/blocks/packs/text-helpers.json");
    let pack = crate::block_packs::read_pack(&std::fs::read_to_string(path).unwrap()).unwrap();
    let id = |name: &str| {
        pack.content.block_defs.iter().find(|b| crate::block_packs::block_name(b) == name).unwrap().id.clone()
    };
    let call = |name: &str, text: &str| Value::Call {
        block_id: id(name),
        args: vec![Value::text(text)],
        branches: vec![],
        saved: Box::new(Value::number(0.0)),
    };
    let set = |var: &str, value: Value| Instruction::new(InstructionKind::SetVariable(var.to_string(), value));
    let mut mac = Macro::new("t".into(), String::new(), vec![
        set("cap", call("capitalize (text)", "hello")),
        set("empty", call("capitalize (text)", "")),
        set("last", call("last letter of (text)", "abc")),
        set("blank", call("(text) is empty?", "  ")),
    ]);
    crate::block_packs::merge_into(&mut mac, &pack, "");
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("cap"), Some(&Evaluated::Text("Hello".into())));
    assert_eq!(vars.get("empty"), Some(&Evaluated::Text(String::new())));
    assert_eq!(vars.get("last"), Some(&Evaluated::Text("c".into())));
    assert_eq!(vars.get("blank"), Some(&Evaluated::Bool(true)));
}

#[test]
fn value_call_runs_its_supplied_branch() {
    let mut mac = macro_with_double_block(vec![Instruction::new(InstructionKind::SetVariable(
        "result".into(),
        Value::Call {
            block_id: "double".into(), args: vec![Value::number(21.0)],
            branches: vec![vec![serde_json::to_value(crate::frontend::instruction_to_dto(&Instruction::new(
                InstructionKind::SetVariable("branch_ran".into(), Value::number(7.0)),
            ))).unwrap()]],
            saved: Box::new(Value::number(0.0)),
        },
    ))]);
    mac.block_defs[0].pieces.push(BlockPiece::Branch { id: "callback".into(), name: "callback".into() });
    let body = mac.strands.iter_mut().find(|s| s.instructions.first().is_some_and(|i| matches!(&i.kind, InstructionKind::BlockHeader(_)))).unwrap();
    body.instructions.insert(1, Instruction::new(InstructionKind::RunBranch("callback".into())));
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("branch_ran"), Some(&Evaluated::Number(7.0)));
    assert_eq!(vars.get("result"), Some(&Evaluated::Number(42.0)));
}

#[test]
fn custom_components_bind_their_enabled_parameter_in_commands_and_reporters() {
    for reporter in [false, true] {
        for enabled in [false, true] {
            let vars = empty_vars();
            let args = vec![Evaluated::Bool(enabled).into_value(), Value::number(21.0)];
            let call = if reporter {
                InstructionKind::SetVariable("x".into(), Value::Call {
                    block_id: "double".into(), args, branches: vec![], saved: Box::new(Value::number(0.0))
                })
            } else {
                InstructionKind::CallBlock { block_id: "double".into(), args, branches: vec![] }
            };
            let mut mac = macro_with_double_block(vec![Instruction::new(call)]);
            mac.graph.block_defs[0].pieces = serde_json::from_value(serde_json::json!([
                {"kind":"Label","id":"name","text":"example"},
                {"kind":"Input","id":"enabled","name":"optional","value_type":"Bool","component":true},
                {"kind":"Input","id":"value","name":"n"}
            ])).unwrap();
            mac.graph.block_defs[0].shape = if reporter { BlockShape::ReturnsValue } else { BlockShape::Normal };
            let result = |value| Instruction::new(if reporter {
                InstructionKind::Return(value)
            } else {
                InstructionKind::SetVariable("x".into(), value)
            });
            mac.graph.strands[1].instructions.truncate(1);
            mac.graph.strands[1].instructions.push(Instruction::new(InstructionKind::IfElse {
                condition: Value::Param { name: "optional".into() },
                then_body: vec![result(Value::Param { name: "n".into() })],
                else_body: vec![result(Value::number(7.0))],
            }));
            let (mac, _) = crate::macros::migrate::from_value(crate::macros::migrate::to_value(&mac).unwrap()).unwrap();
            mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
            assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(if enabled { 21.0 } else { 7.0 })));
        }
    }
}

#[test]
fn value_call_to_reporter_block_resolves_and_runs() {
    let vars = empty_vars();
    let mac = macro_with_double_block(vec![Instruction::new(InstructionKind::SetVariable(
        "x".to_string(),
        Value::Call {
            block_id: "double".to_string(),
            args: vec![Value::number(21.0)],
            branches: vec![],
            saved: Box::new(Value::number(0.0)),
        },
    ))]);
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(42.0))
    );
}

/// A reporter block whose body never hits `Return` should leave
/// the caller's `SetVariable` un-applied rather than panicking or
/// defaulting to some value.
#[test]
fn value_call_to_block_without_return_leaves_variable_unset() {
    let vars = empty_vars();
    let block_id = "empty".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Empty".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "empty_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![Instruction::new(InstructionKind::BlockHeader(
                        block_id.clone(),
                    ))],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

/// A `CallBlock` with real `Wait`s inside its body should take real
/// time, proving the callee's instructions genuinely execute inline.
#[test]
fn call_block_runs_real_instructions_including_wait() {
    let block_id = "waiter".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Waiter".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::CallBlock {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![],
                        }),
                    ],
                },
                Strand {
                    id: "waiter_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Wait(Value::number(150.0))),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::Normal,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run(noop_emulator(), None, 1.0, empty_vars());
    assert!(
        start.elapsed() >= Duration::from_millis(140),
        "CallBlock's nested Wait didn't actually take real time"
    );
}

/// A watcher-fired trigger body can call the macro's custom blocks.
#[test]
fn fired_strand_can_call_custom_block() {
    let block_id = "setter".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Fired".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "trigger".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenClipboardChanged),
                        Instruction::new(InstructionKind::CallBlock {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![],
                        }),
                    ],
                },
                Strand {
                    id: "setter_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::number(1.0),
                        )),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::Normal,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let vars = empty_vars();
    let body = mac.strands[0].instructions[1..].to_vec();
    run_fired_strand(
        crate::run_log::Source::app("Fired"),
        body,
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        mac.block_table(),
        0,
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
}

/// A block that calls itself should error past `MAX_CALL_DEPTH` instead
/// of overflowing the stack.
#[test]
fn self_recursive_reporter_block_errs_past_max_depth() {
    let vars = empty_vars();
    let block_id = "loop".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Loop".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "loop_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Call {
                            block_id: block_id.clone(),
                            args: vec![],
                            branches: vec![],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    // Should return promptly (erroring out at MAX_CALL_DEPTH) rather than
    // hanging or crashing the test process via a stack overflow.
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

/// One reporter block calling another in its own `Return` expression
/// should compose correctly, proving multi-level `Value::Call`
/// resolution nests rather than working one level deep.
#[test]
fn reporter_block_composes_with_another_reporter_block() {
    let vars = empty_vars();
    let double_id = "double".to_string();
    let triple_id = "triple".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "Compose".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: triple_id.clone(),
                                args: vec![Value::number(2.0)],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "double_body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(double_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Op {
                            op: crate::input::value::Op::Mul,
                            args: vec![Value::Param { name: "n".into() }, Value::number(2.0)],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
                Strand {
                    id: "triple_body".into(),
                    x: 0,
                    y: 0,
                    // triple(n) = double(n) + n  =>  triple(2) = 4 + 2 = 6
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(triple_id.clone())),
                        Instruction::new(InstructionKind::Return(Value::Op {
                            op: crate::input::value::Op::Add,
                            args: vec![
                                Value::Call {
                                    block_id: double_id.clone(),
                                    args: vec![Value::Param { name: "n".into() }],
                                    branches: vec![],
                                    saved: Box::new(Value::number(0.0)),
                                },
                                Value::Param { name: "n".into() },
                            ],
                            saved: Box::new(Value::number(0.0)),
                        })),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![
                BlockDef {
                    id: double_id,
                    pieces: vec![BlockPiece::Input {
                        component: false,
                        id: "p1".into(),
                        name: "n".into(),
                        value_type: InputValueType::Any,
                        default_value: None,
                        multiline: false,
                    }],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                identifier: String::new(),
                description: String::new(),
                },
                BlockDef {
                    id: triple_id,
                    pieces: vec![BlockPiece::Input {
                        component: false,
                        id: "p1".into(),
                        name: "n".into(),
                        value_type: InputValueType::Any,
                        default_value: None,
                        multiline: false,
                    }],
                    shape: BlockShape::ReturnsValue,
                    color: default_block_color(),
                    icon: String::new(),
                identifier: String::new(),
                description: String::new(),
                },
            ],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(6.0)));
}

fn true_cond() -> Value {
    Value::Op {
        op: crate::input::value::Op::True,
        args: vec![],
        saved: Box::new(Value::number(0.0)),
    }
}
fn false_cond() -> Value {
    Value::Op {
        op: crate::input::value::Op::False,
        args: vec![],
        saved: Box::new(Value::number(0.0)),
    }
}

fn set(name: &str, value: Value) -> Instruction {
    Instruction::new(InstructionKind::SetVariable(name.to_string(), value))
}

/// Fails to evaluate: text isn't a number.
fn bad_number() -> Value {
    Value::op(Op::Add, vec![Value::text("abc"), Value::number(1.0)])
}

#[test]
fn try_catch_runs_catch_body_with_the_error_message() {
    crate::input::value::register_blockwork_operators();
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::TryCatch {
            body: vec![
                set("a", Value::number(1.0)),
                Instruction::new(InstructionKind::ThrowError(Value::text("boom"))),
                set("a", Value::number(2.0)),
            ],
            catch_body: vec![set("e", Value::op(Op::from_name("LastError"), vec![]))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("a"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("e"), Some(&Evaluated::Text("boom".into())));
}

#[test]
fn failing_block_is_caught_inside_try_and_skipped_outside() {
    let vars = empty_vars();
    run_instructions(
        vec![
            set("y", bad_number()),
            set("after_skip", Value::number(1.0)),
            Instruction::new(InstructionKind::TryCatch {
                body: vec![set("y", bad_number()), set("after_error", Value::number(1.0))],
                catch_body: vec![set("caught", Value::number(1.0))],
            }),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("after_skip"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("after_error"), None);
    assert_eq!(vars.get("caught"), Some(&Evaluated::Number(1.0)));
}

fn bump(name: &str) -> Instruction {
    Instruction::new(InstructionKind::ChangeVariable(name.to_string(), Value::number(1.0)))
}

#[test]
fn retry_runs_the_body_again_until_attempts_run_out() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::TryCatch {
                body: vec![Instruction::new(InstructionKind::Retry {
                    count: Value::number(3.0),
                    delay: Value::number(0.0),
                    body: vec![bump("tries"), Instruction::new(InstructionKind::ThrowError(Value::text("nope")))],
                })],
                catch_body: vec![set("caught", Value::number(1.0))],
            }),
            Instruction::new(InstructionKind::Retry {
                count: Value::number(5.0),
                delay: Value::number(0.0),
                body: vec![bump("ok")],
            }),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("tries"), Some(&Evaluated::Number(3.0)));
    assert_eq!(vars.get("caught"), Some(&Evaluated::Number(1.0)), "giving up is an error a try catches");
    assert_eq!(vars.get("ok"), Some(&Evaluated::Number(1.0)), "a body that works runs once");
}

#[test]
fn loop_limit_stops_a_runaway_loop() {
    let vars = empty_vars();
    run_fired_strand(
        crate::run_log::Source::default(),
        vec![
            Instruction::new(InstructionKind::Forever { body: vec![bump("n")] }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        10,
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("n"), Some(&Evaluated::Number(10.0)));
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)), "outside a try the strand carries on");
}

#[test]
fn run_macro_waits_for_the_other_run_to_end() {
    struct Host;
    impl crate::macros::calls::MacroHost for Host {
        fn find(&self, name: &str) -> Option<String> {
            (name == "Helper").then(|| "helper-id".to_string())
        }
        fn start(&self, _id: &str) -> Result<Arc<AtomicBool>, String> {
            let done = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&done);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(60));
                flag.store(true, Ordering::Relaxed);
            });
            Ok(done)
        }
    }
    crate::macros::calls::set_host(Arc::new(Host));
    let vars = empty_vars();
    let started = Instant::now();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::RunMacro { target: Value::text("Nope"), wait: true }),
            Instruction::new(InstructionKind::RunMacro { target: Value::text("Helper"), wait: true }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert!(started.elapsed() >= Duration::from_millis(60), "waited for the helper");
    assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)), "an unknown macro is a soft error");
}

/// Answers dialogs by their message: "cancel" cancels, permissions are denied.
fn install_test_dialogs() {
    use crate::prompt::{Kind, Reply};
    crate::prompt::set_handler(Arc::new(|request: &crate::prompt::Request, _stopped: &dyn Fn() -> bool| {
        let reply = |answer: &str, text: &str| Ok(Reply { answer: answer.into(), text: text.into() });
        match request.kind {
            _ if request.message == "cancel" => reply("cancel", ""),
            Kind::Text => reply("ok", &format!("{}!", request.default)),
            Kind::Confirm => reply("yes", ""),
            Kind::Choice => reply("ok", &request.options[1]),
            Kind::Message => reply("ok", ""),
            Kind::Permission => reply("deny", ""),
        }
    }));
}

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
            Instruction::new(InstructionKind::AskText { prompt: Value::text("Name?"), default: Value::text("Ada"), name: "name".into() }),
            Instruction::new(InstructionKind::AskYesNo { prompt: Value::text("Sure?"), name: "sure".into() }),
            Instruction::new(InstructionKind::AskChoice { prompt: Value::text("Pick"), list: "colors".into(), name: "pick".into() }),
            Instruction::new(InstructionKind::ShowMessage(Value::text("Done"))),
            Instruction::new(InstructionKind::AskText { prompt: Value::text("cancel"), default: Value::text(""), name: "never".into() }),
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
fn signal_expressions_can_use_unregistered_computed_names() {
    let mut mac = Macro::new(
        "Dynamic signals".into(),
        String::new(),
        vec![
            set("target", Value::text("task:42")),
            Instruction::new(InstructionKind::SendSignal {
                name: Value::op(Op::Join, vec![Value::text("task:"), Value::text("42")]).into(),
                wait: true,
            }),
            set("seen", Value::Var { name: "received".into() }),
        ],
    );
    mac.graph.strands.push(Strand::with_instructions(0, 200, vec![
        Instruction::new(InstructionKind::WhenSignal(Value::Var { name: "target".into() }.into())),
        set("received", Value::number(1.0)),
    ]));
    mac.collect_named_signals();
    assert!(mac.signals.is_empty());
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("seen"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn send_signal_and_wait_runs_the_receivers_first() {
    let mut mac = Macro::new(
        "Signals".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::SendSignal { name: Value::text("Go").into(), wait: true }),
            set("seen", Value::Var { name: "x".into() }),
        ],
    );
    mac.graph.strands.push(Strand::with_instructions(0, 200, vec![
        Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
        Instruction::new(InstructionKind::Wait(Value::number(40.0))),
        set("x", Value::number(5.0)),
    ]));
    mac.graph.strands.push(Strand::with_instructions(0, 400, vec![
        Instruction::new(InstructionKind::WhenSignal(Value::text("other").into())),
        set("wrong", Value::number(1.0)),
    ]));
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("seen"), Some(&Evaluated::Number(5.0)));
    assert_eq!(vars.get("wrong"), None);
}

#[test]
fn run_strand_runs_one_strand_whatever_heads_it() {
    let mut mac = Macro::new("One strand".into(), String::new(), vec![set("main", Value::number(1.0))]);
    let mut keyed = Strand::with_instructions(0, 200, vec![
        Instruction::new(InstructionKind::WhenIdle(Value::number(5.0))),
        Instruction::new(InstructionKind::SendSignal { name: Value::text("go").into(), wait: true }),
        set("keyed", Value::Var { name: "x".into() }),
    ]);
    keyed.id = "keyed".into();
    mac.graph.strands.push(keyed);
    mac.graph.strands.push(Strand::with_instructions(0, 400, vec![
        Instruction::new(InstructionKind::WhenSignal(Value::text("go").into())),
        set("x", Value::number(7.0)),
    ]));
    assert!(!mac.can_run_strand("missing"));
    let vars = empty_vars();
    let lists: ListStore = Arc::new(Mutex::new(HashMap::new()));
    assert!(mac.run_strand_with_lists("keyed", noop_emulator(), None, 1.0, Arc::clone(&vars), lists));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("keyed"), Some(&Evaluated::Number(7.0)));
    assert_eq!(vars.get("main"), None);
}

#[test]
fn globals_accept_variable_and_computed_names() {
    crate::input::value::register_blockwork_operators();
    let name = "runner-dynamic-global-picker";
    crate::globals::remove(name);
    crate::globals::create(name).unwrap();
    let dynamic = Value::Var { name: "target".into() };
    let computed = Value::op(Op::from_name("Join"), vec![Value::text("runner-dynamic-"), Value::text("global-picker")]);
    let mac = Macro::new(
        "Dynamic globals".into(),
        String::new(),
        vec![
            set("target", Value::text(name)),
            set("initial", Value::op(Op::from_name("GlobalVariable"), vec![dynamic.clone()])),
            Instruction::new(InstructionKind::SetGlobal { name: dynamic.clone(), value: Value::number(4.0) }),
            Instruction::new(InstructionKind::ChangeGlobal { name: computed, by: Value::number(3.0) }),
            set("result", Value::op(Op::from_name("GlobalVariable"), vec![dynamic])),
        ],
    );
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("initial"), Some(&Evaluated::Text(String::new())));
    assert_eq!(vars.get("result"), Some(&Evaluated::Number(7.0)));
    crate::globals::remove(name);
}

#[cfg(unix)]
#[test]
fn run_command_moves_on_and_run_command_and_wait_waits() {
    let dir = std::env::temp_dir().join(format!("bw-command-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let run = |kind: InstructionKind| {
        Macro::new("Commands".into(), String::new(), vec![Instruction::new(kind)]).run(noop_emulator(), None, 1.0, empty_vars());
    };
    let (started, waited) = (dir.join("started"), dir.join("waited"));
    run(InstructionKind::Command(format!("sleep 1; touch '{}'", started.display())));
    assert!(!started.exists(), "run command didn't wait");
    run(InstructionKind::CommandWait(format!("sleep 1; touch '{}'", waited.display())));
    assert!(waited.exists(), "run command and wait waited");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !started.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(started.exists(), "the started command still ran");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restricted_macros_skip_blocks_they_may_not_run() {
    install_test_dialogs();
    let marker = std::env::temp_dir().join(format!("bw-perm-{}", std::process::id()));
    let mut mac = Macro::new(
        "Restricted".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::CommandWait(format!("touch '{}'", marker.display()))),
            set("after", Value::number(1.0)),
        ],
    );
    mac.id = "runner-restricted".into();
    crate::permissions::restrict(&mac.id, Vec::new());
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    crate::permissions::trust("runner-restricted");
    assert!(!marker.exists(), "the denied command didn't run");
    assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn restricted_macros_ask_before_globals_files_and_input() {
    install_test_dialogs();
    let read = |op: &str, arg: &str| Value::op(Op::from_name(op), vec![Value::text(arg)]);
    let mut mac = Macro::new(
        "Restricted reads".into(),
        String::new(),
        vec![
            Instruction::new(InstructionKind::SetGlobal { name: Value::text("runner-restricted-global"), value: Value::number(1.0) }),
            set("file", read("ReadFile", "/etc/hostname")),
            set("global", read("GlobalVariable", "runner-restricted-global")),
            Instruction::new(InstructionKind::RunMacro { target: Value::text("anything"), wait: true }),
            set("after", Value::number(1.0)),
        ],
    );
    mac.id = "runner-restricted-reads".into();
    crate::permissions::restrict(&mac.id, Vec::new());
    let vars = empty_vars();
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    crate::permissions::trust("runner-restricted-reads");
    assert_eq!(crate::globals::get("runner-restricted-global"), Evaluated::Number(0.0));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("file"), None);
    assert_eq!(vars.get("global"), None);
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn wait_until_returns_when_true_and_fails_after_its_timeout() {
    let vars = empty_vars();
    let started = Instant::now();
    let gt = |a: Value, b: Value| Value::op(Op::from_name("Gt"), vec![a, b]);
    run_instructions(
        vec![
            Instruction::new(InstructionKind::WaitUntil { condition: gt(Value::number(2.0), Value::number(1.0)), timeout: None, error_timeout: Some(Value::number(0.0)) }),
            Instruction::new(InstructionKind::TryCatch {
                body: vec![
                    Instruction::new(InstructionKind::WaitUntil { condition: gt(Value::number(1.0), Value::number(2.0)), timeout: None, error_timeout: Some(Value::number(150.0)) }),
                    set("missed", Value::number(1.0)),
                ],
                catch_body: vec![set("timed_out", Value::number(1.0))],
            }),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("timed_out"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("missed"), None);
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(150) && elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn wait_limits_continue_or_end_the_strand_and_can_be_caught() {
    let wait = |timeout: Option<f64>, error_timeout: Option<f64>| Instruction::new(InstructionKind::WaitUntil {
        condition: Value::op(Op::from_name("Gt"), vec![Value::number(0.0), Value::number(1.0)]),
        timeout: timeout.map(Value::number),
        error_timeout: error_timeout.map(Value::number),
    });
    // A normal deadline continues even inside try; an error deadline stops the strand.
    let vars = empty_vars();
    run_instructions(vec![
        Instruction::new(InstructionKind::TryCatch {
            body: vec![wait(Some(1.0), None), set("continued", Value::number(1.0))],
            catch_body: vec![set("caught", Value::number(1.0))],
        }),
        wait(None, Some(1.0)),
        set("after_error", Value::number(1.0)),
    ], noop_emulator(), None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("continued"), Some(&Evaluated::Number(1.0)));
    assert_eq!(vars.get("caught"), None);
    assert_eq!(vars.get("after_error"), None);
    drop(vars);

    // Both deadlines start together. The earlier wins, with errors winning a tie.
    for (timeout, error_timeout, caught) in [(1.0, 1000.0, false), (1000.0, 1.0, true), (1.0, 1.0, true)] {
        let vars = empty_vars();
        run_instructions(vec![Instruction::new(InstructionKind::TryCatch {
            body: vec![wait(Some(timeout), Some(error_timeout)), set("continued", Value::number(1.0))],
            catch_body: vec![set("caught", Value::number(1.0))],
        })], noop_emulator(), None, 1.0, Arc::clone(&vars));
        let vars = vars.lock().unwrap();
        assert_eq!(vars.contains_key("caught"), caught);
        assert_eq!(vars.contains_key("continued"), !caught);
    }
}

#[test]
fn lock_blocks_run_their_body_and_give_up_when_stopped() {
    let lock = |name: &str, body| Instruction::new(InstructionKind::WithLock { name: Value::text(name), body });
    let vars = empty_vars();
    run_instructions(vec![lock("runner-free", vec![set("ran", Value::number(1.0))])], noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("ran"), Some(&Evaluated::Number(1.0)));
    assert!(!crate::macros::locks::is_held("runner-free"));

    // Keep the lock held until the stopped strand has returned. Sleeping
    // for a fixed holder lifetime made cancellation depend on CI scheduling.
    let held = crate::macros::locks::acquire("runner-busy", Duration::from_millis(5), || false).unwrap();
    let stop = Arc::new(Mutex::new(true));
    let worker_stop = Arc::clone(&stop);
    let vars = empty_vars();
    let worker_vars = Arc::clone(&vars);
    let instruction = lock("runner-busy", vec![set("ran", Value::number(1.0))]);
    let block_id = instruction.id.clone();
    let (finished, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        run_fired_strand(
            crate::run_log::Source::for_macro("runner-lock-cancellation", "Lock cancellation"),
            vec![instruction], noop_emulator(), Some(worker_stop), 1.0,
            worker_vars, Arc::new(Mutex::new(HashMap::new())), Arc::new(HashMap::new()), 0,
        );
        let _ = finished.send(());
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let reached_lock = loop {
        if crate::macros::trace::snapshot().iter().any(|active| {
            active.macro_id == "runner-lock-cancellation" && active.block_id == block_id
        }) {
            break true;
        }
        if Instant::now() >= deadline { break false; }
        std::thread::sleep(Duration::from_millis(1));
    };
    *stop.lock().unwrap() = false;
    let cancelled_while_held = completion.recv_timeout(Duration::from_secs(2)).is_ok();
    drop(held);
    worker.join().unwrap();
    assert!(reached_lock, "the strand never reached the held lock");
    assert!(cancelled_while_held, "the strand needed the lock released before it could stop");
    assert_eq!(vars.lock().unwrap().get("ran"), None);
}

#[test]
fn image_matches_are_picked_by_number_or_by_nearness() {
    use crate::screen::ScreenMatch;
    // Reading order: two on the top row, one below; the middle one looks best.
    let matches = [
        ScreenMatch { x: 100.0, y: 50.0, diff: 4.0 },
        ScreenMatch { x: 400.0, y: 50.0, diff: 1.0 },
        ScreenMatch { x: 100.0, y: 300.0, diff: 1.0 },
    ];
    let x_of = |target| pick_match(&matches, target).unwrap().map(|m| (m.x, m.y));
    assert_eq!(x_of(MatchTarget::Best), Some((400.0, 50.0)), "ties go to reading order");
    assert_eq!(x_of(MatchTarget::Nth(1.0)), Some((100.0, 50.0)));
    assert_eq!(x_of(MatchTarget::Nth(3.0)), Some((100.0, 300.0)));
    assert_eq!(x_of(MatchTarget::Nth(-1.0)), Some((100.0, 300.0)), "negative counts from the end");
    assert_eq!(x_of(MatchTarget::Nth(-3.0)), Some((100.0, 50.0)));
    assert_eq!(x_of(MatchTarget::Nth(4.0)), None);
    assert_eq!(x_of(MatchTarget::Nth(-4.0)), None);
    assert!(pick_match(&matches, MatchTarget::Nth(0.0)).is_err());
    assert_eq!(x_of(MatchTarget::Near(120.0, 260.0)), Some((100.0, 300.0)));
    assert_eq!(x_of(MatchTarget::Near(390.0, 0.0)), Some((400.0, 50.0)));
    assert_eq!(pick_match(&[], MatchTarget::Near(0.0, 0.0)).unwrap(), None);
}

/// Every image pick against the real screen, for `$BW_LIVE_IMAGE` (a PNG
/// on screen more than once) near `$BW_LIVE_NEAR` (`x,y`); run by hand
/// with `--ignored`.
#[test]
#[ignore]
fn live_image_picks() {
    struct Moves(Arc<Mutex<Vec<(i32, i32)>>>);
    impl InputBackend for Moves {
        fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> { Ok(()) }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> { Ok(()) }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> { Ok(()) }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
        fn move_mouse_abs(&mut self, x: i32, y: i32) -> Result<(), String> {
            self.0.lock().unwrap().push((x, y));
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> { Ok(()) }
        fn text(&mut self, _: &str) -> Result<(), String> { Ok(()) }
        fn cursor_pos(&self) -> Option<(i32, i32)> { None }
    }
    let image = std::env::var("BW_LIVE_IMAGE").expect("set BW_LIVE_IMAGE to a PNG");
    let near = std::env::var("BW_LIVE_NEAR").unwrap_or_else(|_| "0,0".into());
    let (nx, ny) = near.split_once(',').map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).unwrap();
    let picks = [
        ("best", None, 1.0),
        ("match 1", Some(ImagePick::Nth), 1.0),
        ("match 2", Some(ImagePick::Nth), 2.0),
        ("match -1", Some(ImagePick::Nth), -1.0),
        ("nearest mouse", Some(ImagePick::NearMouse), 1.0),
        ("nearest point", Some(ImagePick::NearPoint), 1.0),
    ];
    for (name, mode, index) in picks {
        let pick = mode.map(|mode| ImageMatch { mode, index: Value::number(index), near_x: Value::number(nx), near_y: Value::number(ny) });
        let vars = empty_vars();
        let moves = Arc::new(Mutex::new(Vec::new()));
        let search = |kind| Instruction::new(kind);
        run_instructions(
            vec![
                search(InstructionKind::FindImage {
                    image: image.clone(),
                    tolerance: Some(Value::number(10.0)),
                    x_variable: "x".into(),
                    y_variable: "y".into(),
                    pick: pick.clone(),
                    region: None,
                    count_variable: Some("count".into()),
                }),
                search(InstructionKind::ClickImage {
                    image: image.clone(),
                    tolerance: Some(Value::number(10.0)),
                    button: MacroButton::Left,
                    pick,
                    region: None,
                }),
            ],
            Arc::new(Mutex::new(Moves(Arc::clone(&moves)))),
            None,
            1.0,
            Arc::clone(&vars),
        );
        let vars = vars.lock().unwrap();
        eprintln!("{name}: x {:?} y {:?} count {:?}, click at {:?}", vars.get("x"), vars.get("y"), vars.get("count"), moves.lock().unwrap());
    }
}

#[test]
fn image_blocks_without_an_image_fail_softly() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::FindImage {
                image: String::new(),
                tolerance: Some(Value::number(5.0)),
                x_variable: "x".into(),
                y_variable: "y".into(),
                pick: None,
                region: None,
                count_variable: None,
            }),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("x"), None);
    assert_eq!(vars.get("after"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn uncaught_throw_ends_the_strand() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::ThrowError(Value::text("stop here"))),
            set("after", Value::number(1.0)),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("after"), None);
}

#[test]
fn run_log_errors_name_the_failing_block() {
    let _log = crate::run_log::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let throw = Instruction::new(InstructionKind::ThrowError(Value::text("boom")));
    let source = crate::run_log::Source::for_macro("blame-test", "Blame");
    run_fired_strand(
        source,
        vec![Instruction::new(InstructionKind::Log(Value::text("hi"))), throw.clone()],
        noop_emulator(),
        None,
        1.0,
        empty_vars(),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        0,
    );
    let errors: Vec<_> = crate::run_log::snapshot()
        .into_iter()
        .filter(|e| e.macro_id == "blame-test" && e.level == crate::run_log::Level::Error)
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].block_id, throw.id);
    assert_eq!(errors[0].message, "boom");
}

#[test]
fn without_input_emulation_other_blocks_run_and_input_reports_once() {
    let _log = crate::run_log::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let text = || Instruction::new(InstructionKind::Token(InputToken::Text(Value::text("hi"))));
    let first = text();
    let vars = empty_vars();
    run_fired_strand(
        crate::run_log::Source::for_macro("no-input-test", "No input"),
        vec![first.clone(), text(), set("after", Value::number(1.0))],
        Arc::new(Mutex::new(crate::macros::backend::UnavailableBackend::new())),
        None,
        1.0,
        Arc::clone(&vars),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(HashMap::new()),
        0,
    );
    assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)));
    let errors: Vec<_> = crate::run_log::snapshot()
        .into_iter()
        .filter(|e| e.macro_id == "no-input-test" && e.level == crate::run_log::Level::Error)
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].block_id, first.id);
    assert!(errors[0].message.contains("input emulation isn't available"), "{}", errors[0].message);
}

#[test]
fn keyboard_backend_failures_are_visible_and_catchable_after_combo_release() {
    let _log = crate::run_log::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    struct RejectKey(Arc<Mutex<Vec<(MacroKey, Direction)>>>);
    impl InputBackend for RejectKey {
        fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
            self.0.lock().unwrap().push((key.clone(), dir));
            if key == MacroKey::F21 { Err("F21 has no native macOS virtual-key code".into()) } else { Ok(()) }
        }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> { Ok(()) }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> { Ok(()) }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
        fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> { Ok(()) }
        fn text(&mut self, _: &str) -> Result<(), String> { Ok(()) }
        fn cursor_pos(&self) -> Option<(i32, i32)> { None }
    }
    let events = Arc::new(Mutex::new(Vec::new()));
    let backend: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(RejectKey(Arc::clone(&events))));
    let input = Instruction::new(InstructionKind::Token(InputToken::Key(Value::text("Control+F21"), Direction::Click)));
    let vars = empty_vars();
    run_fired_strand(
        crate::run_log::Source::for_macro("keyboard-error-regression", "Keyboard error"),
        vec![input.clone(), set("after", Value::number(1.0))], Arc::clone(&backend), None, 1.0,
        Arc::clone(&vars), Arc::new(Mutex::new(HashMap::new())), Arc::new(HashMap::new()), 0,
    );
    assert_eq!(vars.lock().unwrap().get("after"), Some(&Evaluated::Number(1.0)));
    let errors: Vec<_> = crate::run_log::snapshot().into_iter()
        .filter(|e| e.macro_id == "keyboard-error-regression" && e.level == crate::run_log::Level::Error).collect();
    assert_eq!(errors.len(), 1, "one visible block error is required");
    assert_eq!(errors[0].block_id, input.id);
    assert!(errors[0].message.contains("F21 has no native macOS virtual-key code"));
    assert_eq!(events.lock().unwrap().last(), Some(&(MacroKey::Control, Direction::Release)));
    crate::input::value::register_blockwork_operators();
    run_instructions(vec![Instruction::new(InstructionKind::TryCatch {
        body: vec![input, set("missed", Value::number(1.0))],
        catch_body: vec![set("caught", Value::op(Op::from_name("LastError"), vec![]))],
    })], backend, None, 1.0, Arc::clone(&vars));
    let vars = vars.lock().unwrap();
    assert_eq!(vars.get("missed"), None);
    assert!(matches!(vars.get("caught"), Some(Evaluated::Text(e)) if e.contains("F21 has no native macOS virtual-key code")));
    assert_eq!(events.lock().unwrap().last(), Some(&(MacroKey::Control, Direction::Release)));
}

#[test]
fn combo_letters_dont_add_shift() {
    #[derive(Default)]
    struct Keys(Arc<Mutex<Vec<(MacroKey, Direction)>>>);
    impl InputBackend for Keys {
        fn key(&mut self, key: MacroKey, dir: Direction) -> Result<(), String> {
            self.0.lock().unwrap().push((key, dir));
            Ok(())
        }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> { Ok(()) }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> { Ok(()) }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
        fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> { Ok(()) }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> { Ok(()) }
        fn text(&mut self, _: &str) -> Result<(), String> { Ok(()) }
        fn cursor_pos(&self) -> Option<(i32, i32)> { None }
    }
    let keys = Keys::default();
    let seen = Arc::clone(&keys.0);
    run_instructions(
        vec![Instruction::new(InstructionKind::Token(InputToken::Key(Value::text("Control+A"), Direction::Click)))],
        Arc::new(Mutex::new(keys)),
        None,
        1.0,
        empty_vars(),
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[1], (MacroKey::Unicode('a'), Direction::Press));
}

#[test]
fn stop_macro_stops_every_strand_of_the_run() {
    let mut stopper = when_ran_strand("b", 0.0);
    stopper.instructions.push(Instruction::new(InstructionKind::StopMacro));
    let mac = Macro {
        id: "m".into(),
        name: "Stops itself".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![when_ran_strand("a", 5_000.0), stopper],
            ..MacroGraph::new()
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    let start = Instant::now();
    mac.run(noop_emulator(), None, 1.0, empty_vars());
    assert!(start.elapsed() < Duration::from_millis(1000), "stop macro didn't end the waiting strand");
}

#[test]
fn if_runs_body_when_condition_true() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: true_cond(),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn if_skips_body_when_condition_false() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: false_cond(),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

#[test]
fn if_else_runs_the_matching_branch() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::IfElse {
            condition: true_cond(),
            then_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
            else_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(2.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));

    let vars2 = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::IfElse {
            condition: false_cond(),
            then_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
            else_body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(2.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars2),
    );
    assert_eq!(
        vars2.lock().unwrap().get("x"),
        Some(&Evaluated::Number(2.0))
    );
}

#[test]
fn nested_if_inside_if_runs_only_when_both_true() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: true_cond(),
            body: vec![Instruction::new(InstructionKind::If {
                condition: true_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));

    let vars2 = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::If {
            condition: true_cond(),
            body: vec![Instruction::new(InstructionKind::If {
                condition: false_cond(),
                body: vec![Instruction::new(InstructionKind::SetVariable(
                    "x".to_string(),
                    Value::number(1.0),
                ))],
            })],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars2),
    );
    assert_eq!(vars2.lock().unwrap().get("x"), None);
}

/// A `Return` inside an `If` branch, within a custom block called via
/// `Value::Call`, should halt the block's body and hand its value back
/// to the caller - proving `run_block`'s `Ok(Some(_))` bubbles up
/// through nested `If` the same way it already does through `CallBlock`.
#[test]
fn return_inside_if_branch_bubbles_up_through_reporter_block() {
    let vars = empty_vars();
    let block_id = "cond_return".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "CondReturn".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::If {
                            condition: true_cond(),
                            body: vec![Instruction::new(InstructionKind::Return(Value::number(
                                42.0,
                            )))],
                        }),
                        // Never reached if the branch's Return correctly halted the body.
                        Instruction::new(InstructionKind::Return(Value::number(0.0))),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(
        vars.lock().unwrap().get("x"),
        Some(&Evaluated::Number(42.0))
    );
}

#[test]
fn repeat_runs_body_n_times() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(5.0),
            body: vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn repeat_with_zero_or_negative_count_never_runs_body() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(-3.0),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

#[test]
fn escape_loop_stops_repeat_early() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(10.0),
            body: vec![
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::If {
                    condition: true_cond(),
                    body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                }),
            ],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(1.0)));
}

#[test]
fn continue_loop_skips_rest_of_iteration_but_keeps_looping() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::Repeat {
            count: Value::number(3.0),
            body: vec![
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::ContinueLoop),
                // Never reached - proves ContinueLoop halted this iteration.
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(100.0),
                )),
            ],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(3.0)));
}

#[test]
fn while_loop_runs_until_condition_goes_false() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(0.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::While {
            condition: Value::Op {
                op: crate::input::value::Op::Lt,
                args: vec![
                    Value::Var {
                        name: "x".to_string(),
                    },
                    Value::number(5.0),
                ],
                saved: Box::new(Value::Bool),
            },
            body: vec![Instruction::new(InstructionKind::ChangeVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(5.0)));
}

#[test]
fn while_loop_with_false_condition_never_runs_body() {
    let vars = empty_vars();
    run_instructions(
        vec![Instruction::new(InstructionKind::While {
            condition: false_cond(),
            body: vec![Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            ))],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), None);
}

#[test]
fn forever_loop_runs_until_escape_loop() {
    let vars = empty_vars();
    vars.lock()
        .unwrap()
        .insert("x".to_string(), Evaluated::Number(0.0));
    run_instructions(
        vec![Instruction::new(InstructionKind::Forever {
            body: vec![
                Instruction::new(InstructionKind::ChangeVariable(
                    "x".to_string(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::IfElse {
                    condition: Value::Op {
                        op: crate::input::value::Op::Gte,
                        args: vec![
                            Value::Var {
                                name: "x".to_string(),
                            },
                            Value::number(3.0),
                        ],
                        saved: Box::new(Value::Bool),
                    },
                    then_body: vec![Instruction::new(InstructionKind::EscapeLoop)],
                    else_body: vec![],
                }),
            ],
        })],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(3.0)));
}

/// A `Forever` loop with no `Break`/`Return` must still be interruptible
/// via the external stop flag rather than spinning the thread forever.
#[test]
fn forever_loop_is_interrupted_by_stop_flag() {
    let stop_flag = Arc::new(Mutex::new(true));
    let flag_clone = Arc::clone(&stop_flag);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        *flag_clone.lock().unwrap() = false;
    });
    let start = Instant::now();
    run_instructions(
        vec![Instruction::new(InstructionKind::Forever { body: vec![] })],
        noop_emulator(),
        Some(stop_flag),
        1.0,
        empty_vars(),
    );
    assert!(
        start.elapsed() < Duration::from_millis(1000),
        "forever loop wasn't stopped by the stop flag"
    );
}

/// `Return` inside a `Repeat` must unwind straight through the loop,
/// same as it already does through `If` - proving loop instructions
/// forward `Flow::Return` rather than swallowing it like `Break`.
#[test]
fn return_inside_repeat_bubbles_up_through_reporter_block() {
    let vars = empty_vars();
    let block_id = "loop_return".to_string();
    let mac = Macro {
        id: "m".into(),
        name: "LoopReturn".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![
                Strand {
                    id: "caller".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::WhenRan),
                        Instruction::new(InstructionKind::SetVariable(
                            "x".to_string(),
                            Value::Call {
                                block_id: block_id.clone(),
                                args: vec![],
                                branches: vec![],
                                saved: Box::new(Value::number(0.0)),
                            },
                        )),
                    ],
                },
                Strand {
                    id: "body".into(),
                    x: 0,
                    y: 0,
                    instructions: vec![
                        Instruction::new(InstructionKind::BlockHeader(block_id.clone())),
                        Instruction::new(InstructionKind::Repeat {
                            count: Value::number(10.0),
                            body: vec![Instruction::new(InstructionKind::Return(Value::number(
                                7.0,
                            )))],
                        }),
                        // Never reached if Return correctly halted the loop and the body.
                        Instruction::new(InstructionKind::Return(Value::number(0.0))),
                    ],
                },
            ],
            floating_values: vec![],
            comments: vec![],
            variables: vec![],
            block_defs: vec![BlockDef {
                id: block_id,
                pieces: vec![],
                shape: BlockShape::ReturnsValue,
                color: default_block_color(),
                icon: String::new(),
                identifier: String::new(),
                description: String::new(),
            }],
            lists: vec![],
            dicts: vec![],
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: crate::macros::MacroSettings::default(),
    };
    mac.run(noop_emulator(), None, 1.0, Arc::clone(&vars));
    assert_eq!(vars.lock().unwrap().get("x"), Some(&Evaluated::Number(7.0)));
}

/// `EscapeLoop`/`ContinueLoop` with no enclosing loop at all (a malformed
/// path, since the UI itself refuses to place one there) should be a
/// harmless no-op, mirroring how a stray `Return` at strand level today
/// is silently discarded.
#[test]
fn escape_loop_with_no_enclosing_loop_is_a_harmless_no_op() {
    let vars = empty_vars();
    run_instructions(
        vec![
            Instruction::new(InstructionKind::EscapeLoop),
            Instruction::new(InstructionKind::SetVariable(
                "x".to_string(),
                Value::number(1.0),
            )),
        ],
        noop_emulator(),
        None,
        1.0,
        Arc::clone(&vars),
    );
    // EscapeLoop with nothing to catch it halts the whole strand, same as
    // Return does today - the SetVariable after it never runs.
    assert_eq!(vars.lock().unwrap().get("x"), None);
}
