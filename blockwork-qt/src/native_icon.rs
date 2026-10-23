//! Native per-app icons for the "Choose an App" picker and the Open/Close App
//! blocks. The backend only ships icon data URIs on some platforms (Linux
//! icon themes); everywhere else the Qt UI resolves the launch target's file
//! icon itself (currently the Windows shell), which already knows how to
//! read them. Pure Rust - no C++ needed.

/// PNG data: URI for `command`'s native file icon, or an empty string when
/// none could be resolved. Pure and thread-safe; callers memoize.
#[cfg(windows)]
pub fn app_icon(command: &str, size: i32) -> String {
    imp::app_icon(command, size)
}

/// Non-Windows builds have no native source wired up yet (Linux listings
/// already carry theme icons from the backend); the UI keeps its placeholder.
#[cfg(not(windows))]
pub fn app_icon(_command: &str, _size: i32) -> String {
    String::new()
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::Com::*;
    use windows_sys::Win32::UI::Shell::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    pub fn app_icon(command: &str, size: i32) -> String {
        let mut path = command.trim().to_string();
        // Stored commands may quote paths containing spaces.
        if path.len() >= 2 && path.starts_with('"') && path.ends_with('"') {
            path = path[1..path.len() - 1].to_string();
        }
        // Shell PIDL parsing is picky about separators; normalize Rust-style
        // forward slashes to backslashes before calling into it.
        let path = path.replace('/', "\\");
        // Only real files have file icons: bare executable names and shell
        // lines with arguments must keep the UI placeholder.
        if path.is_empty() || !std::path::Path::new(&path).exists() {
            return String::new();
        }
        let wide: Vec<u16> = std::ffi::OsStr::new(&path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // Shortcut targets resolve through COM-based shell handlers; make
        // sure this thread is initialized (balanced: only uninitialize
        // what we own).
        let initialized =
            unsafe { CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32) };
        let mut hicon: HICON = std::ptr::null_mut();
        let edge = size.clamp(16, 256) as u32;
        // SHDefExtractIcon resolves shortcuts to their target's icon.
        let extracted = unsafe { SHDefExtractIconW(wide.as_ptr(), 0, 0, &mut hicon, std::ptr::null_mut(), edge) };
        if extracted < 0 {
            hicon = std::ptr::null_mut();
        }
        if hicon.is_null() {
            let mut info = SHFILEINFOW::default();
            let found = unsafe {
                SHGetFileInfoW(
                    wide.as_ptr(),
                    0,
                    &mut info,
                    std::mem::size_of::<SHFILEINFOW>() as u32,
                    SHGFI_ICON | SHGFI_LARGEICON,
                )
            };
            if found != 0 {
                hicon = info.hIcon;
            }
        }
        if initialized >= 0 {
            unsafe { CoUninitialize() };
        }
        if hicon.is_null() {
            return String::new();
        }
        let pixels = hicon_to_rgba(hicon);
        unsafe { DestroyIcon(hicon) };
        let Some((width, height, rgba)) = pixels else {
            return String::new();
        };
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = match encoder.write_header() {
                Ok(writer) => writer,
                Err(_) => return String::new(),
            };
            if writer.write_image_data(&rgba).is_err() {
                return String::new();
            }
        }
        use base64::Engine;
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png)
        )
    }

    /// Converts an HICON to RGBA bytes. Icons without an alpha channel
    /// (alpha byte all zero after GetDIBits) are rebuilt from the monochrome
    /// mask instead, otherwise they would render fully transparent.
    fn hicon_to_rgba(hicon: HICON) -> Option<(u32, u32, Vec<u8>)> {
        unsafe {
            let mut info = ICONINFO::default();
            if GetIconInfo(hicon, &mut info) == 0 {
                return None;
            }
            // Bound the GDI objects to this scope; every early return below
            // must release them first.
            let release = |info: &ICONINFO| {
                if !info.hbmColor.is_null() {
                    DeleteObject(info.hbmColor);
                }
                if !info.hbmMask.is_null() {
                    DeleteObject(info.hbmMask);
                }
            };
            let mut bitmap = BITMAP::default();
            if GetObjectW(
                info.hbmColor,
                std::mem::size_of::<BITMAP>() as i32,
                std::ptr::addr_of_mut!(bitmap) as *mut _,
            ) == 0
            {
                release(&info);
                return None;
            }
            let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
            if width <= 0 || height <= 0 {
                release(&info);
                return None;
            }
            let mut descriptor = BITMAPINFO::default();
            descriptor.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            descriptor.bmiHeader.biWidth = width;
            descriptor.bmiHeader.biHeight = -height; // top-down DIB
            descriptor.bmiHeader.biPlanes = 1;
            descriptor.bmiHeader.biBitCount = 32;
            descriptor.bmiHeader.biCompression = BI_RGB;
            let mut bgra = vec![0u8; width as usize * height as usize * 4];
            let dc: HWND = std::ptr::null_mut();
            let screen = GetDC(dc);
            let lines = GetDIBits(
                screen,
                info.hbmColor,
                0,
                height as u32,
                bgra.as_mut_ptr() as *mut _,
                &mut descriptor,
                DIB_RGB_COLORS,
            );
            ReleaseDC(dc, screen);
            if lines == 0 {
                release(&info);
                return None;
            }
            // 32-bit BI_RGB arrives as BGRA on little-endian; swap to RGBA.
            for pixel in bgra.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            if !bgra.chunks_exact(4).any(|pixel| pixel[3] != 0) && !info.hbmMask.is_null() {
                // 1-bit mask rows are DWORD-aligned, MSB first.
                let stride = ((width as usize + 31) / 32) * 4;
                let mut mask = vec![0u8; stride * height as usize];
                let mut mask_descriptor = BITMAPINFO::default();
                mask_descriptor.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                mask_descriptor.bmiHeader.biWidth = width;
                mask_descriptor.bmiHeader.biHeight = -height;
                mask_descriptor.bmiHeader.biPlanes = 1;
                mask_descriptor.bmiHeader.biBitCount = 1;
                mask_descriptor.bmiHeader.biCompression = BI_RGB;
                let dc: HWND = std::ptr::null_mut();
                let screen = GetDC(dc);
                let ok = GetDIBits(
                    screen,
                    info.hbmMask,
                    0,
                    height as u32,
                    mask.as_mut_ptr() as *mut _,
                    &mut mask_descriptor,
                    DIB_RGB_COLORS,
                );
                ReleaseDC(dc, screen);
                if ok != 0 {
                    for (y, row) in bgra.chunks_exact_mut(width as usize * 4).enumerate() {
                        for x in 0..width as usize {
                            let set = mask[y * stride + x / 8] >> (7 - (x % 8)) & 1 == 0;
                            row[x * 4 + 3] = if set { 0xFF } else { 0x00 };
                        }
                    }
                }
            }
            release(&info);
            Some((width as u32, height as u32, bgra))
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// End-to-end through the real shell: a Start Menu shortcut must resolve
    /// to a loadable PNG data URI. Needs no Qt application object and no
    /// daemon.
    #[test]
    fn resolves_start_menu_shortcut_icon() {
        let dir = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .expect("APPDATA is set on Windows")
            .join("Microsoft/Windows/Start Menu/Programs");
        let shortcut = walk_for_shortcut(&dir).expect("a .lnk exists in the Start Menu");
        let uri = app_icon(&shortcut.to_string_lossy(), 64);
        assert!(
            uri.starts_with("data:image/png;base64,"),
            "expected a PNG data URI, got {uri:?}"
        );
        use base64::Engine;
        let png = base64::engine::general_purpose::STANDARD
            .decode(uri.strip_prefix("data:image/png;base64,").expect("checked above"))
            .expect("valid base64");
        // A real PNG signature; QML's image loader accepts the same bytes.
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']));
        assert!(png.len() > 100, "icon payload suspiciously small");
    }

    fn walk_for_shortcut(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = walk_for_shortcut(&path) {
                    return Some(found);
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
            {
                return Some(path);
            }
        }
        None
    }
}
