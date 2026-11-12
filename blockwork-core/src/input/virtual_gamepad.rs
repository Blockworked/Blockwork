//! One strand owns the virtual controller; cancellation can revoke that ownership.
use crate::input::gamepad::GamepadState;
use crate::macros::backend::{InputBackend, ResetHandle};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
static OWNER: Mutex<Option<u64>> = Mutex::new(None);
static NEXT: AtomicU64 = AtomicU64::new(1);
pub struct Lease {
    id: u64,
    backend: Arc<Mutex<dyn InputBackend>>,
    reset: Option<ResetHandle>,
    active: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
    state: GamepadState,
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
}
fn release(
    id: u64,
    backend: &Arc<Mutex<dyn InputBackend>>,
    reset: &Option<ResetHandle>,
    active: &AtomicBool,
) {
    let mut owner = OWNER.lock().unwrap_or_else(|p| p.into_inner());
    if *owner == Some(id) {
        let result = if let Some(reset) = reset {
            reset()
        } else {
            backend
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .reset_gamepad()
        };
        if let Err(error) = result {
            tracing::warn!("Virtual controller reset failed: {error}");
        }
        *owner = None;
    }
    active.store(false, Ordering::Release);
}
impl Lease {
    pub fn acquire(
        backend: Arc<Mutex<dyn InputBackend>>,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<Self, String> {
        if cancelled() {
            return Err("Virtual controller run was cancelled".into());
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let reset = {
            let mut owner = OWNER.lock().map_err(|e| e.to_string())?;
            if owner.is_some() {
                return Err("Another strand owns the virtual controller".into());
            }
            let mut backend = backend.lock().map_err(|e| e.to_string())?;
            backend.ensure_gamepad_support()?;
            let reset = backend.gamepad_reset_handle();
            *owner = Some(id);
            reset
        };
        let active = Arc::new(AtomicBool::new(true));
        let quit = Arc::new(AtomicBool::new(false));
        let (watch_active, watch_quit, watch_backend, watch_reset, watch_cancelled) = (
            Arc::clone(&active),
            Arc::clone(&quit),
            Arc::clone(&backend),
            reset.clone(),
            Arc::clone(&cancelled),
        );
        if let Err(error) = std::thread::Builder::new()
            .name("gamepad-safety".into())
            .spawn(move || {
                while !watch_quit.load(Ordering::Acquire) {
                    if watch_cancelled() {
                        release(id, &watch_backend, &watch_reset, &watch_active);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
        {
            release(id, &backend, &reset, &active);
            return Err(format!("Couldn't start controller safety monitor: {error}"));
        }
        Ok(Self {
            id,
            backend,
            reset,
            active,
            quit,
            state: GamepadState::default(),
            cancelled,
        })
    }
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn state(&self) -> &GamepadState {
        &self.state
    }
    pub fn set(&mut self, state: GamepadState) -> Result<(), String> {
        state.validate()?;
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            if (self.cancelled)() || Instant::now() > deadline {
                release(self.id, &self.backend, &self.reset, &self.active);
                return Err("Virtual controller was cancelled or its backend was busy".into());
            }
            let owner = OWNER.lock().map_err(|e| e.to_string())?;
            if *owner != Some(self.id) || !self.is_active() {
                return Err(
                    "Virtual controller was reset while the run was stopped or paused".into(),
                );
            }
            match self.backend.try_lock() {
                Ok(mut backend) => {
                    let result = backend.gamepad_state(&state);
                    drop(backend);
                    drop(owner);
                    if let Err(error) = result {
                        release(self.id, &self.backend, &self.reset, &self.active);
                        return Err(error);
                    }
                    self.state = state;
                    return Ok(());
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    drop(owner);
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => {
                    drop(owner);
                    release(self.id, &self.backend, &self.reset, &self.active);
                    return Err(error.to_string());
                }
            }
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Release);
        release(self.id, &self.backend, &self.reset, &self.active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{
        gamepad::GamepadButton,
        types::{Axis, Direction, MacroButton, MacroKey},
    };
    static SERIAL: Mutex<()> = Mutex::new(());
    struct Fake {
        states: Arc<Mutex<Vec<GamepadState>>>,
        fail: Arc<AtomicBool>,
    }
    impl InputBackend for Fake {
        fn ensure_gamepad_support(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn gamepad_state(&mut self, state: &GamepadState) -> Result<(), String> {
            self.states.lock().unwrap().push(state.clone());
            if self.fail.load(Ordering::Relaxed) {
                Err("partial failure".into())
            } else {
                Ok(())
            }
        }
        fn gamepad_reset_handle(&self) -> Option<ResetHandle> {
            let states = Arc::clone(&self.states);
            Some(Arc::new(move || {
                states.lock().unwrap().push(GamepadState::default());
                Ok(())
            }))
        }
        fn key(&mut self, _: MacroKey, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn raw_keycode(&mut self, _: u16, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn button(&mut self, _: MacroButton, _: Direction) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_rel(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn move_mouse_abs(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: Axis) -> Result<(), String> {
            Ok(())
        }
        fn text(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            None
        }
    }
    #[test]
    fn cancellation_releases_controls_while_keyboard_backend_is_busy_and_old_drop_cannot_reset_a_new_owner()
     {
        let _serial = SERIAL.lock().unwrap();
        let states = Arc::new(Mutex::new(vec![]));
        let fail = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let backend: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(Fake {
            states: Arc::clone(&states),
            fail,
        }));
        let flag = Arc::clone(&stop);
        let mut first = Lease::acquire(
            Arc::clone(&backend),
            Arc::new(move || flag.load(Ordering::Acquire)),
        )
        .unwrap();
        let mut held = GamepadState::default();
        held.buttons.insert(GamepadButton::South);
        first.set(held.clone()).unwrap();
        assert!(Lease::acquire(Arc::clone(&backend), Arc::new(|| false)).is_err());
        let keyboard = backend.lock().unwrap();
        stop.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(1);
        while first.is_active() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            states.lock().unwrap().last().unwrap(),
            &GamepadState::default()
        );
        drop(keyboard);
        let mut next = Lease::acquire(Arc::clone(&backend), Arc::new(|| false)).unwrap();
        next.set(held.clone()).unwrap();
        drop(first);
        assert_eq!(states.lock().unwrap().last().unwrap(), &held);
        drop(next);
        assert_eq!(
            states.lock().unwrap().last().unwrap(),
            &GamepadState::default()
        );
    }
    #[test]
    fn an_injection_failure_neutralizes_immediately_and_frees_ownership() {
        let _serial = SERIAL.lock().unwrap();
        let states = Arc::new(Mutex::new(vec![]));
        let fail = Arc::new(AtomicBool::new(true));
        let backend: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(Fake {
            states: Arc::clone(&states),
            fail: Arc::clone(&fail),
        }));
        let mut lease = Lease::acquire(Arc::clone(&backend), Arc::new(|| false)).unwrap();
        let mut held = GamepadState::default();
        held.buttons.insert(GamepadButton::South);
        assert!(lease.set(held).is_err());
        assert!(!lease.is_active());
        assert_eq!(
            states.lock().unwrap().last().unwrap(),
            &GamepadState::default()
        );
        fail.store(false, Ordering::Release);
        let next = Lease::acquire(backend, Arc::new(|| false)).unwrap();
        drop(next);
    }
}
