//! Named secrets kept in the OS keyring (Secret Service on Linux, Credential
//! Manager on Windows, the Keychain on macOS). Macros refer to them by name,
//! so values never land in macro files, and [`redact`] keeps them out of
//! logs and the editor. The names live in `secrets.json`.

use std::collections::HashMap;
use std::sync::Mutex;

/// Keyring service / target prefix.
#[cfg_attr(test, allow(dead_code))]
const SERVICE: &str = "com.blockworked.Blockwork";
/// Shown in place of a secret's value.
pub const MASK: &str = "••••••";
/// Shorter values are too likely to match ordinary text to redact.
const MIN_REDACT_LEN: usize = 4;

/// Values read or written this session, by name, so a loop doesn't hit the
/// keyring (and maybe an unlock prompt) on every read.
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn cache<R>(f: impl FnOnce(&mut HashMap<String, String>) -> R) -> R {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

fn names_path() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    crate::config::config_root().ok().map(|root| root.join("secrets.json"))
}

static TEST_NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Names of the saved secrets, sorted.
pub fn names() -> Vec<String> {
    let mut names: Vec<String> = match names_path() {
        Some(path) => std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default(),
        None => TEST_NAMES.lock().map(|n| n.clone()).unwrap_or_default(),
    };
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn save_names(names: &[String]) -> Result<(), String> {
    match names_path() {
        Some(path) => {
            let text = serde_json::to_string_pretty(names).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| format!("couldn't save the secret names: {e}"))
        }
        None => {
            if let Ok(mut n) = TEST_NAMES.lock() {
                *n = names.to_vec();
            }
            Ok(())
        }
    }
}

fn check_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("the secret needs a name".into());
    }
    if name.chars().count() > 100 {
        return Err("that secret name is too long".into());
    }
    Ok(name.to_string())
}

/// Saves (or replaces) a secret.
pub fn set(name: &str, value: &str) -> Result<(), String> {
    let name = check_name(name)?;
    if value.is_empty() {
        return Err("the secret is empty".into());
    }
    backend::store(&name, value)?;
    cache(|c| c.insert(name.clone(), value.to_string()));
    let mut all = names();
    if !all.contains(&name) {
        all.push(name);
        save_names(&all)?;
    }
    Ok(())
}

/// The value of secret `name`.
pub fn get(name: &str) -> Result<String, String> {
    let name = check_name(name)?;
    if let Some(value) = cache(|c| c.get(&name).cloned()) {
        return Ok(value);
    }
    if !names().contains(&name) {
        return Err(format!("there's no secret named \"{name}\" (add it in Settings > Secrets)"));
    }
    let value = backend::load(&name)?.ok_or_else(|| format!("the keyring has no value for secret \"{name}\""))?;
    cache(|c| c.insert(name, value.clone()));
    Ok(value)
}

pub fn delete(name: &str) -> Result<(), String> {
    let name = check_name(name)?;
    backend::remove(&name)?;
    cache(|c| c.remove(&name));
    let mut all = names();
    all.retain(|n| n != &name);
    save_names(&all)
}

/// `text` with every secret value read or saved this session masked.
pub fn redact(text: &str) -> String {
    let guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(values) = guard.as_ref() else { return text.to_string() };
    let mut out = text.to_string();
    // Longest first, so a secret containing another is masked whole.
    let mut secrets: Vec<&String> = values.values().filter(|v| v.chars().count() >= MIN_REDACT_LEN).collect();
    secrets.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for secret in secrets {
        if out.contains(secret.as_str()) {
            out = out.replace(secret.as_str(), MASK);
        }
    }
    out
}

/// Whether `text` holds a known secret value.
pub fn contains_secret(text: &str) -> bool {
    redact(text) != text
}

#[cfg(test)]
mod backend {
    use std::collections::HashMap;
    use std::sync::Mutex;

    static STORE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

    pub fn store(name: &str, value: &str) -> Result<(), String> {
        STORE.lock().unwrap().get_or_insert_with(HashMap::new).insert(name.into(), value.into());
        Ok(())
    }

