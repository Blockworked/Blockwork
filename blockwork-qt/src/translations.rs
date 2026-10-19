//! UI translations: `qml_<code>.qm` files compiled in under `:/i18n`.

use std::pin::Pin;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qqmlengine.h");
        type QQmlEngine = cxx_qt_lib::QQmlEngine;
        include!("translations.h");

        fn blockwork_set_qml_engine(engine: Pin<&mut QQmlEngine>);
        fn blockwork_set_language(code: &QString) -> bool;
        fn blockwork_languages() -> QStringList;
    }
}

pub fn set_engine(engine: Pin<&mut cxx_qt_lib::QQmlEngine>) {
    ffi::blockwork_set_qml_engine(engine);
}

/// Empty means the system language.
pub fn set_language(code: &str) -> bool {
    ffi::blockwork_set_language(&cxx_qt_lib::QString::from(code))
}

/// Codes of the compiled translations.
pub fn languages() -> cxx_qt_lib::QStringList {
    ffi::blockwork_languages()
}

/// The language saved in Settings, read before the daemon connects so the
/// first frame is already translated.
pub fn saved_language() -> String {
    let Some(path) = dirs::config_dir().map(|d| d.join("Blockwork").join("settings.json")) else {
        return String::new();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("language")?.as_str().map(str::to_string))
        .unwrap_or_default()
}
