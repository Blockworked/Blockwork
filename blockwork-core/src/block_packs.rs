//! Block packs: custom blocks shared as one file, and the library indexes
//! that list them. An index pins every pack by its SHA-256, so a download
//! that doesn't match is refused.

use crate::input::value::Value;
use crate::macros::{BlockKind, InstructionKind, Macro, MacroGraph, Strand, migrate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

mod variables;
pub use variables::*;

/// Top-level key marking a block pack file.
pub const PACK_KEY: &str = "blockwork_block_pack";
/// Top-level key marking a library index.
pub const INDEX_KEY: &str = "blockwork_block_index";
/// The library that ships with Blockwork, served by community.blockworked.com.
pub const DEFAULT_INDEX: &str = "https://community.blockworked.com/index.json";
/// Earlier built-in addresses, swapped for `DEFAULT_INDEX` in saved settings.
pub const OLD_DEFAULT_INDEXES: [&str; 2] = [
    "https://raw.githubusercontent.com/Blockworked/Blockwork/master/community/blocks/index.json",
    "https://raw.githubusercontent.com/Blockworked/Blockwork-community-blocks/master/index.json",
];
/// Packs and indexes over this size are refused.
pub const MAX_DOWNLOAD: u64 = 2 * 1024 * 1024;

/// A pack's description, as its file and the index give it.
#[derive(Debug, Clone, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct PackInfo {
    /// Stable id, so a newer version replaces the blocks an older one added.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
}

/// Last successful export of a pack authored in this macro.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct PackExport {
    pub info: PackInfo,
    pub block_ids: Vec<String>,
    #[serde(default)]
    pub variable_roles: Vec<VariableInterface>,
}

/// A pack: some custom blocks with their bodies, plus the variables and
/// lists those bodies use.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockPack {
    pub info: PackInfo,
    /// Holds only the blocks, as a macro document so file upgrades apply.
    pub content: Macro,
    /// What upgrading an older pack changed.
    pub notes: Vec<String>,
    pub variable_roles: Vec<VariableInterface>,
}

/// One pack an index lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    #[serde(flatten)]
    pub info: PackInfo,
    /// Where the pack file is; relative to the index's own address.
    pub url: String,
    /// SHA-256 of the pack file, in hex.
    pub sha256: String,
    /// The block names, for showing before download.
    #[serde(default)]
    pub blocks: Vec<String>,
    /// Published by the Blockworked team. Only believed from the built-in
    /// library, since any index could claim it.
    #[serde(default)]
    pub official: bool,
    /// What its blocks can do, by capability kind, for filtering before download.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// When its current version and its first one were shared, in ms since 1970.
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub comments: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    pub name: String,
    pub packs: Vec<IndexEntry>,
}

/// Which pack a macro's blocks came from, so the library can offer updates.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct PackRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// Ids of the blocks it added.
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub variables: Vec<PackVariable>,
    #[serde(default)]
    pub variables_scoped: bool,
}

/// The installed pack that owns a definition, including older imported packs.
pub fn owner<'a>(mac: &'a Macro, block_id: &str) -> Option<&'a PackRecord> {
    mac.settings
        .block_packs
        .iter()
        .find(|pack| pack.blocks.iter().any(|id| id == block_id))
}

pub fn is_pack_strand(mac: &Macro, strand: &Strand) -> bool {
    strand
        .block_header_id()
        .is_some_and(|id| owner(mac, id).is_some())
}

pub fn check_editable(mac: &Macro, block_id: &str) -> Result<(), String> {
    if owner(mac, block_id).is_some() {
        return Err(
            "Community blocks are managed as packs. Remove the pack in the block library".into(),
        );
    }
    Ok(())
}

/// Separates the author from the block's own name in a Details id, so a
/// block's own identifier can't contain it.
pub const ID_SEPARATOR: char = ':';

/// Refuses a block identifier with the author separator in it.
pub fn check_identifier(identifier: &str) -> Result<(), String> {
    if identifier.contains(ID_SEPARATOR) {
        return Err(format!(
            "Block IDs can't contain \"{ID_SEPARATOR}\". Your name is added in front when the block is shared"
        ));
    }
    Ok(())
}

