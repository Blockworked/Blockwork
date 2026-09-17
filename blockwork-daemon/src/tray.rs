//! The tray icon shown while "close to tray" is enabled: left click opens the
//! editor, right click shows the Open/Quit menu.

use crate::MainThreadEvent;
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

pub(crate) struct Tray {
    icon: Option<TrayIcon>,
}

impl Tray {
    /// Must be created on the main thread, inside the event loop's lifetime.
    pub(crate) fn new(proxy: EventLoopProxy<MainThreadEvent>) -> Tray {
        let menu_proxy = proxy.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id().as_ref() {
                OPEN_ID => MainThreadEvent::OpenUi,
                QUIT_ID => MainThreadEvent::Quit,
                _ => return,
            };
            let _ = menu_proxy.send_event(action);
        }));
        let click_proxy = proxy;
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = click_proxy.send_event(MainThreadEvent::OpenUi);
            }
        }));
        Tray { icon: None }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.icon = None;
            return;
        }
        if self.icon.is_some() {
            return;
        }
        match build() {
            Ok(icon) => self.icon = Some(icon),
            Err(e) => tracing::warn!("Failed to create tray icon: {e}"),
        }
    }
}

fn build() -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    menu.append(&MenuItem::with_id(OPEN_ID, "Open", true, None))?;
    menu.append(&MenuItem::with_id(QUIT_ID, "Quit", true, None))?;

    Ok(TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Blockwork")
        .with_icon(icon()?)
        .build()?)
}

fn icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../../src-tauri/icons/128x128.png"
    )));
    let mut reader = decoder.read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size().ok_or("icon too large")?];
    let info = reader.next_frame(&mut rgba)?;
    rgba.truncate(info.buffer_size());
    Ok(Icon::from_rgba(rgba, info.width, info.height)?)
}
