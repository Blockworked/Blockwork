//! Starter macros the user can pick when making a new one.

use blockwork_core::input::gamepad::GamepadButton;
use blockwork_core::input::touch::TouchGestureKind;
use blockwork_core::input::types::{Direction, InputToken, MacroButton};
use blockwork_core::input::value::{Op, Value};
use blockwork_core::macros::{Instruction, InstructionKind, Macro};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub(crate) struct TemplateDto {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
}

struct Template {
    info: TemplateDto,
    build: fn() -> Vec<Instruction>,
    /// More strands, beside the first.
    more: fn() -> Vec<Vec<Instruction>>,
    /// Keeps its key triggers working while another macro is open.
    always_listen: bool,
}

fn ins(kind: InstructionKind) -> Instruction {
    Instruction::new(kind)
}

fn wait(ms: f64) -> Instruction {
    ins(InstructionKind::Wait(Value::number(ms)))
}

fn key(name: &str, direction: Direction) -> Instruction {
    ins(InstructionKind::Token(InputToken::Key(
        Value::text(name),
        direction,
    )))
}

fn when_south(pressed: bool) -> Instruction {
    ins(InstructionKind::WhenGamepadButton {
        controller: Value::text(""),
        button: GamepadButton::South,
        pressed,
    })
}

fn templates() -> [Template; 7] {
    [
        Template {
            info: TemplateDto {
                id: "auto_clicker",
                name: "Auto clicker",
                description: "Clicks the left mouse button 10 times a second until you stop it.",
                icon: "mouse-pointer-click",
            },
            build: || {
                vec![
                    ins(InstructionKind::WhenRan),
                    ins(InstructionKind::Forever {
                        body: vec![
                            ins(InstructionKind::Token(InputToken::Button(
                                MacroButton::Left,
                                Direction::Click,
                            ))),
                            wait(100.0),
                        ],
                    }),
                ]
            },
            more: Vec::new,
            always_listen: false,
        },
        Template {
            info: TemplateDto {
                id: "text_snippet",
                name: "Text snippet",
                description: "Type ;thx anywhere and it turns into a saved reply.",
                icon: "type",
            },
            build: || {
                vec![
                    ins(InstructionKind::WhenTyped {
                        text: Value::text(";thx"),
                        erase: true,
                    }),
                    ins(InstructionKind::PasteText(Value::text(
                        "Thanks for reaching out! I'll get back to you soon.",
                    ))),
                ]
            },
            more: Vec::new,
            always_listen: true,
        },
        Template {
            info: TemplateDto {
                id: "break_reminder",
                name: "Break reminder",
                description: "Reminds you to take a break every 30 minutes, counting each one.",
                icon: "clock",
            },
            build: || {
                vec![
                    ins(InstructionKind::WhenRan),
                    ins(InstructionKind::Forever {
                        body: vec![
                            wait(30.0 * 60.0 * 1000.0),
                            ins(InstructionKind::ChangeGlobal {
                                name: Value::text("breaks"),
                                by: Value::number(1.0),
                            }),
                            ins(InstructionKind::Notify {
                                title: Value::text("Time for a break"),
                                message: Value::op(
                                    Op::from_name("Join"),
                                    vec![
                                        Value::text(
                                            "Stand up and look away from the screen. Breaks so far: ",
                                        ),
                                        Value::op(
                                            Op::from_name("GlobalVariable"),
                                            vec![Value::text("breaks")],
                                        ),
                                    ],
                                ),
                            }),
                        ],
                    }),
                ]
            },
            more: Vec::new,
            always_listen: false,
        },
        Template {
            info: TemplateDto {
                id: "website_watcher",
                name: "Website watcher",
                description: "Checks a website every 5 minutes and tells you if it's down.",
                icon: "cloud",
            },
            build: || {
                vec![
                    ins(InstructionKind::WhenRan),
                    ins(InstructionKind::Forever {
                        body: vec![
                            ins(InstructionKind::TryCatch {
                                body: vec![ins(InstructionKind::Retry {
                                    count: Value::number(3.0),
                                    delay: Value::number(10_000.0),
                                    body: vec![ins(InstructionKind::HttpRequest {
                                        method: "GET".into(),
                                        url: Value::text("https://example.com"),
                                        body: Value::text(""),
                                        headers: Value::text(""),
                                        variable: String::new(),
                                    })],
                                })],
                                catch_body: vec![ins(InstructionKind::Notify {
                                    title: Value::text("Website is down"),
                                    message: Value::op(Op::from_name("LastError"), vec![]),
                                })],
                            }),
                            wait(5.0 * 60.0 * 1000.0),
                        ],
                    }),
                ]
            },
            more: Vec::new,
            always_listen: false,
        },
        Template {
            info: TemplateDto {
                id: "controller_to_keyboard",
                name: "Controller to keyboard",
                description: "Holds Space while a controller's bottom face button is held. Turn on controller input in Settings first.",
                icon: "gamepad-2",
            },
            build: || vec![when_south(true), key("Space", Direction::Press)],
            more: || vec![vec![when_south(false), key("Space", Direction::Release)]],
            always_listen: true,
        },
        Template {
            info: TemplateDto {
                id: "touch_swipe",
                name: "Swipe left",
                description: "Swipes one finger right to left across the middle of your first screen. Needs touch output (Linux with uinput, or Windows).",
                icon: "hand",
            },
            build: || {
                let n = |v: f64| Box::new(Value::number(v));
                vec![
                    ins(InstructionKind::WhenRan),
                    ins(InstructionKind::TouchGesture {
                        gesture: TouchGestureKind::Swipe,
                        fingers: 1,
                        x: n(80.0),
                        y: n(50.0),
                        end_x: n(20.0),
                        end_y: n(50.0),
                        duration: n(300.0),
                        span: n(0.0),
                        amount: n(1.0),
                        normalized: true,
                        monitor: 0,
                    }),
                ]
            },
            more: Vec::new,
            always_listen: false,
        },
        Template {
            info: TemplateDto {
                id: "startup_notice",
                name: "At login",
                description: "Runs once each time you log in. Turn on Start at system startup in Settings first.",
                icon: "power",
            },
            build: || {
                vec![
                    ins(InstructionKind::WhenSystemStarts),
                    ins(InstructionKind::Notify {
                        title: Value::text("Blockwork is running"),
                        message: Value::text(
                            "Your startup macro ran. Add blocks under its header.",
                        ),
                    }),
                ]
            },
            more: Vec::new,
            always_listen: true,
        },
    ]
}

