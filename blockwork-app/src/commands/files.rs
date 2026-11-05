//! Saving, exporting and importing macros, and the capabilities a macro asks for.

use super::*;

pub(crate) fn save_macro(
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if let Some(mac) = &s.current_macro {
        if let Err(e) = mac.save() {
            return Err(format!("Failed to save macro: {e}"));
        }
    }
    refresh_macro_list(&mut s);
    emit_state_updated(&app, &s);
    Ok(())
}

/// Strips characters illegal in filenames on common platforms, so a macro's
/// freeform name can double as a sane default export filename.
pub(super) fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "macro".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Default file name offered by the UI's save dialog when exporting a macro.
pub(crate) fn export_macro_file_name(macro_id: String) -> Result<String, String> {
    let mac = config::get_macros_from_config()
        .into_iter()
        .find(|m| m.id == macro_id)
        .ok_or("Macro not found")?;
    Ok(format!("{}.macro", sanitize_filename(&mac.name)))
}

/// Exports a single macro to a `.macro` file the user picked in the UI (same
/// JSON shape as the app's own on-disk storage, just under a different
/// extension so it reads as a portable macro file rather than an app-internal
/// one).
pub(crate) fn export_macro(macro_id: String, path: String) -> Result<(), String> {
    let macros = config::get_macros_from_config();
    let mac = macros
        .into_iter()
        .find(|m| m.id == macro_id)
        .ok_or("Macro not found")?;
    config::write_macro_file(std::path::Path::new(&path), &mac)
}

/// What an imported macro can do, shown before it's added: (kind, label,
/// whether it deserves a warning). Listed in display order.
pub(super) const CAPABILITIES: [(&str, &str, bool); 13] = [
    ("commands", "Runs commands and opens or closes apps", true),
    ("secrets", "Reads your saved secrets", true),
    ("network", "Goes online", true),
    ("browser", "Controls your web browser (clicks, types and reads pages)", true),
    ("files_write", "Writes to files", true),
    ("files_read", "Reads files and folders", false),
    ("input", "Presses keys, clicks and moves the mouse", false),
    ("clipboard", "Reads or changes the clipboard", false),
    ("windows", "Reads or controls other apps and windows", false),
    ("triggers", "Starts on its own (schedules, keys, typing, apps, idle, windows, files and more)", false),
    ("screen", "Reads the screen (colors, images and text)", false),
    ("macros", "Runs or stops your other macros", false),
    ("globals", "Reads or changes global variables", false),
];

/// The capability a reporter needs, by operator name.
pub(super) fn operator_capability(op: &str) -> Option<&'static str> {
    Some(match op {
        "ShellOutput" | "ShellSucceeded" => "commands",
        "HttpGet" => "network",
        "ReadFile" | "FileExists" | "ListDirectory" => "files_read",
        "ClipboardText" | "ClipboardHasImage" | "ClipboardHasFiles" => "clipboard",
        "ActiveWindowTitle" | "ActiveWindowApp" | "WindowExists" | "WindowInfo" | "AppRunning" => "windows",
        "MacroRunning" => "macros",
        "PixelColor" | "AverageColor" | "ScreenText" => "screen",
        "GlobalVariable" => "globals",
        "Secret" => "secrets",
        "PageUrl" | "PageTitle" | "ElementText" | "ElementExists" | "ElementCount" => "browser",
        _ => return None,
    })
}

pub(super) fn value_capabilities(value: &Value, found: &mut std::collections::HashSet<&'static str>) {
    match value {
        Value::Op { op, args, .. } => {
            found.extend(operator_capability(op.name()));
            args.iter().for_each(|a| value_capabilities(a, found));
        }
        Value::Call { args, .. } => args.iter().for_each(|a| value_capabilities(a, found)),
        _ => {}
    }
}

/// Everything `mac` can do anywhere, nested blocks included, for the import
/// review. `OpenApp`'s command can be any shell string in a hand-edited
/// file, so it counts as running commands.
/// Capabilities named by kind (as a library index lists them), in the usual order.
pub(crate) fn capabilities_named(kinds: &[String]) -> Vec<crate::state::CapabilityDto> {
    CAPABILITIES
        .iter()
        .filter(|(kind, ..)| kinds.iter().any(|k| k == kind))
        .map(|(kind, label, risky)| crate::state::CapabilityDto {
            kind: kind.to_string(),
            label: label.to_string(),
            risky: *risky,
            guarded: blockwork_core::permissions::GUARDED.iter().any(|(k, _)| k == kind),
        })
        .collect()
}

