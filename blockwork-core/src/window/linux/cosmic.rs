//! COSMIC lists windows and acts on them through its own Wayland protocols
//! (cosmic-toplevel-info and -management, from github.com/pop-os/cosmic-protocols,
//! vendored in protocols/ with the workspace protocol they refer to). They
//! can't move or resize a window, keep it on top, or say its process.
use super::{Action, ActiveWindow, MatchBy, Rect, ScreenWindow, no_match, sized};
use std::collections::HashMap;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_output, wl_registry, wl_seat};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop, event_created_child,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1 as ext_handle, ext_foreign_toplevel_list_v1 as ext_list,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};

/// Generates client code for a vendored protocol, the way the
/// wayland-protocols crate does for its own.
macro_rules! protocol {
    ($name:ident, $path:literal, [$($imports:path),*]) => {
        #[allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
        #[allow(non_upper_case_globals, non_snake_case, unused_imports, missing_docs)]
        #[allow(clippy::all)]
        mod $name {
            use wayland_client;
            use wayland_client::protocol::*;
            $(use $imports::{*};)*
            pub mod __interfaces {
                use wayland_client::protocol::__interfaces::*;
                $(use $imports::{__interfaces::*};)*
                wayland_scanner::generate_interfaces!($path);
            }
            use self::__interfaces::*;
            wayland_scanner::generate_client_code!($path);
        }
    };
}

protocol!(workspace, "protocols/cosmic-workspace-unstable-v1.xml", []);
protocol!(
    info,
    "protocols/cosmic-toplevel-info-unstable-v1.xml",
    [
        crate::window::linux::cosmic::workspace,
        wayland_protocols::ext::foreign_toplevel_list::v1::client,
        wayland_protocols::ext::workspace::v1::client
    ]
);
protocol!(
    management,
    "protocols/cosmic-toplevel-management-unstable-v1.xml",
    [
        crate::window::linux::cosmic::info,
        crate::window::linux::cosmic::workspace,
        wayland_protocols::ext::workspace::v1::client
    ]
);

use info::zcosmic_toplevel_handle_v1 as cosmic_handle;
use info::zcosmic_toplevel_info_v1 as cosmic_info;
use management::zcosmic_toplevel_manager_v1 as cosmic_manager;

/// The `zcosmic_toplevel_handle_v1.state` values Blockwork reads.
const MAXIMIZED: u32 = 0;
const MINIMIZED: u32 = 1;
const ACTIVATED: u32 = 2;
const FULLSCREEN: u32 = 3;

#[derive(Default)]
struct Toplevel {
    cosmic: Option<cosmic_handle::ZcosmicToplevelHandleV1>,
    title: String,
    app_id: String,
    states: Vec<u32>,
    /// Output, then x, y, width and height on it.
    geometry: Option<(ObjectId, i32, i32, i32, i32)>,
    closed: bool,
}

#[derive(Default)]
struct State {
    info: Option<cosmic_info::ZcosmicToplevelInfoV1>,
    manager: Option<cosmic_manager::ZcosmicToplevelManagerV1>,
    list: Option<ext_list::ExtForeignToplevelListV1>,
    seat: Option<wl_seat::WlSeat>,
    xdg_outputs: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    outputs: Vec<wl_output::WlOutput>,
    /// Each output's corner in the desktop's logical space.
    origins: HashMap<ObjectId, (i32, i32)>,
    /// By the ext handle's id, in the order the compositor listed them.
    toplevels: Vec<(ObjectId, Toplevel)>,
}

impl State {
    fn toplevel(&mut self, id: &ObjectId) -> Option<&mut Toplevel> {
        self.toplevels
            .iter_mut()
            .find(|(k, _)| k == id)
            .map(|(_, t)| t)
    }
}

/// A connection with every window described.
struct Session {
    queue: EventQueue<State>,
    state: State,
}

fn session() -> Result<Session, String> {
    let connection =
        Connection::connect_to_env().map_err(|e| format!("no Wayland connection: {e}"))?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    connection.display().get_registry(&qh, ());
    let mut state = State::default();
    // Globals, then the windows and outputs, then what each window says.
    for _ in 0..3 {
        queue
            .roundtrip(&mut state)
            .map_err(|e| format!("Wayland error: {e}"))?;
    }
    if state.info.is_none() || state.list.is_none() {
        return Err("this COSMIC version doesn't share its window list".into());
    }
    Ok(Session { queue, state })
}

/// One window, as the window blocks see it.
#[derive(Clone)]
pub(super) struct Window {
    id: ObjectId,
    title: String,
    app: String,
    focused: bool,
    minimized: bool,
    maximized: bool,
    fullscreen: bool,
    bounds: Option<Rect>,
}