    pub fn load(name: &str) -> Result<Option<String>, String> {
        Ok(STORE.lock().unwrap().get_or_insert_with(HashMap::new).get(name).cloned())
    }

    pub fn remove(name: &str) -> Result<(), String> {
        STORE.lock().unwrap().get_or_insert_with(HashMap::new).remove(name);
        Ok(())
    }
}

/// Secret Service (GNOME Keyring, KWallet, KeePassXC) over D-Bus. Values go
/// over the session bus with the "plain" algorithm, like libsecret's default.
#[cfg(all(target_os = "linux", not(test)))]
mod backend {
    use super::SERVICE;
    use std::collections::HashMap;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

    const BUS_NAME: &str = "org.freedesktop.secrets";
    const SERVICE_PATH: &str = "/org/freedesktop/secrets";
    const SERVICE_IFACE: &str = "org.freedesktop.Secret.Service";
    const ITEM_IFACE: &str = "org.freedesktop.Secret.Item";
    const COLLECTION_IFACE: &str = "org.freedesktop.Secret.Collection";
    const PROMPT_IFACE: &str = "org.freedesktop.Secret.Prompt";

    type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);

    struct Session {
        connection: Connection,
        path: OwnedObjectPath,
    }

    fn fail(what: &str) -> impl Fn(zbus::Error) -> String + '_ {
        move |e| format!("keyring: couldn't {what} ({e})")
    }

    fn proxy<'a>(connection: &Connection, path: &'a str, iface: &'a str) -> Result<Proxy<'a>, String> {
        Proxy::new(connection, BUS_NAME, path, iface).map_err(fail("reach the Secret Service"))
    }

    fn open() -> Result<Session, String> {
        let connection = Connection::session().map_err(|e| format!("keyring: no session bus ({e})"))?;
        let service = proxy(&connection, SERVICE_PATH, SERVICE_IFACE)?;
        let (_, path): (OwnedValue, OwnedObjectPath) = service.call("OpenSession", &("plain", Value::from(""))).map_err(|e| {
            format!("keyring: no Secret Service is running ({e}). Install GNOME Keyring, KWallet or KeePassXC")
        })?;
        Ok(Session { connection, path })
    }

    fn attributes(name: &str) -> HashMap<&str, &str> {
        HashMap::from([("application", SERVICE), ("blockwork-secret", name)])
    }

    /// Runs a prompt (an unlock dialog, say) and waits for the user.
    fn prompt(session: &Session, path: &OwnedObjectPath) -> Result<(), String> {
        if path.as_str() == "/" {
            return Ok(());
        }
        let prompt = proxy(&session.connection, path.as_str(), PROMPT_IFACE)?;
        let mut completed = prompt.receive_signal("Completed").map_err(fail("show the keyring prompt"))?;
        prompt.call_method("Prompt", &("",)).map_err(fail("show the keyring prompt"))?;
        let message = completed.next().ok_or("keyring: the prompt went away")?;
        let (dismissed, _): (bool, OwnedValue) = message.body().deserialize().map_err(fail("read the prompt result"))?;
        if dismissed {
            return Err("keyring: the unlock prompt was dismissed".into());
        }
        Ok(())
    }

    fn unlock(session: &Session, paths: Vec<OwnedObjectPath>) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        let service = proxy(&session.connection, SERVICE_PATH, SERVICE_IFACE)?;
        let (_, prompt_path): (Vec<OwnedObjectPath>, OwnedObjectPath) =
            service.call("Unlock", &(paths,)).map_err(fail("unlock the keyring"))?;
        prompt(session, &prompt_path)
    }

    /// Unlocked items holding secret `name`.
    fn find(session: &Session, name: &str) -> Result<Vec<OwnedObjectPath>, String> {
        let service = proxy(&session.connection, SERVICE_PATH, SERVICE_IFACE)?;
        let (mut unlocked, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) =
            service.call("SearchItems", &(attributes(name),)).map_err(fail("search the keyring"))?;
        if !locked.is_empty() {
            unlock(session, locked.clone())?;
            unlocked.extend(locked);
        }
        Ok(unlocked)
    }

    /// The keyring to save into: the default one, else the login one, else
    /// any, else a new one (the service asks for its password).
    fn collection(session: &Session, service: &Proxy) -> Result<OwnedObjectPath, String> {
        for alias in ["default", "login"] {
            let path: OwnedObjectPath = service.call("ReadAlias", &(alias,)).map_err(fail("find the keyring"))?;
            if path.as_str() != "/" {
                return Ok(path);
            }
        }
        let all: Vec<OwnedObjectPath> = service.get_property("Collections").unwrap_or_default();
        if let Some(first) = all.into_iter().find(|p| !p.as_str().ends_with("/session")) {
            return Ok(first);
        }
        let properties: HashMap<&str, Value> = HashMap::from([("org.freedesktop.Secret.Collection.Label", Value::from("Blockwork"))]);
        let (created, prompt_path): (OwnedObjectPath, OwnedObjectPath) =
            service.call("CreateCollection", &(properties, "default")).map_err(fail("create a keyring"))?;
        if created.as_str() != "/" {
            return Ok(created);
        }
        prompt(session, &prompt_path)?;
        let path: OwnedObjectPath = service.call("ReadAlias", &("default",)).map_err(fail("find the keyring"))?;
        if path.as_str() == "/" {
            return Err("keyring: there's no keyring to save into".into());
        }
        Ok(path)
    }

    /// zbus's blocking API panics on a thread inside a tokio runtime (the
    /// daemon's tray turns on zbus's tokio support), so calls run on a plain thread.
    fn off_runtime<T: Send>(f: impl FnOnce() -> Result<T, String> + Send) -> Result<T, String> {
        std::thread::scope(|scope| scope.spawn(f).join().unwrap_or_else(|_| Err("keyring: the request crashed".into())))
    }

    pub fn store(name: &str, value: &str) -> Result<(), String> {
        off_runtime(|| store_now(name, value))
    }

    pub fn load(name: &str) -> Result<Option<String>, String> {
        off_runtime(|| load_now(name))
    }

    pub fn remove(name: &str) -> Result<(), String> {
        off_runtime(|| remove_now(name))
    }

    fn store_now(name: &str, value: &str) -> Result<(), String> {
        let session = open()?;
        let service = proxy(&session.connection, SERVICE_PATH, SERVICE_IFACE)?;
        let collection = collection(&session, &service)?;
        unlock(&session, vec![collection.clone()])?;
        let label = format!("Blockwork secret: {name}");
        let properties: HashMap<&str, Value> = HashMap::from([
            ("org.freedesktop.Secret.Item.Label", Value::from(label.as_str())),
            ("org.freedesktop.Secret.Item.Attributes", Value::from(attributes(name))),
        ]);
        let secret: Secret = (session.path.clone(), Vec::new(), value.as_bytes().to_vec(), "text/plain".into());
        let collection_proxy = proxy(&session.connection, collection.as_str(), COLLECTION_IFACE)?;
        let (_, prompt_path): (OwnedObjectPath, OwnedObjectPath) =
            collection_proxy.call("CreateItem", &(properties, secret, true)).map_err(fail("save the secret"))?;
        prompt(&session, &prompt_path)
    }

    fn load_now(name: &str) -> Result<Option<String>, String> {
        let session = open()?;
        let Some(item) = find(&session, name)?.into_iter().next() else { return Ok(None) };
        let item = proxy(&session.connection, item.as_str(), ITEM_IFACE)?;
        let (_, _, value, _): Secret = item.call("GetSecret", &(&session.path,)).map_err(fail("read the secret"))?;
        String::from_utf8(value).map(Some).map_err(|_| "keyring: the secret isn't text".into())
    }

    fn remove_now(name: &str) -> Result<(), String> {
        let session = open()?;
        for item in find(&session, name)? {
            let item = proxy(&session.connection, item.as_str(), ITEM_IFACE)?;
            let prompt_path: OwnedObjectPath = item.call("Delete", &()).map_err(fail("delete the secret"))?;
            prompt(&session, &prompt_path)?;
        }
        Ok(())
    }
}