pub(crate) fn list() -> Vec<TemplateDto> {
    templates().into_iter().map(|t| t.info).collect()
}

/// A new, unsaved macro made from template `id`.
pub(crate) fn build(id: &str) -> Result<Macro, String> {
    let template = templates()
        .into_iter()
        .find(|t| t.info.id == id)
        .ok_or("unknown template")?;
    let mut mac = Macro::new(
        template.info.name.into(),
        template.info.description.into(),
        vec![],
    );
    mac.graph.strands[0].instructions = (template.build)();
    for (index, instructions) in (template.more)().into_iter().enumerate() {
        let mut strand = blockwork_core::macros::Strand::new(0, 160 * (index as i32 + 1));
        strand.instructions = instructions;
        mac.graph.strands.push(strand);
    }
    mac.settings.always_listen = template.always_listen;
    Ok(mac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_survives_a_save_round_trip() {
        blockwork_core::init();
        for template in list() {
            let mac = build(template.id).unwrap();
            let json = serde_json::to_string(&mac).unwrap();
            let back: Macro = serde_json::from_str(&json).unwrap();
            assert_eq!(
                back.graph.strands[0].instructions.len(),
                mac.graph.strands[0].instructions.len(),
                "{}",
                template.id
            );
        }
        assert!(build("nope").is_err());
        // Two-strand templates keep both strands, each under its own header.
        let pad = build("controller_to_keyboard").unwrap();
        assert_eq!(pad.graph.strands.len(), 2);
        assert!(
            pad.graph
                .strands
                .iter()
                .all(|s| s.instructions[0].is_header())
        );
        assert!(pad.settings.always_listen);
    }
}
