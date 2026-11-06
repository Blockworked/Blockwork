//! Every block's wire shape survives the trip editor -> backend -> editor
//! unchanged. `fixtures/wire_blocks.json` holds one of each `InstructionDto`
//! variant; a new block adds one there.

use blockwork_core::frontend::{dto_to_instruction, instruction_to_dto, InstructionDto};
use serde_json::Value;

#[test]
fn every_block_round_trips_through_the_wire() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/wire_blocks.json");
    let blocks: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for block in &blocks {
        let dto: InstructionDto = serde_json::from_value(block.clone()).unwrap_or_else(|e| panic!("{block}: {e}"));
        let ins = dto_to_instruction(&dto).unwrap_or_else(|| panic!("{block} did not convert"));
        // Compared as serialized DTOs, so `1` and `1.0` agree.
        assert_eq!(serde_json::to_value(instruction_to_dto(&ins)).unwrap(), serde_json::to_value(&dto).unwrap());
    }
}

#[test]
fn the_fixture_has_every_block() {
    let source = include_str!("../src/frontend/instructions.rs");
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/wire_blocks.json");
    let fixture = std::fs::read_to_string(path).unwrap();
    let table = &source[source.find("wire_blocks! {").unwrap()..source.find("to_wire(ins, id)").unwrap()];
    for line in table.lines().map(str::trim) {
        let name: String = line.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        assert!(fixture.contains(&format!("\"type\": \"{name}\"")), "{name} has no case in wire_blocks.json");
    }
}
