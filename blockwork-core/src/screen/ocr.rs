//! Reading text out of an image. Uses the OS's own OCR on Windows and macOS,
//! and Tesseract elsewhere (or when it's installed and the built-in one fails).

use super::Image;
use std::io::Write;
use std::process::Stdio;

/// Text in `image`, lines joined with newlines and trimmed.
pub(super) fn read(image: &Image) -> Result<String, String> {
    // There can be no visible glyphs in a uniform or fully transparent image.
    // Besides avoiding unnecessary work, this avoids Vision stalls observed
    // on empty images (even its CPU path can wait indefinitely on those).
    let uniform = image
        .rgba
        .chunks_exact(4)
        .next()
        .is_none_or(|first| image.rgba.chunks_exact(4).all(|pixel| pixel == first));
    if uniform || image.rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        return Ok(String::new());
    }
    // OCR engines want text a good 20px+ tall; small regions are blown up.
    let factor = (90 / image.height.max(1)).clamp(1, 4);
    let image = image.scale_up(factor);
    let png = image.to_png()?;
    let text = match native(&png) {
        Some(Ok(text)) => text,
        native_result => match tesseract(&png) {
            Ok(text) => text,
            Err(e) => return Err(native_result.and_then(Result::err).unwrap_or(e)),
        },
    };
    Ok(text
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string())
}

