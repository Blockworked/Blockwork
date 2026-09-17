//! The tray icon shown while "close to tray" is enabled: left click opens the
//! editor, right click shows the Open/Quit menu.

use crate::MainThreadEvent;
use tao::event_loop::EventLoopProxy;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{
    Icon as TrayImage, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

pub(crate) struct Tray {
    icon: Option<BuiltTray>,
    proxy: EventLoopProxy<MainThreadEvent>,
}

#[allow(dead_code)]
enum BuiltTray {
    Native(TrayIcon),
    #[cfg(all(unix, not(target_os = "macos")))]
    Flatpak(FlatpakTray),
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
        let click_proxy = proxy.clone();
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
        Tray { icon: None, proxy }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.icon = None;
            return;
        }
        if self.icon.is_some() {
            return;
        }
        match build(self.proxy.clone()) {
            Ok(icon) => self.icon = Some(icon),
            Err(e) => tracing::warn!("Failed to create tray icon: {e}"),
        }
    }
}

fn build(proxy: EventLoopProxy<MainThreadEvent>) -> Result<BuiltTray, Box<dyn std::error::Error>> {
    #[cfg(all(unix, not(target_os = "macos")))]
    if std::env::var_os("FLATPAK_ID").is_some() {
        return FlatpakTray::new(proxy).map(BuiltTray::Flatpak);
    }

    let menu = Menu::new();
    menu.append(&MenuItem::with_id(OPEN_ID, "Open", true, None))?;
    menu.append(&MenuItem::with_id(QUIT_ID, "Quit", true, None))?;

    Ok(BuiltTray::Native(
        TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Blockwork")
            .with_icon(icon()?)
            .build()?,
    ))
}

fn icon_rgba() -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../../src-tauri/icons/128x128.png"
    )));
    let mut reader = decoder.read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size().ok_or("icon too large")?];
    let info = reader.next_frame(&mut rgba)?;
    rgba.truncate(info.buffer_size());
    Ok((rgba, info.width, info.height))
}

fn icon() -> Result<TrayImage, Box<dyn std::error::Error>> {
    let (rgba, width, height) = icon_rgba()?;
    Ok(TrayImage::from_rgba(rgba, width, height)?)
}

#[cfg(all(unix, not(target_os = "macos")))]
struct FlatpakTray {
    handle: ksni::blocking::Handle<FlatpakNotifier>,
}

#[cfg(all(unix, not(target_os = "macos")))]
impl FlatpakTray {
    fn new(proxy: EventLoopProxy<MainThreadEvent>) -> Result<Self, Box<dyn std::error::Error>> {
        use ksni::blocking::TrayMethods;

        let (mut data, width, height) = icon_rgba()?;
        for pixel in data.as_chunks_mut::<4>().0 {
            pixel.rotate_right(1);
        }
        let notifier = FlatpakNotifier {
            proxy,
            icon: ksni::Icon {
                width: width as i32,
                height: height as i32,
                data,
            },
        };
        let handle = notifier
            .disable_dbus_name(true)
            .assume_sni_available(true)
            .spawn()?;
        Ok(Self { handle })
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
impl Drop for FlatpakTray {
    fn drop(&mut self) {
        self.handle.shutdown().wait();
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
struct FlatpakNotifier {
    proxy: EventLoopProxy<MainThreadEvent>,
    icon: ksni::Icon,
}

#[cfg(all(unix, not(target_os = "macos")))]
impl ksni::Tray for FlatpakNotifier {
    fn id(&self) -> String {
        "blockwork".into()
    }

    fn title(&self) -> String {
        "Blockwork".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_pixmap: self.icon_pixmap(),
            title: self.title(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _: i32, _: i32) {
        let _ = self.proxy.send_event(MainThreadEvent::OpenUi);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;

        vec![
            StandardItem {
                label: "Open".into(),
                activate: Box::new(|notifier: &mut Self| {
                    let _ = notifier.proxy.send_event(MainThreadEvent::OpenUi);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|notifier: &mut Self| {
                    let _ = notifier.proxy.send_event(MainThreadEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
