//! Runs instruction edits inside a reporter's branches through
//! blockstitch's `edit_reporter_branch`; this supplies Blockwork's
//! instruction JSON.

use crate::macros::InstructionKind;
use blockstitch_core::editor::{BranchArgKeys, BranchCodec, CANVAS_ARG_KEYS};
use blockstitch_core::graph::BlockGraph;
use serde_json::Value as Json;

type Graph = BlockGraph<InstructionKind>;

/// Where Blockwork's commands put strand ids and value locations.
pub const BRANCH_ARG_KEYS: BranchArgKeys<'static> = CANVAS_ARG_KEYS;

/// Whether a command's arguments name a reporter branch.
pub fn targets_reporter_branch(args: &Json) -> bool {
    blockstitch_core::editor::targets_reporter_branch(args, BRANCH_ARG_KEYS)
}

fn read_body(body: &[Json]) -> Result<Vec<crate::macros::Instruction>, String> {
    crate::frontend::reporter_branch_to_instructions(body)
}

fn write_instruction(instruction: &crate::macros::Instruction) -> Result<Json, String> {
    serde_json::to_value(crate::frontend::instruction_to_dto(instruction))
        .map_err(|e| e.to_string())
}

fn read_instruction(json: &Json) -> Result<crate::macros::Instruction, String> {
    let dto = serde_json::from_value(json.clone()).map_err(|e| e.to_string())?;
    crate::frontend::dto_to_instruction(&dto).ok_or_else(|| "Unknown instruction type".into())
}

fn write_fragment(
    fragment: &blockstitch_core::graph::Fragment<InstructionKind>,
) -> Result<Json, String> {
    serde_json::to_value(crate::frontend::fragment_to_dto(fragment)).map_err(|e| e.to_string())
}

/// Runs one editor command inside a reporter's branches (an `args` naming a
/// `@reporter-branch:` strand). Call it on a clone and commit only on `Ok`.
pub fn edit_reporter_branch(graph: &mut Graph, cmd: &str, args: &Json) -> Result<Json, String> {
    let codec = BranchCodec {
        read: &read_body,
        write: &write_instruction,
        instruction: &read_instruction,
        fragment: &write_fragment,
    };
    graph.edit_reporter_branch(cmd, args, &codec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::value::Value;
    use blockstitch_core::editor::REPORTER_BRANCH_PREFIX as PREFIX;
    use blockstitch_core::editor::ValueLocation;
    use blockstitch_core::graph::Strand;
    use blockstitch_core::graph::{FloatingValue, Instruction};
    use serde_json::json;

    fn fixture() -> (Graph, String, ValueLocation) {
        let mut graph = Graph::new();
        graph.floating_values.push(FloatingValue {
            id: "reporter".into(),
            x: 0,
            y: 0,
            origin_block_id: None,
            value: Value::Call {
                block_id: "custom".into(),
                args: vec![],
                branches: vec![],
                saved: Box::new(Value::number(0.0)),
            },
        });
        let location = ValueLocation::Floating {
            floating_id: "reporter".into(),
            path: vec![],
        };
        let id = format!("{PREFIX}{}", json!({ "location": location, "slot": 0 }));
        (graph, id, location)
    }
    fn wait(id: &str) -> Json {
        json!({ "id": id, "type": "Wait", "duration": { "kind": "Number", "value": 10 } })
    }
    fn body(graph: &mut Graph, location: &ValueLocation) -> Vec<Json> {
        let Value::Call { branches, .. } = graph.value_at_mut(location).unwrap() else {
            panic!()
        };
        branches[0].clone()
    }
    #[test]
    fn palette_drop_edit_and_detach_survive_serialization() {
        let (mut graph, id, location) = fixture();
        edit_reporter_branch(
            &mut graph,
            "add_instruction",
            &json!({"strandId": id, "path": [{"index": 0}], "instruction": wait("wait")}),
        )
        .unwrap();
        assert!(graph.strands.is_empty());
        graph = serde_json::from_value(serde_json::to_value(&graph).unwrap()).unwrap();
        let value_location = json!({"kind": "Field", "strand_id": id, "index": [{"index": 0}], "field_id": "WaitDuration", "path": []});
        edit_reporter_branch(
            &mut graph,
            "put_value",
            &json!({"location": value_location, "value": {"kind": "Number", "value": 42}}),
        )
        .unwrap();
        assert_eq!(body(&mut graph, &location)[0]["duration"]["value"], 42.0);
        edit_reporter_branch(
            &mut graph,
            "move_strand",
            &json!({"strandId": id, "x": 100, "y": 200}),
        )
        .unwrap();
        assert!(body(&mut graph, &location).is_empty());
        assert_eq!(graph.strands.len(), 1);
        assert_eq!(graph.strands[0].instructions[0].id, "wait");
        assert_eq!((graph.strands[0].x, graph.strands[0].y), (100, 200));
    }
    #[test]
    fn canvas_stack_can_enter_and_leave_a_reporter() {
        let (mut graph, id, location) = fixture();
        let source = graph.add_strand(
            0,
            0,
            vec![
                crate::frontend::dto_to_instruction(
                    &serde_json::from_value(wait("first")).unwrap(),
                )
                .unwrap(),
                crate::frontend::dto_to_instruction(
                    &serde_json::from_value(wait("second")).unwrap(),
                )
                .unwrap(),
            ],
        );
        edit_reporter_branch(
            &mut graph,
            "merge_strand",
            &json!({"draggedId": source, "targetId": id, "path": [{"index": 0}]}),
        )
        .unwrap();
        assert!(graph.strands.is_empty());
        assert_eq!(body(&mut graph, &location).len(), 2);
        let target = graph.add_strand(0, 0, vec![]);
        edit_reporter_branch(&mut graph, "merge_tail", &json!({"strandId": id, "path": [{"index": 1}], "targetId": target, "targetPath": [{"index": 0}]})).unwrap();
        assert_eq!(body(&mut graph, &location).len(), 1);
        assert_eq!(graph.strands[0].instructions[0].id, "second");
    }
    #[test]
    fn branch_restore_follows_a_field_owner_after_indices_shift() {
        let mut graph = Graph::new();
        let value = Value::Call {
            block_id: "custom".into(),
            args: vec![],
            branches: vec![],
            saved: Box::new(Value::number(0.0)),
        };
        graph.strands.push(Strand {
            id: "source".into(),
            x: 0,
            y: 0,
            instructions: vec![
                crate::frontend::dto_to_instruction(
                    &serde_json::from_value(wait("first")).unwrap(),
                )
                .unwrap(),
                Instruction {
                    id: "owner".into(),
                    kind: InstructionKind::Wait(value),
                },
            ],
        });
        let location = json!({ "kind": "Field", "strand_id": "source", "index": [{"index": 1}], "field_id": "WaitDuration", "path": [] });
        let id = format!("{PREFIX}{}", json!({ "location": location, "slot": 0 }));
        let branches = graph
            .open_reporter_branches(std::slice::from_ref(&id), &read_body)
            .unwrap();
        graph.strands[0].instructions.remove(0);
        graph.strand_mut(&id).unwrap().instructions.push(
            crate::frontend::dto_to_instruction(&serde_json::from_value(wait("nested")).unwrap())
                .unwrap(),
        );
        graph
            .close_reporter_branches(branches, &write_instruction)
            .unwrap();
        assert_eq!(graph.strands.len(), 1);
        let InstructionKind::Wait(Value::Call { branches, .. }) =
            &graph.strands[0].instructions[0].kind
        else {
            panic!()
        };
        assert_eq!(branches[0][0]["id"], "nested");
    }
}
