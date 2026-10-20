//! Windows screen access through GDI.
//!
//! The process is per-monitor DPI aware (the daemon's event loop sets it),
//! so screen coordinates are physical pixels and every shot has scale 1.

use super::{Image, Rect, Shot};
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC, GetDIBits,
    GetMonitorInfoW, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HDC, HMONITOR,
    MONITORINFO, SRCCOPY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// GDI copies just the asked-for pixels, so a small area is far cheaper
/// than the whole desktop (a 4K desktop takes ~100 ms).
pub(super) const CHEAP_AREAS: bool = true;

/// The box around every monitor: (x, y, width, height).
fn virtual_screen() -> (i32, i32, i32, i32) {
    // SAFETY: plain metrics queries.
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

/// `rect`, clipped to the desktop. `None` when none of it is on screen, so
/// the caller falls back to a full capture and its off-screen error.
pub(super) fn capture_area(rect: Rect, _native: bool) -> Option<Shot> {
    let (vx, vy, vw, vh) = virtual_screen();
    let x0 = (rect.x.floor() as i32).max(vx);
    let y0 = (rect.y.floor() as i32).max(vy);
    let x1 = ((rect.x + rect.width).ceil() as i32).min(vx + vw);
    let y1 = ((rect.y + rect.height).ceil() as i32).min(vy + vh);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    grab(x0, y0, x1 - x0, y1 - y0).ok()
}

pub(super) fn capture() -> Result<Shot, String> {
    let (x, y, w, h) = virtual_screen();
    if w <= 0 || h <= 0 {
        return Err("couldn't find the screen".into());
    }
    grab(x, y, w, h)
}

/// Copies the `w` x `h` screen pixels at (`x`, `y`).
fn grab(x: i32, y: i32, w: i32, h: i32) -> Result<Shot, String> {
    // SAFETY: plain GDI calls; every handle made here is released below.
    unsafe {
        let screen = GetDC(null_mut());
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let previous = SelectObject(memory, bitmap);
        let copied = BitBlt(memory, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT);
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down rows
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut rgba = vec![0u8; w as usize * h as usize * 4];
        let rows = GetDIBits(memory, bitmap, 0, h as u32, rgba.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        SelectObject(memory, previous);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(null_mut(), screen);
        if copied == 0 || rows == 0 {
            // BitBlt fails while the secure desktop (lock screen, UAC) shows.
            return Err("couldn't capture the screen (is it locked?)".into());
        }
        // BGRA to RGBA, in place.
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Ok(Shot { image: Image { width: w as u32, height: h as u32, rgba }, origin: (x as f64, y as f64), scale: 1.0 })
    }
}

pub(super) fn monitors() -> Vec<Rect> {
    unsafe extern "system" fn add(monitor: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> i32 {
        // SAFETY: `out` is the Vec passed to EnumDisplayMonitors below.
        unsafe {
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) != 0 {
                let r = info.rcMonitor;
                (*(out as *mut Vec<Rect>)).push(Rect {
                    x: r.left as f64,
                    y: r.top as f64,
                    width: (r.right - r.left) as f64,
                    height: (r.bottom - r.top) as f64,
                });
            }
        }
        1
    }
    let mut out: Vec<Rect> = Vec::new();
    // SAFETY: the callback only runs during this call, while `out` lives.
    unsafe { EnumDisplayMonitors(null_mut(), std::ptr::null(), Some(add), &mut out as *mut Vec<Rect> as LPARAM) };
    out
}

pub(super) fn cursor() -> Result<(i32, i32), String> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: writes one POINT.
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err("couldn't read the cursor position".into());
    }
    Ok((point.x, point.y))
}
