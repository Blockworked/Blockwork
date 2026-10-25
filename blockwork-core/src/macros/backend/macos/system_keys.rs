//! macOS media keys use NX_SYSDEFINED, not ordinary virtual-key events.
//! The core-graphics event enum omits event type 14, so the keyboard tap uses
//! a raw integer ABI rather than constructing an invalid Rust enum value.
use super::{CGEvent, CGEventFlags, CaptureDecision, CaptureEvent, MacroKey};
use core_foundation::base::{CFRetain, TCFType};
use core_foundation::mach_port::{CFMachPort, CFMachPortInvalidate, CFMachPortRef};
use core_graphics::sys::CGEventRef;
use foreign_types::ForeignType;
use objc2::rc::{Retained, autoreleasepool};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use std::ffi::c_void;

pub(super) fn code(key: &MacroKey) -> Option<u16> {
    match key {
        MacroKey::VolumeUp => Some(0),
        MacroKey::VolumeDown => Some(1),
        MacroKey::VolumeMute => Some(7),
        _ => None,
    }
}

pub(super) fn event(code: u16, down: bool, flags: CGEventFlags) -> Result<CGEvent, String> {
    autoreleasepool(|_| {
        let native = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::SystemDefined, Default::default(), NSEventModifierFlags::empty(),
            0.0, 0, None, 8, ((code as isize) << 16) | ((if down { 0xa } else { 0xb }) << 8), -1,
        ).ok_or("couldn't create a macOS system-key event")?;
        let cg = native
            .CGEvent()
            .ok_or("system-key event has no CoreGraphics representation")?;
        let raw = Retained::as_ptr(&cg);
        // Keep our CGEvent alive independently of NSEvent and the local pool.
        let event = unsafe { CGEvent::from_ptr(CFRetain(raw.cast()) as CGEventRef) };
        event.set_flags(flags);
        Ok(event)
    })
}

pub(super) fn capture(event: &CGEvent) -> Option<CaptureEvent> {
    autoreleasepool(|_| {
        let native = NSEvent::eventWithCGEvent(unsafe { &*event.as_ptr().cast() })?;
        if native.subtype().0 != 8 {
            return None;
        }
        let data = native.data1();
        let key = match (data >> 16) & 0xffff {
            0 => MacroKey::VolumeUp,
            1 => MacroKey::VolumeDown,
            7 => MacroKey::VolumeMute,
            _ => return None,
        };
        match (data >> 8) & 0xff {
            0xa => Some(CaptureEvent::KeyPress(key)),
            0xb => Some(CaptureEvent::KeyRelease(key)),
            _ => None,
        }
    })
}

type Callback = Box<dyn FnMut(u32, &CGEvent) -> CaptureDecision>;
struct Context {
    port: CFMachPortRef,
    callback: Callback,
}

pub(super) struct KeyTap {
    port: CFMachPort,
    _context: Box<Context>,
}
impl KeyTap {
    pub(super) fn new(
        callback: impl FnMut(u32, &CGEvent) -> CaptureDecision + 'static,
    ) -> Result<Self, ()> {
        let mut context = Box::new(Context {
            port: std::ptr::null_mut(),
            callback: Box::new(callback),
        });
        let port = unsafe {
            create_tap(
                0,
                0,
                0,
                (1 << 10) | (1 << 11) | (1 << 12) | (1 << 14),
                dispatch,
                (&mut *context as *mut Context).cast(),
            )
        };
        if port.is_null() {
            return Err(());
        }
        context.port = port;
        Ok(Self {
            port: unsafe { CFMachPort::wrap_under_create_rule(port) },
            _context: context,
        })
    }
    pub(super) fn mach_port(&self) -> &CFMachPort {
        &self.port
    }
    pub(super) fn enable(&self) {
        unsafe { super::CGEventTapEnable(self.port.as_concrete_TypeRef(), true) }
    }
}
impl Drop for KeyTap {
    fn drop(&mut self) {
        unsafe { CFMachPortInvalidate(self.port.as_concrete_TypeRef()) }
    }
}

unsafe extern "C" fn dispatch(
    _proxy: *mut c_void,
    kind: u32,
    raw: CGEventRef,
    user: *mut c_void,
) -> CGEventRef {
    let context = unsafe { &mut *user.cast::<Context>() };
    if super::recover_tap_type(kind, context.port) || raw.is_null() {
        return raw;
    }
    let event = unsafe { CGEvent::from_ptr(CFRetain(raw.cast()) as CGEventRef) };
    // Never unwind across the native event-tap callback boundary.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (context.callback)(kind, &event)
    })) {
        Ok(CaptureDecision::Suppress) => std::ptr::null_mut(),
        Ok(_) => raw,
        Err(_) => {
            tracing::warn!("macOS keyboard callback panicked; passing the event through");
            raw
        }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGEventTapCreate"]
    fn create_tap(
        location: u32,
        placement: u32,
        options: u32,
        mask: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef,
        user: *mut c_void,
    ) -> CFMachPortRef;
}
