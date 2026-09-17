//! Blockwork's long-lived core process. Owns all app state and every
//! background service (input grab, global hotkeys, playback, battery/time
//! watchers, the macro-control IPC server) plus the tray icon, and serves the
//! editor UI over a local socket (see `blockwork-protocol`).
//!
//! The UI (`blockwork`, which embeds CEF) is a separate process so closing the
//! window frees Chromium's browser, GPU, network and renderer processes
//! entirely; this process stays small and keeps hotkeys working while "close
//! to tray" is on.
//!
//! Lifecycle:
//! - Started by the first `blockwork` launch; a second daemon exits
//!   immediately because the socket is already taken.
//! - Exits once the last UI disconnects, unless "close to tray" is on.
//! - Tray "Open" relaunches the UI; tray "Quit" closes the UI and exits.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(feature = "dev-bridge")]
mod dev_bridge;
mod server;
mod tray;

use blockwork_app::{Backend, Event};
use std::sync::Arc;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};

/// Work the socket server and backend hand to the main (tray) thread.
#[derive(Debug)]
pub(crate) enum MainThreadEvent {
    SetTrayVisible(bool),
    OpenUi,
    Quit,
}

fn main() {
    tracing_subscriber::fmt::init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to start the tokio runtime");

    let listener = match runtime.block_on(blockwork_protocol::Listener::bind()) {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::info!("Another Blockwork daemon is already running");
            return;
        }
        Err(e) => {
            tracing::error!("Failed to open the daemon socket: {e}");
            std::process::exit(1);
        }
    };

    let listener = Arc::new(listener);

    blockwork_app::migrate_legacy_app_id();

    let backend = {
        let _guard = runtime.enter();
        Backend::start(runtime.handle().clone())
    };

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<MainThreadEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        // A background process: no Dock icon or menu bar of its own.
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }
    let proxy = event_loop.create_proxy();

    let clients = Arc::new(server::Clients::default());
    runtime.spawn(server::serve(
        Arc::clone(&listener),
        backend.clone(),
        Arc::clone(&clients),
        proxy.clone(),
    ));
    runtime.spawn(forward_backend_events(
        backend.clone(),
        proxy.clone(),
        listener,
    ));

    #[cfg(feature = "dev-bridge")]
    runtime.spawn(dev_bridge::run(backend.clone()));

    let mut tray = tray::Tray::new(proxy.clone());
    tray.set_visible(backend.close_to_tray());

    // Hold the runtime for the life of the process: the event loop below
    // never returns, and dropping it would stop every backend task.
    let runtime = Arc::new(runtime);
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let _ = &runtime;
        if let tao::event::Event::UserEvent(event) = event {
            match event {
                MainThreadEvent::SetTrayVisible(visible) => tray.set_visible(visible),
                MainThreadEvent::OpenUi => {
                    if !clients.focus_ui() {
                        server::spawn_ui();
                    }
                }
                MainThreadEvent::Quit => {
                    backend.request_quit();
                }
            }
        }
    });
}

/// Turns backend [`Event`]s that concern the process as a whole into work for
/// the main thread. (UI-facing events are forwarded per connection instead,
/// in `server`.)
async fn forward_backend_events(
    backend: Backend,
    proxy: EventLoopProxy<MainThreadEvent>,
    listener: Arc<blockwork_protocol::Listener>,
) {
    let mut events = backend.subscribe();
    loop {
        match events.recv().await {
            Ok(Event::CloseToTray(visible)) => {
                let _ = proxy.send_event(MainThreadEvent::SetTrayVisible(visible));
            }
            Ok(Event::Quit) => {
                // Give each connection a moment to deliver its `quit` line
                // before the process (and the socket) goes away.
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                listener.cleanup();
                std::process::exit(0);
            }
            Ok(Event::State(_)) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}
