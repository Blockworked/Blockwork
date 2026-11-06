use super::*;
use blockwork_core::input::value::Op;
use blockwork_core::macros::MacroGraph;

#[test]
fn lists_what_a_macro_can_do_including_nested_blocks_and_reporters() {
    blockwork_core::init();
    let nested = Instruction::new(InstructionKind::If {
        condition: Value::Bool,
        body: vec![Instruction::new(InstructionKind::Log(Value::op(
            Op::from_name("ShellOutput"),
            vec![Value::text("ls")],
        )))],
    });
    let mac = Macro {
        id: "m".into(),
        name: "Test".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![Strand {
                id: "s1".into(),
                x: 0,
                y: 0,
                instructions: vec![
                    Instruction::new(InstructionKind::WhenFileChanged(Value::text("~/a"))),
                    nested,
                    Instruction::new(InstructionKind::Token(InputToken::Text(Value::text("hi")))),
                ],
            }],
            ..MacroGraph::new()
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: blockwork_core::macros::MacroSettings::default(),
    };
    let kinds: Vec<_> = macro_capabilities(&mac)
        .into_iter()
        .map(|c| (c.kind, c.risky))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("commands".to_string(), true),
            ("files_read".to_string(), false),
            ("input".to_string(), false),
            ("triggers".to_string(), false),
        ]
    );
}

fn macro_of(instructions: Vec<Instruction>) -> Macro {
    Macro {
        id: "m".into(),
        name: "Test".into(),
        description: "".into(),
        graph: MacroGraph {
            strands: vec![Strand {
                id: "s1".into(),
                x: 0,
                y: 0,
                instructions,
            }],
            ..MacroGraph::new()
        },
        signals: Vec::new(),
        recording_target: None,
        speed_multiplier: 1.0,
        settings: blockwork_core::macros::MacroSettings::default(),
    }
}

#[test]
fn spots_browser_blocks_and_reporters_anywhere() {
    blockwork_core::init();
    assert!(!uses_browser(&macro_of(vec![Instruction::new(
        InstructionKind::Log(Value::text("hi"))
    )])));
    let open = Instruction::new(InstructionKind::BrowserOpen {
        url: Value::text("https://example.com"),
        new_tab: false,
    });
    assert!(uses_browser(&macro_of(vec![open])));
    let nested = Instruction::new(InstructionKind::If {
        condition: Value::op(Op::from_name("BrowserConnected"), vec![]),
        body: vec![],
    });
    assert!(uses_browser(&macro_of(vec![nested])));
}
