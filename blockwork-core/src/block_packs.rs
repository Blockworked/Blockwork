//! Block packs: custom blocks shared as one file, and the library indexes
//! that list them. An index pins every pack by its SHA-256, so a download
//! that doesn't match is refused.

use crate::input::value::Value;
use crate::macros::{migrate, BlockKind, InstructionKind, Macro, MacroGraph, Strand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

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
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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

/// A pack: some custom blocks with their bodies, plus the variables and
/// lists those bodies use.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockPack {
    pub info: PackInfo,
    /// Holds only the blocks, as a macro document so file upgrades apply.
    pub content: Macro,
    /// What upgrading an older pack changed.
    pub notes: Vec<String>,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    pub name: String,
    pub packs: Vec<IndexEntry>,
}

/// Which pack a macro's blocks came from, so the library can offer updates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}

/// The installed pack that owns a definition, including older imported packs.
pub fn owner<'a>(mac: &'a Macro, block_id: &str) -> Option<&'a PackRecord> {
    mac.settings.block_packs.iter().find(|pack| pack.blocks.iter().any(|id| id == block_id))
}

pub fn is_pack_strand(mac: &Macro, strand: &Strand) -> bool {
    strand.block_header_id().is_some_and(|id| owner(mac, id).is_some())
}

pub fn check_editable(mac: &Macro, block_id: &str) -> Result<(), String> {
    if owner(mac, block_id).is_some() {
        return Err("Community blocks are managed as packs. Remove the pack in the block library".into());
    }
    Ok(())
}

/// Public Details ids use the author's name and chosen block name.
pub fn details_identifier(author: &str, name: &str) -> String {
    fn part(text: &str) -> String {
        text.chars().filter(|c| c.is_alphanumeric()).collect()
    }
    let author = part(author);
    let author = if author.is_empty() { "Community" } else { &author };
    let prefix = format!("{author}_");
    let name = name.strip_prefix(&prefix).unwrap_or(name);
    let name = part(name);
    format!("{prefix}{}", if name.is_empty() { "Block" } else { &name })
}

