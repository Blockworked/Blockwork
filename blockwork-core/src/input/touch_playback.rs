//! Preflight, exclusive contact ownership and cleanup for touch playback.
use crate::input::touch::{NORMALIZED_MAX, TouchSequence};
use crate::macros::backend::InputBackend;
use crate::screen::Rect;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
static LEASE: Mutex<()> = Mutex::new(());

pub fn map_to_screen(
    sequence: &TouchSequence,
    normalized: bool,
    bounds: Rect,
) -> Result<TouchSequence, String> {
    sequence.validate()?;
    if bounds.width < 1.0 || bounds.height < 1.0 {
        return Err("Touch target monitor has no usable geometry".into());
    }
    let mut result = sequence.clone();
    for frame in &mut result.frames {
        for point in &mut frame.contacts {
            if normalized {
                if !(0..=NORMALIZED_MAX).contains(&point.x)
                    || !(0..=NORMALIZED_MAX).contains(&point.y)
                {
                    return Err("Normalized touch points must be inside the target monitor".into());
                }
                point.x = (bounds.x + point.x as f64 / NORMALIZED_MAX as f64 * (bounds.width - 1.0))
                    .round() as i32;
                point.y = (bounds.y
                    + point.y as f64 / NORMALIZED_MAX as f64 * (bounds.height - 1.0))
                    .round() as i32;
            }
            if (point.x as f64) < bounds.x
                || (point.y as f64) < bounds.y
                || (point.x as f64) >= bounds.x + bounds.width
                || (point.y as f64) >= bounds.y + bounds.height
            {
                return Err("A touch point falls outside the target monitor".into());
            }
        }
    }
    Ok(result)
}
struct Contacts(
    Arc<Mutex<dyn InputBackend>>,
    Option<crate::macros::backend::ResetHandle>,
);
impl Drop for Contacts {
    fn drop(&mut self) {
        let result = if let Some(reset) = &self.1 {
            reset()
        } else {
            self.0
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .cancel_touch()
        };
        if let Err(error) = result {
            tracing::warn!("Touch release failed: {error}");
        }
    }
}
pub fn play(
    sequence: &TouchSequence,
    backend: Arc<Mutex<dyn InputBackend>>,
    speed: f64,
    mut interrupted: impl FnMut() -> bool,
) -> Result<(), String> {
    sequence.validate()?;
    if !speed.is_finite() || speed <= 0.0 {
        return Err("Touch playback speed must be positive and finite".into());
    }
    let seconds = sequence.frames.last().unwrap().at_ms as f64 / 1000.0 / speed;
    if !seconds.is_finite() || seconds > 3600.0 {
        return Err("Touch playback exceeds one hour at this speed".into());
    }
    let _lease = LEASE
        .try_lock()
        .map_err(|_| "Another strand is using touch input")?;
    backend
        .lock()
        .map_err(|e| e.to_string())?
        .ensure_touch_support()?;
    let reset = backend
        .lock()
        .map_err(|e| e.to_string())?
        .touch_reset_handle();
    let _contacts = Contacts(Arc::clone(&backend), reset);
    let start = Instant::now();
    for frame in &sequence.frames {
        let deadline = start + Duration::from_secs_f64(frame.at_ms as f64 / 1000.0 / speed);
        loop {
            if interrupted() {
                return Err("Touch playback stopped, paused or its monitor changed".into());
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            std::thread::sleep((deadline - now).min(Duration::from_millis(5)));
        }
        if deadline.elapsed() > Duration::from_millis(250) {
            return Err("Touch playback fell behind; contacts were released".into());
        }
        loop {
            if interrupted() {
                return Err("Touch playback was interrupted".into());
            }
            if deadline.elapsed() > Duration::from_millis(250) {
                return Err("Touch backend was busy; contacts were released".into());
            }
            match backend.try_lock() {
                Ok(mut backend) => {
                    backend.touch_frame(&frame.contacts)?;
                    break;
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::touch::{TouchContact, TouchFrame};
    #[test]
    fn normalized_paths_map_to_negative_monitor_origins_and_preflight_all_frames() {
        let sequence = TouchSequence {
            version: 1,
            frames: vec![
                TouchFrame {
                    at_ms: 0,
                    contacts: vec![TouchContact {
                        id: 0,
                        x: 1000000,
                        y: 0,
                    }],
                },
                TouchFrame {
                    at_ms: 10,
                    contacts: vec![],
                },
            ],
        };
        let bounds = Rect {
            x: -1920.0,
            y: 100.0,
            width: 1920.0,
            height: 1080.0,
        };
        let mapped = map_to_screen(&sequence, true, bounds).unwrap();
        assert_eq!(
            (
                mapped.frames[0].contacts[0].x,
                mapped.frames[0].contacts[0].y
            ),
            (-1, 100)
        );
        assert!(map_to_screen(&sequence, false, bounds).is_err());
    }
}

#[cfg(test)]
mod playback_tests {
    use super::*;
    use crate::input::{
        touch::{TouchContact, TouchFrame},
        types::{Axis, Direction, MacroButton, MacroKey},
    };
    static SERIAL: Mutex<()> = Mutex::new(());
    #[derive(Default)]
    struct Events {
        frames: usize,
        cancelled: usize,
    }
    struct Fake {
        events: Arc<Mutex<Events>>,
        fail: bool,
    }
    impl InputBackend for Fake {
        fn ensure_touch_support(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn touch_frame(&mut self, _: &[TouchContact]) -> Result<(), String> {
            self.events.lock().unwrap().frames += 1;
            if self.fail {
                Err("partial injection failure".into())
            } else {
                Ok(())
            }
        }
        fn cancel_touch(&mut self) -> Result<(), String> {
            self.events.lock().unwrap().cancelled += 1;
            Ok(())
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
    fn sequence() -> TouchSequence {
        TouchSequence {
            version: 1,
            frames: vec![
                TouchFrame {
                    at_ms: 0,
                    contacts: vec![TouchContact { id: 0, x: 1, y: 2 }],
                },
                TouchFrame {
                    at_ms: 1,
                    contacts: vec![],
                },
            ],
        }
    }
    #[test]
    fn failure_and_cancellation_release_all_contacts_and_the_lease() {
        let _serial = SERIAL.lock().unwrap();
        for (fail, stopped, sent) in [(true, false, 1), (false, true, 0), (false, false, 2)] {
            let events = Arc::new(Mutex::new(Events::default()));
            let backend = Arc::new(Mutex::new(Fake {
                events: Arc::clone(&events),
                fail,
            }));
            let result = play(&sequence(), backend, 1.0, || stopped);
            assert_eq!(result.is_err(), fail || stopped);
            let log = events.lock().unwrap();
            assert_eq!(log.frames, sent);
            assert_eq!(log.cancelled, 1);
        }
    }
    #[test]
    fn invalid_paths_and_extreme_speed_fail_before_touching_a_device() {
        let _serial = SERIAL.lock().unwrap();
        let events = Arc::new(Mutex::new(Events::default()));
        let backend: Arc<Mutex<dyn InputBackend>> = Arc::new(Mutex::new(Fake {
            events: Arc::clone(&events),
            fail: false,
        }));
        let mut invalid = sequence();
        invalid.frames[1].at_ms = 0;
        assert!(play(&invalid, Arc::clone(&backend), 1.0, || false).is_err());
        assert!(play(&sequence(), backend, 1e-300, || false).is_err());
        assert_eq!(events.lock().unwrap().cancelled, 0);
    }
}