/// Whether `mac` has any Browser block, reporters included, which keeps the
/// Browser palette category shown before a browser has ever paired.
pub(crate) fn uses_browser(mac: &Macro) -> bool {
    use blockwork_core::macros::BlockKind;
    fn in_value(value: &Value) -> bool {
        match value {
            Value::Op { op, args, .. } => {
                matches!(op.name(), "BrowserConnected" | "PageUrl" | "PageTitle" | "ElementText" | "ElementExists" | "ElementCount")
                    || args.iter().any(in_value)
            }
            Value::Call { args, .. } => args.iter().any(in_value),
            _ => false,
        }
    }
    let mut found = false;
    mac.graph.walk_instructions(&mut |ins| {
        if found {
            return;
        }
        found = matches!(
            ins.kind,
            InstructionKind::BrowserOpen { .. }
                | InstructionKind::BrowserClick { .. }
                | InstructionKind::BrowserType { .. }
                | InstructionKind::BrowserWaitFor { .. }
                | InstructionKind::BrowserScript { .. }
        );
        if !found {
            let mut kind = ins.kind.clone();
            kind.visit_values_mut(&mut |value, _| found |= in_value(value));
        }
    });
    found
}

pub(crate) fn macro_capabilities(mac: &Macro) -> Vec<crate::state::CapabilityDto> {
    use blockwork_core::macros::BlockKind;
    let mut found = std::collections::HashSet::new();
    mac.graph.walk_instructions(&mut |ins| {
        let kind = match &ins.kind {
            InstructionKind::Command(_)
            | InstructionKind::CommandWait(_)
            | InstructionKind::OpenApp { .. }
            | InstructionKind::CloseApp { .. } => Some("commands"),
            InstructionKind::HttpRequest { .. } => Some("network"),
            InstructionKind::WriteFile { .. } => Some("files_write"),
            InstructionKind::Token(_) => Some("input"),
            InstructionKind::SetClipboard(_) | InstructionKind::WhenClipboardChanged => Some("clipboard"),
            InstructionKind::WindowAction { .. }
            | InstructionKind::MoveWindow { .. }
            | InstructionKind::WhenWindow { .. }
            | InstructionKind::WhenApp { .. } => Some("windows"),
            InstructionKind::WhenTyped { .. } => Some("input"),
            InstructionKind::WhenFileChanged(_) => Some("files_read"),
            InstructionKind::RunMacro { .. } | InstructionKind::StopMacroNamed(_) => Some("macros"),
            InstructionKind::FindImage { .. } | InstructionKind::WaitForImage { .. } => Some("screen"),
            InstructionKind::DetectObjectsInWindow { .. } | InstructionKind::WaitForDetection { .. } => Some("screen"),
            InstructionKind::ClickImage { .. } | InstructionKind::AimStep { .. } | InstructionKind::AimAtDetection { .. } => Some("input"),
            InstructionKind::SetGlobal { .. } | InstructionKind::ChangeGlobal { .. } => Some("globals"),
            InstructionKind::BrowserOpen { .. }
            | InstructionKind::BrowserClick { .. }
            | InstructionKind::BrowserType { .. }
            | InstructionKind::BrowserWaitFor { .. }
            | InstructionKind::BrowserScript { .. } => Some("browser"),
            _ => None,
        };
        found.extend(kind);
        if matches!(ins.kind, InstructionKind::ClickImage { .. } | InstructionKind::AimAtDetection { .. }) {
            found.insert("screen");
        }
        if matches!(ins.kind, InstructionKind::PasteText(_)) {
            found.extend(["input", "clipboard"]);
        }
        if ins.kind.is_header() && !matches!(ins.kind, InstructionKind::WhenRan | InstructionKind::BlockHeader(_)) {
            found.insert("triggers");
        }
        let mut kind = ins.kind.clone();
        kind.visit_values_mut(&mut |value, _| value_capabilities(value, &mut found));
    });
    CAPABILITIES
        .iter()
        .filter(|(kind, ..)| found.contains(kind))
        .map(|(kind, label, risky)| crate::state::CapabilityDto {
            kind: kind.to_string(),
            label: label.to_string(),
            risky: *risky,
            guarded: blockwork_core::permissions::GUARDED.iter().any(|(k, _)| k == kind),
        })
        .collect()
}

