//! Blockwork's reporters as blockstitch describes them, and the copy the
//! editor's QML carries (`BLESS=1` rewrites it).

use blockwork_core::input::value::{
    host_operator_descriptors, register_blockwork_operators, render_descriptors_qml,
    replace_generated,
};

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
fn the_catalog_carries_the_current_table() {
    register_blockwork_operators();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../blockwork-qt/qml/BlockCatalog.qml"
    );
    // A Windows checkout may have turned the file's line ends into CRLF.
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let block = format!(
        "    readonly property var hostDescriptors: {}",
        render_descriptors_qml(&host_operator_descriptors(), "    ")
    );
    let next = replace_generated(
        &text,
        "// BEGIN GENERATED operator descriptors",
        "// END GENERATED operator descriptors",
        &block,
    )
    .expect("markers in BlockCatalog.qml");
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(path, &next).unwrap();
    } else {
        assert!(
            next == text,
            "BlockCatalog.qml's operator table is stale; rerun with BLESS=1"
        );
    }
}

/// Every host reporter has its label row, a palette entry and help text.
#[test]
fn the_catalog_covers_every_reporter() {
    register_blockwork_operators();
    let catalog = include_str!("../../blockwork-qt/qml/BlockCatalog.qml").replace("\r\n", "\n");
    let help = include_str!("../../blockwork-qt/qml/BlockHelp.qml").replace("\r\n", "\n");
    let between = |start: &str, end: &str| {
        let at = catalog.find(start).unwrap();
        &catalog[at..at + catalog[at..].find(end).unwrap()]
    };
    let rows = between("function operatorRows()", "\n    }\n");
    let sections = between("paletteSections: [", "\n    ]");
    let mut missing = Vec::new();
    for d in host_operator_descriptors() {
        let kind = &d.kind;
        if !rows.contains(&format!(" {kind}: {{")) {
            missing.push(format!("{kind}: no operatorRows() entry"));
        }
        if !sections.contains(&format!("\"{kind}\"")) {
            missing.push(format!("{kind}: no paletteSections entry"));
        }
        if !help.contains(&format!("{kind}: qsTr(")) {
            missing.push(format!("{kind}: no BlockHelp explainer"));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