/// Windows Credential Manager, as generic credentials named `<SERVICE>/<name>`.
#[cfg(all(windows, not(test)))]
mod backend {
    use super::SERVICE;
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
    };

    fn target(name: &str) -> Vec<u16> {
        format!("{SERVICE}/{name}").encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn store(name: &str, value: &str) -> Result<(), String> {
        let mut target = target(name);
        let mut user: Vec<u16> = "Blockwork".encode_utf16().chain(std::iter::once(0)).collect();
        let mut blob = value.as_bytes().to_vec();
        // SAFETY: every pointer refers to a live buffer for the duration of the call.
        let ok = unsafe {
            let mut credential: CREDENTIALW = std::mem::zeroed();
            credential.Type = CRED_TYPE_GENERIC;
            credential.TargetName = target.as_mut_ptr();
            credential.UserName = user.as_mut_ptr();
            credential.CredentialBlobSize = blob.len() as u32;
            credential.CredentialBlob = blob.as_mut_ptr();
            credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
            CredWriteW(&credential, 0)
        };
        if ok == 0 {
            return Err(format!("Credential Manager: couldn't save the secret (error {})", unsafe { GetLastError() }));
        }
        Ok(())
    }

    pub fn load(name: &str) -> Result<Option<String>, String> {
        let target = target(name);
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: CredReadW fills `credential`, which CredFree releases.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) == 0 {
                let error = GetLastError();
                return if error == ERROR_NOT_FOUND { Ok(None) } else { Err(format!("Credential Manager: couldn't read the secret (error {error})")) };
            }
            let blob = std::slice::from_raw_parts((*credential).CredentialBlob, (*credential).CredentialBlobSize as usize).to_vec();
            CredFree(credential as *const _);
            String::from_utf8(blob).map(Some).map_err(|_| "Credential Manager: the secret isn't text".into())
        }
    }

    pub fn remove(name: &str) -> Result<(), String> {
        let target = target(name);
        // SAFETY: `target` is a NUL-terminated UTF-16 string.
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_NOT_FOUND {
                return Err(format!("Credential Manager: couldn't delete the secret (error {error})"));
            }
        }
        Ok(())
    }
}

