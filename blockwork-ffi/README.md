# blockwork-ffi

A C ABI over `blockwork-core` for recording and playing macros inside another
process. The Geometry Dash mod (blockwork-gd) is the one host today. There is
no daemon, editor or tokio runtime: every call is a plain synchronous
function, and every export catches panics at the boundary.

The header is `include/blockwork_ffi.h`. `build.rs` regenerates it with
cbindgen on every build, so commit it along with any change to an export.
The crate stays on edition 2021 because cbindgen 0.27 skips edition 2024's
`#[unsafe(no_mangle)]` (see `Cargo.toml`).

## Building

| Target | Command |
| --- | --- |
| This machine | `cargo build -p blockwork-ffi --release` |
| Windows, from Linux | `SPLAT_DIR=... blockwork-ffi/build-windows.sh [host-resources-dir]` |
| macOS universal | `blockwork-ffi/build-macos.sh` (on a Mac) |

`build-windows.sh` also builds `blockwork-linux-bridge` and copies it into the
resources folder as `linux-input.so`. Under Proton/Wine, `blockwork_init`
starts that helper so input reaches the Linux desktop (`src/wine_bridge.rs`).

## Using it

Call `blockwork_set_log_callback` (optional) and then `blockwork_init` once on
the host's main thread before anything else. Strings are NUL-terminated UTF-8.
A returned `char *` belongs to the caller and is freed with
`blockwork_free_string`. Integer results are 0 or a count on success and
negative on failure; each function's comment in `src/lib.rs` lists its codes.

| Group | Functions |
| --- | --- |
| Setup | `blockwork_set_log_callback`, `blockwork_init`, `blockwork_grab_failed`, `blockwork_ping` |
| Macro library | `blockwork_create_macro`, `blockwork_list_macros`, `blockwork_get_selected_macro_id`, `blockwork_select_macro`, `blockwork_rename_macro`, `blockwork_delete_macro`, `blockwork_clear_recording_target_instructions` |
| Recording (wall clock) | `blockwork_start_recording`, `blockwork_stop_recording` |
| Recording (host clock) | `blockwork_step_record_begin`, `blockwork_step_record_input`, `blockwork_step_record_end` |
| Playback (own thread) | `blockwork_run_macro`, `blockwork_stop_loop`, `blockwork_set_speed_multiplier` |
| Playback (lockstep) | `blockwork_sync_begin`, `blockwork_sync_step` |
| Replays | `blockwork_import_gdr`, `blockwork_imports_dir`, `blockwork_import_pending_gdrs` |

Most library and recording calls act on the selected macro. The selection is
saved in the config folder's settings, so it is shared with the desktop app
unless `blockwork_init` is given another folder.

### Two ways to play

`blockwork_run_macro` runs a macro on a background thread against the wall
clock and sends input to the OS, like the desktop app's Run button.

The lockstep pair is for a host with its own fixed tick. After
`blockwork_sync_begin`, the macro's `Wait`s follow the virtual time the host
passes to `blockwork_sync_step` each tick, and its input is handed to that
call's callback instead of the OS. The run is exact to the host's ticks
however the frame rate moves. Host-clocked recording is the same idea in
reverse: the host stamps each input with its own time.

## Tests

`cargo test -p blockwork-ffi` runs `tests/abi.rs`, which drives the exported
functions the way a host does: C strings, return codes, a host-clocked
recording played back in lockstep through a callback, and the macro library.
