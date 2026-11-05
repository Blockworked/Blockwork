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
#[cfg(feature = "system")]
fn components_survive_pack_install_update_and_save() {
    let mut author = Macro::new("author".into(), String::new(), vec![]);
    let pieces = serde_json::from_value(serde_json::json!([
        {"kind":"Label","id":"name","text":"example"},
        {"kind":"Input","id":"enabled","name":"timeout","value_type":"Bool","component":true},
        {"kind":"Input","id":"duration","name":"duration","default_value":{"kind":"Number","value":100}}
    ])).unwrap();
    let id = author.create_block(pieces, BlockShape::Normal, "#4C97FF".into(), 0, 0);
    let pack = pack_from_macro(&author, &[id.clone()], PackInfo {
        id: "components".into(), name: "Components".into(), ..PackInfo::default()
    }).unwrap();
    let mut pack = read_pack(&write_pack(&pack).unwrap()).unwrap();
    let mut target = Macro::new("target".into(), String::new(), vec![]);
    merge_into(&mut target, &pack, "");
    let def = target.block_def(&id).unwrap();
    assert!(matches!(def.pieces[1], BlockPiece::Input { component: true, .. }));
    assert_eq!(def.input_defaults().next().unwrap().eval(), Ok(crate::input::value::Evaluated::Bool(false)));
    let args = vec![crate::input::value::Evaluated::Bool(true).into_value(), Value::number(250.0)];
    target.graph.strands.push(Strand::with_instructions(0, 0, vec![Instruction::new(
        InstructionKind::CallBlock { block_id: id.clone(), args: args.clone(), branches: vec![] }
    )]));
    if let BlockPiece::Input { name, .. } = &mut pack.content.graph.block_defs[0].pieces[1] {
        *name = "with timeout".into();
    }
    merge_into(&mut target, &pack, "updated");
    let (loaded, _) = migrate::from_value(migrate::to_value(&target).unwrap()).unwrap();
    assert_eq!(loaded.block_def(&id).unwrap().pieces, pack.content.block_def(&id).unwrap().pieces);
    let InstructionKind::CallBlock { args: saved, .. } = &loaded.strands.last().unwrap().instructions[0].kind else { panic!("call") };
    assert_eq!(saved[0].eval(), args[0].eval());
    assert_eq!(saved[1], args[1]);
}

#[test]
#[cfg(feature = "system")]
fn input_defaults_survive_pack_updates_without_resetting_calls() {
    use crate::input::value::Evaluated;
    let mut author = Macro::new("author".into(), String::new(), vec![]);
    let id = author.create_block(vec![label("example"), BlockPiece::Input {
        component: false,
        id: "input".into(), name: "text".into(), value_type: Default::default(),
        default_value: Some(Evaluated::Text("example".into())),
        multiline: false,
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
        component: false,
        id: "attempts".into(), name: "attempts".into(), value_type: Default::default(),
        default_value: Some(Evaluated::Number(3.0)),
        multiline: false,
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
#[cfg(feature = "system")]
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
    // The file carries bare ids; reading puts the author in front.
    assert_eq!(pack.content.block_defs[0].identifier, "inner");
    assert_eq!(read.content.block_defs[0].identifier, "Community:inner");
    let bare: Vec<_> = read.content.block_defs.iter()
        .map(|d| blockstitch_core::graph::BlockDef { identifier: bare_identifier("", &d.identifier), ..d.clone() }).collect();
    assert_eq!(bare, pack.content.block_defs);

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
        tolerance: Some(Value::number(0.0)),
        x_variable: "hit x".into(),
        y_variable: "hit y".into(),
        pick: None,
        region: None,
        count_variable: None,
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
#[cfg(feature = "system")]
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
    assert_eq!(def.identifier, "Community:WaitSeconds");
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
#[cfg(feature = "system")]
fn details_ids_use_author_and_block_name_without_changing_internal_ids() {
    assert_eq!(details_identifier("Example Author", "WaitSeconds"), "ExampleAuthor:WaitSeconds");
    // Older packs used an underscore, and the library already prefixes.
    assert_eq!(details_identifier("ExampleAuthor", "ExampleAuthor_WaitSeconds"), "ExampleAuthor:WaitSeconds");
    assert_eq!(details_identifier("Example Author", "ExampleAuthor:WaitSeconds"), "ExampleAuthor:WaitSeconds");
    // Whatever a file puts in front, the pack's own author wins.
    assert_eq!(details_identifier("Alice", "Mallory:WaitSeconds"), "Alice:WaitSeconds");
    assert_eq!(bare_identifier("Alice", "Alice_Wait Seconds"), "WaitSeconds");
    assert!(check_identifier("Alice:Wait").is_err());
    assert!(check_identifier("Wait").is_ok());
    let mut author = Macro::new("Author".into(), String::new(), vec![]);
    let id = make_block(&mut author, "wait", vec![]);
    author.graph.block_def_mut(&id).unwrap().identifier = "WaitSeconds".into();
    author.graph.block_def_mut(&id).unwrap().description = "Author's description".into();
    let pack = pack_from_macro(&author, &[id.clone()], PackInfo {
        name: "Timing".into(), author: "Alice".into(), ..Default::default()
    }).unwrap();
    // The file carries the bare id; reading it puts the author in front.
    let text = write_pack(&pack).unwrap();
    assert!(text.contains("\"identifier\": \"WaitSeconds\""));
    let pack = read_pack(&text).unwrap();
    let def = pack.content.block_def(&id).unwrap();
    assert_eq!(def.identifier, "Alice:WaitSeconds");
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
