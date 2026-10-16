//! KWin (KDE Plasma) over D-Bus: a shared session connection, plus short
//! KWin scripts for window state kdotool doesn't expose.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use zbus::blocking::Connection;

const PATH: &str = "/com/blockworked/Blockwork/KWin";
const INTERFACE: &str = "com.blockworked.Blockwork.KWin";

/// Where scripts send their answer, via `callDBus`.
struct Listener(Sender<(String, String)>);

#[zbus::interface(name = "com.blockworked.Blockwork.KWin")]
impl Listener {
    fn reply(&self, token: String, text: String) {
        let _ = self.0.send((token, text));
    }
}

struct Bus {
    connection: Connection,
    replies: Mutex<Receiver<(String, String)>>,
}

fn bus() -> Result<&'static Bus, String> {
    static BUS: OnceLock<Result<Bus, String>> = OnceLock::new();
    BUS.get_or_init(|| {
        let connection = Connection::session().map_err(|e| format!("no session bus: {e}"))?;
        let (tx, rx) = channel();
        connection.object_server().at(PATH, Listener(tx)).map_err(|e| e.to_string())?;
        Ok(Bus { connection, replies: Mutex::new(rx) })
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// The session bus, shared so each call skips the connection handshake.
pub fn connection() -> Result<&'static Connection, String> {
    bus().map(|b| &b.connection)
}

/// Set once a script went unanswered (not KWin, or an older one), so later
/// calls fail fast instead of waiting out the timeout each time.
static NO_SCRIPTS: AtomicBool = AtomicBool::new(false);

/// Runs a KWin script and returns the text it passes to `reply(text)`.
pub fn script(body: &str) -> Result<String, String> {
    if NO_SCRIPTS.load(Ordering::Relaxed) {
        return Err("KWin scripts aren't available".into());
    }
    let bus = bus()?;
    // One script at a time, so a late answer can't be read as the next one's.
    let replies = bus.replies.lock().unwrap_or_else(|e| e.into_inner());
    while replies.try_recv().is_ok() {}
    let token = uuid::Uuid::new_v4().simple().to_string();
    let sender = bus.connection.unique_name().ok_or("no D-Bus name")?.to_string();
    let source = format!(
        "function reply(text) {{ callDBus({}, {}, {}, \"Reply\", {}, String(text)); }}\n{body}\n",
        js_string(&sender),
        js_string(PATH),
        js_string(INTERFACE),
        js_string(&token)
    );
    // Under the user's cache dir, so KWin can read it from a Flatpak too.
    let dir = dirs::cache_dir().ok_or("no cache directory")?.join("blockwork");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("kwin-{token}.js"));
    std::fs::write(&file, source).map_err(|e| e.to_string())?;
    let plugin = format!("blockwork-{token}");
    let result = run(bus, &file, &plugin, &token, &replies);
    let _ = bus.connection.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "unloadScript",
        &(plugin.as_str(),),
    );
    let _ = std::fs::remove_file(&file);
    result
}

fn run(bus: &Bus, file: &std::path::Path, plugin: &str, token: &str, replies: &Receiver<(String, String)>) -> Result<String, String> {
    let failed = |e: zbus::Error| {
        NO_SCRIPTS.store(true, Ordering::Relaxed);
        format!("couldn't run a KWin script: {e}")
    };
    let id: i32 = bus
        .connection
        .call_method(
            Some("org.kde.KWin"),
            "/Scripting",
            Some("org.kde.kwin.Scripting"),
            "loadScript",
            &(file.to_string_lossy().as_ref(), plugin),
        )
        .map_err(failed)?
        .body()
        .deserialize()
        .map_err(failed)?;
    if id < 0 {
        return Err("KWin wouldn't load the script".into());
    }
    bus.connection
        .call_method(Some("org.kde.KWin"), format!("/Scripting/Script{id}").as_str(), Some("org.kde.kwin.Script"), "run", &())
        .map_err(failed)?;
    loop {
        match replies.recv_timeout(Duration::from_secs(1)) {
            Ok((t, text)) if t == token => return Ok(text),
            Ok(_) => continue,
            Err(_) => {
                NO_SCRIPTS.store(true, Ordering::Relaxed);
                return Err("KWin didn't answer the script".into());
            }
        }
    }
}

/// A JS string literal (JSON's is valid JS).
pub fn js_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}