fn apply_details(content: &mut Macro, info: &PackInfo) {
    for def in &mut content.graph.block_defs {
        let name = if def.identifier.is_empty() { block_name(def) } else { def.identifier.clone() };
        def.identifier = details_identifier(&info.author, &name);
        if def.description.trim().is_empty() {
            def.description = info.description.clone();
        }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
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
    mac.block_defs.iter().map(|b| b.id.clone()).filter(|id| out.contains(id)).collect()
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
            Value::Op { args, .. } | Value::Call { args, .. } => args.iter().for_each(|a| walk_value(a, out)),
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
pub fn pack_from_macro(mac: &Macro, block_ids: &[String], info: PackInfo) -> Result<BlockPack, String> {
    let ids = with_dependencies(mac, block_ids);
    if ids.is_empty() {
        return Err("Pick at least one of this macro's blocks".into());
    }
    for id in &ids {
        check_editable(mac, id)?;
    }
    let strands: Vec<Strand> = ids.iter().filter_map(|id| mac.block_body(id).cloned()).collect();
    let used = names_used(&strands);
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
        block_defs: mac.block_defs.iter().filter(|b| ids.contains(&b.id)).cloned().collect(),
        strands,
        // Their starting values travel too, minus anything secret.
        variables: mac.variables.iter().filter(|v| used.contains(&v.name)).cloned().collect(),
        lists: mac.lists.iter().filter(|l| used.contains(&l.name)).cloned().collect(),
        comments: mac
            .comments
            .iter()
            .filter(|c| c.attached_to.as_ref().is_some_and(|id| instruction_ids.contains(id)))
            .cloned()
            .collect(),
        ..MacroGraph::new()
    };
    let info = clean_info(info)?;
    for def in &content.block_defs {
        crate::macros::BlockDef::validate_input_defaults(&def.pieces)?;
    }
    apply_details(&mut content, &info);
    Ok(BlockPack { info, content, notes: Vec::new() })
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

/// The pack file's text; secret values are masked like an export.
pub fn write_pack(pack: &BlockPack) -> Result<String, String> {
    let mut content = pack.content.clone();
    apply_details(&mut content, &pack.info);
    for var in &mut content.graph.variables {
        if let crate::input::value::Evaluated::Text(t) = &mut var.value {
            *t = crate::secrets::redact(t);
        }
    }
    let mut value = serde_json::to_value(&pack.info).map_err(|e| e.to_string())?;
    let object = value.as_object_mut().ok_or("bad pack info")?;
    object.insert(PACK_KEY.into(), 1.into());
    object.insert("blocks".into(), migrate::to_value(&content)?);
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

pub fn read_pack(text: &str) -> Result<BlockPack, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("This isn't a block pack: {e}"))?;
    let Some(version) = value.get(PACK_KEY) else {
        return Err("This isn't a block pack".into());
    };
    if version.as_u64().is_none_or(|v| v > 1) {
        return Err("This block pack was made by a newer version of Blockwork. Update Blockwork to use it".into());
    }
    let info: PackInfo = serde_json::from_value(value.clone()).map_err(|e| format!("The block pack is broken: {e}"))?;
    let info = clean_info(info)?;
    let blocks = value.get("blocks").cloned().ok_or("The block pack has no blocks")?;
    let (mut content, upgrade) = migrate::from_value(blocks).map_err(|e| format!("The block pack is broken: {e}"))?;
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
    Ok(BlockPack { info, content, notes: upgrade.notes })
}

pub fn read_index(text: &str, base_url: &str) -> Result<Index, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("This isn't a block library: {e}"))?;
    let Some(version) = value.get(INDEX_KEY) else {
        return Err("This isn't a block library".into());
    };
    if version.as_u64().is_none_or(|v| v > 1) {
        return Err("This block library needs a newer version of Blockwork".into());
    }
    let name = value.get("name").and_then(|n| n.as_str()).unwrap_or("Block library").to_string();
    let mut packs = Vec::new();
    for entry in value.get("packs").and_then(|p| p.as_array()).into_iter().flatten() {
        // A broken entry is skipped rather than hiding the whole library.
        let Ok(mut entry) = serde_json::from_value::<IndexEntry>(entry.clone()) else { continue };
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
        let origin_end = base.find("://").map(|i| i + 3).and_then(|start| base[start..].find('/').map(|j| start + j));
        return format!("{}/{rest}", origin_end.map_or(base, |end| &base[..end]));
    }
    // Local Windows paths use backslashes too.
    let separators: &[char] = if base.contains("://") { &['/'] } else { &['/', '\\'] };
    let dir = base.rfind(separators).map_or(base, |i| &base[..i]);
    format!("{dir}/{}", url.trim_start_matches("./"))
}

