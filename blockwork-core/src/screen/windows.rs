//! Windows screen access through GDI.

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

pub(super) fn capture_area(_: Rect) -> Option<Shot> {
    None
}

pub(super) fn capture() -> Result<Shot, String> {
    // SAFETY: plain GDI calls; every handle made here is released below.
    unsafe {
        let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
        let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
        if w <= 0 || h <= 0 {
            return Err("couldn't find the screen".into());
        }
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
        let mut bgra = vec![0u8; w as usize * h as usize * 4];
        let rows = GetDIBits(memory, bitmap, 0, h as u32, bgra.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        SelectObject(memory, previous);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(null_mut(), screen);
        if copied == 0 || rows == 0 {
            return Err("couldn't capture the screen".into());
        }
        let rgba = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
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
