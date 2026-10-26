//! Exposes a reporter's serialized branch bodies to the instruction editor.

use blockstitch_core::editor::{EditCommand, EditContext, ValueDestination, ValueLocation};
use blockstitch_core::graph::{BlockGraph, BlockKind, Strand};
use blockwork_core::input::value::Value;
use blockwork_core::macros::InstructionKind;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

const PREFIX: &str = "@reporter-branch:";
type Graph = BlockGraph<InstructionKind>;

#[derive(Deserialize)]
struct BranchAddress {
    location: ValueLocation,
    slot: usize,
}

struct Branch {
    id: String,
    address: BranchAddress,
    owner: Option<String>,
}

fn arg<T: DeserializeOwned>(args: &Json, name: &str) -> Result<T, String> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Json::Null)).map_err(|e| e.to_string())
}

pub(crate) fn targets_branch(args: &Json) -> bool {
    match args {
        Json::Object(fields) => fields.iter().any(|(key, value)| {
            if ["strandId", "targetId", "draggedId", "strand_id"].contains(&key.as_str()) {
                value.as_str().is_some_and(|id| id.starts_with(PREFIX))
            } else if ["location", "from", "to"].contains(&key.as_str()) {
                targets_branch(value)
            } else {
                false
            }
        }),
        _ => false,
    }
}

fn materialize(graph: &mut Graph, id: &str, branches: &mut Vec<Branch>) -> Result<(), String> {
    let Some(encoded) = id.strip_prefix(PREFIX) else {
        return Ok(());
    };
    if branches.iter().any(|branch| branch.id == id) {
        return Ok(());
    }
    if branches.len() >= 32 {
        return Err("Reporter branches are nested too deeply".into());
    }
    let address: BranchAddress = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
    let owner = if let ValueLocation::Field {
        strand_id, index, ..
    } = &address.location
    {
        materialize(graph, strand_id, branches)?;
        Some(
            graph
                .instruction_at(strand_id, index)
                .ok_or("Reporter owner is missing")?
                .id
                .clone(),
        )
    } else {
        None
    };
    let Value::Call {
        branches: bodies, ..
    } = graph
        .value_at_mut(&address.location)
        .ok_or("Reporter is missing")?
    else {
        return Err("The value is not a custom reporter".into());
    };
    if address.slot >= 255 {
        return Err("Invalid reporter branch".into());
    }
    bodies.resize_with(bodies.len().max(address.slot + 1), Vec::new);
    let instructions =
        blockwork_core::frontend::reporter_branch_to_instructions(&bodies[address.slot])?;
    graph.strands.push(Strand {
        id: id.into(),
        x: 0,
        y: 0,
        instructions,
    });
    branches.push(Branch {
        id: id.into(),
        address,
        owner,
    });
    Ok(())
}

fn collect(graph: &mut Graph, args: &Json, branches: &mut Vec<Branch>) -> Result<(), String> {
    if let Json::Object(fields) = args {
        for (key, value) in fields {
            if ["strandId", "targetId", "draggedId", "strand_id"].contains(&key.as_str()) {
                if let Some(id) = value.as_str() {
                    materialize(graph, id, branches)?;
                }
            } else if ["location", "from", "to"].contains(&key.as_str()) {
                collect(graph, value, branches)?;
            }
        }
    }
    Ok(())
}