impl Session {
    fn windows(&self) -> Vec<Window> {
        let mut list: Vec<Window> = self
            .state
            .toplevels
            .iter()
            .filter(|(_, t)| !t.closed && t.cosmic.is_some())
            .map(|(id, t)| {
                let bounds = t.geometry.as_ref().and_then(|(output, x, y, w, h)| {
                    let (ox, oy) = self.state.origins.get(output).copied().unwrap_or((0, 0));
                    sized((ox + x).into(), (oy + y).into(), (*w).into(), (*h).into()).ok()
                });
                Window {
                    id: id.clone(),
                    title: t.title.clone(),
                    app: t.app_id.clone(),
                    focused: t.states.contains(&ACTIVATED),
                    minimized: t.states.contains(&MINIMIZED),
                    maximized: t.states.contains(&MAXIMIZED),
                    fullscreen: t.states.contains(&FULLSCREEN),
                    bounds,
                }
            })
            .collect();
        // No stacking order is shared; the focused window is at least on top.
        list.sort_by_key(|w| !w.focused);
        list
    }

    fn act(&mut self, window: &Window, action: Action) -> Result<(), String> {
        let manager = self
            .state
            .manager
            .clone()
            .ok_or("this COSMIC version doesn't let apps control windows")?;
        let handle = self
            .state
            .toplevel(&window.id)
            .and_then(|t| t.cosmic.clone())
            .ok_or("the window closed")?;
        let activate = |manager: &cosmic_manager::ZcosmicToplevelManagerV1| match &self.state.seat {
            Some(seat) => {
                manager.activate(&handle, seat);
                Ok(())
            }
            None => Err("no seat to focus the window from".to_string()),
        };
        match action {
            Action::Focus => activate(&manager)?,
            Action::Close => manager.close(&handle),
            Action::Minimize | Action::Hide => manager.set_minimized(&handle),
            Action::Maximize => manager.set_maximized(&handle),
            Action::Restore => {
                if window.minimized {
                    manager.unset_minimized(&handle);
                }
                if window.fullscreen {
                    manager.unset_fullscreen(&handle);
                }
                if window.maximized {
                    manager.unset_maximized(&handle);
                }
            }
            Action::Show => {
                manager.unset_minimized(&handle);
                activate(&manager)?;
            }
            Action::AlwaysOnTop | Action::NotOnTop => {
                return Err("COSMIC doesn't let apps keep a window on top".into());
            }
        }
        self.queue
            .roundtrip(&mut self.state)
            .map(drop)
            .map_err(|e| format!("Wayland error: {e}"))
    }
}

fn info_of(w: &Window) -> ActiveWindow {
    ActiveWindow {
        title: w.title.clone(),
        app: w.app.clone(),
    }
}

pub(super) fn active() -> Result<ActiveWindow, String> {
    Ok(session()?
        .windows()
        .iter()
        .find(|w| w.focused)
        .map(info_of)
        .unwrap_or_default())
}

pub(super) fn list_for_agent() -> Result<Vec<crate::window::WindowInfo>, String> {
    Ok(session()?
        .windows()
        .into_iter()
        .map(|w| crate::window::WindowInfo {
            title: w.title,
            app: w.app,
            rect: w.bounds.unwrap_or_default(),
            focused: w.focused,
        })
        .collect())
}

fn find_in(session: &Session, pattern: &str, by: MatchBy) -> Result<Window, String> {
    let list = session.windows();
    if pattern.trim().is_empty() {
        return list
            .into_iter()
            .find(|w| w.focused)
            .ok_or_else(|| "no window has focus".to_string());
    }
    list.into_iter()
        .find(|w| info_of(w).matches(pattern, by))
        .ok_or_else(|| no_match(pattern))
}

pub(super) fn act(action: Action, pattern: &str, by: MatchBy) -> Result<(), String> {
    let mut session = session()?;
    let window = find_in(&session, pattern, by)?;
    session.act(&window, action)
}

pub(super) fn bounds(pattern: &str, by: MatchBy) -> Result<Rect, String> {
    let session = session()?;
    let window = find_in(&session, pattern, by)?;
    if window.minimized {
        return Err("the window is minimized or hidden".into());
    }
    window
        .bounds
        .ok_or_else(|| "COSMIC didn't say where the window is".to_string())
}

pub(super) fn exists(pattern: &str) -> Result<bool, String> {
    Ok(find_in(&session()?, pattern, MatchBy::Any).is_ok())
}

