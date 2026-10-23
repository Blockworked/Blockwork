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
#[cfg(unix)]
pub fn raise_current_thread_priority() {
    let param = unsafe {
        let mut p = std::mem::MaybeUninit::<libc::sched_param>::zeroed().assume_init();
        p.sched_priority = 10;
        p
    };
    let ok = unsafe {
        libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &param)
    } == 0;
    if ok {
        info!("Raised thread priority to SCHED_FIFO:10");
    } else {
        warn!("Failed to raise thread priority (SCHED_FIFO); continuing at default priority");
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
