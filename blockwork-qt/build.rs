use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    let blockwork = [
        "qml/Main.qml",
        "qml/EditorPage.qml",
        "qml/SettingsPage.qml",
        "qml/SectionCard.qml",
        "qml/MakeBlockDialog.qml",
    ];

    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("com.blockworked.Blockwork")
            .version(1, 0)
            .qml_files(blockwork),
    )
    .file("src/app_bridge.rs")
    .file("src/qt_diagnostics.rs")
    .qt_module("QuickControls2")
    .qt_module("QuickDialogs2")
    .qt_module("Network");

    unsafe {
        builder
            .cc_builder(|cc| {
                cc.include("src");
                cc.file("src/qt_diagnostics.cpp");
            })
            .build();
    }
}
