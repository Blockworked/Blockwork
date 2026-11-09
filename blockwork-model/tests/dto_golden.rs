//! Golden fixture for the agent-facing wire shape (`docs/agent.md`).
//! If `MacroDto` field names change, this test fails on purpose - bump
//! `DTO_VERSION` and update the fixture in the same commit.

use blockwork_model::frontend::macro_to_dto;
use blockwork_model::macros::Macro;

#[test]
fn macro_dto_golden_shape() {
    let mac = Macro::new("Hello".into(), "desc".into(), vec![]);
    let dto = macro_to_dto(&mac);
    let v = serde_json::to_value(&dto).unwrap();
    // Pinned keys: adding a field is fine, renaming/removing is breaking.
    for key in [
        "dto_version",
        "id",
        "name",
        "description",
        "strands",
        "floating_values",
        "comments",
        "variables",
        "pack_data",
        "pack_exports",
        "lists",
        "signals",
        "block_defs",
        "settings",
    ] {
        assert!(v.get(key).is_some(), "MacroDto lost field '{key}'");
    }
    assert_eq!(v["dto_version"], serde_json::json!(1));
    assert_eq!(v["name"], "Hello");
    let golden = serde_json::json!({
        "dto_version": 1,
        "id": dto.id,
        "name": "Hello",
    });
    for (k, val) in golden.as_object().unwrap() {
        assert_eq!(
            v[k], *val,
            "golden value drifted for '{k}' (fixture at blockwork-model/tests/fixtures/dto_golden.json)"
        );
    }
}
