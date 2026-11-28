//! Background input on KDE Plasma through Blockwork's KWin plugin
//! (`blockwork-kwin/`), which offers `blockwork_seat_host_v1`. Each lane
//! gets its own Wayland connection, so closing it (or Blockwork exiting)
//! lets go of everything the lane held.

use super::Lane;
use crate::input::types::Axis;
use crate::window::WindowRef;
use std::collections::HashMap;
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};

#[allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
#[allow(non_upper_case_globals, non_snake_case, unused_imports, missing_docs)]
#[allow(clippy::all)]
mod protocol {
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/blockwork-seat-host-v1.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/blockwork-seat-host-v1.xml");
}

use protocol::blockwork_seat_host_v1::{self as host, BlockworkSeatHostV1};
use protocol::blockwork_seat_lane_v1::{self as lane, BlockworkSeatLaneV1};

/// The app needs a moment to make a keyboard and pointer on a lane's seat
/// the first time; the plugin answers as soon as it has.
const BIND_TIMEOUT: Duration = Duration::from_secs(2);
/// After the user touches the same window, wait this long for them.
const PREEMPT_QUIET: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, PartialEq)]
enum Status {
    Waiting,
    Bound,
    Refused(String),
    Lost(String),
}

struct State {
    info: Option<(String, String, u32)>,
    status: Status,
    preempted: Option<Instant>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<BlockworkSeatHostV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &BlockworkSeatHostV1,
        event: host::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let host::Event::Info {
            kwin_version,
            build,
            lanes,
        } = event;
        state.info = Some((kwin_version, build, lanes));
    }
}

impl Dispatch<BlockworkSeatLaneV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &BlockworkSeatLaneV1,
        event: lane::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            lane::Event::Bound { .. } => state.status = Status::Bound,
            lane::Event::Refused { message, .. } => state.status = Status::Refused(message),
            lane::Event::Lost { message, .. } => state.status = Status::Lost(message),
            lane::Event::Preempted => state.preempted = Some(Instant::now()),
        }
    }
}

const NOT_INSTALLED: &str = "background input needs the Blockwork KWin plugin: install the blockwork-kwin package, then log out and back in";