fn tesseract(png: &[u8]) -> Result<String, String> {
    let mut child = crate::flatpak::host_command("tesseract")
        .args(["stdin", "stdout", "--psm", "6"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            "reading text needs Tesseract OCR installed (package 'tesseract')".to_string()
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(png)
            .map_err(|e| format!("couldn't send the image to Tesseract: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Tesseract failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Tesseract failed: {}",
            err.lines().last().unwrap_or("unknown error").trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Writes `png` to a temp file for a tool that only reads paths.
#[cfg(any(windows, target_os = "macos"))]
fn temp_png(png: &[u8]) -> Result<tempfile_path::TempPath, String> {
    let path = std::env::temp_dir().join(format!(
        "blockwork-ocr-{}.png",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&path, png).map_err(|e| format!("couldn't write a temp image: {e}"))?;
    Ok(tempfile_path::TempPath(path))
}

#[cfg(any(windows, target_os = "macos"))]
mod tempfile_path {
    /// Deletes the file when dropped.
    pub(super) struct TempPath(pub std::path::PathBuf);
    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

/// Windows.Media.Ocr through PowerShell, which can await WinRT calls.
#[cfg(windows)]
fn native(png: &[u8]) -> Option<Result<String, String>> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const SCRIPT: &str = r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, [Type]$type) { $task = $asTask.MakeGenericMethod($type).Invoke($null, @($op)); $task.Wait(-1) | Out-Null; $task.Result }
[Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime] | Out-Null
[Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null
[Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics, ContentType = WindowsRuntime] | Out-Null
$file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($env:BLOCKWORK_OCR_IMAGE)) ([Windows.Storage.StorageFile])
$stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
$decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
$bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
if ($engine -eq $null) { throw 'none of your languages has text recognition; add one under Settings > Time & language > Language & region' }
$result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
# One word per line: x, y, width, height and the text, tab separated.
foreach ($line in $result.Lines) { foreach ($w in $line.Words) { $r = $w.BoundingRect; "{0}`t{1}`t{2}`t{3}`t{4}" -f [int]$r.X, [int]$r.Y, [int]$r.Width, [int]$r.Height, $w.Text } }
"#;
    let file = match temp_png(png) {
        Ok(file) => file,
        Err(e) => return Some(Err(e)),
    };
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("BLOCKWORK_OCR_IMAGE", &file.0)
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    Some(match output {
        Ok(out) if out.status.success() => Ok(rows(&String::from_utf8_lossy(&out.stdout))),
        Ok(out) => Err(format!(
            "Windows OCR failed: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
        )),
        Err(e) => Err(format!("couldn't run Windows OCR: {e}")),
    })
}

/// Puts `x y width height text` words back into lines by where they sit.
/// Windows OCR's own lines split a row whenever the gap between words is
/// wide (`"key":` and its value land on separate lines).
#[cfg(any(windows, test))]
fn rows(words: &str) -> String {
    struct Word<'a> {
        x: f64,
        top: f64,
        bottom: f64,
        text: &'a str,
    }
    let mut words: Vec<Word> = words
        .lines()
        .filter_map(|line| {
            let mut parts = line.trim_end_matches('\r').splitn(5, '\t');
            let mut num = || parts.next()?.trim().parse::<f64>().ok();
            let (x, y, _, height) = (num()?, num()?, num()?, num()?);
            let text = parts.next()?;
            Some(Word {
                x,
                top: y,
                bottom: y + height,
                text,
            })
        })
        .collect();
    words.sort_by(|a, b| (a.top + a.bottom).total_cmp(&(b.top + b.bottom)));
    // A word joins the row above when at least half of it overlaps the row
    // vertically (an underscore sits low but inside it; the next line doesn't).
    // The row grows to cover its words, so a short quote mark that starts a
    // row doesn't shut out the rest of it.
    let mut rows: Vec<(f64, f64, Vec<Word>)> = Vec::new();
    for word in words {
        let height = (word.bottom - word.top).max(1.0);
        match rows.last_mut() {
            Some((top, bottom, row))
                if word.bottom.min(*bottom) - word.top.max(*top)
                    >= height.min(*bottom - *top) / 2.0 =>
            {
                *top = top.min(word.top);
                *bottom = bottom.max(word.bottom);
                row.push(word);
            }
            _ => rows.push((word.top, word.bottom, vec![word])),
        }
    }
    rows.into_iter()
        .map(|(_, _, mut row)| {
            row.sort_by(|a, b| a.x.total_cmp(&b.x));
            row.iter().map(|w| w.text).collect::<Vec<_>>().join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The Vision framework through JavaScript for Automation.
// Drain both pipes while waiting: large OCR results must not fill a pipe and
// deadlock the child. A stalled Vision service must not hang the macro forever.
#[cfg(target_os = "macos")]
fn output_with_timeout(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    use std::io::Read;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()));
    let stderr = drain(Box::new(child.stderr.take().unwrap()));
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e);
            }
            Ok(None) => {}
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "macOS text recognition timed out",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let stdout = stdout
        .join()
        .map_err(|_| std::io::Error::other("OCR output reader panicked"))??;
    let stderr = stderr
        .join()
        .map_err(|_| std::io::Error::other("OCR error reader panicked"))??;
    Ok(std::process::Output {
        status: status?,
        stdout,
        stderr,
    })
}

#[cfg(target_os = "macos")]
fn vision(file: &std::path::Path, mode: VisionMode) -> std::io::Result<std::process::Output> {
    const SCRIPT: &str = r#"
ObjC.import('Vision');
ObjC.import('CoreML');
function run(argv) {
    const url = $.NSURL.fileURLWithPath(argv[0]);
    const handler = $.VNImageRequestHandler.alloc.initWithURLOptions(url, $.NSDictionary.dictionary);
    const request = $.VNRecognizeTextRequest.alloc.init;
    if (argv[1] === 'fast-cpu') request.recognitionLevel = $.VNRequestTextRecognitionLevelFast;
    if (argv[1] !== 'auto') {
        if (request.respondsToSelector($.NSSelectorFromString('setComputeDevice:forComputeStage:'))) {
            const stages = request.supportedComputeStageDevicesAndReturnError(null);
            const keys = stages.allKeys;
            for (let i = 0; i < keys.count; i++) {
                const stage = keys.objectAtIndex(i);
                const devices = stages.objectForKey(stage);
                for (let j = 0; j < devices.count; j++) {
                    const device = devices.objectAtIndex(j);
                    if (device.isKindOfClass($.MLCPUComputeDevice)) {
                        request.setComputeDeviceForComputeStage(device, stage);
                        break;
                    }
                }
            }
        } else {
            request.usesCPUOnly = true;
        }
    }
    if (!handler.performRequestsError($.NSArray.arrayWithObject(request), null)) throw new Error('Vision could not read the image');
    const results = request.results;
    const lines = [];
    for (let i = 0; i < results.count; i++) {
        const candidates = results.objectAtIndex(i).topCandidates(1);
        if (candidates.count > 0) lines.push(candidates.objectAtIndex(0).string.js);
    }
    return lines.join('\n');
}
"#;
    output_with_timeout(
        std::process::Command::new("osascript")
            .args(["-l", "JavaScript", "-e", SCRIPT])
            .arg(file)
            .arg(match mode {
                VisionMode::Auto => "auto",
                VisionMode::Cpu => "cpu",
                VisionMode::FastCpu => "fast-cpu",
            }),
        std::time::Duration::from_secs(if mode == VisionMode::Auto { 5 } else { 15 }),
    )
}

#[cfg(target_os = "macos")]
fn native(png: &[u8]) -> Option<Result<String, String>> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::time::{Duration, Instant};
    static MODE: AtomicU8 = AtomicU8::new(0);
    // A stall can be transient, so retry accurate recognition now and then
    // rather than reading everything with the fast recognizer until restart.
    static DEGRADED_AT: Mutex<Option<Instant>> = Mutex::new(None);
    const RETRY_ACCURATE_AFTER: Duration = Duration::from_secs(10 * 60);
    if let Ok(mut since) = DEGRADED_AT.lock() {
        if since.is_some_and(|at| at.elapsed() >= RETRY_ACCURATE_AFTER) {
            MODE.store(0, Ordering::Relaxed);
            *since = None;
        }
    }
    let file = match temp_png(png) {
        Ok(file) => file,
        Err(e) => return Some(Err(e)),
    };
    let before = MODE.load(Ordering::Relaxed);
    let output = retry_vision(&MODE, |mode| vision(&file.0, mode));
    if MODE.load(Ordering::Relaxed) != before {
        if let Ok(mut since) = DEGRADED_AT.lock() {
            *since = Some(Instant::now());
        }
    }
    Some(match output {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_owned())
        }
        Ok(out) => Err(format!(
            "macOS text recognition failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => Err(format!("couldn't run macOS text recognition: {e}")),
    })
}

// Accurate recognition can stall compiling an ANE model even with a CPU
// device selected. The fast recognizer uses a different path. Retain accurate
// recognition first, then degrade only after a timeout, within a 35s budget.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VisionMode {
    Auto,
    Cpu,
    FastCpu,
}

#[cfg(target_os = "macos")]
fn retry_vision<T>(
    preferred: &std::sync::atomic::AtomicU8,
    mut run: impl FnMut(VisionMode) -> std::io::Result<T>,
) -> std::io::Result<T> {
    use std::sync::atomic::Ordering;
    let start = preferred.load(Ordering::Relaxed).min(2);
    let modes = [VisionMode::Auto, VisionMode::Cpu, VisionMode::FastCpu];
    for index in start..=2 {
        let output = run(modes[index as usize]);
        if index == 2
            || !output
                .as_ref()
                .is_err_and(|e| e.kind() == std::io::ErrorKind::TimedOut)
        {
            return output;
        }
        tracing::warn!(
            "macOS Vision stalled; retrying recognition with {:?}",
            modes[index as usize + 1]
        );
        preferred.store(index + 1, Ordering::Relaxed);
    }
    unreachable!()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn native(_png: &[u8]) -> Option<Result<String, String>> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn vision_uses_fast_recognizer_after_cpu_stall_and_caches_fallback() {
        let preferred = std::sync::atomic::AtomicU8::new(0);
        let mut calls = Vec::new();
        let result = super::retry_vision(&preferred, |mode| {
            calls.push(mode);
            if calls.len() < 3 {
                Err(std::io::ErrorKind::TimedOut.into())
            } else {
                Ok("recovered")
            }
        });
        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(
            calls,
            [
                super::VisionMode::Auto,
                super::VisionMode::Cpu,
                super::VisionMode::FastCpu
            ]
        );
        let result = super::retry_vision(&preferred, |mode| {
            assert_eq!(mode, super::VisionMode::FastCpu);
            Ok(())
        });
        result.unwrap();
        let mut attempts = 0;
        let result: std::io::Result<()> = super::retry_vision(&preferred, |_| {
            attempts += 1;
            Err(std::io::ErrorKind::TimedOut.into())
        });
        assert!(result.is_err());
        assert_eq!(attempts, 1, "fast fallback must not loop indefinitely");
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "recognizes text in the dedicated native fixture using Vision's fast CPU fallback"]
    fn live_vision_cpu_fallback_recognizes_the_fixture() {
        let title = std::env::var("BW_LIVE_WINDOW").expect("set BW_LIVE_WINDOW");
        assert_eq!(title, "Blockwork macOS Test Window");
        let shot =
            crate::screen::capture_window_small(&title, crate::window::MatchBy::Title).unwrap();
        let file = super::temp_png(&shot.image.to_png().unwrap()).unwrap();
        let out = super::vision(&file.0, super::VisionMode::FastCpu).unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("BLOCKWORK MACOS 12345"), "{text}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_tool_timeout_terminates_a_stalled_process() {
        let started = std::time::Instant::now();
        let error = super::output_with_timeout(
            std::process::Command::new("/bin/sleep").arg("10"),
            std::time::Duration::from_millis(100),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_tool_drains_large_results_and_errors_without_deadlock() {
        let output = super::output_with_timeout(
            std::process::Command::new("/bin/sh").args([
                "-c",
                "head -c 131072 /dev/zero; head -c 131072 /dev/zero >&2",
            ]),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 131072);
        assert_eq!(output.stderr.len(), 131072);
    }

    #[test]
    fn empty_and_uniform_images_have_no_text_without_an_ocr_engine() {
        assert_eq!(super::read(&super::Image::new(0, 0)).unwrap(), "");
        let mut image = super::Image::new(10, 10);
        assert_eq!(super::read(&image).unwrap(), "");
        image.rgba.fill(255);
        assert_eq!(super::read(&image).unwrap(), "");
        for (i, pixel) in image.rgba.chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[i as u8, 50, 100, 0]);
        }
        assert_eq!(super::read(&image).unwrap(), "");
    }

    #[test]
    fn rows_from_real_windows_ocr() {
        // Words Windows OCR gave for six lines of JSON in Notepad.
        let words = [
            "67 70 138 20 \"version\"",
            "214 76 5 14 :",
            "67 104 10 7 \"",
            "82 104 124 26 settings",
            "211 104 10 7 \"",
            "229 110 6 14 :",
            "242 71 25 24 1,",
            "99 138 124 20 \"texture",
            "241 138 60 20 max\"",
            "309 144 6 14 :",
            "338 139 73 24 2048,",
            "99 172 92 20 \"audio",
            "192 196 16 2 _",
            "209 172 138 26 quality\":",
            "370 173 25 24 5,",
            "99 206 91 20 \"atlas",
            "209 206 74 20 max\":",
            "306 207 73 24 2048,",
            "99 240 60 20 \"hdr",
            "177 240 74 20 max\":",
            "272 241 63 19 4096",
        ]
        .map(|w| w.replacen(' ', "\t", 4))
        .join("\n");
        assert_eq!(
            super::rows(&words),
            "\"version\" : 1,\n\" settings \" :\n\"texture max\" : 2048,\n\"audio _ quality\": 5,\n\"atlas max\": 2048,\n\"hdr max\": 4096"
        );
    }

    #[test]
    fn words_are_put_back_into_rows() {
        // Windows OCR order: the value of a row can come after the next row.
        let words = "10\t10\t60\t20\t\"version\":\r\n10\t40\t70\t20\t\"settings\":\n90\t12\t20\t18\t1,\n120\t40\t10\t20\t{\nbad line\n";
        assert_eq!(super::rows(words), "\"version\": 1,\n\"settings\": {");
        assert_eq!(super::rows(""), "");
    }
}
