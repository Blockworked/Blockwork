#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app_bridge;
mod app_icon;
mod daemon_client;
mod qt_diagnostics;

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QUrl};
use std::pin::Pin;

fn main() {
    // Every control is drawn from the app's own theme; pin the Basic style so the
    // platform style (e.g. Fluent on Windows 11) doesn't add native chrome, shadows
    // or light/dark palettes underneath it. Must be set before the first control.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: still single-threaded; nothing else has started or reads the environment yet.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Basic") };
    }
    qt_diagnostics::install();
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    // The QML module is linked into the executable instead of loaded through a
    // shared plugin, so register its generated resources and Rust QObjects.
    blockstitch_qml::init();
    cxx_qt::init_qml_module!("com.blockworked.Blockwork");

    let mut app = QGuiApplication::new();
    // Title-bar, taskbar and dock fallback icon (see src/app_icon.cpp).
    app_icon::apply();
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