/// Checks a download against the hash its index pins.
pub fn verify(bytes: &[u8], sha256: &str) -> Result<(), String> {
    if sha256_hex(bytes) != sha256.trim().to_ascii_lowercase() {
        return Err("The download doesn't match the checksum the library lists, so it wasn't used".into());
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
    let mut report = MergeReport::default();
    // New bodies go to the right of everything on the canvas.
    let right = mac.strands.iter().map(|s| s.x).max().map_or(0, |x| x + 420);
    let mut y = 40;
    for def in &pack.content.block_defs {
        let body = pack.content.block_body(&def.id).cloned();
        if let Some(old) = mac.block_def(&def.id).cloned() {
            mac.graph.reconcile_block_call_args(&def.id, &old.pieces, &def.pieces);
            if let Some(slot) = mac.graph.block_def_mut(&def.id) {
                *slot = def.clone();
            }
            if let Some(body) = body {
                match mac.graph.strands.iter_mut().find(|s| s.block_header_id() == Some(def.id.as_str())) {
                    Some(strand) => strand.instructions = body.instructions,
                    None => mac.graph.strands.push(Strand { x: right, y, ..body }),
                }
            }
            report.updated.push(block_name(def));
        } else {
            mac.graph.block_defs.push(def.clone());
            if let Some(body) = body {
                let height = 60 + 44 * body.instructions.len() as i32;
                mac.graph.strands.push(Strand { id: blockstitch_core::graph::new_id(), x: right, y, ..body });
                y += height;
            }
            report.added.push(block_name(def));
        }
    }
    for var in &pack.content.variables {
        if !mac.variables.iter().any(|v| v.name == var.name) {
            mac.graph.variables.push(var.clone());
        }
    }
    for list in &pack.content.lists {
        if !mac.lists.iter().any(|l| l.name == list.name) {
            mac.graph.lists.push(blockstitch_core::graph::ListDef { editor_visible: false, ..list.clone() });
        }
    }
    let live = mac.graph.instruction_ids();
    for comment in &pack.content.comments {
        if comment.attached_to.as_ref().is_some_and(|id| live.contains(id)) && !mac.comments.iter().any(|c| c.id == comment.id) {
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
        blocks: pack.content.block_defs.iter().map(|b| b.id.clone()).collect(),
    });
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
mod tests {
    use super::*;
    use crate::macros::Instruction;
    use blockstitch_core::graph::{BlockPiece, BlockShape};

    fn label(text: &str) -> BlockPiece {
        BlockPiece::Label { id: blockstitch_core::graph::new_id(), text: text.into() }
    }

    fn make_block(mac: &mut Macro, name: &str, body: Vec<InstructionKind>) -> String {
        let id = mac.graph.create_block(vec![label(name)], BlockShape::Normal, "#4C97FF".into(), 0, 0, |id| {
            InstructionKind::BlockHeader(id.to_string())
        });
        let strand = mac.graph.strands.iter_mut().find(|s| s.block_header_id() == Some(id.as_str())).unwrap();
        strand.instructions.extend(body.into_iter().map(Instruction::new));
        id
    }

    fn call(id: &str) -> InstructionKind {
        InstructionKind::CallBlock { block_id: id.into(), args: Vec::new(), branches: Vec::new() }
    }

    #[test]
    fn input_defaults_survive_pack_updates_without_resetting_calls() {
        use crate::input::value::Evaluated;
        let mut author = Macro::new("author".into(), String::new(), vec![]);
        let id = author.create_block(vec![label("example"), BlockPiece::Input {
            id: "input".into(), name: "text".into(), value_type: Default::default(),
            default_value: Some(Evaluated::Text("example".into())),
        }], crate::macros::BlockShape::Normal, "#4C97FF".into(), 0, 0);
        let info = PackInfo { id: "example".into(), name: "Example".into(), ..PackInfo::default() };
        let pack = pack_from_macro(&author, &[id.clone()], info).unwrap();
        let mut pack = read_pack(&write_pack(&pack).unwrap()).unwrap();
        assert_eq!(pack.content.block_def(&id).unwrap().input_defaults().collect::<Vec<_>>(), vec![Value::text("example")]);
        let mut target = Macro::new("target".into(), String::new(), vec![]);
        merge_into(&mut target, &pack, "");
        target.graph.strands.push(Strand::with_instructions(0, 0, vec![crate::macros::Instruction::new(InstructionKind::CallBlock {
            block_id: id.clone(), args: vec![Value::text("edited")], branches: vec![],
        })]));
        if let BlockPiece::Input { default_value, .. } = &mut pack.content.graph.block_defs[0].pieces[1] {
            *default_value = Some(Evaluated::Text("new default".into()));
        }
        pack.content.graph.block_defs[0].pieces.push(BlockPiece::Input {
            id: "attempts".into(), name: "attempts".into(), value_type: Default::default(),
            default_value: Some(Evaluated::Number(3.0)),
        });
        merge_into(&mut target, &pack, "");
        let call = target.strands.last().unwrap().instructions.first().unwrap();
        let InstructionKind::CallBlock { args, .. } = &call.kind else { panic!("call") };
        assert_eq!(args, &vec![Value::text("edited"), Value::number(3.0)]);
        assert_eq!(target.block_def(&id).unwrap().input_defaults().collect::<Vec<_>>(), vec![Value::text("new default"), Value::number(3.0)]);
        let saved = migrate::to_value(&target).unwrap();
        let (loaded, _) = migrate::from_value(saved).unwrap();
        assert_eq!(loaded.block_def(&id).unwrap(), target.block_def(&id).unwrap());
    }

    #[test]
    fn packs_take_dependencies_and_update_in_place() {
        let mut author = Macro::new("Author".into(), String::new(), Vec::new());
        let inner = make_block(&mut author, "inner", vec![InstructionKind::Log(Value::text("hi"))]);
        let outer = make_block(&mut author, "outer", vec![call(&inner)]);
        let _unrelated = make_block(&mut author, "unrelated", Vec::new());
        let info = PackInfo { id: String::new(), name: "My Pack".into(), version: "1.0".into(), ..Default::default() };
        let pack = pack_from_macro(&author, &[outer.clone()], info).unwrap();
        assert_eq!(pack.info.id, "my-pack");
        assert_eq!(pack.content.block_defs.len(), 2, "outer and the inner block it calls");

        let text = write_pack(&pack).unwrap();
        let read = read_pack(&text).unwrap();
        assert_eq!(read.content.block_defs, pack.content.block_defs);

        let mut user = Macro::new("User".into(), String::new(), vec![Instruction::new(call(&outer))]);
        let report = merge_into(&mut user, &read, &sha256_hex(text.as_bytes()));
        assert_eq!(report.added.len(), 2);
        assert_eq!(user.block_defs.len(), 2);
        assert_eq!(user.settings.block_packs[0].version, "1.0");

        // Version 2 changes outer's body; the user's copy is replaced, not doubled.
        let strand = author.graph.strands.iter_mut().find(|s| s.block_header_id() == Some(outer.as_str())).unwrap();
        strand.instructions.push(Instruction::new(InstructionKind::Log(Value::text("v2"))));
        let info = PackInfo { name: "My Pack".into(), version: "2.0".into(), ..Default::default() };
        let v2 = pack_from_macro(&author, &[outer.clone()], info).unwrap();
        let report = merge_into(&mut user, &v2, "");
        assert_eq!((report.added.len(), report.updated.len()), (0, 2));
        assert_eq!(user.block_defs.len(), 2);
        assert_eq!(user.block_body(&outer).unwrap().instructions.len(), 3);
        assert_eq!(user.settings.block_packs.len(), 1);
        assert_eq!(user.settings.block_packs[0].version, "2.0");
    }

    #[test]
    fn packs_carry_every_output_variable_of_nested_blocks() {
        let mut author = Macro::new("Author".into(), String::new(), Vec::new());
        for name in ["hit x", "hit y", "count", "unused"] {
            author.create_variable(name).unwrap();
        }
        author.create_list("boxes").unwrap();
        let find = InstructionKind::FindImage {
            image: String::new(),
            tolerance: Value::number(0.0),
            x_variable: "hit x".into(),
            y_variable: "hit y".into(),
        };
        let detect = InstructionKind::DetectObjectsInWindow {
            pattern: Value::text(""),
            by: "title".into(),
            detector: Value::text(""),
            label: Value::text(""),
            confidence: Value::number(50.0),
            sort: "score".into(),
            results_list: "boxes".into(),
            count_variable: "count".into(),
            region: String::new(),
        };
        let repeat = InstructionKind::Repeat {
            count: Value::number(2.0),
            body: vec![Instruction::new(find), Instruction::new(detect)],
        };
        let id = make_block(&mut author, "look", vec![repeat]);
        let pack = pack_from_macro(&author, &[id], PackInfo { name: "Look".into(), ..Default::default() }).unwrap();
        let names: Vec<_> = pack.content.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["hit x", "hit y", "count"]);
        assert_eq!(pack.content.lists.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["boxes"]);
    }

    #[test]
    fn removing_packs_scrubs_nested_calls_and_preserves_shared_blocks() {
        let mut author = Macro::new("Author".into(), String::new(), Vec::new());
        let shared = make_block(&mut author, "shared", Vec::new());
        let outer = make_block(&mut author, "outer", vec![call(&shared)]);
        let pack = pack_from_macro(&author, &[outer.clone()], PackInfo {
            id: "first".into(), name: "First".into(), ..Default::default()
        }).unwrap();
        let other = pack_from_macro(&author, &[shared.clone()], PackInfo {
            id: "second".into(), name: "Second".into(), ..Default::default()
        }).unwrap();
        let mut mac = Macro::new("User".into(), String::new(), vec![
            Instruction::new(call(&outer)),
            Instruction::new(InstructionKind::Repeat {
                count: Value::number(2.0), body: vec![Instruction::new(call(&outer))],
            }),
            Instruction::new(InstructionKind::Log(Value::Call {
                block_id: outer.clone(), args: vec![], branches: vec![], saved: Box::new(Value::number(0.0)),
            })),
        ]);
        merge_into(&mut mac, &pack, "");
        merge_into(&mut mac, &other, "");
        let before = mac.clone();
        assert!(remove_from(&mut mac, "missing").is_err());
        assert_eq!(mac, before);
        remove_from(&mut mac, "first").unwrap();
        assert!(mac.block_def(&outer).is_none());
        assert!(mac.block_body(&outer).is_none());
        assert!(mac.block_def(&shared).is_some());
        assert_eq!(mac.settings.block_packs.len(), 1);
        let instructions = &mac.strands[0].instructions;
        assert_eq!(instructions.len(), 3);
        assert!(matches!(instructions[0].kind, InstructionKind::WhenRan));
        let InstructionKind::Repeat { body, .. } = &instructions[1].kind else { panic!("repeat") };
        assert!(body.is_empty());
        assert_eq!(instructions[2].kind, InstructionKind::Log(Value::number(0.0)));
        remove_from(&mut mac, "second").unwrap();
        assert!(mac.block_defs.is_empty());
        assert!(mac.settings.block_packs.is_empty());
    }

    #[test]
    fn pack_details_survive_export_install_and_save() {
        let mut author = Macro::new("Author".into(), String::new(), Vec::new());
        let id = make_block(&mut author, "wait seconds", Vec::new());
        let def = author.graph.block_def_mut(&id).unwrap();
        def.identifier = "WaitSeconds".into();
        def.description = "Pauses playback for the given number of seconds.".into();
        let pack = pack_from_macro(&author, &[id.clone()], PackInfo {
            name: "Timing".into(), ..Default::default()
        }).unwrap();
        let pack = read_pack(&write_pack(&pack).unwrap()).unwrap();
        let mut mac = Macro::new("User".into(), String::new(), Vec::new());
        merge_into(&mut mac, &pack, "");
        let (saved, _) = migrate::from_value(migrate::to_value(&mac).unwrap()).unwrap();
        let def = saved.block_def(&id).unwrap();
        assert_eq!(def.identifier, "Community_WaitSeconds");
        assert_eq!(def.description, author.block_def(&id).unwrap().description);
        let legacy: blockstitch_core::graph::BlockDef = serde_json::from_str(
            r#"{"id":"legacy","pieces":[],"shape":"Normal"}"#
        ).unwrap();
        assert!(legacy.identifier.is_empty());
        assert!(legacy.description.is_empty());
    }

    #[test]
    fn installed_implementations_stay_outside_editor_and_recording() {
        let mut author = Macro::new("Author".into(), String::new(), vec![]);
        let id = make_block(&mut author, "wait", vec![InstructionKind::Wait(Value::number(1.0))]);
        let pack = pack_from_macro(&author, &[id.clone()], PackInfo {
            name: "Timing".into(), author: "Example Author".into(), ..Default::default()
        }).unwrap();
        let mut mac = Macro::new("User".into(), String::new(), vec![]);
        merge_into(&mut mac, &pack, "checksum");
        assert_eq!(mac.editor_strands().count(), 1);
        assert!(check_editable(&mac, &id).is_err());
        assert!(pack_from_macro(&mac, &[id.clone()], pack.info.clone()).is_err());
        let local = make_block(&mut mac, "calls community", vec![call(&id)]);
        assert!(check_editable(&mac, &local).is_ok());
        assert!(pack_from_macro(&mac, &[local], pack.info.clone()).is_err());
        assert!(mac.block_body(&id).is_some());
        mac.recording_target = Some(mac.block_body(&id).unwrap().id.clone());
        assert_ne!(mac.recording_target_id(), mac.recording_target);
        mac.clear_editor();
        assert!(mac.editor_strands().next().is_none());
        assert!(mac.recording_target_id().is_none());
        assert!(mac.block_body(&id).is_some());
        let target = mac.recording_target_mut().id.clone();
        assert_ne!(Some(target), mac.recording_target);
        assert!(mac.block_body(&id).is_some());
        let (saved, _) = migrate::from_value(migrate::to_value(&mac).unwrap()).unwrap();
        assert_eq!(saved.editor_strands().count(), 1);
        assert!(saved.block_body(&id).is_some());
    }

    #[test]
    fn details_ids_use_author_and_block_name_without_changing_internal_ids() {
        assert_eq!(details_identifier("Example Author", "WaitSeconds"), "ExampleAuthor_WaitSeconds");
        assert_eq!(details_identifier("ExampleAuthor", "ExampleAuthor_WaitSeconds"), "ExampleAuthor_WaitSeconds");
        let mut author = Macro::new("Author".into(), String::new(), vec![]);
        let id = make_block(&mut author, "wait", vec![]);
        author.graph.block_def_mut(&id).unwrap().identifier = "WaitSeconds".into();
        author.graph.block_def_mut(&id).unwrap().description = "Author's description".into();
        let pack = pack_from_macro(&author, &[id.clone()], PackInfo {
            name: "Timing".into(), author: "Alice".into(), ..Default::default()
        }).unwrap();
        let pack = read_pack(&write_pack(&pack).unwrap()).unwrap();
        let def = pack.content.block_def(&id).unwrap();
        assert_eq!(def.identifier, "Alice_WaitSeconds");
        assert_eq!(def.description, "Author's description");
        assert!(pack.content.block_body(&id).is_some());
    }

    #[test]
    fn library_fixture_matches_its_index() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blocks");
        let text = std::fs::read_to_string(root.join("index.json")).unwrap();
        let index = read_index(&text, &root.join("index.json").to_string_lossy()).unwrap();
        assert!(!index.packs.is_empty());
        for entry in &index.packs {
            let bytes = std::fs::read(&entry.url).unwrap();
            verify(&bytes, &entry.sha256).expect("refresh the fixture from the community blocks repo");
            let pack = read_pack(std::str::from_utf8(&bytes).unwrap()).unwrap();
            assert_eq!(pack.info.id, entry.info.id);
            assert!(pack.notes.is_empty(), "{}: {:?}", entry.info.id, pack.notes);
        }
    }

    #[test]
    fn index_urls_resolve_and_hashes_are_checked() {
        let base = "https://example.com/blocks/index.json";
        assert_eq!(resolve_url(base, "packs/a.json"), "https://example.com/blocks/packs/a.json");
        assert_eq!(resolve_url(base, "/x/a.json"), "https://example.com/x/a.json");
        assert_eq!(resolve_url(base, "https://other.org/a.json"), "https://other.org/a.json");
        let hash = sha256_hex(b"abc");
        assert!(verify(b"abc", &hash).is_ok());
        assert!(verify(b"abd", &hash).is_err());

        let index = format!(
            r#"{{"blockwork_block_index":1,"name":"Test","packs":[
                {{"id":"a","name":"A","url":"a.json","sha256":"{hash}"}},
                {{"id":"b","name":"B","url":"b.json","sha256":"short"}}]}}"#
        );
        let index = read_index(&index, base).unwrap();
        assert_eq!(index.packs.len(), 1, "the entry without a real hash is skipped");
        assert_eq!(index.packs[0].url, "https://example.com/blocks/a.json");
    }

    #[test]
    fn only_the_built_in_library_marks_packs_official() {
        let hash = sha256_hex(b"abc");
        let index = format!(
            r#"{{"blockwork_block_index":1,"name":"Test","packs":[
                {{"id":"a","name":"A","url":"a.json","sha256":"{hash}","official":true}},
                {{"id":"b","name":"B","url":"b.json","sha256":"{hash}"}}]}}"#
        );
        let official = |base| read_index(&index, base).unwrap().packs.iter().map(|p| p.official).collect::<Vec<_>>();
        assert_eq!(official(DEFAULT_INDEX), [true, false], "older indexes without the field have no badge");
        assert_eq!(official("https://example.com/index.json"), [false, false]);
    }
}