pub(super) fn stacked() -> Result<Vec<ScreenWindow>, String> {
    Ok(session()?
        .windows()
        .into_iter()
        .filter(|w| !w.minimized)
        .filter_map(|w| {
            Some(ScreenWindow {
                bounds: w.bounds?,
                title: w.title,
                app: w.app,
                pid: 0,
            })
        })
        .collect())
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "zcosmic_toplevel_info_v1" if version >= 2 => {
                state.info = Some(registry.bind(name, version.min(3), qh, ()));
            }
            "zcosmic_toplevel_manager_v1" => {
                state.manager = Some(registry.bind(name, version.min(4), qh, ()));
            }
            "ext_foreign_toplevel_list_v1" => {
                state.list = Some(registry.bind(name, 1, qh, ()));
            }
            "wl_seat" if state.seat.is_none() => {
                state.seat = Some(registry.bind(name, 1, qh, ()));
            }
            "wl_output" => {
                let output: wl_output::WlOutput = registry.bind(name, version.min(3), qh, ());
                if let Some(manager) = &state.xdg_outputs {
                    manager.get_xdg_output(&output, qh, output.id());
                }
                state.outputs.push(output);
            }
            "zxdg_output_manager_v1" => {
                let manager: zxdg_output_manager_v1::ZxdgOutputManagerV1 =
                    registry.bind(name, version.min(3), qh, ());
                for output in &state.outputs {
                    manager.get_xdg_output(output, qh, output.id());
                }
                state.xdg_outputs = Some(manager);
            }
            _ => {}
        }
    }
}

impl Dispatch<ext_list::ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_list::ExtForeignToplevelListV1,
        event: ext_list::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let ext_list::Event::Toplevel { toplevel } = event {
            let cosmic = state
                .info
                .as_ref()
                .map(|info| info.get_cosmic_toplevel(&toplevel, qh, toplevel.id()));
            state.toplevels.push((
                toplevel.id(),
                Toplevel {
                    cosmic,
                    ..Toplevel::default()
                },
            ));
        }
    }

    event_created_child!(State, ext_list::ExtForeignToplevelListV1, [
        ext_list::EVT_TOPLEVEL_OPCODE => (ext_handle::ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ext_handle::ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ext_handle::ExtForeignToplevelHandleV1,
        event: ext_handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(toplevel) = state.toplevel(&handle.id()) else {
            return;
        };
        match event {
            ext_handle::Event::Title { title } => toplevel.title = title,
            ext_handle::Event::AppId { app_id } => toplevel.app_id = app_id,
            ext_handle::Event::Closed => toplevel.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<cosmic_handle::ZcosmicToplevelHandleV1, ObjectId> for State {
    fn event(
        state: &mut Self,
        _: &cosmic_handle::ZcosmicToplevelHandleV1,
        event: cosmic_handle::Event,
        ext_id: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(toplevel) = state.toplevel(ext_id) else {
            return;
        };
        match event {
            cosmic_handle::Event::State { state } => toplevel.states = states(&state),
            cosmic_handle::Event::Geometry {
                output,
                x,
                y,
                width,
                height,
            } => toplevel.geometry = Some((output.id(), x, y, width, height)),
            _ => {}
        }
    }
}

/// A Wayland `array` of native-endian u32 values.
fn states(raw: &[u8]) -> Vec<u32> {
    raw.chunks_exact(4)
        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

impl Dispatch<cosmic_info::ZcosmicToplevelInfoV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &cosmic_info::ZcosmicToplevelInfoV1,
        _: cosmic_info::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    // Only version 1 sends this; Blockwork binds 2 or later.
    event_created_child!(State, cosmic_info::ZcosmicToplevelInfoV1, [
        cosmic_info::EVT_TOPLEVEL_OPCODE => (cosmic_handle::ZcosmicToplevelHandleV1, ObjectId::null()),
    ]);
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, ObjectId> for State {
    fn event(
        state: &mut Self,
        _: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        output: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_output_v1::Event::LogicalPosition { x, y } = event {
            state.origins.insert(output.clone(), (x, y));
        }
    }
}

delegate_noop!(State: ignore cosmic_manager::ZcosmicToplevelManagerV1);
delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore wl_output::WlOutput);
delegate_noop!(State: ignore zxdg_output_manager_v1::ZxdgOutputManagerV1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_array_reads_each_value() {
        let raw: Vec<u8> = [ACTIVATED, MAXIMIZED]
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        assert_eq!(states(&raw), vec![ACTIVATED, MAXIMIZED]);
        assert!(states(&[1, 2, 3]).is_empty());
    }
}
