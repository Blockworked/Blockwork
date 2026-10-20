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
        Ok(out) => Err(format!("Windows OCR failed: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("").trim())),
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
            Some(Word { x, top: y, bottom: y + height, text })
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
            Some((top, bottom, row)) if word.bottom.min(*bottom) - word.top.max(*top) >= height.min(*bottom - *top) / 2.0 => {
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
        Ok(out) if out.status.success() => Ok(rows(&String::from_utf8_lossy(&out.stdout))),
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


    #[test]
    fn rows_from_real_windows_ocr() {
        // Words Windows OCR gave for six lines of JSON in Notepad.
        let words = [
            "67 70 138 20 \"version\"", "214 76 5 14 :", "67 104 10 7 \"", "82 104 124 26 settings",
            "211 104 10 7 \"", "229 110 6 14 :", "242 71 25 24 1,", "99 138 124 20 \"texture",
            "241 138 60 20 max\"", "309 144 6 14 :", "338 139 73 24 2048,", "99 172 92 20 \"audio",
            "192 196 16 2 _", "209 172 138 26 quality\":", "370 173 25 24 5,", "99 206 91 20 \"atlas",
            "209 206 74 20 max\":", "306 207 73 24 2048,", "99 240 60 20 \"hdr", "177 240 74 20 max\":",
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
