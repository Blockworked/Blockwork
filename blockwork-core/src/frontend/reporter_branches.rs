//! Runs instruction edits inside a reporter's branches: blockstitch opens
//! them as temporary strands (`open_reporter_branches`), this runs the edit,
//! and blockstitch writes them back.

use crate::macros::InstructionKind;
use blockstitch_core::editor::{EditCommand, EditContext, ValueDestination, ValueLocation};
use blockstitch_core::graph::BlockGraph;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

type Graph = BlockGraph<InstructionKind>;

fn arg<T: DeserializeOwned>(args: &Json, name: &str) -> Result<T, String> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Json::Null)).map_err(|e| e.to_string())
}

fn read_body(body: &[Json]) -> Result<Vec<crate::macros::Instruction>, String> {
    crate::frontend::reporter_branch_to_instructions(body)
}

fn write_instruction(instruction: &crate::macros::Instruction) -> Result<Json, String> {
    serde_json::to_value(crate::frontend::instruction_to_dto(instruction))
        .map_err(|e| e.to_string())
}

/// Runs one editor command inside a reporter's branches (an `args` naming a
/// `@reporter-branch:` strand). Call it on a clone and commit only on `Ok`.
pub fn edit_reporter_branch(graph: &mut Graph, cmd: &str, args: &Json) -> Result<Json, String> {
    let ids = blockstitch_core::editor::reporter_branch_ids(args);
    let branches = graph.open_reporter_branches(&ids, &read_body)?;
    let strand = || arg::<String>(args, "strandId");
    let path = || arg(args, "path");
    let result = match cmd {
        "add_instruction" => {
            let instruction = crate::frontend::dto_to_instruction(&arg(args, "instruction")?)
                .ok_or("Unknown instruction type")?;
            graph
                .apply_edit(
                    EditCommand::InsertInstruction {
                        strand_id: strand()?,
                        path: path()?,
                        instruction,
                    },
                    &EditContext::default(),
                )
                .map_err(String::from)?;
            Json::Null
        }
        "edit_instruction" => {
            let instruction = crate::frontend::dto_to_instruction(&arg(args, "instruction")?)
                .ok_or("Unknown instruction type")?;
            if !graph.replace_instruction(&strand()?, &path()?, instruction) {
                return Err("Instruction is missing".into());
            }
            Json::Null
        }
        "merge_strand" => {
            graph.merge_strand(
                &arg::<String>(args, "draggedId")?,
                &arg::<String>(args, "targetId")?,
                &path()?,
            )?;
            Json::Null
        }
        "merge_tail" => {
            graph.merge_tail(
                &strand()?,
                &path()?,
                &arg::<String>(args, "targetId")?,
                &arg::<Vec<blockstitch_core::editor::PathStep>>(args, "targetPath")?,
            )?;
            Json::Null
        }
        "split_strand" => {
            json!(graph.split_strand(&strand()?, &path()?, arg(args, "x")?, arg(args, "y")?)?)
        }
        "move_strand" => json!(graph.split_strand(
            &strand()?,
            &[blockstitch_core::editor::PathStep {
                index: 0,
                slot: None
            }],
            arg(args, "x")?,
            arg(args, "y")?
        )?),
        "remove_strand" => {
            graph
                .strands
                .retain(|s| s.id != strand().unwrap_or_default());
            Json::Null
        }
        "remove_instruction" => {
            graph
                .apply_edit(
                    EditCommand::RemoveInstructions {
                        strand_id: strand()?,
                        path: path()?,
                        count: arg::<Option<usize>>(args, "count")?.unwrap_or(1),
                    },
                    &EditContext::default(),
                )
                .map_err(String::from)?;
            Json::Null
        }
        "duplicate_instruction" => {
            graph
                .apply_edit(
                    EditCommand::DuplicateInstruction {
                        strand_id: strand()?,
                        path: path()?,
                    },
                    &EditContext::default(),
                )
                .map_err(String::from)?;
            Json::Null
        }
        "reorder_instruction" => {
            graph.reorder_instruction(&strand()?, &path()?, arg(args, "direction")?);
            Json::Null
        }
        "copy_instructions" => serde_json::to_value(crate::frontend::fragment_to_dto(
            &graph.copy_fragment(&strand()?, &path()?, arg(args, "count")?)?,
        ))
        .map_err(|e| e.to_string())?,
        "edit_value_field" => {
            graph
                .apply_edit(
                    EditCommand::EditFieldText {
                        location: arg(args, "location")?,
                        text: arg(args, "text")?,
                    },
                    &EditContext::default(),
                )
                .map_err(String::from)?;
            Json::Null
        }
        "put_value" => {
            if !graph.put_value(&arg(args, "location")?, arg(args, "value")?) {
                return Err("Value slot is missing".into());
            }
            Json::Null
        }
        "take_value" => serde_json::to_value(graph.take_value(&arg(args, "location")?))
            .map_err(|e| e.to_string())?,
        "move_value" => {
            let to = arg::<Option<ValueLocation>>(args, "to")?
                .map(ValueDestination::Slot)
                .unwrap_or(ValueDestination::Canvas {
                    x: arg::<Option<i32>>(args, "x")?.unwrap_or(0),
                    y: arg::<Option<i32>>(args, "y")?.unwrap_or(0),
                    origin_block_id: arg(args, "originBlockId")?,
                });
            graph
                .apply_edit(
                    EditCommand::MoveValue {
                        from: arg(args, "from")?,
                        to,
                    },
                    &EditContext::default(),
                )
                .map_err(String::from)?;
            Json::Null
        }
        _ => {
            return Err(format!(
                "Command '{cmd}' is unavailable inside a reporter branch"
            ));
        }
    };
    if matches!(cmd, "merge_strand" | "merge_tail")
        && let Some(y) = arg::<Option<i32>>(args, "targetY")?
    {
        let target_path: Vec<blockstitch_core::editor::PathStep> = arg(
            args,
            if cmd == "merge_tail" {
                "targetPath"
            } else {
                "path"
            },
        )?;
        if target_path.len() == 1
            && target_path[0].index == 0
            && let Some(target) = graph.strand_mut(&arg::<String>(args, "targetId")?)
        {
            target.y = y;
        }
    }
    graph.close_reporter_branches(branches, &write_instruction)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::value::Value;
    use blockstitch_core::editor::REPORTER_BRANCH_PREFIX as PREFIX;
    use blockstitch_core::graph::Strand;
    use blockstitch_core::graph::{FloatingValue, Instruction};

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
            .open_reporter_branches(&[id.clone()], &read_body)
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
