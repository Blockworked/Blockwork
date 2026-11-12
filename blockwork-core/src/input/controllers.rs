//! One physical controller reader. Subscribers receive ordered state changes.
use crate::input::gamepad::{AXES, BUTTONS, GamepadAxis, GamepadButton};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

/// One connected physical controller as last read.
#[derive(Debug, Clone, Serialize)]
pub struct Controller {
    /// `<device uuid>:<name>`, what a block stores to pick this controller.
    pub profile: String,
    /// gilrs' id for this connection; changes on reconnect.
    pub instance: usize,
    pub name: String,
    pub buttons: BTreeMap<GamepadButton, bool>,
    pub axes: BTreeMap<GamepadAxis, f64>,
}

/// A change on any controller, with every controller's state after it.
#[derive(Clone)]
pub struct Event {
    pub at: Instant,
    pub controllers: Vec<Controller>,
}

struct Subscriber {
    sender: mpsc::SyncSender<Event>,
    overflow: Arc<AtomicBool>,
}

struct State {
    controllers: Vec<Controller>,
    error: Option<String>,
    subscribers: Vec<Subscriber>,
}

/// The shared reader: one thread, any number of subscribers.
struct Hub {
    enabled: AtomicBool,
    state: Mutex<State>,
}

static HUB: OnceLock<Arc<Hub>> = OnceLock::new();

const DISABLED: &str = "Controller input is disabled in Settings";

/// Ordered changes for one reader. A full queue drops events and sets
/// [`Subscription::overflowed`].
pub struct Subscription {
    receiver: mpsc::Receiver<Event>,
    overflow: Arc<AtomicBool>,
}

impl Subscription {
    pub fn try_recv(&self) -> Result<Event, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
    pub fn overflowed(&self) -> bool {
        self.overflow.swap(false, Ordering::AcqRel)
    }
}

/// Our logical button as gilrs names it.
fn button(button: GamepadButton) -> gilrs::Button {
    use gilrs::Button as B;
    match button {
        GamepadButton::South => B::South,
        GamepadButton::East => B::East,
        GamepadButton::West => B::West,
        GamepadButton::North => B::North,
        GamepadButton::LeftShoulder => B::LeftTrigger,
        GamepadButton::RightShoulder => B::RightTrigger,
        GamepadButton::LeftStick => B::LeftThumb,
        GamepadButton::RightStick => B::RightThumb,
        GamepadButton::Select => B::Select,
        GamepadButton::Start => B::Start,
        GamepadButton::Mode => B::Mode,
        GamepadButton::DPadUp => B::DPadUp,
        GamepadButton::DPadDown => B::DPadDown,
        GamepadButton::DPadLeft => B::DPadLeft,
        GamepadButton::DPadRight => B::DPadRight,
    }
}

/// One axis in -1..=1; triggers read their analog button value.
fn axis(gamepad: &gilrs::Gamepad<'_>, axis: GamepadAxis) -> f64 {
    use gilrs::{Axis as A, Button as B};
    let value = match axis {
        GamepadAxis::LeftX => gamepad.value(A::LeftStickX),
        GamepadAxis::LeftY => gamepad.value(A::LeftStickY),
        GamepadAxis::RightX => gamepad.value(A::RightStickX),
        GamepadAxis::RightY => gamepad.value(A::RightStickY),
        GamepadAxis::LeftTrigger => gamepad
            .button_data(B::LeftTrigger2)
            .map_or(0.0, |b| b.value()),
        GamepadAxis::RightTrigger => gamepad
            .button_data(B::RightTrigger2)
            .map_or(0.0, |b| b.value()),
    } as f64;
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Every connected controller except Blockwork's own virtual one.
fn snapshot(gilrs: &gilrs::Gilrs) -> Vec<Controller> {
    gilrs
        .gamepads()
        .filter(|(_, g)| g.is_connected() && !crate::input::is_own_device(g.name()))
        .map(|(id, g)| Controller {
            profile: format!("{}:{}", uuid::Uuid::from_bytes(g.uuid()).simple(), g.name()),
            instance: usize::from(id),
            name: g.name().into(),
            buttons: BUTTONS
                .iter()
                .map(|b| (*b, g.is_pressed(button(*b))))
                .collect(),
            axes: AXES.iter().map(|a| (*a, axis(&g, *a))).collect(),
        })
        .collect()
}

/// Stores the new state and hands it to every live subscriber.
fn publish(hub: &Hub, controllers: Vec<Controller>) {
    let mut state = hub.state.lock().unwrap_or_else(|p| p.into_inner());
    state.controllers = controllers;
    state.error = None;
    let event = Event {
        at: Instant::now(),
        controllers: state.controllers.clone(),
    };
    state.subscribers.retain(
        |subscriber| match subscriber.sender.try_send(event.clone()) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full(_)) => {
                subscriber.overflow.store(true, Ordering::Release);
                true
            }
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        },
    );
}

