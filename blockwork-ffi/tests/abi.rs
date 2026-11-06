//! The C ABI the way a host calls it: C strings in and out, return codes, and
//! a lockstep run that hands its input to a callback. One test, since every
//! call shares the library's global state (config folder, selection, run).

use blockwork_ffi::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Mutex;

/// What the lockstep callback was handed: (name, is_button, down).
static DELIVERED: Mutex<Vec<(String, bool, bool)>> = Mutex::new(Vec::new());

extern "C" fn record_input(name: *const c_char, is_button: bool, down: bool) {
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy().into_owned();
    DELIVERED.lock().unwrap().push((name, is_button, down));
}

/// Takes a string the library returned, then frees it the way a host must.
fn take(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    unsafe { blockwork_free_string(ptr) };
    Some(text)
}

fn c(text: &str) -> CString {
    CString::new(text).unwrap()
}

#[test]
fn a_host_records_plays_in_lockstep_and_manages_macros() {
    assert_eq!(blockwork_ping(), 0);
    let dir = std::env::temp_dir().join(format!("blockwork-ffi-abi-{}", std::process::id()));
    // No input backend in CI is fine: the lockstep run never touches the OS.
    let init = unsafe { blockwork_init(c(dir.to_str().unwrap()).as_ptr(), ptr::null()) };
    assert!(init == 0 || init == -1, "init returned {init}");

    // The macro library.
    assert_eq!(unsafe { blockwork_create_macro(c("Lockstep").as_ptr()) }, 0);
    let id = take(blockwork_get_selected_macro_id()).expect("the new macro is selected");
    let list: serde_json::Value = serde_json::from_str(&take(blockwork_list_macros()).unwrap()).unwrap();
    assert!(list.as_array().unwrap().iter().any(|m| m["id"] == id.as_str() && m["name"] == "Lockstep"), "{list}");
    assert_eq!(unsafe { blockwork_rename_macro(c("  Jump run ").as_ptr()) }, 0);
    assert_eq!(unsafe { blockwork_rename_macro(c("   ").as_ptr()) }, -3, "a blank name is refused");
    assert_eq!(unsafe { blockwork_select_macro(c("no-such-id").as_ptr()) }, -1);
    assert_eq!(unsafe { blockwork_select_macro(c(&id).as_ptr()) }, 0);
    let list = take(blockwork_list_macros()).unwrap();
    assert!(list.contains("\"Jump run\""), "{list}");

    // A host-clocked recording: press at 0 ms, release at 100 ms.
    blockwork_step_record_begin();
    blockwork_step_record_input(0.0, false, true);
    blockwork_step_record_input(100.0, false, false);
    assert_eq!(blockwork_step_record_end(), 3, "press, wait, release");
    blockwork_step_record_begin();
    assert_eq!(blockwork_step_record_end(), 0, "an empty recording leaves the macro alone");

    // Played back in lockstep, the input arrives on the host's ticks.
    assert_eq!(unsafe { blockwork_sync_begin(ptr::null()) }, 0);
    let mut delivered = 0;
    for tick in 0..=20 {
        delivered += blockwork_sync_step(f64::from(tick) * 10.0, Some(record_input));
    }
    let events = DELIVERED.lock().unwrap().clone();
    assert_eq!(delivered as usize, events.len());
    assert_eq!(events, [("Left".to_string(), true, true), ("Left".to_string(), true, false)]);

    // Clearing keeps the strand's header; deleting clears the selection.
    assert!(blockwork_clear_recording_target_instructions() >= 3);
    assert_eq!(blockwork_delete_macro(), 0);
    assert_eq!(take(blockwork_get_selected_macro_id()), None);
    assert_eq!(blockwork_delete_macro(), -1, "nothing is selected now");
    unsafe { blockwork_free_string(ptr::null_mut()) };
    let _ = std::fs::remove_dir_all(dir);
}
