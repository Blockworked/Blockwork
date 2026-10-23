use cxx_qt_lib::{QByteArray, QGuiApplication, QVector};

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qguiapplication.h");
        type QGuiApplication = cxx_qt_lib::QGuiApplication;
        include!("cxx-qt-lib/core/qvector/qvector_QByteArray.h");
        type QVector_QByteArray = cxx_qt_lib::QVector<cxx_qt_lib::QByteArray>;
        include!("cxx-qt-lib/qqmlengine.h");
        type QQmlEngine = cxx_qt_lib::QQmlEngine;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("application.h");

        fn blockwork_new_application(args: &QVector_QByteArray) -> UniquePtr<QGuiApplication>;
        fn blockwork_init_appearance(engine: Pin<&mut QQmlEngine>, scheme: &QString);
    }
}

pub fn new() -> cxx::UniquePtr<QGuiApplication> {
    let mut args = QVector::<QByteArray>::default();
    for arg in std::env::args_os() {
        #[cfg(unix)]
        use std::os::unix::ffi::OsStrExt;
        #[cfg(windows)]
        let arg = arg.to_string_lossy();
        args.append(QByteArray::from(arg.as_bytes()));
    }
    ffi::blockwork_new_application(&args)
}

/// Apply the saved appearance before either window's first frame.
pub fn init_appearance(engine: std::pin::Pin<&mut cxx_qt_lib::QQmlEngine>) {
    let scheme = dirs::config_dir()
        .and_then(|d| std::fs::read_to_string(d.join("Blockwork").join("settings.json")).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("color_scheme")?.as_str().map(str::to_string))
        .unwrap_or_default();
    ffi::blockwork_init_appearance(engine, &cxx_qt_lib::QString::from(scheme.as_str()));
}
