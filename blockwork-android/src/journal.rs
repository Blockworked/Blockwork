//! A run boundary is persisted before execution can send its first action.
//! Process restart records interruption and restores documents, never actions.
use std::path::PathBuf;
use std::sync::Arc;

pub fn install(root: &std::path::Path) -> Result<(), String> {
    let path = root.join("active-runs.json");
    if path.exists() {
        let ids: Vec<String> =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("could not read interrupted-run journal: {e}"))?;
        for id in ids {
            let name = blockwork_core::config::read_macro_by_id(&id)
                .map(|m| m.name)
                .unwrap_or_else(|| id.clone());
            blockwork_core::run_log::error(
                &blockwork_core::run_log::Source::for_macro(&id, &name),
                "Run interrupted by process exit. Partially completed actions were not replayed.",
            );
        }
    }
    write(&path, &[])?;
    blockwork_core::macros::run_registry::set_observer(Arc::new(move |ids| {
        if let Err(error) = write(&path, ids) {
            eprintln!("Blockwork run journal: {error}");
        }
    }));
    Ok(())
}

fn write(path: &PathBuf, ids: &[String]) -> Result<(), String> {
    use std::io::Write;
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(ids).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}