fn id_part(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The block's own name in a Details id: any "Author:" (or older
/// "Author_") prefix dropped, letters and digits only.
pub fn bare_identifier(author: &str, name: &str) -> String {
    let name = name.rsplit(ID_SEPARATOR).next().unwrap_or(name);
    let author = id_part(author);
    let name = if author.is_empty() {
        name
    } else {
        name.strip_prefix(&format!("{author}_")).unwrap_or(name)
    };
    let name = id_part(name);
    if name.is_empty() {
        "Block".into()
    } else {
        name
    }
}

/// Public Details ids are "Author:BlockName".
pub fn details_identifier(author: &str, name: &str) -> String {
    let bare = bare_identifier(author, name);
    let author = id_part(author);
    let author = if author.is_empty() {
        "Community"
    } else {
        &author
    };
    format!("{author}{ID_SEPARATOR}{bare}")
}

fn def_name(def: &blockstitch_core::graph::BlockDef) -> String {
    if def.identifier.is_empty() {
        block_name(def)
    } else {
        def.identifier.clone()
    }
}

/// Installed blocks show their author in front of their id.
fn apply_details(content: &mut Macro, info: &PackInfo) {
    for def in &mut content.graph.block_defs {
        def.identifier = details_identifier(&info.author, &def_name(def));
        if def.description.trim().is_empty() {
            def.description = info.description.clone();
        }
    }
}

/// A shared file carries bare ids; the library puts the uploader's name in
/// front, and an install the pack's author.
fn export_details(content: &mut Macro, info: &PackInfo) {
    for def in &mut content.graph.block_defs {
        def.identifier = bare_identifier(&info.author, &def_name(def));
        if def.description.trim().is_empty() {
            def.description = info.description.clone();
        }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Ids of `roots` and every custom block their bodies call, in `mac`.
fn with_dependencies(mac: &Macro, roots: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut queue: Vec<String> = roots.to_vec();
    while let Some(id) = queue.pop() {
        if out.contains(&id) || mac.block_def(&id).is_none() {
            continue;
        }
        if let Some(body) = mac.block_body(&id) {
            for ins in &body.instructions {
                ins.walk(&mut |ins| {
                    if let InstructionKind::CallBlock { block_id, .. } = &ins.kind {
                        queue.push(block_id.clone());
                    }
                    let mut kind = ins.kind.clone();
                    kind.visit_values_mut(&mut |value, _| called_blocks(value, &mut queue));
                });
            }
        }
        out.push(id);
    }
    // Keep the macro's own order.
    mac.block_defs
        .iter()
        .map(|b| b.id.clone())
        .filter(|id| out.contains(id))
        .collect()
}

fn called_blocks(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Call { block_id, args, .. } => {
            out.push(block_id.clone());
            args.iter().for_each(|a| called_blocks(a, out));
        }
        Value::Op { args, .. } => args.iter().for_each(|a| called_blocks(a, out)),
        _ => {}
    }
}

/// Variable names a body reads or writes, and text leaves (list blocks name
/// their list with one).
fn names_used(strands: &[Strand]) -> HashSet<String> {
    fn walk_value(value: &Value, out: &mut HashSet<String>) {
        match value {
            Value::Var { name } => {
                out.insert(name.clone());
            }
            Value::Text { value } => {
                out.insert(value.clone());
            }
            Value::Op { args, .. } => args.iter().for_each(|a| walk_value(a, out)),
            Value::Call { args, branches, .. } => {
                args.iter().for_each(|a| walk_value(a, out));
                for branch in branches {
                    if let Ok(body) = crate::frontend::reporter_branch_to_instructions(branch) {
                        out.extend(names_used(&[Strand::with_instructions(0, 0, body)]));
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = HashSet::new();
    for strand in strands {
        for ins in &strand.instructions {
            ins.walk(&mut |ins| {
                ins.kind.visit_refs(&mut |_, name| {
                    out.insert(name.to_string());
                });
                let mut kind = ins.kind.clone();
                kind.visit_values_mut(&mut |value, _| walk_value(value, &mut out));
            });
        }
    }
    out
}

/// A pack of `block_ids` from `mac`, with every block they call.
pub fn pack_from_macro(
    mac: &Macro,
    block_ids: &[String],
    info: PackInfo,
) -> Result<BlockPack, String> {
    let ids = with_dependencies(mac, block_ids);
    if ids.is_empty() {
        return Err("Pick at least one of this macro's blocks".into());
    }
    for id in &ids {
        check_editable(mac, id)?;
    }
    let strands: Vec<Strand> = ids
        .iter()
        .filter_map(|id| mac.block_body(id).cloned())
        .collect();
    let used = names_used(&strands);
    let used_variables = variable_names_used(&strands);
    let instruction_ids: HashSet<String> = {
        let mut set = HashSet::new();
        for s in &strands {
            for ins in &s.instructions {
                ins.walk(&mut |i| {
                    set.insert(i.id.clone());
                });
            }
        }
        set
    };
    let mut content = Macro::new(info.name.clone(), info.description.clone(), Vec::new());
    content.id = String::new();
    content.graph = MacroGraph {
        block_defs: mac
            .block_defs
            .iter()
            .filter(|b| ids.contains(&b.id))
            .cloned()
            .collect(),
        strands,
        // Their starting values travel too, minus anything secret.
        variables: {
            let mut vars: Vec<_> = mac
                .variables
                .iter()
                .filter(|v| used_variables.contains(&v.name))
                .cloned()
                .collect();
            let mut missing: Vec<_> = used_variables
                .into_iter()
                .filter(|name| !vars.iter().any(|v| &v.name == name))
                .collect();
            missing.sort();
            vars.extend(missing.into_iter().map(|name| crate::macros::VariableDef {
                name,
                value: crate::input::value::Evaluated::Number(0.0),
            }));
            vars
        },
        lists: mac
            .lists
            .iter()
            .filter(|l| used.contains(&l.name))
            .cloned()
            .collect(),
        comments: mac
            .comments
            .iter()
            .filter(|c| {
                c.attached_to
                    .as_ref()
                    .is_some_and(|id| instruction_ids.contains(id))
            })
            .cloned()
            .collect(),
        ..MacroGraph::new()
    };
    let info = clean_info(info)?;
    for def in &content.block_defs {
        crate::macros::BlockDef::validate_input_defaults(&def.pieces)?;
    }
    export_details(&mut content, &info);
    Ok(BlockPack {
        info,
        content,
        notes: Vec::new(),
        variable_roles: Vec::new(),
    })
}

fn clean_info(mut info: PackInfo) -> Result<PackInfo, String> {
    info.name = info.name.trim().to_string();
    if info.name.is_empty() {
        return Err("Give the pack a name".into());
    }
    if info.id.trim().is_empty() {
        info.id = info.name.clone();
    }
    info.id = slug(&info.id);
    info.version = info.version.trim().to_string();
    info.author = info.author.trim().to_string();
    info.description = info.description.trim().to_string();
    Ok(info)
}

/// Lowercase letters, digits and dashes.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// The pack file's text; secret values are masked like an export. Needs the
/// keyring, so it isn't in the headless build.
#[cfg(feature = "system")]
pub fn write_pack(pack: &BlockPack) -> Result<String, String> {
    let mut content = pack.content.clone();
    export_details(&mut content, &pack.info);
    for var in &mut content.graph.variables {
        if let crate::input::value::Evaluated::Text(t) = &mut var.value {
            *t = crate::secrets::redact(t);
        }
    }
    let mut value = serde_json::to_value(&pack.info).map_err(|e| e.to_string())?;
    let object = value.as_object_mut().ok_or("bad pack info")?;
    object.insert(PACK_KEY.into(), 1.into());
    object.insert(
        "variable_roles".into(),
        serde_json::to_value(&pack.variable_roles).map_err(|e| e.to_string())?,
    );
    object.insert("blocks".into(), migrate::to_value(&content)?);
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

pub fn read_pack(text: &str) -> Result<BlockPack, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("This isn't a block pack: {e}"))?;
    let Some(version) = value.get(PACK_KEY) else {
        return Err("This isn't a block pack".into());
    };
    if version.as_u64().is_none_or(|v| v > 1) {
        return Err(
            "This block pack was made by a newer version of Blockwork. Update Blockwork to use it"
                .into(),
        );
    }
    let info: PackInfo = serde_json::from_value(value.clone())
        .map_err(|e| format!("The block pack is broken: {e}"))?;
    let info = clean_info(info)?;
    let blocks = value
        .get("blocks")
        .cloned()
        .ok_or("The block pack has no blocks")?;
    let (mut content, upgrade) =
        migrate::from_value(blocks).map_err(|e| format!("The block pack is broken: {e}"))?;
    // Only blocks and what they use; any other strand is dropped.
    let defs: HashSet<String> = content.block_defs.iter().map(|b| b.id.clone()).collect();
    content
        .graph
        .strands
        .retain(|s| s.block_header_id().is_some_and(|id| defs.contains(id)));
    content.graph.floating_values.clear();
    if content.block_defs.is_empty() {
        return Err("The block pack has no blocks".into());
    }
    for def in &content.block_defs {
        crate::macros::BlockDef::validate_input_defaults(&def.pieces)?;
    }
    apply_details(&mut content, &info);
    let variable_roles: Vec<VariableInterface> = value
        .get("variable_roles")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("Invalid pack variable interface: {e}"))?
        .unwrap_or_default();
    Ok(BlockPack {
        info,
        content,
        notes: upgrade.notes,
        variable_roles,
    })
}

pub fn read_index(text: &str, base_url: &str) -> Result<Index, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("This isn't a block library: {e}"))?;
    let Some(version) = value.get(INDEX_KEY) else {
        return Err("This isn't a block library".into());
    };
    if version.as_u64().is_none_or(|v| v > 1) {
        return Err("This block library needs a newer version of Blockwork".into());
    }
    let name = value
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("Block library")
        .to_string();
    let mut packs = Vec::new();
    for entry in value
        .get("packs")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
    {
        // A broken entry is skipped rather than hiding the whole library.
        let Ok(mut entry) = serde_json::from_value::<IndexEntry>(entry.clone()) else {
            continue;
        };
        entry.url = resolve_url(base_url, &entry.url);
        entry.sha256 = entry.sha256.trim().to_ascii_lowercase();
        if entry.sha256.len() != 64 || entry.info.id.is_empty() {
            continue;
        }
        entry.official &= base_url == DEFAULT_INDEX;
        packs.push(entry);
    }
    Ok(Index { name, packs })
}

/// `url` relative to the index at `base`.
pub fn resolve_url(base: &str, url: &str) -> String {
    let url = url.trim();
    if url.contains("://") {
        return url.to_string();
    }
    if let Some(rest) = url.strip_prefix('/') {
        // Root-relative: keep the scheme and host.
        let origin_end = base
            .find("://")
            .map(|i| i + 3)
            .and_then(|start| base[start..].find('/').map(|j| start + j));
        return format!("{}/{rest}", origin_end.map_or(base, |end| &base[..end]));
    }
    // Local Windows paths use backslashes too.
    let separators: &[char] = if base.contains("://") {
        &['/']
    } else {
        &['/', '\\']
    };
    let dir = base.rfind(separators).map_or(base, |i| &base[..i]);
    format!("{dir}/{}", url.trim_start_matches("./"))
}

/// Checks a download against the hash its index pins.
pub fn verify(bytes: &[u8], sha256: &str) -> Result<(), String> {
    if sha256_hex(bytes) != sha256.trim().to_ascii_lowercase() {
        return Err(
            "The download doesn't match the checksum the library lists, so it wasn't used".into(),
        );
    }
    Ok(())
}

/// What adding a pack did to a macro.
#[derive(Debug, Default, PartialEq)]
pub struct MergeReport {
    pub added: Vec<String>,
    pub updated: Vec<String>,
}

/// The block's label text, for lists.
pub fn block_name(def: &blockstitch_core::graph::BlockDef) -> String {
    use blockstitch_core::graph::BlockPiece;
    let words: Vec<String> = def
        .pieces
        .iter()
        .map(|p| match p {
            BlockPiece::Label { text, .. } => text.trim().to_string(),
            BlockPiece::Input { name, .. } | BlockPiece::Branch { name, .. } => format!("({name})"),
        })
        .filter(|w| !w.is_empty())
        .collect();
    words.join(" ")
}

/// Which of the pack's blocks `mac` already has, by name.
pub fn existing_blocks(mac: &Macro, pack: &BlockPack) -> Vec<String> {
    pack.content
        .block_defs
        .iter()
        .filter(|b| mac.block_def(&b.id).is_some())
        .map(block_name)
        .collect()
}

/// Adds the pack's blocks to `mac`. Blocks it already has from an earlier
/// version are replaced in place, keeping their call sites.
pub fn merge_into(mac: &mut Macro, pack: &BlockPack, sha256: &str) -> MergeReport {
    migrate_variables(mac);
    let previous = mac
        .settings
        .block_packs
        .iter()
        .find(|p| p.id == pack.info.id)
        .cloned();
    // Names the installed bodies read, before the new bodies replace them.
    let old_names = previous
        .as_ref()
        .map(|p| {
            let strands: Vec<_> = mac
                .strands
                .iter()
                .filter(|s| {
                    s.block_header_id()
                        .is_some_and(|id| p.blocks.iter().any(|b| b == id))
                })
                .cloned()
                .collect();
            variable_names_used(&strands)
        })
        .unwrap_or_default();
    let mut report = MergeReport::default();
    // New bodies go to the right of everything on the canvas.
    let right = mac.strands.iter().map(|s| s.x).max().map_or(0, |x| x + 420);
    let mut y = 40;
    for def in &pack.content.block_defs {
        let body = pack.content.block_body(&def.id).cloned();
        if let Some(old) = mac.block_def(&def.id).cloned() {
            mac.graph
                .reconcile_block_call_args(&def.id, &old.pieces, &def.pieces);
            if let Some(slot) = mac.graph.block_def_mut(&def.id) {
                *slot = def.clone();
            }
            if let Some(body) = body {
                match mac
                    .graph
                    .strands
                    .iter_mut()
                    .find(|s| s.block_header_id() == Some(def.id.as_str()))
                {
                    Some(strand) => strand.instructions = body.instructions,
                    None => mac.graph.strands.push(Strand {
                        x: right,
                        y,
                        ..body
                    }),
                }
            }
            report.updated.push(block_name(def));
        } else {
            mac.graph.block_defs.push(def.clone());
            if let Some(body) = body {
                let height = 60 + 44 * body.instructions.len() as i32;
                mac.graph.strands.push(Strand {
                    id: blockstitch_core::graph::new_id(),
                    x: right,
                    y,
                    ..body
                });
                y += height;
            }
            report.added.push(block_name(def));
        }
    }
    let mut variables = previous
        .as_ref()
        .map(|p| p.variables.clone())
        .unwrap_or_default();
    let mut declared = pack.content.variables.clone();
    let mut used_names: Vec<_> = variable_names_used(&pack.content.strands)
        .into_iter()
        .collect();
    used_names.sort();
    for name in used_names {
        if !declared.iter().any(|v| v.name == name) {
            declared.push(crate::macros::VariableDef {
                name,
                value: crate::input::value::Evaluated::Number(0.0),
            });
        }
    }
    for var in &declared {
        // Existing shared variables remain shared when updating an old install.
        // A name the old bodies never read is new and stays private, even if
        // the user has a variable called that.
        let shared = old_names.contains(&var.name)
            && !variables.iter().any(|v| v.name == var.name)
            && mac.variables.iter().any(|v| v.name == var.name);
        if shared {
            continue;
        }
        let role = pack
            .variable_roles
            .iter()
            .find(|r| r.name == var.name)
            .map(|r| r.role)
            .unwrap_or_default();
        if let Some(old) = variables.iter_mut().find(|v| v.name == var.name) {
            // Keep old outputs callable even when a newer pack hides them.
            if old.role != VariableRole::Output {
                old.role = role;
            }
        } else {
            variables.push(PackVariable {
                name: var.name.clone(),
                value: var.value.clone(),
                role,
            });
        }
    }
    // Private scratch the new version no longer uses, e.g. renamed variables.
    variables
        .retain(|v| v.role != VariableRole::Internal || declared.iter().any(|d| d.name == v.name));
    for list in &pack.content.lists {
        if !mac.lists.iter().any(|l| l.name == list.name) {
            mac.graph.lists.push(blockstitch_core::graph::ListDef {
                editor_visible: false,
                ..list.clone()
            });
        }
    }
    let live = mac.graph.instruction_ids();
    for comment in &pack.content.comments {
        if comment
            .attached_to
            .as_ref()
            .is_some_and(|id| live.contains(id))
            && !mac.comments.iter().any(|c| c.id == comment.id)
        {
            mac.graph.comments.push(comment.clone());
        }
    }
    mac.settings.block_packs.retain(|r| r.id != pack.info.id);
    mac.settings.block_packs.push(PackRecord {
        id: pack.info.id.clone(),
        name: pack.info.name.clone(),
        version: pack.info.version.clone(),
        sha256: sha256.to_string(),
        author: pack.info.author.clone(),
        description: pack.info.description.clone(),
        blocks: pack
            .content
            .block_defs
            .iter()
            .map(|b| b.id.clone())
            .collect(),
        variables,
        variables_scoped: true,
    });
    install_outputs(mac);
    report
}

/// Removes a pack's blocks and their calls, preserving blocks another pack owns.
pub fn remove_from(mac: &mut Macro, pack_id: &str) -> Result<(), String> {
    let record = mac
        .settings
        .block_packs
        .iter()
        .find(|r| r.id == pack_id)
        .cloned()
        .ok_or("That pack isn't installed in this macro")?;
    mac.settings.block_packs.retain(|r| r.id != pack_id);
    for id in record.blocks {
        if !mac
            .settings
            .block_packs
            .iter()
            .any(|r| r.blocks.contains(&id))
        {
            mac.graph.remove_block(&id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