/// Connects and binds the host, or says what's missing.
fn connect() -> Result<(Connection, EventQueue<State>, BlockworkSeatHostV1, State), String> {
    let conn = Connection::connect_to_env()
        .map_err(|e| format!("can't reach the Wayland compositor: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)
        .map_err(|e| format!("can't read the compositor's globals: {e}"))?;
    let qh = queue.handle();
    let host: BlockworkSeatHostV1 = globals.bind(&qh, 1..=1, ()).map_err(|_| {
        if crate::flatpak::is_flatpak() {
            format!("{NOT_INSTALLED} on the host system (a Flatpak can't add it itself)")
        } else {
            NOT_INSTALLED.to_string()
        }
    })?;
    let mut state = State {
        info: None,
        status: Status::Waiting,
        preempted: None,
    };
    queue
        .roundtrip(&mut state)
        .map_err(|e| format!("the KWin plugin stopped answering: {e}"))?;
    Ok((conn, queue, host, state))
}

/// "ready (4 lanes)" or why background input isn't there.
pub(super) fn availability() -> Result<String, String> {
    let (_conn, _queue, _host, state) = connect()?;
    let (kwin, build, lanes) = state.info.unwrap_or_default();
    Ok(format!(
        "ready: {lanes} lanes, plugin {build} built for KWin {kwin}"
    ))
}

pub(super) fn open(window: &WindowRef) -> Result<KdeLane, String> {
    let (conn, queue, host, state) = connect()?;
    let lane = host.get_lane(&queue.handle(), ());
    let mut lane = KdeLane {
        _conn: conn,
        queue,
        _host: host,
        lane,
        state,
        pointer: (0.0, 0.0),
        held_keys: Vec::new(),
        held_buttons: Vec::new(),
        custom_keymap: false,
    };
    lane.lane.bind(window.id.clone());
    lane.wait(|s| s.status != Status::Waiting, BIND_TIMEOUT)?;
    match lane.state.status.clone() {
        Status::Bound => Ok(lane),
        Status::Waiting => Err("the window didn't take background input in time: its app may not support it (Chromium, Electron and XWayland apps can't)".into()),
        Status::Refused(why) | Status::Lost(why) => Err(why),
    }
}

pub(super) struct KdeLane {
    _conn: Connection,
    queue: EventQueue<State>,
    _host: BlockworkSeatHostV1,
    lane: BlockworkSeatLaneV1,
    state: State,
    pointer: (f64, f64),
    held_keys: Vec<u16>,
    held_buttons: Vec<u16>,
    /// Set while `text` has swapped in its own keymap.
    custom_keymap: bool,
}

impl KdeLane {
    /// Reads events until `done` or `timeout`.
    fn wait(&mut self, done: impl Fn(&State) -> bool, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            self.queue
                .dispatch_pending(&mut self.state)
                .map_err(|e| e.to_string())?;
            if done(&self.state) {
                return Ok(());
            }
            let left = timeout.saturating_sub(start.elapsed());
            self.queue.flush().map_err(|e| e.to_string())?;
            if let Some(guard) = self.queue.prepare_read() {
                let mut fd = libc::pollfd {
                    fd: guard.connection_fd().as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: one valid pollfd for the duration of the call.
                let ready = unsafe { libc::poll(&mut fd, 1, left.as_millis() as i32) };
                let would_block = |e: &wayland_client::backend::WaylandError| matches!(e, wayland_client::backend::WaylandError::Io(io) if io.kind() == std::io::ErrorKind::WouldBlock);
                if ready > 0
                    && let Err(e) = guard.read()
                    && !would_block(&e)
                {
                    return Err(format!("the KWin plugin connection closed: {e}"));
                }
            }
            if left.is_zero() {
                return Ok(());
            }
        }
    }

    /// Takes in whatever the plugin said, then refuses to go on if the
    /// window is gone, and gives way briefly if the user is in it.
    fn check(&mut self) -> Result<(), String> {
        self.wait(|_| true, Duration::ZERO)?;
        if let Status::Lost(why) = &self.state.status {
            return Err(why.clone());
        }
        if let Some(at) = self.state.preempted {
            let quiet = PREEMPT_QUIET.saturating_sub(at.elapsed());
            if !quiet.is_zero() {
                std::thread::sleep(quiet);
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        self.queue
            .flush()
            .map_err(|e| format!("the KWin plugin connection closed: {e}"))
    }

    fn send_keymap(&mut self, text: &str) -> Result<(), String> {
        let fd = keymap_fd(text)?;
        self.lane.set_keymap(fd.as_fd(), text.len() as u32 + 1);
        self.flush()
    }

    fn restore_keymap(&mut self) -> Result<(), String> {
        if !self.custom_keymap {
            return Ok(());
        }
        self.custom_keymap = false;
        self.send_keymap(&us_keymap()?)
    }
}

fn state(pressed: bool) -> lane::KeyState {
    if pressed {
        lane::KeyState::Pressed
    } else {
        lane::KeyState::Released
    }
}

impl Lane for KdeLane {
    fn key(&mut self, code: u16, pressed: bool) -> Result<(), String> {
        self.check()?;
        self.restore_keymap()?;
        self.lane.key(code as u32, state(pressed));
        self.held_keys.retain(|k| *k != code);
        if pressed {
            self.held_keys.push(code);
        }
        self.flush()
    }

    fn button(&mut self, code: u16, pressed: bool) -> Result<(), String> {
        self.check()?;
        self.lane.button(code as u32, state(pressed));
        self.held_buttons.retain(|b| *b != code);
        if pressed {
            self.held_buttons.push(code);
        }
        self.flush()
    }

    fn motion(&mut self, x: f64, y: f64) -> Result<(), String> {
        self.check()?;
        self.lane.motion(x, y);
        self.pointer = (x, y);
        self.flush()
    }

    fn relative(&mut self, dx: f64, dy: f64) -> Result<(), String> {
        self.check()?;
        self.lane.relative_motion(dx, dy);
        self.pointer = (self.pointer.0 + dx, self.pointer.1 + dy);
        self.flush()
    }

    fn axis(&mut self, axis: Axis, value120: i32) -> Result<(), String> {
        self.check()?;
        let axis = match axis {
            Axis::Vertical => lane::Axis::Vertical,
            Axis::Horizontal => lane::Axis::Horizontal,
        };
        self.lane.axis(axis, value120);
        self.flush()
    }

    fn text(&mut self, text: &str) -> Result<(), String> {
        // Every character gets its own key in a keymap made for this text,
        // so the user's layout and Shift never come into it.
        let chars: Vec<char> = text.chars().collect();
        for chunk in chars.chunks(MAX_KEYS) {
            let (keymap, codes) = text_keymap(chunk);
            self.check()?;
            self.send_keymap(&keymap)?;
            self.custom_keymap = true;
            let mut first = true;
            for c in chunk {
                if !first {
                    std::thread::sleep(crate::macros::backend::INTER_CHAR_DELAY);
                }
                first = false;
                let code = codes[c];
                self.lane.key(code as u32, lane::KeyState::Pressed);
                self.lane.key(code as u32, lane::KeyState::Released);
                self.flush()?;
            }
        }
        self.restore_keymap()
    }

    fn release_all(&mut self) -> Result<(), String> {
        self.lane.release_all();
        self.held_keys.clear();
        self.held_buttons.clear();
        self.flush()
    }

    fn pointer(&self) -> (f64, f64) {
        self.pointer
    }
}

impl Drop for KdeLane {
    fn drop(&mut self) {
        self.lane.destroy();
        let _ = self.queue.flush();
    }
}

/// Distinct characters one text keymap holds (evdev codes 1 to 200).
const MAX_KEYS: usize = 200;

/// An XKB keymap with one key per distinct character in `chars`, and the
/// evdev code each one is on.
fn text_keymap(chars: &[char]) -> (String, HashMap<char, u16>) {
    let mut codes = HashMap::new();
    let mut keycodes = String::new();
    let mut symbols = String::new();
    for c in chars {
        if codes.contains_key(c) {
            continue;
        }
        let code = codes.len() as u16 + 1;
        codes.insert(*c, code);
        let xkb = code as u32 + 8;
        keycodes.push_str(&format!("<K{xkb}> = {xkb}; "));
        symbols.push_str(&format!("key <K{xkb}> {{ [ {} ] }}; ", keysym_name(*c)));
    }
    let keymap = format!(
        "xkb_keymap {{\n\
         xkb_keycodes \"blockwork\" {{ minimum = 8; maximum = 255; {keycodes}}};\n\
         xkb_types \"blockwork\" {{ include \"complete\" }};\n\
         xkb_compatibility \"blockwork\" {{ include \"complete\" }};\n\
         xkb_symbols \"blockwork\" {{ {symbols}}};\n\
         }};\n"
    );
    (keymap, codes)
}

fn keysym_name(c: char) -> String {
    match c {
        '\n' | '\r' => "Return".into(),
        '\t' => "Tab".into(),
        '\u{8}' => "BackSpace".into(),
        c => format!("U{:04X}", c as u32),
    }
}

/// The plain US layout lanes start with, which Blockwork's key names map onto.
fn us_keymap() -> Result<String, String> {
    use xkbcommon::xkb;
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    xkb::Keymap::new_from_names(
        &context,
        "",
        "pc105",
        "us",
        "",
        None,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .map(|k| k.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1))
    .ok_or_else(|| "couldn't build a US keymap: is xkeyboard-config installed?".into())
}

/// The keymap text, NUL-terminated, in a sealed memfd.
fn keymap_fd(text: &str) -> Result<OwnedFd, String> {
    // SAFETY: memfd_create with a static NUL-terminated name.
    let raw = unsafe {
        libc::memfd_create(
            c"blockwork-keymap".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if raw < 0 {
        return Err(format!(
            "couldn't make a keymap file: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: memfd_create just returned this descriptor to us.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut file = std::fs::File::from(fd);
    file.write_all(text.as_bytes())
        .and_then(|_| file.write_all(&[0]))
        .map_err(|e| format!("couldn't write the keymap: {e}"))?;
    // SAFETY: plain fcntl on a descriptor we own.
    unsafe {
        libc::fcntl(
            file.as_raw_fd(),
            libc::F_ADD_SEALS,
            libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE | libc::F_SEAL_SEAL,
        );
    }
    Ok(OwnedFd::from(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_keymap_has_one_key_per_character() {
        let (keymap, codes) = text_keymap(&['h', 'é', 'h', '\n', '😀']);
        assert_eq!(codes.len(), 4);
        assert_eq!(codes[&'h'], 1);
        assert!(keymap.contains("key <K9> { [ U0068 ] };"));
        assert!(keymap.contains("[ U00E9 ]"));
        assert!(keymap.contains("[ Return ]"));
        assert!(keymap.contains("[ U1F600 ]"));
    }

    #[test]
    fn a_text_keymap_compiles() {
        use xkbcommon::xkb;
        let (keymap, codes) = text_keymap(&['a', 'Ω', '😀']);
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let Some(compiled) = xkb::Keymap::new_from_string(
            &context,
            keymap,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            // No xkeyboard-config on this machine ("complete" can't be found).
            return;
        };
        let state = xkb::State::new(&compiled);
        let typed =
            |c: char| char::from_u32(state.key_get_utf32(xkb::Keycode::new(codes[&c] as u32 + 8)));
        assert_eq!(typed('a'), Some('a'));
        assert_eq!(typed('Ω'), Some('Ω'));
        assert_eq!(typed('😀'), Some('😀'));
    }
}
