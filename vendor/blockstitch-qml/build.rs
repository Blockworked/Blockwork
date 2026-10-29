use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};
use serde_json::Value;
use std::collections::BTreeSet;

// qml/module.json is the one list of the module's files; cmake/BlockstitchQml.cmake reads it too.
const MANIFEST: &str = "qml/module.json";

fn main() {
    println!("cargo:rerun-if-changed={MANIFEST}");
    let text = std::fs::read_to_string(MANIFEST).expect("cannot read qml/module.json");
    let manifest: Value = serde_json::from_str(&text).expect("qml/module.json is not valid JSON");
    let strings = |key: &str| -> Vec<String> {
        manifest[key]
            .as_array()
            .unwrap_or_else(|| panic!("qml/module.json: `{key}` must be a list"))
            .iter()
            .map(|v| v.as_str().expect("qml/module.json: lists hold strings").to_owned())
            .collect()
    };
    let field = |key: &str| manifest[key].as_str().unwrap_or_else(|| panic!("qml/module.json: `{key}` missing")).to_owned();

    let dir = format!("qml/{}", field("dir"));
    println!("cargo:rerun-if-changed={dir}");
    let files = strings("files");
    let singletons = strings("singletons");
    check_listing(&dir, &files, &singletons);

    let (major, minor) = field("version")
        .split_once('.')
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .expect("qml/module.json: `version` must be MAJOR.MINOR");
    let module = files.iter().fold(
        QmlModule::new(field("uri").as_str()).version(major, minor),
        |module, name| module.qml_file(QmlFile::from(format!("{dir}/{name}")).singleton(singletons.contains(name))),
    );
    strings("qt_modules")
        .iter()
        .fold(CxxQtBuilder::new_qml_module(module), |builder, qt| builder.qt_module(qt))
        .build();
}

/// Every .qml file in the directory is listed, and every listed file exists.
fn check_listing(dir: &str, files: &[String], singletons: &[String]) {
    let listed: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let on_disk: BTreeSet<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {dir}: {e}"))
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.ends_with(".qml"))
        .collect();
    let unlisted: Vec<_> = on_disk.iter().filter(|n| !listed.contains(n.as_str())).collect();
    let missing: Vec<_> = listed.iter().filter(|n| !on_disk.contains(**n)).collect();
    let stray: Vec<_> = singletons.iter().filter(|n| !listed.contains(n.as_str())).collect();
    if !unlisted.is_empty() || !missing.is_empty() || !stray.is_empty() || listed.len() != files.len() {
        panic!(
            "qml/module.json is out of step with {dir}: not listed {unlisted:?}, missing {missing:?}, \
             singletons not in files {stray:?}, duplicates {}",
            files.len() - listed.len()
        );
    }
}