/// One (key, label, getter, setter) entry per `MacroSettings` field - the
/// single place a new setting needs registering to participate in the
/// "review custom settings on import" flow below. `key` is the wire
/// identifier the frontend's toggle list and `confirm_import_macro`'s
/// `keep_settings` map address it by; `label` is the human-readable text.
pub(super) type SettingAccessor = (
    &'static str,
    &'static str,
    fn(&blockwork_core::macros::MacroSettings) -> bool,
    fn(&mut blockwork_core::macros::MacroSettings, bool),
);

pub(super) const SETTING_ACCESSORS: &[SettingAccessor] = &[
    (
        "always_listen",
        "Always listen for events, even when a different macro is selected",
        |s| s.always_listen,
        |s, v| s.always_listen = v,
    ),
    (
        "catch_up_missed",
        "Run schedules it missed while Blockwork was closed or the computer slept",
        |s| s.catch_up_missed,
        |s, v| s.catch_up_missed = v,
    ),
];

/// Every `MacroSettings` field on `settings` that differs from its default -
/// shown to the user by `import_macro` for confirmation, since non-default
/// behavior deserves a second look before it's silently applied.
pub(super) fn non_default_macro_settings(
    settings: &blockwork_core::macros::MacroSettings,
) -> Vec<crate::state::CustomMacroSettingDto> {
    let default = blockwork_core::macros::MacroSettings::default();
    SETTING_ACCESSORS
        .iter()
        .filter_map(|(key, label, get, _)| {
            let value = get(settings);
            (value != get(&default)).then(|| crate::state::CustomMacroSettingDto {
                key: key.to_string(),
                label: label.to_string(),
                enabled: value,
            })
        })
        .collect()
}

/// Resets every setting the user unchecked in the import review popup (i.e.
/// present in `keep` as `false`) back to its default; anything not mentioned
/// in `keep` is left as imported.
pub(super) fn apply_setting_overrides(
    settings: &mut blockwork_core::macros::MacroSettings,
    keep: &HashMap<String, bool>,
) {
    let default = blockwork_core::macros::MacroSettings::default();
    for (key, _, get, set) in SETTING_ACCESSORS {
        if !keep.get(*key).copied().unwrap_or(true) {
            set(settings, get(&default));
        }
    }
}

/// Saves imported macros under fresh ids (so importing never overwrites
/// anything), restricted to the abilities in `allow`, and opens the first.
pub(super) fn commit_imported_macros(
    macros: Vec<Macro>,
    allow: &[String],
    state: &SharedState,
    app: &AppHandle,
) -> Result<(), String> {
    let mut first = None;
    for mut mac in macros {
        mac.id = uuid::Uuid::new_v4().simple().to_string();
        blockwork_core::permissions::restrict(&mac.id, allow.to_vec());
        first.get_or_insert_with(|| mac.id.clone());
        mac.add()?;
    }

    let mut s = state.lock().map_err(|e| e.to_string())?;
    refresh_macro_list(&mut s);
    if let Some((idx, found)) = s
        .macros_list
        .iter()
        .enumerate()
        .find(|(_, m)| Some(&m.id) == first.as_ref())
        .map(|(i, m)| (i, m.clone()))
    {
        config::set_selected_macro_id(Some(&found.id));
        s.macro_selected = Some(idx);
        s.current_macro = Some(found);
    }
    s.invalid_field_buffers.clear();
    sync_variable_values(&mut s);
    sync_list_values(&mut s);
    emit_state_updated(app, &s);
    Ok(())
}

