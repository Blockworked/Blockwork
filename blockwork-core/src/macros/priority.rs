use tracing::{info, warn};

/// Best-effort: raise the calling thread's OS scheduling priority so it's less
/// likely to be preempted for multiple milliseconds under load. Falls back to
/// default priority if the platform/permissions don't allow it - logged
/// either way (not just on failure) since "did this actually take effect"
/// is otherwise impossible to tell from the log alone, and a silent failure
/// here (e.g. `SCHED_FIFO` requires `CAP_SYS_NICE`/an `rtprio` limit that a
/// process's launch environment may or may not actually grant - a login
/// session's `/etc/security/limits.d` rule doesn't necessarily reach a
/// process launched through several layers of Steam/Proton/Wine) is exactly
/// the kind of thing that silently reproduces the imprecision this call
/// exists to prevent.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn raise_current_thread_priority() {
    let param = unsafe {
        let mut p = std::mem::MaybeUninit::<libc::sched_param>::zeroed().assume_init();
        p.sched_priority = 10;
        p
    };
    let ok =
        unsafe { libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &param) } == 0;
    if ok {
        info!("Raised thread priority to SCHED_FIFO:10");
    } else {
        warn!("Failed to raise thread priority (SCHED_FIFO); continuing at default priority");
    }
}

#[cfg(target_os = "macos")]
pub fn raise_current_thread_priority() {
    // Darwin schedules latency-sensitive work through QoS. A POSIX FIFO
    // priority does not select this class and can clear an inherited QoS.
    let result = unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0)
    };
    if result == 0 {
        info!("Raised thread priority to QOS_CLASS_USER_INTERACTIVE");
    } else {
        warn!("Failed to set macOS interactive QoS ({result}); continuing at default priority");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn playback_threads_request_interactive_qos() {
        std::thread::spawn(|| {
            super::raise_current_thread_priority();
            let mut class = libc::qos_class_t::QOS_CLASS_UNSPECIFIED;
            let mut relative = 0;
            assert_eq!(
                unsafe {
                    libc::pthread_get_qos_class_np(libc::pthread_self(), &mut class, &mut relative)
                },
                0
            );
            assert_eq!(
                class as u32,
                libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE as u32
            );
            assert_eq!(relative, 0);
        })
        .join()
        .unwrap();
    }
}

#[cfg(windows)]
pub fn raise_current_thread_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
    };
    let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) } != 0;
    if ok {
        info!("Raised thread priority to THREAD_PRIORITY_TIME_CRITICAL");
    } else {
        warn!("Failed to raise thread priority; continuing at default priority");
    }
}
