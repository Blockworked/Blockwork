//! Shared-memory wire format between the Wine-hosted embedder and
//! `blockwork-linux-bridge`.

use crate::input::types::{MacroButton, MacroKey};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

/// Captured on the Linux side (real hardware input), consumed on the
/// Windows side and fed into `recording::build_capture_callback()` exactly
/// as if it came from a native OS hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireCaptureEvent {
    KeyPress(MacroKey),
    KeyRelease(MacroKey),
    ButtonPress(MacroButton),
    ButtonRelease(MacroButton),
    MouseMoveRel(i32, i32),
    MouseMoveAbs(f64, f64),
    Scroll(i32, i32),
}

/// A `CaptureTimestamp::Hardware` value can't cross the wire directly
/// (`SystemTime` isn't `#[repr(C)]`-portable) - split into seconds+nanos
/// since `UNIX_EPOCH` and reconstructed on the other side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WireTimestamp {
    pub secs: u64,
    pub nanos: u32,
}

impl WireTimestamp {
    pub fn from_system_time(t: std::time::SystemTime) -> Self {
        let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        Self { secs: d.as_secs(), nanos: d.subsec_nanos() }
    }

    pub fn to_system_time(self) -> std::time::SystemTime {
        std::time::UNIX_EPOCH + std::time::Duration::new(self.secs, self.nanos)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCapture {
    pub event: WireCaptureEvent,
    pub ts: WireTimestamp,
}

impl From<WireCaptureEvent> for crate::macros::backend::CaptureEvent {
    fn from(e: WireCaptureEvent) -> Self {
        use crate::macros::backend::CaptureEvent as CE;
        match e {
            WireCaptureEvent::KeyPress(k) => CE::KeyPress(k),
            WireCaptureEvent::KeyRelease(k) => CE::KeyRelease(k),
            WireCaptureEvent::ButtonPress(b) => CE::ButtonPress(b),
            WireCaptureEvent::ButtonRelease(b) => CE::ButtonRelease(b),
            WireCaptureEvent::MouseMoveRel(dx, dy) => CE::MouseMoveRel(dx, dy),
            WireCaptureEvent::MouseMoveAbs(x, y) => CE::MouseMoveAbs(x, y),
            WireCaptureEvent::Scroll(h, v) => CE::Scroll(h, v),
        }
    }
}

/// Control commands, Windows -> Linux. Runs happen on the Linux side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireControlCommand {
    /// Run the macro with this id - fire-and-forget, same contract as
    /// `blockwork_run_macro`. The `f64` is `elapsed_overshoot_ms`: real time
    /// already elapsed before this command was sent, since playback was
    /// supposed to start. Fed into `Macro::run_with_offset` so the first
    /// `Wait` deadline anchors to the intended start instant, not whenever
    /// this command gets dispatched.
    RunMacro(String, f64),
    /// Stops every in-flight run started via `RunMacro`, mirroring
    /// `blockwork_stop_loop`.
    StopLoop,
}

/// Max encoded message size.
pub const SLOT_SIZE: usize = 128;
/// Ring capacity.
pub const RING_CAPACITY: usize = 16384;

#[repr(C)]
pub struct RingSlot {
    pub len: u32,
    pub data: [u8; SLOT_SIZE - 4],
}

#[repr(C)]
pub struct RingBuffer {
    pub head: AtomicU32,
    pub tail: AtomicU32,
    /// Lock for multi-producer pushes.
    push_lock: AtomicU32,
    pub slots: [RingSlot; RING_CAPACITY],
}

impl RingBuffer {
    /// Safe for multiple concurrent producers (see `push_lock`). Returns
    /// `false` if the ring is full (caller's responsibility to retry/back
    /// off).
    pub fn try_push(&self, bytes: &[u8]) -> bool {
        assert!(bytes.len() <= SLOT_SIZE - 4, "encoded message exceeds SLOT_SIZE");

        while self.push_lock.compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }

        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) as usize >= RING_CAPACITY {
            self.push_lock.store(0, Ordering::Release);
            return false;
        }
        let idx = (head as usize) % RING_CAPACITY;
        // SAFETY: `push_lock` above ensures only one producer is ever in
        // this block at a time, and the consumer only reads a slot after
        // observing `head` advance past it (Release below).
        unsafe {
            let slot = &self.slots[idx] as *const RingSlot as *mut RingSlot;
            (&raw mut (*slot).len).write(bytes.len() as u32);
            (&mut (*slot).data)[..bytes.len()].copy_from_slice(bytes);
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
        self.push_lock.store(0, Ordering::Release);
        true
    }

    /// Single-consumer only. Returns `None` if the ring is empty.
    pub fn try_pop(&self, out: &mut [u8; SLOT_SIZE - 4]) -> Option<usize> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let idx = (tail as usize) % RING_CAPACITY;
        let slot = &self.slots[idx];
        let len = slot.len as usize;
        out[..len].copy_from_slice(&slot.data[..len]);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(len)
    }
}

