//! Pack-owned state and the public settings/output interface.

use super::*;
use crate::input::value::Evaluated;
use crate::macros::{BlockDef, BlockPiece, BlockShape, Instruction, RefKind};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableRole {
    #[default]
    Internal,
    Output,
    Setting,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariableInterface {
    pub name: String,
    #[serde(default)]
    pub role: VariableRole,
}

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct PackVariable {
    pub name: String,
    pub value: Evaluated,
    #[serde(default)]
    pub role: VariableRole,
}

impl PackRecord {
    pub fn variable_key(&self, name: &str) -> String {
        format!("\u{1f}pack:{}:{}:{name}", self.id.len(), self.id)
    }

    pub fn output_id(&self, name: &str) -> String {
        format!("pack-output:{}:{}:{name}", self.id.len(), self.id)
    }
}

/// Prefixes are presentation only. Ownership comes from the saved record.
pub fn variable_label(name: &str) -> &str {
    name.rsplit_once(": ").map_or(name, |(_, label)| label)
}

pub fn output_definition(pack: &PackRecord, var: &PackVariable) -> BlockDef {
    let id = pack.output_id(&var.name);
    BlockDef {
        id: id.clone(),
        identifier: String::new(),
        description: String::new(),
        pieces: vec![BlockPiece::Label {
            id: format!("{id}:label"),
            text: variable_label(&var.name).into(),
        }],
        shape: if matches!(var.value, Evaluated::Bool(_)) {
            BlockShape::ReturnsBool
        } else {
            BlockShape::ReturnsValue
        },
        color: "#4C97FF".into(),
        icon: "".into(),
    }
}

pub fn install_outputs(mac: &mut Macro) {
    let outputs: Vec<_> = mac
        .settings
        .block_packs
        .iter()
        .flat_map(|pack| {
            pack.variables
                .iter()
                .filter(|v| v.role == VariableRole::Output)
                .map(|v| (pack.id.clone(), output_definition(pack, v)))
                .collect::<Vec<_>>()
        })
        .collect();
    for (pack_id, def) in outputs {
        if mac.block_def(&def.id).is_none() {
            mac.graph.block_defs.push(def.clone());
        }
        if let Some(pack) = mac
            .settings
            .block_packs
            .iter_mut()
            .find(|p| p.id == pack_id)
            && !pack.blocks.contains(&def.id)
        {
            pack.blocks.push(def.id);
        }
    }
}

pub(super) fn variable_names_used(strands: &[Strand]) -> HashSet<String> {
    fn value_names(value: &Value, names: &mut HashSet<String>) {
        match value {
            Value::Var { name } => {
                names.insert(name.clone());
            }
            Value::Op { args, saved, .. } => {
                args.iter().for_each(|v| value_names(v, names));
                value_names(saved, names);
            }
            Value::Call {
                args,
                branches,
                saved,
                ..
            } => {
                args.iter().for_each(|v| value_names(v, names));
                value_names(saved, names);
                for branch in branches {
                    if let Ok(body) = crate::frontend::reporter_branch_to_instructions(branch) {
                        names.extend(variable_names_used(&[Strand::with_instructions(
                            0, 0, body,
                        )]));
                    }
                }
            }
            _ => {}
        }
    }
    let mut names = HashSet::new();
    for strand in strands {
        for ins in &strand.instructions {
            ins.walk(&mut |ins| {
                ins.kind.visit_refs(&mut |kind, name| {
                    if kind == RefKind::Variable {
                        names.insert(name.into());
                    }
                });
                ins.kind
                    .clone()
                    .visit_values_mut(&mut |v, _| value_names(v, &mut names));
            });
        }
    }
    names.retain(|name| !name.trim().is_empty());
    names
}

/// Old saves have no ownership metadata. Keep every externally used or
/// shared name in the macro so existing reads and writes keep their meaning.
pub fn migrate_variables(mac: &mut Macro) {
    let mut owners: HashMap<String, Vec<String>> = HashMap::new();
    let external: Vec<_> = mac
        .strands
        .iter()
        .filter(|s| !is_pack_strand(mac, s))
        .cloned()
        .collect();
    let mut external_names = variable_names_used(&external);
    let floating: Vec<_> = mac
        .floating_values
        .iter()
        .map(|v| Instruction::new(crate::macros::InstructionKind::Return(v.value.clone())))
        .collect();
    external_names.extend(variable_names_used(&[Strand::with_instructions(
        0, 0, floating,
    )]));
    for pack in &mac.settings.block_packs {
        let strands: Vec<_> = mac
            .strands
            .iter()
            .filter(|s| {
                s.block_header_id()
                    .is_some_and(|id| pack.blocks.iter().any(|b| b == id))
            })
            .cloned()
            .collect();
        for name in variable_names_used(&strands) {
            owners.entry(name).or_default().push(pack.id.clone());
        }
    }
    let mut moved = HashSet::new();
    for pack in &mut mac.settings.block_packs {
        if pack.variables_scoped {
            continue;
        }
        let mut candidates: Vec<_> = owners
            .iter()
            .filter(|(name, ids)| {
                ids.len() == 1 && ids[0] == pack.id && !external_names.contains(*name)
            })
            .map(|(name, _)| name.clone())
            .collect();
        candidates.sort();
        for name in candidates {
            let value = mac
                .graph
                .variables
                .iter()
                .find(|v| v.name == name)
                .map(|v| v.value.clone())
                .unwrap_or(Evaluated::Number(0.0));
            pack.variables.push(PackVariable {
                name: name.clone(),
                value,
                role: VariableRole::Internal,
            });
            moved.insert(name);
        }
        pack.variables_scoped = true;
    }
    mac.graph.variables.retain(|v| !moved.contains(&v.name));
    install_outputs(mac);
}

/// Compiles variable references without changing the authored pack bodies.
pub fn scope_body(body: &mut [Instruction], pack: &PackRecord) {
    fn walk_value(value: &mut Value, pack: &PackRecord) {
        match value {
            Value::Var { name } if pack.variables.iter().any(|v| &v.name == name) => {
                *name = pack.variable_key(name)
            }
            Value::Op { args, saved, .. } => {
                args.iter_mut().for_each(|v| walk_value(v, pack));
                walk_value(saved, pack);
            }
            Value::Call {
                args,
                branches,
                saved,
                ..
            } => {
                walk_value(saved, pack);
                args.iter_mut().for_each(|v| walk_value(v, pack));
                for branch in branches {
                    if let Ok(mut body) = crate::frontend::reporter_branch_to_instructions(branch) {
                        scope_body(&mut body, pack);
                        *branch = body
                            .iter()
                            .map(|ins| {
                                serde_json::to_value(crate::frontend::instruction_to_dto(ins))
                                    .unwrap()
                            })
                            .collect();
                    }
                }
            }
            _ => {}
        }
    }
    for ins in body {
        ins.walk_mut(&mut |ins| {
            ins.kind.visit_refs_mut(&mut |kind, name| {
                if kind == RefKind::Variable && pack.variables.iter().any(|v| &v.name == name) {
                    *name = pack.variable_key(name);
                }
            });
            ins.kind.visit_values_mut(&mut |v, _| walk_value(v, pack));
        });
    }
}

impl Macro {
    /// Writes private and user variable state back after any kind of run.
    pub fn sync_variables_from(&mut self, values: &HashMap<String, Evaluated>) {
        self.graph.sync_variables_from(values);
        for pack in &mut self.settings.block_packs {
            let id = pack.clone();
            for var in &mut pack.variables {
                if let Some(value) = values.get(&id.variable_key(&var.name)) {
                    var.value = value.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::{InstructionKind, VariableDef};

    fn pack(id: &str) -> BlockPack {
        let mut mac = Macro::new("Author".into(), String::new(), vec![]);
        let block = mac.graph.create_block(
            vec![BlockPiece::Label {
                id: "label".into(),
                text: id.into(),
            }],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
            |id| InstructionKind::BlockHeader(id.into()),
        );
        mac.graph.variables = vec![
            VariableDef {
                name: "counter".into(),
                value: Evaluated::Number(3.0),
            },
            VariableDef {
                name: "status".into(),
                value: Evaluated::Text("ready".into()),
            },
        ];
        mac.graph
            .strands
            .iter_mut()
            .find(|s| s.block_header_id() == Some(block.as_str()))
            .unwrap()
            .instructions
            .extend([
                Instruction::new(InstructionKind::ChangeVariable(
                    "counter".into(),
                    Value::number(1.0),
                )),
                Instruction::new(InstructionKind::SetVariable(
                    "status".into(),
                    Value::text("done"),
                )),
            ]);
        pack_from_macro(
            &mac,
            &[block],
            PackInfo {
                id: id.into(),
                name: id.into(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn new_installs_own_state_and_leave_user_names_alone() {
        let mut mac = Macro::new("User".into(), String::new(), vec![]);
        mac.graph.variables.push(VariableDef {
            name: "counter".into(),
            value: Evaluated::Number(99.0),
        });
        merge_into(&mut mac, &pack("one"), "1");
        merge_into(&mut mac, &pack("two"), "2");
        assert_eq!(mac.variables.len(), 1);
        assert_eq!(mac.variables[0].value, Evaluated::Number(99.0));
        assert_ne!(
            mac.settings.block_packs[0].variable_key("counter"),
            mac.settings.block_packs[1].variable_key("counter")
        );
        assert_eq!(
            crate::frontend::macro_to_dto(&mac).variables,
            vec!["counter"]
        );
    }

    #[test]
    fn legacy_migration_preserves_values_external_access_and_is_idempotent() {
        let pack = pack("legacy");
        let mut mac = Macro::new(
            "User".into(),
            String::new(),
            vec![Instruction::new(InstructionKind::SetVariable(
                "status".into(),
                Value::text("user value"),
            ))],
        );
        merge_into(&mut mac, &pack, "1");
        mac.graph.variables = pack.content.variables.clone();
        mac.graph
            .variables
            .iter_mut()
            .find(|v| v.name == "counter")
            .unwrap()
            .value = Evaluated::Number(42.0);
        let record = &mut mac.settings.block_packs[0];
        record.variables.clear();
        record.variables_scoped = false;
        let mut json = serde_json::to_value(&mac).unwrap();
        json["settings"]["block_packs"][0]
            .as_object_mut()
            .unwrap()
            .remove("variables");
        json["settings"]["block_packs"][0]
            .as_object_mut()
            .unwrap()
            .remove("variables_scoped");
        let (mut loaded, _) = migrate::from_value(json).unwrap();
        assert_eq!(
            loaded
                .variables
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            vec!["status"]
        );
        assert_eq!(
            loaded.settings.block_packs[0].variables[0].value,
            Evaluated::Number(42.0)
        );
        let saved = migrate::to_value(&loaded).unwrap();
        migrate_variables(&mut loaded);
        assert_eq!(migrate::to_value(&loaded).unwrap(), saved);
        merge_into(&mut loaded, &pack, "2");
        assert_eq!(loaded.variables.len(), 1);
        assert_eq!(loaded.settings.block_packs[0].variables.len(), 1);
        assert_eq!(
            loaded.settings.block_packs[0].variables[0].value,
            Evaluated::Number(42.0)
        );
    }

    #[test]
    fn interfaces_and_values_survive_updates_round_trips_and_removal() {
        let mut pack = pack("interfaces");
        pack.variable_roles = vec![
            VariableInterface {
                name: "counter".into(),
                role: VariableRole::Output,
            },
            VariableInterface {
                name: "status".into(),
                role: VariableRole::Setting,
            },
        ];
        let mut mac = Macro::new("User".into(), String::new(), vec![]);
        merge_into(&mut mac, &pack, "1");
        let record = &mac.settings.block_packs[0];
        let output = record.output_id("counter");
        assert_eq!(
            mac.block_def(&output).unwrap().shape,
            BlockShape::ReturnsValue
        );
        let key = record.variable_key("counter");
        mac.sync_variables_from(&HashMap::from([(key, Evaluated::Number(17.0))]));
        merge_into(&mut mac, &pack, "2");
        let (loaded, _) = migrate::from_value(migrate::to_value(&mac).unwrap()).unwrap();
        assert_eq!(
            loaded.settings.block_packs[0]
                .variables
                .iter()
                .find(|v| v.name == "counter")
                .unwrap()
                .value,
            Evaluated::Number(17.0)
        );
        assert!(loaded.variables.is_empty());
        assert!(loaded.settings.block_packs[0].blocks.contains(&output));
        remove_from(&mut mac, "interfaces").unwrap();
        assert!(mac.settings.block_packs.is_empty());
        assert!(mac.block_def(&output).is_none());
    }

    #[test]
    fn renamed_variables_stay_private_and_drop_the_old_names() {
        fn rename(pack: &mut BlockPack, from: &str, to: &str) {
            for var in &mut pack.content.graph.variables {
                if var.name == from {
                    var.name = to.into();
                }
            }
            for strand in &mut pack.content.graph.strands {
                for ins in &mut strand.instructions {
                    ins.kind.visit_refs_mut(&mut |_, name| {
                        if name == from {
                            *name = to.into();
                        }
                    });
                }
            }
        }
        let mut old = pack("renamed");
        rename(&mut old, "counter", "Author renamed: counter");
        let mut mac = Macro::new("User".into(), String::new(), vec![]);
        mac.graph.variables.push(VariableDef {
            name: "counter".into(),
            value: Evaluated::Number(99.0),
        });
        merge_into(&mut mac, &old, "1");
        merge_into(&mut mac, &pack("renamed"), "2");
        assert_eq!(mac.variables.len(), 1);
        assert_eq!(mac.variables[0].value, Evaluated::Number(99.0));
        let mut names: Vec<_> = mac.settings.block_packs[0]
            .variables
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        names.sort();
        assert_eq!(names, vec!["counter", "status"]);
    }

    #[test]
    fn legacy_shared_names_are_not_split_between_packs() {
        let mut mac = Macro::new("User".into(), String::new(), vec![]);
        let one = pack("one");
        merge_into(&mut mac, &one, "1");
        merge_into(&mut mac, &pack("two"), "2");
        mac.graph.variables = one.content.variables.clone();
        for record in &mut mac.settings.block_packs {
            record.variables.clear();
            record.variables_scoped = false;
        }
        migrate_variables(&mut mac);
        assert_eq!(mac.variables.len(), 2);
        assert!(
            mac.settings
                .block_packs
                .iter()
                .all(|p| p.variables.is_empty())
        );
    }

    #[test]
    #[cfg(feature = "system")]
    fn pack_files_keep_the_interface_and_legacy_files_default_to_internal() {
        let mut pack = pack("roles");
        pack.variable_roles.push(VariableInterface {
            name: "counter".into(),
            role: VariableRole::Output,
        });
        let text = write_pack(&pack).unwrap();
        assert_eq!(
            read_pack(&text).unwrap().variable_roles,
            pack.variable_roles
        );
        let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
        json.as_object_mut().unwrap().remove("variable_roles");
        assert!(
            read_pack(&json.to_string())
                .unwrap()
                .variable_roles
                .is_empty()
        );
    }
}
