//! Read the desktop's logical coordinate bounds through XDG Output.
//!
//! libei places the cursor in this space, and `cursor_track` clamps to it.

use std::sync::OnceLock;

use wayland_client::{
    delegate_noop,
    protocol::{wl_output, wl_registry},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};

#[derive(Clone, Copy)]
pub struct DesktopBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Whether this session talks to a Wayland compositor, which decides where
/// the cursor position comes from (see `cursor_track`).
pub fn is_wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("WAYLAND_SOCKET").is_some()
}

pub fn logical_desktop_bounds() -> Option<DesktopBounds> {
    static BOUNDS: OnceLock<Option<DesktopBounds>> = OnceLock::new();
    *BOUNDS.get_or_init(query_logical_desktop_bounds)
}

#[derive(Clone)]
pub struct LogicalOutput {
    pub bounds: DesktopBounds,
}

pub fn logical_outputs() -> Option<&'static [LogicalOutput]> {
    static OUTPUTS: OnceLock<Option<Vec<LogicalOutput>>> = OnceLock::new();
    OUTPUTS.get_or_init(query_logical_outputs).as_deref()
}

fn query_logical_desktop_bounds() -> Option<DesktopBounds> {
    desktop_state().and_then(|state| state.bounds())
}

fn query_logical_outputs() -> Option<Vec<LogicalOutput>> {
    let state = desktop_state()?;
    let outputs = state
        .outputs
        .into_iter()
        .filter_map(|output| {
            Some(LogicalOutput {
                bounds: DesktopBounds {
                    x: output.logical_position?.0,
                    y: output.logical_position?.1,
                    width: output.logical_size?.0,
                    height: output.logical_size?.1,
                },
            })
        })
        .filter(|output| output.bounds.width > 0 && output.bounds.height > 0)
        .collect::<Vec<_>>();
    (!outputs.is_empty()).then_some(outputs)
}

fn desktop_state() -> Option<OutputState> {
    if !is_wayland_session() {
        return None;
    }

    let connection = Connection::connect_to_env().ok()?;
    let mut event_queue = connection.new_event_queue();
    let queue_handle = event_queue.handle();
    connection.display().get_registry(&queue_handle, ());

    let mut state = OutputState::default();
    event_queue.roundtrip(&mut state).ok()?;
    event_queue.roundtrip(&mut state).ok()?;
    Some(state)
}

#[derive(Default)]
struct OutputState {
    manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    outputs: Vec<Output>,
}

struct Output {
    output: wl_output::WlOutput,
    logical_position: Option<(i32, i32)>,
    logical_size: Option<(i32, i32)>,
}

impl OutputState {
    fn create_xdg_outputs(&mut self, queue_handle: &QueueHandle<Self>) {
        let Some(manager) = &self.manager else {
            return;
        };
        for output in &self.outputs {
            manager.get_xdg_output(&output.output, queue_handle, output.output.clone());
        }
    }

    fn output_mut(&mut self, output: &wl_output::WlOutput) -> Option<&mut Output> {
        self.outputs
            .iter_mut()
            .find(|entry| entry.output == *output)
    }

    fn bounds(&self) -> Option<DesktopBounds> {
        let mut bounds: Option<DesktopBounds> = None;
        for output in &self.outputs {
            let ((x, y), (width, height)) = (output.logical_position?, output.logical_size?);
            if width <= 0 || height <= 0 {
                continue;
            }
            bounds = Some(match bounds {
                Some(current) => {
                    let right = (current.x + current.width).max(x + width);
                    let bottom = (current.y + current.height).max(y + height);
                    DesktopBounds {
                        x: current.x.min(x),
                        y: current.y.min(y),
                        width: right - current.x.min(x),
                        height: bottom - current.y.min(y),
                    }
                }
                None => DesktopBounds {
                    x,
                    y,
                    width,
                    height,
                },
            });
        }
        bounds
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for OutputState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };

        if interface == wl_output::WlOutput::interface().name {
            let output =
                registry.bind::<wl_output::WlOutput, _, _>(name, version.min(3), queue_handle, ());
            if let Some(manager) = &state.manager {
                manager.get_xdg_output(&output, queue_handle, output.clone());
            }
            state.outputs.push(Output {
                output,
                logical_position: None,
                logical_size: None,
            });
        } else if interface == zxdg_output_manager_v1::ZxdgOutputManagerV1::interface().name {
            state.manager = Some(
                registry.bind::<zxdg_output_manager_v1::ZxdgOutputManagerV1, _, _>(
                    name,
                    version.min(3),
                    queue_handle,
                    (),
                ),
            );
            state.create_xdg_outputs(queue_handle);
        }
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, wl_output::WlOutput> for OutputState {
    fn event(
        state: &mut Self,
        _output: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        wl_output: &wl_output::WlOutput,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(entry) = state.output_mut(wl_output) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                entry.logical_position = Some((x, y))
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                entry.logical_size = Some((width, height))
            }
            _ => {}
        }
    }
}

delegate_noop!(OutputState: ignore wl_output::WlOutput);
delegate_noop!(OutputState: ignore zxdg_output_manager_v1::ZxdgOutputManagerV1);
