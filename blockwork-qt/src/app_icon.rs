#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("app_icon.h");
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        fn blockwork_native_app_icon(command: &QString, size: i32) -> QString;

        fn blockwork_apply_window_icon();
        fn blockwork_apply_window_icon_to_windows();
    }
}

pub fn apply() {
    ffi::blockwork_apply_window_icon();
}

pub fn apply_to_windows() {
    ffi::blockwork_apply_window_icon_to_windows();
}

#[cfg(target_os = "macos")]
pub fn native_app_icon(command: &str, size: i32) -> String {
    ffi::blockwork_native_app_icon(&cxx_qt_lib::QString::from(command), size).to_string()
}
