//! `blockwork --prompt`: one macro dialog in its own window, started by the
//! daemon. The request arrives as JSON on stdin and the answer leaves as
//! JSON on stdout.

use std::io::{Read, Write};
use std::sync::OnceLock;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, request_json, cxx_name = "requestJson")]
        type PromptBridge = super::PromptBridgeRust;

        /// Prints the answer and quits.
        #[qinvokable]
        fn finish(self: &PromptBridge, answer: &QString, text: &QString);
    }
}

static REQUEST: OnceLock<String> = OnceLock::new();

/// Whether this launch is a prompt; reads the request if so. Call before Qt starts.
pub fn requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--prompt") {
        return false;
    }
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    let _ = REQUEST.set(text);
    true
}

pub struct PromptBridgeRust {
    request_json: cxx_qt_lib::QString,
}

impl Default for PromptBridgeRust {
    fn default() -> Self {
        Self { request_json: cxx_qt_lib::QString::from(REQUEST.get().map(String::as_str).unwrap_or("{}")) }
    }
}

impl qobject::PromptBridge {
    fn finish(&self, answer: &cxx_qt_lib::QString, text: &cxx_qt_lib::QString) {
        let reply = serde_json::json!({ "answer": answer.to_string(), "text": text.to_string() });
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
        std::process::exit(0);
    }
}
