//! Blockwork's reporters as blockstitch describes them, and the copy the
//! editor's QML carries (`BLESS=1` rewrites it).

use blockwork_core::input::value::{host_operator_descriptors, register_blockwork_operators, render_descriptors_qml, replace_generated};

#[test]
fn every_reporter_has_a_slot_per_arg() {
    register_blockwork_operators();
    let hosts = host_operator_descriptors();
    assert!(hosts.iter().any(|d| d.kind == "BatteryPercentage"));
    for d in &hosts {
        assert_eq!(d.args.len(), d.arity, "{}", d.kind);
    }
}

#[test]
fn editor_page_carries_the_current_table() {
    register_blockwork_operators();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../blockwork-qt/qml/EditorPage.qml");
    let text = std::fs::read_to_string(path).unwrap();
    let block = format!(
        "    readonly property var hostDescriptors: {}",
        render_descriptors_qml(&host_operator_descriptors(), "    ")
    );
    let next = replace_generated(&text, "// BEGIN GENERATED operator descriptors", "// END GENERATED operator descriptors", &block)
        .expect("markers in EditorPage.qml");
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(path, &next).unwrap();
    } else {
        assert!(next == text, "EditorPage.qml's operator table is stale; rerun with BLESS=1");
    }
}
