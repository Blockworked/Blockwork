//! Where a block may go beyond blockstitch's header rules. Checked before an
//! insert by every host that edits a macro (the app and the website).

use super::{Instruction, InstructionKind, Macro, Strand};
use blockstitch_core::editor::PathStep;

/// A `Return` block only makes sense inside a value-returning custom
/// block's body - enforced here so it can't be placed somewhere confusing
/// (the interpreter otherwise tolerates a stray one harmlessly).
pub fn check_return(mac: &Macro, strand: &Strand, ins: &Instruction) -> Result<(), String> {
    if !matches!(&ins.kind, InstructionKind::Return(_)) {
        return Ok(());
    }
    let valid = matches!(strand.instructions.first().map(|i| &i.kind), Some(InstructionKind::BlockHeader(id))
        if mac.block_defs.iter().any(|b| &b.id == id && b.shape.returns_value()));
    if valid {
        Ok(())
    } else {
        Err(
            "A Return block can only be used inside a custom block that returns a value"
                .to_string(),
        )
    }
}

/// `escape loop`/`continue loop` only make sense inside a `Repeat`/`Forever`/
/// `While` body - enforced by walking every ancestor bracket named in
/// `path` and checking whether any of them is a loop instruction.
pub fn check_loop_control(
    strand: &Strand,
    path: &[PathStep],
    ins: &Instruction,
) -> Result<(), String> {
    if !matches!(
        &ins.kind,
        InstructionKind::EscapeLoop | InstructionKind::ContinueLoop
    ) {
        return Ok(());
    }
    let mut list: &[Instruction] = &strand.instructions;
    for step in path {
        let Some(slot) = step.slot else { break };
        let Some(ancestor) = list.get(step.index) else {
            break;
        };
        if matches!(
            &ancestor.kind,
            InstructionKind::Repeat { .. }
                | InstructionKind::Forever { .. }
                | InstructionKind::While { .. }
                | InstructionKind::ForEachItem { .. }
                | InstructionKind::ForEachKey { .. }
                | InstructionKind::ForEachPart { .. }
                | InstructionKind::ForEachFile { .. }
        ) {
            return Ok(());
        }
        let Some(body) = ancestor.body(slot) else {
            break;
        };
        list = body;
    }
    Err("An Escape Loop/Continue Loop block can only be used inside a loop".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::value::Value;

    fn strand_with(instructions: Vec<Instruction>) -> Strand {
        Strand {
            id: "s1".into(),
            x: 0,
            y: 0,
            instructions,
        }
    }

    #[test]
    fn non_loop_control_instructions_are_always_allowed() {
        let strand = strand_with(vec![Instruction::new(InstructionKind::WhenRan)]);
        assert!(
            check_loop_control(
                &strand,
                &[PathStep {
                    index: 1,
                    slot: None
                }],
                &Instruction::new(InstructionKind::Comment("x".into()))
            )
            .is_ok()
        );
    }

    #[test]
    fn escape_loop_at_strand_top_level_is_rejected() {
        let strand = strand_with(vec![Instruction::new(InstructionKind::WhenRan)]);
        let path = vec![PathStep {
            index: 1,
            slot: None,
        }];
        assert!(
            check_loop_control(
                &strand,
                &path,
                &Instruction::new(InstructionKind::EscapeLoop)
            )
            .is_err()
        );
    }

    #[test]
    fn escape_loop_directly_inside_repeat_body_is_allowed() {
        let strand = strand_with(vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::Repeat {
                count: Value::number(3.0),
                body: vec![],
            }),
        ]);
        // Path: strand[1] (Repeat) -> slot 0 (its body) -> insertion index 0.
        let path = vec![
            PathStep {
                index: 1,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: None,
            },
        ];
        assert!(
            check_loop_control(
                &strand,
                &path,
                &Instruction::new(InstructionKind::ContinueLoop)
            )
            .is_ok()
        );
    }

    #[test]
    fn escape_loop_inside_if_inside_while_is_allowed() {
        let strand = strand_with(vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::While {
                condition: Value::Bool,
                body: vec![Instruction::new(InstructionKind::If {
                    condition: Value::Bool,
                    body: vec![],
                })],
            }),
        ]);
        // strand[1] (While) -> slot 0 -> [0] (If) -> slot 0 -> insertion index 0.
        let path = vec![
            PathStep {
                index: 1,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: None,
            },
        ];
        assert!(
            check_loop_control(
                &strand,
                &path,
                &Instruction::new(InstructionKind::EscapeLoop)
            )
            .is_ok()
        );
    }

    #[test]
    fn escape_loop_inside_if_with_no_enclosing_loop_is_rejected() {
        let strand = strand_with(vec![
            Instruction::new(InstructionKind::WhenRan),
            Instruction::new(InstructionKind::If {
                condition: Value::Bool,
                body: vec![],
            }),
        ]);
        let path = vec![
            PathStep {
                index: 1,
                slot: Some(0),
            },
            PathStep {
                index: 0,
                slot: None,
            },
        ];
        assert!(
            check_loop_control(
                &strand,
                &path,
                &Instruction::new(InstructionKind::EscapeLoop)
            )
            .is_err()
        );
    }

    #[test]
    fn only_window_controls_go_inside_show_window() {
        let window = Instruction::new(InstructionKind::ShowWindow {
            title: Value::text("Hi"),
            body: vec![],
        });
        // Macro::new puts a When Ran block first, so the window is at index 1.
        let mut mac = Macro::new("m".into(), String::new(), vec![window]);
        let strand = mac.strands[0].id.clone();
        let inside = [PathStep::into_body(1, 0), PathStep::at(0)];
        let note = || Instruction::new(InstructionKind::WindowNote(Value::text("x")));
        let other = Instruction::new(InstructionKind::Comment("x".into()));
        assert_eq!(
            mac.insert_instruction(&strand, &inside, other),
            Err("Only window controls go inside a show window block".to_string())
        );
        assert_eq!(mac.insert_instruction(&strand, &inside, note()), Ok(true));
        assert_eq!(
            mac.insert_instruction(&strand, &[PathStep::at(2)], note()),
            Err("Window controls only go inside a show window block".to_string())
        );
    }
}
