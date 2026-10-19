//! `Translator` for QML: switches the UI language at runtime.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type Translator = super::TranslatorRust;

        /// Empty means the system language. False when it has no translation.
        #[qinvokable]
        #[cxx_name = "setLanguage"]
        fn set_language(self: &Translator, code: &QString) -> bool;
    }
}

#[derive(Default)]
pub struct TranslatorRust;

impl qobject::Translator {
    fn set_language(&self, code: &cxx_qt_lib::QString) -> bool {
        crate::translations::set_language(&code.to_string())
    }
}
