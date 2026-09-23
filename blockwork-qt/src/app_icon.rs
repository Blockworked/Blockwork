#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("app_icon.h");

        fn blockwork_apply_window_icon();
    }
}

pub fn apply() {
    ffi::blockwork_apply_window_icon();
}
