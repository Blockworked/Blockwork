use cxx_qt_build::{CxxQtBuilder, QmlModule};
use qt_build_utils::{QResource, QResourceFile, QResources};

fn main() {
    #[cfg(windows)]
    {
        // Explorer, taskbar grouping and the installer pick the exe's embedded
        // icon; the runtime title-bar/taskbar icon is set separately in
        // src/app_icon.cpp from the Qt resource below.
        let mut res = winres::WindowsResource::new();
        res.set_icon("../res/icons/blockwork.ico");
        res.compile().expect("failed to embed Windows icon");
    }

    let blockwork = [
        "qml/Main.qml",
        "qml/EditorPage.qml",
        "qml/SettingsPage.qml",
        "qml/SectionCard.qml",
        "qml/MakeBlockDialog.qml",
        "qml/AppSelectorDialog.qml",
    ];

    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("com.blockworked.Blockwork")
            .version(1, 0)
            .qml_files(blockwork),
    )
    .file("src/app_bridge.rs")
    .file("src/app_icon.rs")
    .file("src/qt_diagnostics.rs")
    // Runtime window icon (see src/app_icon.cpp, addressed as ":/icons/..."
    // in C++); kept under /icons so its resource path stays stable.
    .qrc_resources(QResources::new().resource(
        QResource::new().prefix("/icons").file(
            QResourceFile::new("../res/icons/blockwork.png").alias("blockwork.png"),
        ),
    ))
    .qt_module("QuickControls2")
    .qt_module("QuickDialogs2")
    .qt_module("Network");

    unsafe {
        builder
            .cc_builder(|cc| {
                cc.include("src");
                cc.file("src/qt_diagnostics.cpp");
                cc.file("src/app_icon.cpp");
            })
            .build();
    }
}