fn restore(graph: &mut Graph, branch: Branch) -> Result<(), String> {
    let instructions = if let Some(index) = graph.strands.iter().position(|s| s.id == branch.id) {
        graph.strands.remove(index).instructions
    } else {
        Vec::new()
    };
    if instructions
        .iter()
        .any(|instruction| instruction.is_header())
    {
        return Err("A header block can't be placed in a reporter branch".into());
    }
    let body = instructions
        .iter()
        .map(|instruction| {
            serde_json::to_value(blockwork_core::frontend::instruction_to_dto(instruction))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut restored = false;
    if let (Some(owner), ValueLocation::Field { field_id, path, .. }) =
        (branch.owner, &branch.address.location)
    {
        graph.walk_instructions_mut(&mut |instruction| {
            if instruction.id == owner
                && let Some(Value::Call { branches, .. }) = instruction
                    .kind
                    .value_slot_mut(field_id)
                    .and_then(|v| v.get_mut(path))
                && let Some(slot) = branches.get_mut(branch.address.slot)
            {
                *slot = body.clone();
                restored = true;
            }
        });
    } else if let Some(Value::Call { branches, .. }) = graph.value_at_mut(&branch.address.location)
        && let Some(slot) = branches.get_mut(branch.address.slot)
    {
        *slot = body;
        restored = true;
    }
    if restored {
        Ok(())
    } else {
        Err("Can't move a reporter into its own branch".into())
    }
}

/// Edits a clone; only a completely restored graph can be committed.
pub(crate) fn edit(graph: &mut Graph, cmd: &str, args: &Json) -> Result<Json, String> {
    let mut branches = Vec::new();
    collect(graph, args, &mut branches)?;
    let strand = || arg::<String>(args, "strandId");
    let path = || arg(args, "path");
    let result = match cmd {
        "add_instruction" => {
            let instruction = crate::state::dto_to_instruction(&arg(args, "instruction")?)
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
            let instruction = crate::state::dto_to_instruction(&arg(args, "instruction")?)
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
                &arg::<Vec<crate::state::PathStep>>(args, "targetPath")?,
            )?;
            Json::Null
        }
        "split_strand" => {
            json!(graph.split_strand(&strand()?, &path()?, arg(args, "x")?, arg(args, "y")?)?)
        }
        "move_strand" => json!(graph.split_strand(
            &strand()?,
            &[crate::state::PathStep {
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
        "copy_instructions" => serde_json::to_value(crate::state::fragment_to_dto(
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
    for branch in branches.into_iter().rev() {
        restore(graph, branch)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        edit(
            &mut graph,
            "add_instruction",
            &json!({"strandId": id, "path": [{"index": 0}], "instruction": wait("wait")}),
        )
        .unwrap();
        assert!(graph.strands.is_empty());
        graph = serde_json::from_value(serde_json::to_value(&graph).unwrap()).unwrap();
        let value_location = json!({"kind": "Field", "strand_id": id, "index": [{"index": 0}], "field_id": "WaitDuration", "path": []});
        edit(
            &mut graph,
            "put_value",
            &json!({"location": value_location, "value": {"kind": "Number", "value": 42}}),
        )
        .unwrap();
        assert_eq!(body(&mut graph, &location)[0]["duration"]["value"], 42.0);
        edit(
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
                crate::state::dto_to_instruction(&serde_json::from_value(wait("first")).unwrap())
                    .unwrap(),
                crate::state::dto_to_instruction(&serde_json::from_value(wait("second")).unwrap())
                    .unwrap(),
            ],
        );
        edit(
            &mut graph,
            "merge_strand",
            &json!({"draggedId": source, "targetId": id, "path": [{"index": 0}]}),
        )
        .unwrap();
        assert!(graph.strands.is_empty());
        assert_eq!(body(&mut graph, &location).len(), 2);
        let target = graph.add_strand(0, 0, vec![]);
        edit(&mut graph, "merge_tail", &json!({"strandId": id, "path": [{"index": 1}], "targetId": target, "targetPath": [{"index": 0}]})).unwrap();
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
                crate::state::dto_to_instruction(&serde_json::from_value(wait("first")).unwrap())
                    .unwrap(),
                Instruction {
                    id: "owner".into(),
                    kind: InstructionKind::Wait(value),
                },
            ],
        });
        let location = json!({ "kind": "Field", "strand_id": "source", "index": [{"index": 1}], "field_id": "WaitDuration", "path": [] });
        let id = format!("{PREFIX}{}", json!({ "location": location, "slot": 0 }));
        let mut branches = Vec::new();
        materialize(&mut graph, &id, &mut branches).unwrap();
        graph.strands[0].instructions.remove(0);
        graph.strand_mut(&id).unwrap().instructions.push(
            crate::state::dto_to_instruction(&serde_json::from_value(wait("nested")).unwrap())
                .unwrap(),
        );
        restore(&mut graph, branches.pop().unwrap()).unwrap();
        assert_eq!(graph.strands.len(), 1);
        let InstructionKind::Wait(Value::Call { branches, .. }) =
            &graph.strands[0].instructions[0].kind
        else {
            panic!()
        };
        assert_eq!(branches[0][0]["id"], "nested");
    }
}
