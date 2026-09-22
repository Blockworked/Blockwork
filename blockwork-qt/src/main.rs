#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app_bridge;
mod daemon_client;
mod qt_diagnostics;

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QUrl};
use std::pin::Pin;

fn main() {
    qt_diagnostics::install();
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    // The QML module is linked into the executable instead of loaded through a
    // shared plugin, so register its generated resources and Rust QObjects.
    blockstitch_qml::init();
    cxx_qt::init_qml_module!("com.blockworked.Blockwork");

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        engine
            .as_mut()
            .on_object_creation_failed(|_, url| {
                eprintln!("failed to create QML root object from {}", url);
            })
            .release();
        engine.load(&QUrl::from(
            "qrc:/qt/qml/com/blockworked/Blockwork/qml/Main.qml",
        ));
    }
    if let Some(engine) = engine.as_mut() {
        let engine: Pin<&mut QQmlEngine> = engine.upcast_pin();
        engine.on_quit(|_| {}).release();
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