/// Starts the reader thread and waits briefly for its first snapshot.
fn start() -> Arc<Hub> {
    let hub = Arc::new(Hub {
        enabled: AtomicBool::new(true),
        state: Mutex::new(State {
            controllers: vec![],
            error: Some("Controller input is starting".into()),
            subscribers: vec![],
        }),
    });
    let worker = Arc::clone(&hub);
    let (ready, wait) = mpsc::sync_channel(1);
    if let Err(error) = std::thread::Builder::new()
        .name("controller-input".into())
        .spawn(move || read(&worker, &ready))
    {
        hub.state.lock().unwrap().error = Some(format!("Couldn't start controller input: {error}"));
    }
    let _ = wait.recv_timeout(Duration::from_secs(2));
    hub
}

/// The reader thread: opens gilrs while input is enabled and publishes a
/// snapshot after every event, so short press/release pairs survive.
fn read(worker: &Hub, ready: &mpsc::SyncSender<()>) {
    let mut reader = None;
    loop {
        if !worker.enabled.load(Ordering::Acquire) {
            reader = None;
            {
                let mut state = worker.state.lock().unwrap_or_else(|p| p.into_inner());
                state.controllers.clear();
                state.error = Some(DISABLED.into());
            }
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        if reader.is_none() {
            match gilrs::Gilrs::new() {
                Ok(gilrs) => {
                    publish(worker, snapshot(&gilrs));
                    reader = Some(gilrs);
                    let _ = ready.try_send(());
                }
                Err(error) => {
                    worker.state.lock().unwrap_or_else(|p| p.into_inner()).error =
                        Some(format!("Controller service unavailable: {error}"));
                    let _ = ready.try_send(());
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            }
        }
        let gilrs = reader.as_mut().unwrap();
        // Publish each change, preserving short press/release pairs.
        for _ in 0..1024 {
            if gilrs.next_event().is_none() {
                break;
            }
            publish(worker, snapshot(gilrs));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The running hub, or why controller input is off.
fn service() -> Result<&'static Arc<Hub>, String> {
    if !crate::config::load_settings()
        .controller_input_enabled
        .unwrap_or(false)
    {
        return Err(DISABLED.into());
    }
    let hub = HUB.get_or_init(start);
    hub.enabled.store(true, Ordering::Release);
    Ok(hub)
}

/// Pauses or resumes reading when the setting changes.
pub fn set_enabled(enabled: bool) {
    if let Some(hub) = HUB.get() {
        hub.enabled.store(enabled, Ordering::Release);
    }
}

/// The controllers connected now.
pub fn list() -> Result<Vec<Controller>, String> {
    let hub = service()?;
    let state = hub.state.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &state.error {
        return Err(error.clone());
    }
    Ok(state.controllers.clone())
}

/// The controllers now, plus every change after this call.
pub fn subscribe() -> Result<(Vec<Controller>, Subscription), String> {
    let hub = service()?;
    let mut state = hub.state.lock().map_err(|e| e.to_string())?;
    if let Some(error) = &state.error {
        return Err(error.clone());
    }
    let (sender, receiver) = mpsc::sync_channel(256);
    let overflow = Arc::new(AtomicBool::new(false));
    state.subscribers.push(Subscriber {
        sender,
        overflow: Arc::clone(&overflow),
    });
    Ok((
        state.controllers.clone(),
        Subscription { receiver, overflow },
    ))
}

/// The one controller a profile names; an empty profile means the only
/// one connected. Two matches are an error rather than a guess.
pub fn select<'a>(
    controllers: &'a [Controller],
    profile: &str,
) -> Result<Option<&'a Controller>, String> {
    let mut matching = controllers
        .iter()
        .filter(|g| profile.is_empty() || g.profile == profile);
    let found = matching.next();
    if matching.next().is_some() {
        return Err("More than one controller matches this profile; disconnect the identical device or choose another profile".into());
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_profiles_never_select_an_arbitrary_controller() {
        let one = Controller {
            profile: "same".into(),
            instance: 1,
            name: "Pad".into(),
            buttons: BTreeMap::new(),
            axes: BTreeMap::new(),
        };
        let mut two = one.clone();
        two.instance = 2;
        assert!(
            select(std::slice::from_ref(&one), "same")
                .unwrap()
                .is_some()
        );
        assert!(select(&[one.clone(), two.clone()], "same").is_err());
        assert!(select(&[one.clone(), two], "").is_err());
        assert!(select(&[one], "missing").unwrap().is_none());
    }
}
