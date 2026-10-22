#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app_bridge;
mod app_icon;
mod application;
mod cli;
mod daemon_client;
mod native_icon;
mod prompt_bridge;
mod qt_diagnostics;
mod translations;
mod translator;
mod window_frame;

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QQmlApplicationEngine, QQmlEngine, QUrl};
use std::pin::Pin;

fn main() {
    if let Some(args) = cli::requested() {
        std::process::exit(cli::main(args));
    }
    let prompt = prompt_bridge::requested();
    // Use themed text menus with Basic as the fallback for other controls.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: still single-threaded; nothing else has started or reads the environment yet.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "BlockworkStyle") };
    }
    qt_diagnostics::install();
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    // The QML module is linked into the executable instead of loaded through a
    // shared plugin, so register its generated resources and Rust QObjects.
    blockstitch_qml::init();
    cxx_qt::init_qml_module!("com.blockworked.Blockwork");

    let mut app = application::new();
    // Title-bar, taskbar and dock fallback icon (see src/app_icon.cpp).
    app_icon::apply();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        translations::set_engine(engine.upcast_pin());
    }
    translations::set_language(&if prompt { prompt_bridge::language() } else { translations::saved_language() });
    if let Some(mut engine) = engine.as_mut() {
        engine
            .as_mut()
            .on_object_creation_failed(|_, url| {
                eprintln!("failed to create QML root object from {}", url);
            })
            .release();
        engine.load(&QUrl::from(if prompt {
            "qrc:/qt/qml/com/blockworked/Blockwork/qml/PromptWindow.qml"
        } else {
            "qrc:/qt/qml/com/blockworked/Blockwork/qml/Main.qml"
        }));
        // The QML root is a plain ApplicationWindow with no icon of its own,
        // so give every existing top-level window the application icon.
        app_icon::apply_to_windows();
    }
    if let Some(engine) = engine.as_mut() {
        let engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
        engine.on_quit(|_| {}).release();
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