/// Reads the macro or `.blockwork` bundle the user picked and stages it in
/// `pending_import` with a review of what it can do (see
/// `macro_capabilities`), any non-default settings and upgrade notes. The
/// review shows through `StateDto::import_review`; the frontend resolves it
/// via `confirm_import_macro`/`cancel_import_macro`.
pub(crate) fn import_macro(
    state: &SharedState,
    app: &AppHandle,
    path: String,
) -> Result<(), String> {
    let file = config::persistence::read_import_file(std::path::Path::new(&path))?;
    let mut capabilities: Vec<crate::state::CapabilityDto> = Vec::new();
    let mut custom_settings: Vec<crate::state::CustomMacroSettingDto> = Vec::new();
    for mac in &file.macros {
        for cap in macro_capabilities(mac) {
            if !capabilities.iter().any(|c| c.kind == cap.kind) {
                capabilities.push(cap);
            }
        }
        for setting in non_default_macro_settings(&mac.settings) {
            if !custom_settings.iter().any(|c| c.key == setting.key) {
                custom_settings.push(setting);
            }
        }
    }
    capabilities.sort_by_key(|c| CAPABILITIES.iter().position(|(kind, ..)| *kind == c.kind));
    let review = crate::state::ImportReviewDto {
        name: file.name,
        macros: file.macros.iter().map(|m| m.name.clone()).collect(),
        notes: file.notes,
        capabilities,
        custom_settings,
    };
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.pending_import = Some((file.macros, review));
    emit_state_updated(app, &s);
    Ok(())
}

/// Finishes an import staged by `import_macro` after the user resolves its
/// review. `keep_settings` maps each `ImportReviewDto::custom_settings`
/// entry's `key` to whether the user kept its imported value (`true`) or
/// reset it to default (`false`); a key the popup never showed is absent,
/// which `apply_setting_overrides` treats as "keep". `allow` lists the
/// guarded abilities the user ticked; the rest ask when first used.
pub(crate) async fn confirm_import_macro(
    state: &SharedState,
    app: &AppHandle,
    keep_settings: HashMap<String, bool>,
    allow: Vec<String>,
) -> Result<(), String> {
    let pending = {
        state
            .lock()
            .map_err(|e| e.to_string())?
            .pending_import
            .take()
    };
    let Some((mut macros, _)) = pending else {
        return Ok(());
    };
    for mac in &mut macros {
        apply_setting_overrides(&mut mac.settings, &keep_settings);
    }
    let allow: Vec<String> = allow
        .into_iter()
        .filter(|kind| blockwork_core::permissions::GUARDED.iter().any(|(k, _)| k == kind))
        .collect();
    commit_imported_macros(macros, &allow, state, app)
}

pub(super) fn bundle_macros(all: &[Macro], macro_ids: &[String]) -> Vec<Macro> {
    let mut picked: Vec<Macro> = macro_ids.iter().filter_map(|id| all.iter().find(|m| &m.id == id).cloned()).collect();
    let mut i = 0;
    while i < picked.len() {
        let mut targets = Vec::new();
        picked[i].graph.walk_instructions(&mut |ins| {
            if let InstructionKind::RunMacro { target: Value::Text { value }, .. } | InstructionKind::StopMacroNamed(Value::Text { value }) = &ins.kind {
                targets.push(value.trim().to_string());
            }
        });
        for target in targets {
            let found = all.iter().find(|m| m.name == target).or_else(|| all.iter().find(|m| m.name.eq_ignore_ascii_case(&target)));
            if let Some(found) = found
                && !picked.iter().any(|m| m.id == found.id)
            {
                picked.push(found.clone());
            }
        }
        i += 1;
    }
    picked
}

pub(crate) fn bundle_macro_ids(macro_ids: Vec<String>) -> Vec<String> {
    bundle_macros(&config::get_macros_from_config(), &macro_ids).into_iter().map(|m| m.id).collect()
}

/// Exports macros and their dependencies together. Returns how many it wrote.
pub(crate) fn export_bundle(macro_ids: Vec<String>, name: String, path: String) -> Result<usize, String> {
    let picked = bundle_macros(&config::get_macros_from_config(), &macro_ids);
    if picked.is_empty() {
        return Err("No macros to export".into());
    }
    config::persistence::write_bundle(std::path::Path::new(&path), name.trim(), &picked)?;
    Ok(picked.len())
}

/// Discards an import staged by `import_macro` after the user declines its review.
pub(crate) fn cancel_import_macro(state: &SharedState, app: &AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.pending_import = None;
    emit_state_updated(app, &s);
    Ok(())
}