/// The login Keychain, as generic passwords under `SERVICE`.
#[cfg(all(target_os = "macos", not(test)))]
mod backend {
    use super::SERVICE;
    use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

    // errSecItemNotFound
    const NOT_FOUND: i32 = -25300;

    pub fn store(name: &str, value: &str) -> Result<(), String> {
        set_generic_password(SERVICE, name, value.as_bytes()).map_err(|e| format!("Keychain: couldn't save the secret ({e})"))
    }

    pub fn load(name: &str) -> Result<Option<String>, String> {
        match get_generic_password(SERVICE, name) {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| "Keychain: the secret isn't text".into()),
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) => Err(format!("Keychain: couldn't read the secret ({e})")),
        }
    }

    pub fn remove(name: &str) -> Result<(), String> {
        match delete_generic_password(SERVICE, name) {
            Err(e) if e.code() != NOT_FOUND => Err(format!("Keychain: couldn't delete the secret ({e})")),
            _ => Ok(()),
        }
    }
}

#[cfg(not(any(test, target_os = "linux", windows, target_os = "macos")))]
mod backend {
    pub fn store(_name: &str, _value: &str) -> Result<(), String> {
        Err("secrets aren't supported on this platform".into())
    }

    pub fn load(_name: &str) -> Result<Option<String>, String> {
        Err("secrets aren't supported on this platform".into())
    }

    pub fn remove(_name: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_are_redacted() {
        assert!(set(" ", "x").is_err());
        assert!(get("missing-secret").unwrap_err().contains("no secret named"));
        set("api token", "tok-12345").unwrap();
        assert!(names().contains(&"api token".to_string()));
        assert_eq!(get("api token").unwrap(), "tok-12345");
        assert_eq!(redact("Authorization: tok-12345!"), format!("Authorization: {MASK}!"));
        assert!(contains_secret("x tok-12345"));
        assert!(!contains_secret("nothing here"));
        delete("api token").unwrap();
        assert!(!names().contains(&"api token".to_string()));
        assert!(get("api token").is_err());
    }
}
