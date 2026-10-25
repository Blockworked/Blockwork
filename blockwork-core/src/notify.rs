//! Desktop notifications for the `Notify` block.

#[cfg(target_os = "linux")]
pub fn show(title: &str, message: &str) -> Result<(), String> {
    use std::collections::HashMap;
    let connection =
        zbus::blocking::Connection::session().map_err(|e| format!("no session bus: {e}"))?;
    let hints: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
    connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Blockwork",
                0u32,
                "com.blockworked.Blockwork",
                title,
                message,
                Vec::<&str>::new(),
                hints,
                -1i32,
            ),
        )
        .map(|_| ())
        .map_err(|e| format!("couldn't show notification: {e}"))
}

#[cfg(target_os = "macos")]
pub fn show(title: &str, message: &str) -> Result<(), String> {
    // AppleScript string literals: escape backslashes and quotes.
    let quote = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        quote(message),
        quote(title)
    );
    let out = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("couldn't show notification: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "couldn't show notification: {}",
            err.lines().next().unwrap_or("osascript failed").trim()
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub fn show(title: &str, message: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // XML-escaping also turns `'` into `&apos;`, so the PowerShell
    // single-quoted string below can't be broken out of.
    let xml = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    };
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
         [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null; \
         $x = New-Object Windows.Data.Xml.Dom.XmlDocument; \
         $x.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>'); \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe').Show((New-Object Windows.UI.Notifications.ToastNotification $x))",
        xml(title),
        xml(message)
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("couldn't show notification: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "couldn't show notification: {}",
            err.lines().next().unwrap_or("PowerShell failed").trim()
        ));
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn show(_title: &str, _message: &str) -> Result<(), String> {
    Err("notifications aren't supported on this platform".into())
}