#[repr(C)]
pub struct SharedRegion {
    /// Bumped by blockwork-ffi; blockwork-linux-bridge exits if this goes stale,
    /// so it never orphans itself if the host is killed/crashes.
    pub windows_heartbeat: AtomicU32,
    /// Bumped by blockwork-linux-bridge; lets blockwork-ffi notice the bridge
    /// process died (best-effort, not load-bearing in v1).
    pub linux_heartbeat: AtomicU32,
    /// Linux (producer) → Windows (consumer).
    pub capture: RingBuffer,
    /// Windows (producer) → Linux (consumer). Carries `WireControlCommand`s
    /// (was per-event `WireEmitCommand`s before playback timing moved to
    /// the Linux side).
    pub control: RingBuffer,
}

pub const SHARED_REGION_SIZE: usize = std::mem::size_of::<SharedRegion>();

pub fn encode_capture(msg: &WireCapture) -> Vec<u8> {
    bincode::serde::encode_to_vec(msg, bincode::config::standard()).expect("WireCapture encode")
}

pub fn decode_capture(bytes: &[u8]) -> Option<WireCapture> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard()).ok().map(|(v, _)| v)
}

pub fn encode_control(msg: &WireControlCommand) -> Vec<u8> {
    bincode::serde::encode_to_vec(msg, bincode::config::standard()).expect("WireControlCommand encode")
}

pub fn decode_control(bytes: &[u8]) -> Option<WireControlCommand> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard()).ok().map(|(v, _)| v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::types::MacroKey;

    #[test]
    fn capture_roundtrips_and_fits_slot() {
        let msg = WireCapture {
            event: WireCaptureEvent::KeyPress(MacroKey::Unicode('x')),
            ts: WireTimestamp::from_system_time(std::time::SystemTime::now()),
        };
        let bytes = encode_capture(&msg);
        assert!(bytes.len() <= SLOT_SIZE - 4, "encoded WireCapture ({} bytes) exceeds slot capacity", bytes.len());
        let decoded = decode_capture(&bytes).unwrap();
        match decoded.event {
            WireCaptureEvent::KeyPress(MacroKey::Unicode('x')) => {}
            other => panic!("roundtrip mismatch: {other:?}"),
        }
    }

    #[test]
    fn control_roundtrips_and_fits_slot() {
        // A real macro id (uuid::Uuid::new_v4().simple()) is 32 hex chars -
        // exercises the actual worst-case length rather than a short literal.
        let id = "0123456789abcdef0123456789abcdef".to_string();
        let msg = WireControlCommand::RunMacro(id.clone(), 37.5);
        let bytes = encode_control(&msg);
        assert!(bytes.len() <= SLOT_SIZE - 4, "encoded WireControlCommand ({} bytes) exceeds slot capacity", bytes.len());
        let decoded = decode_control(&bytes).unwrap();
        match decoded {
            WireControlCommand::RunMacro(decoded_id, overshoot_ms) if decoded_id == id && overshoot_ms == 37.5 => {}
            other => panic!("roundtrip mismatch: {other:?}"),
        }
    }

    #[test]
    fn ring_buffer_push_pop() {
        // Boxed: RING_CAPACITY * SLOT_SIZE is too large for the test thread's stack.
        let ring: Box<RingBuffer> = unsafe {
            let layout = std::alloc::Layout::new::<RingBuffer>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut RingBuffer;
            Box::from_raw(ptr)
        };
        assert!(ring.try_push(b"hello"));
        let mut buf = [0u8; SLOT_SIZE - 4];
        let len = ring.try_pop(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"hello");
        assert!(ring.try_pop(&mut buf).is_none());
    }

    /// Regression test for the multi-producer race (see `push_lock`): needs
    /// actual concurrent pushers to exercise the lock. Every pushed value
    /// must be popped exactly once, intact, with no lost count in `head`.
    #[test]
    fn ring_buffer_survives_concurrent_producers() {
        // Boxed for the same reason as above; `thread::scope` can borrow it
        // directly, no `'static` needed unlike `thread::spawn`.
        let ring: Box<RingBuffer> = unsafe {
            let layout = std::alloc::Layout::new::<RingBuffer>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut RingBuffer;
            Box::from_raw(ptr)
        };

        const PRODUCERS: usize = 10;
        const PER_PRODUCER: usize = 500;

        std::thread::scope(|scope| {
            for p in 0..PRODUCERS {
                let ring = &ring;
                scope.spawn(move || {
                    for i in 0..PER_PRODUCER {
                        let msg = format!("p{p}-{i}");
                        while !ring.try_push(msg.as_bytes()) {
                            std::thread::yield_now();
                        }
                    }
                });
            }
        });

        let mut seen = std::collections::HashSet::new();
        let mut buf = [0u8; SLOT_SIZE - 4];
        for _ in 0..(PRODUCERS * PER_PRODUCER) {
            let len = ring.try_pop(&mut buf).expect("ring should have exactly PRODUCERS*PER_PRODUCER items, got fewer");
            let s = std::str::from_utf8(&buf[..len]).unwrap().to_string();
            assert!(seen.insert(s.clone()), "duplicate/corrupted entry popped: {s}");
        }
        assert!(ring.try_pop(&mut buf).is_none(), "ring had extra items beyond what was pushed");
        assert_eq!(seen.len(), PRODUCERS * PER_PRODUCER);
    }
}
