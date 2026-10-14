//! Lets a block start, stop or check another macro. Where macros live and how
//! a run starts is the host app's business, so it installs a [`MacroHost`].

use super::run_registry;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

pub trait MacroHost: Send + Sync {
    /// The id of the macro named `name` (or whose id is `name`).
    fn find(&self, name: &str) -> Option<String>;
    /// Starts a run of macro `id`; the flag is set once that run ends.
    fn start(&self, id: &str) -> Result<Arc<AtomicBool>, String>;
}

static HOST: RwLock<Option<Arc<dyn MacroHost>>> = RwLock::new(None);

/// Runs in flight past which `Run macro` refuses, so a macro that runs
/// itself can't spawn threads without end.
const MAX_RUNS: usize = 64;

pub fn set_host(host: Arc<dyn MacroHost>) {
    if let Ok(mut slot) = HOST.write() {
        *slot = Some(host);
    }
}

fn host() -> Result<Arc<dyn MacroHost>, String> {
    HOST.read()
        .ok()
        .and_then(|h| h.clone())
        .ok_or_else(|| "running other macros isn't available here".to_string())
}

/// The id of the macro called `name`.
pub fn find(name: &str) -> Result<String, String> {
    let name = name.trim();
    host()?.find(name).ok_or_else(|| format!("no macro named \"{name}\""))
}

/// Starts the macro called `name`. Returns its id and a flag set when it ends.
pub fn start(name: &str) -> Result<(String, Arc<AtomicBool>), String> {
    let id = find(name)?;
    if run_registry::count() >= MAX_RUNS {
        return Err(format!("{MAX_RUNS} runs are already going; is a macro starting itself?"));
    }
    let done = host()?.start(&id)?;
    Ok((id, done))
}

/// Stops every run of the macro called `name`, returning how many.
pub fn stop(name: &str) -> Result<usize, String> {
    Ok(run_registry::stop_macro(&find(name)?))
}

pub fn is_running(name: &str) -> Result<bool, String> {
    Ok(run_registry::is_running(&find(name)?))
}
