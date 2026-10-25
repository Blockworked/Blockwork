//! Exercise the actual macOS app helpers against the disposable AppKit fixture.
//! Set BW_LIVE_APP to the fixture's BlockworkTestWindow.app path.
#[cfg(all(target_os = "macos", feature = "system"))]
fn main() -> Result<(), String> {
    use blockwork_core::system::{app_running, close_application, open_application};
    use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
    use std::{path::Path, time::{Duration, Instant}};
    let target = std::env::var("BW_LIVE_APP").map_err(|e| e.to_string())?;
    if Path::new(&target).file_name().and_then(|name| name.to_str()) != Some("BlockworkTestWindow.app") {
        return Err("only target the dedicated native fixture".into());
    }
    let turn = || {
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, Duration::from_millis(30), false);
    };
    let close = || -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            close_application(&target, "Blockwork Test Window").map_err(|e| e.to_string())?;
            turn();
            if !app_running("BlockworkTestWindow")? { return Ok(()) }
            if Instant::now() >= deadline { return Err("fixture did not quit".into()) }
        }
    };
    close()?;
    open_application(&target).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app_running("BlockworkTestWindow")? {
        turn();
        if Instant::now() >= deadline { return Err("fixture did not launch".into()) }
    }
    close()?;
    println!("Native close → open → close passed for the dedicated fixture, without Apple Events.");
    Ok(())
}

#[cfg(not(all(target_os = "macos", feature = "system")))]
fn main() { eprintln!("This probe requires macOS and the system feature."); }
