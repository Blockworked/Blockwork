#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("window_frame.h");

        fn blockwork_set_window_frame_margin(margin: i32);
    }
}

pub fn set_margin(margin: i32) {
    ffi::blockwork_set_window_frame_margin(margin);
}
