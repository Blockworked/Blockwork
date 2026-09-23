#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("app_icon.h");

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
