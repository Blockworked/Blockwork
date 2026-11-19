//! The sound output: volume, mute and playing sound files, for the volume
//! blocks and reporters and the Play sound block.

use blockwork_model::macros::MuteMode;

/// The output volume, from 0 to 100.
pub fn volume() -> Result<f64, String> {
    imp::volume().map(|v| (v * 100.0).round())
}

/// Sets the output volume, from 0 to 100.
pub fn set_volume(percent: f64) -> Result<(), String> {
    if !percent.is_finite() {
        return Err(format!("{percent} isn't a volume"));
    }
    imp::set_volume(percent.clamp(0.0, 100.0) / 100.0)
}

pub fn muted() -> Result<bool, String> {
    imp::muted()
}

pub fn set_muted(mode: MuteMode) -> Result<(), String> {
    let mute = match mode {
        MuteMode::Mute => true,
        MuteMode::Unmute => false,
        MuteMode::Toggle => !imp::muted()?,
    };
    imp::set_muted(mute)
}

/// Plays a sound file; with `wait`, returns once it has ended.
pub fn play(path: &str, wait: bool) -> Result<(), String> {
    let full = crate::files::expand(path);
    if path.trim().is_empty() {
        return Err("no sound file given".into());
    }
    if !full.is_file() {
        return Err(format!("{} isn't a file", full.display()));
    }
    if wait {
        return imp::play(&full);
    }
    std::thread::Builder::new()
        .name("play-sound".into())
        .spawn(move || {
            if let Err(e) = imp::play(&full) {
                tracing::warn!("Play sound: {e}");
            }
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `wpctl get-volume` prints `Volume: 0.40` and ` [MUTED]` when muted.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_wpctl(text: &str) -> Option<(f64, bool)> {
    let level = text
        .trim()
        .strip_prefix("Volume:")?
        .split_whitespace()
        .next()?;
    Some((level.parse().ok()?, text.contains("[MUTED]")))
}

/// `pactl get-sink-volume` prints each channel as `65536 / 100% / 0.00 dB`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_pactl_volume(text: &str) -> Option<f64> {
    let percent = text.split('/').nth(1)?.trim().strip_suffix('%')?;
    Some(percent.trim().parse::<f64>().ok()? / 100.0)
}

#[cfg(target_os = "linux")]
mod imp {
    use std::path::Path;

    const SINK: &str = "@DEFAULT_AUDIO_SINK@";
    const PULSE_SINK: &str = "@DEFAULT_SINK@";

    fn run(program: &str, args: &[&str]) -> Option<String> {
        let out = crate::flatpak::host_command(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn no_mixer() -> String {
        "couldn't reach the sound server (needs wpctl or pactl)".into()
    }

    fn state() -> Result<(f64, bool), String> {
        if let Some(state) =
            run("wpctl", &["get-volume", SINK]).and_then(|t| super::parse_wpctl(&t))
        {
            return Ok(state);
        }
        let volume = run("pactl", &["get-sink-volume", PULSE_SINK])
            .and_then(|t| super::parse_pactl_volume(&t))
            .ok_or_else(no_mixer)?;
        let muted = run("pactl", &["get-sink-mute", PULSE_SINK]).is_some_and(|t| t.contains("yes"));
        Ok((volume, muted))
    }

    pub fn volume() -> Result<f64, String> {
        state().map(|(v, _)| v)
    }

    pub fn muted() -> Result<bool, String> {
        state().map(|(_, m)| m)
    }

    pub fn set_volume(level: f64) -> Result<(), String> {
        let wp = format!("{level:.2}");
        let pulse = format!("{}%", (level * 100.0).round());
        run("wpctl", &["set-volume", SINK, &wp])
            .or_else(|| run("pactl", &["set-sink-volume", PULSE_SINK, &pulse]))
            .map(|_| ())
            .ok_or_else(no_mixer)
    }

    pub fn set_muted(mute: bool) -> Result<(), String> {
        let flag = if mute { "1" } else { "0" };
        run("wpctl", &["set-mute", SINK, flag])
            .or_else(|| run("pactl", &["set-sink-mute", PULSE_SINK, flag]))
            .map(|_| ())
            .ok_or_else(no_mixer)
    }

    pub fn play(path: &Path) -> Result<(), String> {
        let path = path.to_string_lossy();
        for player in ["pw-play", "paplay", "aplay"] {
            if let Ok(status) = crate::flatpak::host_command(player)
                .arg(path.as_ref())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("{player} couldn't play {path}"))
                };
            }
        }
        Err("no sound player found (needs pw-play, paplay or aplay)".into())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::path::Path;

    fn script(source: &str) -> Result<String, String> {
        let out = std::process::Command::new("osascript")
            .args(["-e", source])
            .output()
            .map_err(|e| format!("couldn't run osascript: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn volume() -> Result<f64, String> {
        let text = script("output volume of (get volume settings)")?;
        text.parse::<f64>()
            .map(|v| v / 100.0)
            .map_err(|_| format!("unexpected volume '{text}'"))
    }

    pub fn muted() -> Result<bool, String> {
        Ok(script("output muted of (get volume settings)")? == "true")
    }

    pub fn set_volume(level: f64) -> Result<(), String> {
        script(&format!(
            "set volume output volume {}",
            (level * 100.0).round()
        ))
        .map(|_| ())
    }

    pub fn set_muted(mute: bool) -> Result<(), String> {
        script(&format!("set volume output muted {mute}")).map(|_| ())
    }

    pub fn play(path: &Path) -> Result<(), String> {
        let status = std::process::Command::new("afplay")
            .arg(path)
            .status()
            .map_err(|e| format!("couldn't run afplay: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("couldn't play {}", path.display()))
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, PlaySoundW, SND_FILENAME, SND_NODEFAULT, SND_SYNC,
        eConsole, eRender,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::core::PCWSTR;

    fn endpoint() -> Result<IAudioEndpointVolume, String> {
        let fail = |e: windows::core::Error| format!("couldn't reach the speakers: {e}");
        // SAFETY: COM calls on this thread, which is set up for COM first;
        // the interfaces release themselves when dropped.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let devices: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(fail)?;
            let device = devices
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(fail)?;
            device
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                .map_err(fail)
        }
    }

    pub fn volume() -> Result<f64, String> {
        // SAFETY: see `endpoint`.
        unsafe { endpoint()?.GetMasterVolumeLevelScalar() }
            .map(f64::from)
            .map_err(|e| e.to_string())
    }

    pub fn muted() -> Result<bool, String> {
        // SAFETY: see `endpoint`.
        unsafe { endpoint()?.GetMute() }
            .map(|m| m.as_bool())
            .map_err(|e| e.to_string())
    }

    pub fn set_volume(level: f64) -> Result<(), String> {
        // SAFETY: see `endpoint`; no event context is passed.
        unsafe { endpoint()?.SetMasterVolumeLevelScalar(level as f32, std::ptr::null()) }
            .map_err(|e| e.to_string())
    }

    pub fn set_muted(mute: bool) -> Result<(), String> {
        // SAFETY: see `endpoint`; no event context is passed.
        unsafe { endpoint()?.SetMute(mute, std::ptr::null()) }.map_err(|e| e.to_string())
    }

    /// Plays a .wav file to the end.
    pub fn play(path: &Path) -> Result<(), String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: `wide` is a nul-terminated path that outlives the call.
        let played = unsafe {
            PlaySoundW(
                PCWSTR(wide.as_ptr()),
                None,
                SND_FILENAME | SND_SYNC | SND_NODEFAULT,
            )
        };
        if played.as_bool() {
            Ok(())
        } else {
            Err(format!(
                "couldn't play {} (use a .wav file)",
                path.display()
            ))
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod imp {
    const MSG: &str = "sound isn't supported on this system";
    pub fn volume() -> Result<f64, String> {
        Err(MSG.into())
    }
    pub fn muted() -> Result<bool, String> {
        Err(MSG.into())
    }
    pub fn set_volume(_: f64) -> Result<(), String> {
        Err(MSG.into())
    }
    pub fn set_muted(_: bool) -> Result<(), String> {
        Err(MSG.into())
    }
    pub fn play(_: &std::path::Path) -> Result<(), String> {
        Err(MSG.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_output_parses() {
        assert_eq!(parse_wpctl("Volume: 0.40\n"), Some((0.40, false)));
        assert_eq!(parse_wpctl("Volume: 1.00 [MUTED]\n"), Some((1.0, true)));
        assert_eq!(parse_wpctl("nonsense"), None);
        assert_eq!(
            parse_pactl_volume(
                "Volume: front-left: 26214 /  40% / -23.88 dB,   front-right: 26214 /  40% / -23.88 dB"
            ),
            Some(0.40)
        );
        assert_eq!(parse_pactl_volume(""), None);
    }
}
