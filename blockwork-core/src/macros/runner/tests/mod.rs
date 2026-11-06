use super::vision::*;
use super::*;
use crate::input::types::{Axis, Direction, MacroButton, MacroKey};
use crate::macros::{
    BlockDef, BlockPiece, BlockShape, InputValueType, MacroGraph, Strand, default_block_color,
};
use blockstitch_core::graph::resolve_list_reporters;

mod apps;
mod control_flow;
mod custom_blocks;
mod dialogs;
mod errors;
mod input;
mod runs;
mod variables;
mod vision;

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

fn bump(name: &str) -> Instruction {
    Instruction::new(InstructionKind::ChangeVariable(
        name.to_string(),
        Value::number(1.0),
    ))
}

/// Answers dialogs by their message: "cancel" cancels, permissions are denied.
fn install_test_dialogs() {
    use crate::prompt::{Kind, Reply};
    crate::prompt::set_handler(Arc::new(
        |request: &crate::prompt::Request, _stopped: &dyn Fn() -> bool| {
            let reply = |answer: &str, text: &str| {
                Ok(Reply {
                    answer: answer.into(),
                    text: text.into(),
                })
            };
            match request.kind {
                _ if request.message == "cancel" => reply("cancel", ""),
                Kind::Text => reply("ok", &format!("{}!", request.default)),
                Kind::Confirm => reply("yes", ""),
                Kind::Choice => reply("ok", &request.options[1]),
                Kind::Message => reply("ok", ""),
                Kind::Permission => reply("deny", ""),
            }
        },
    ));
}

/// `run_instructions` with a list store the test can read back.
fn run_instructions_with_lists(
    instructions: Vec<Instruction>,
    emulator: Arc<Mutex<dyn InputBackend>>,
    stop_flag: Option<Arc<Mutex<bool>>>,
    speed_multiplier: f64,
    variables: VariableStore,
    lists: ListStore,
) {
    run_fired_strand(
        crate::run_log::Source::default(),
        instructions,
        emulator,
        stop_flag,
        speed_multiplier,
        variables,
        lists,
        Arc::new(HashMap::new()),
        0,
    );
}
