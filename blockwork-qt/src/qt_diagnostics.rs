#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("qt_diagnostics.h");

        fn install_qt_message_handler();
    }
}

pub fn install() {
    ffi::install_qt_message_handler();
}
