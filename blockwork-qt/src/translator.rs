//! `Translator` for QML: switches the UI language at runtime.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type Translator = super::TranslatorRust;

        /// Empty means the system language. False when it has no translation.
        #[qinvokable]
        #[cxx_name = "setLanguage"]
        fn set_language(self: &Translator, code: &QString) -> bool;

        /// Codes of the languages with a translation, like "pirate" or "pt_BR".
        #[qinvokable]
        fn languages(self: &Translator) -> QStringList;
    }
}

#[derive(Default)]
pub struct TranslatorRust;

impl qobject::Translator {
    fn set_language(&self, code: &cxx_qt_lib::QString) -> bool {
        crate::translations::set_language(&code.to_string())
    }

    fn languages(&self) -> cxx_qt_lib::QStringList {
        crate::translations::languages()
    }
}
