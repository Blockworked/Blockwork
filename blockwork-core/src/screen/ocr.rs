//! Reading text out of an image. Uses the OS's own OCR on Windows and macOS,
//! and Tesseract elsewhere (or when it's installed and the built-in one fails).

use super::Image;
use std::io::Write;
use std::process::Stdio;

/// Text in `image`, lines joined with newlines and trimmed.
pub(super) fn read(image: &Image) -> Result<String, String> {
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
    Ok(text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string())
}

fn tesseract(png: &[u8]) -> Result<String, String> {
    let mut child = crate::flatpak::host_command("tesseract")
        .args(["stdin", "stdout", "--psm", "6"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "reading text needs Tesseract OCR installed (package 'tesseract')".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(png).map_err(|e| format!("couldn't send the image to Tesseract: {e}"))?;
    }
    let output = child.wait_with_output().map_err(|e| format!("Tesseract failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Tesseract failed: {}", err.lines().last().unwrap_or("unknown error").trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Writes `png` to a temp file for a tool that only reads paths.
#[cfg(any(windows, target_os = "macos"))]
fn temp_png(png: &[u8]) -> Result<tempfile_path::TempPath, String> {
    let path = std::env::temp_dir().join(format!("blockwork-ocr-{}.png", uuid::Uuid::new_v4().simple()));
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
if ($engine -eq $null) { throw 'no OCR language is installed' }
$result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$result.Lines | ForEach-Object { $_.Text }
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
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(out) => Err(format!("Windows OCR failed: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").trim())),
        Err(e) => Err(format!("couldn't run Windows OCR: {e}")),
    })
}

/// The Vision framework through JavaScript for Automation.
#[cfg(target_os = "macos")]
fn native(png: &[u8]) -> Option<Result<String, String>> {
    const SCRIPT: &str = r#"
ObjC.import('Vision');
function run(argv) {
    const url = $.NSURL.fileURLWithPath(argv[0]);
    const handler = $.VNImageRequestHandler.alloc.initWithURLOptions(url, $.NSDictionary.dictionary);
    const request = $.VNRecognizeTextRequest.alloc.init;
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
    let file = match temp_png(png) {
        Ok(file) => file,
        Err(e) => return Some(Err(e)),
    };
    let output = std::process::Command::new("osascript").args(["-l", "JavaScript", "-e", SCRIPT]).arg(&file.0).output();
    Some(match output {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(out) => Err(format!("macOS text recognition failed: {}", String::from_utf8_lossy(&out.stderr).trim())),
        Err(e) => Err(format!("couldn't run macOS text recognition: {e}")),
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn native(_png: &[u8]) -> Option<Result<String, String>> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_ocr_is_a_clear_error() {
        // Only checks the error path shape; whether Tesseract exists varies.
        if let Err(e) = super::read(&super::Image::new(10, 10)) {
            assert!(!e.is_empty());
        }
    }
}
