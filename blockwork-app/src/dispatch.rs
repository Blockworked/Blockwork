//! Maps a command name plus its JSON arguments onto the matching function in
//! `commands/`. Arguments are camelCase, as the Qt editor sends them over
//! the daemon socket.

use crate::Backend;
use crate::commands;
use crate::state::{HotkeyActionDto, PathStep};
use blockwork_core::input::value::Value as BlockValue;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;

fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    let value = args.get(name).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| format!("invalid argument '{name}': {e}"))
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

impl Backend {
    /// Runs the command named `cmd` with `args` (a JSON object keyed by
    /// camelCase argument name) and returns its result as JSON.
    pub async fn dispatch(&self, cmd: &str, args: Value) -> Result<Value, String> {
        if let Some(result) = commands::edit_command(&self.state, &self.app, cmd, &args).await {
            return result;
        }
        match cmd {
            "get_state" => to_json(commands::get_state(&self.state)?),
            "select_macro" => {
                let index: usize = arg(&args, "index")?;
                to_json(commands::select_macro(&self.state, &self.app, index)?)
            }
            "new_macro" => to_json(commands::new_macro(&self.state, &self.app)?),
            "select_macro_by_id" => {
                let id: String = arg(&args, "id")?;
                to_json(commands::select_macro_by_id(&self.state, &self.app, &id)?)
            }
            "duplicate_macro" => {
                let id: String = arg(&args, "id")?;
                to_json(commands::duplicate_macro(&self.state, &self.app, &id)?)
            }
            "set_macro_folder" => {
                let (id, folder): (String, String) = (arg(&args, "id")?, arg(&args, "folder")?);
                to_json(commands::set_macro_folder(
                    &self.state,
                    &self.app,
                    &id,
                    &folder,
                )?)
            }
            "delete_macro" => {
                let id: String = arg(&args, "id")?;
                to_json(commands::delete_macro(&self.state, &self.app, &id)?)
            }
            "toggle_breakpoint" => {
                let block_id: String = arg(&args, "blockId")?;
                to_json(commands::toggle_breakpoint(&self.state, &block_id)?)
            }
            "clear_breakpoints" => to_json(commands::clear_breakpoints(&self.state)?),
            "debug_action" => {
                let action: String = arg(&args, "action")?;
                to_json(commands::debug_action(&self.state, &action)?)
            }
            "set_secret" | "delete_secret" => {
                let name: String = arg(&args, "name")?;
                let value: String = if cmd == "set_secret" {
                    arg(&args, "value")?
                } else {
                    String::new()
                };
                let (state, app, set) = (self.state.clone(), self.app.clone(), cmd == "set_secret");
                to_json(
                    tokio::task::spawn_blocking(move || {
                        if set {
                            commands::set_secret(&state, &app, &name, &value)
                        } else {
                            commands::delete_secret(&state, &app, &name)
                        }
                    })
                    .await
                    .map_err(|e| e.to_string())??,
                )
            }
            "new_macro_from_template" => {
                let template: String = arg(&args, "template")?;
                to_json(commands::new_macro_from_template(
                    &self.state,
                    &self.app,
                    &template,
                )?)
            }
            "remove_macro" => to_json(commands::remove_macro(&self.state, &self.app)?),
            "set_title" => {
                let title: String = arg(&args, "title")?;
                to_json(commands::set_title(&self.state, &self.app, title)?)
            }
            "set_macro_speed_multiplier" => {
                let multiplier: f64 = arg(&args, "multiplier")?;
                to_json(commands::set_macro_speed_multiplier(
                    &self.state,
                    &self.app,
                    multiplier,
                )?)
            }
            "set_macro_always_listen" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_macro_always_listen(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "set_macro_catch_up" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_macro_catch_up(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "set_macro_restricted" => {
                let restricted: bool = arg(&args, "restricted")?;
                to_json(commands::set_macro_restricted(
                    &self.state,
                    &self.app,
                    restricted,
                )?)
            }
            "set_macro_permission" => {
                let (kind, allowed): (String, bool) = (arg(&args, "kind")?, arg(&args, "allowed")?);
                to_json(commands::set_macro_permission(
                    &self.state,
                    &self.app,
                    &kind,
                    allowed,
                )?)
            }
            "bundle_macro_ids" => to_json(commands::bundle_macro_ids(arg(&args, "macroIds")?)),
            "export_bundle" => {
                let ids: Vec<String> = arg(&args, "macroIds")?;
                let (name, path): (String, String) = (arg(&args, "name")?, arg(&args, "path")?);
                to_json(commands::export_bundle(ids, name, path)?)
            }
            "set_webhook" => {
                let (enabled, port): (bool, u16) = (arg(&args, "enabled")?, arg(&args, "port")?);
                to_json(crate::webhook::configure(
                    &self.state,
                    &self.app,
                    enabled,
                    port,
                )?)
            }
            "refresh_block_library" => {
                to_json(crate::block_library::refresh(&self.state, &self.app)?)
            }
            "add_block_library_source" | "remove_block_library_source" => {
                let url: String = arg(&args, "url")?;
                if cmd == "add_block_library_source" {
                    to_json(crate::block_library::add_source(
                        &self.state,
                        &self.app,
                        url,
                    )?)
                } else {
                    to_json(crate::block_library::remove_source(
                        &self.state,
                        &self.app,
                        url,
                    )?)
                }
            }
            "review_block_pack" => {
                let key: String = arg(&args, "key")?;
                to_json(crate::block_library::review(&self.state, &self.app, key)?)
            }
            "review_block_pack_file" => {
                let path: String = arg(&args, "path")?;
                to_json(crate::block_library::review_file(
                    &self.state,
                    &self.app,
                    path,
                )?)
            }
            "remove_block_pack" => {
                let pack_id: String = arg(&args, "packId")?;
                to_json(crate::block_library::remove(
                    &self.state,
                    &self.app,
                    pack_id,
                )?)
            }
            "confirm_block_pack" => to_json(crate::block_library::confirm(&self.state, &self.app)?),
            "cancel_block_pack" => to_json(crate::block_library::cancel(&self.state, &self.app)?),
            "set_pack_setting" => to_json(crate::block_library::set_setting(
                &self.state,
                &self.app,
                arg(&args, "packId")?,
                arg(&args, "name")?,
                arg(&args, "value")?,
            )?),
            "pack_variable_candidates" => {
                let s = self.state.lock().map_err(|e| e.to_string())?;
                let mac = s.current_macro.as_ref().ok_or("No macro selected")?;
                let pack = blockwork_core::block_packs::pack_from_macro(
                    mac,
                    &arg::<Vec<String>>(&args, "blockIds")?,
                    blockwork_core::block_packs::PackInfo {
                        name: "Preview".into(),
                        ..Default::default()
                    },
                )?;
                to_json(
                    pack.content
                        .variables
                        .iter()
                        .map(|v| v.name.clone())
                        .collect::<Vec<_>>(),
                )
            }
            "export_block_pack" => {
                let block_ids: Vec<String> = arg(&args, "blockIds")?;
                let info = blockwork_core::block_packs::PackInfo {
                    id: arg::<Option<String>>(&args, "id")?.unwrap_or_default(),
                    name: arg(&args, "name")?,
                    description: arg::<Option<String>>(&args, "description")?.unwrap_or_default(),
                    author: arg::<Option<String>>(&args, "author")?.unwrap_or_default(),
                    version: arg::<Option<String>>(&args, "version")?.unwrap_or_default(),
                };
                let path: String = arg(&args, "path")?;
                to_json(crate::block_library::export(
                    &self.state,
                    &self.app,
                    block_ids,
                    info,
                    path,
                    arg::<Option<Vec<blockwork_core::block_packs::VariableInterface>>>(
                        &args,
                        "variableRoles",
                    )?
                    .unwrap_or_default(),
                )?)
            }
            "block_library_previews" => to_json(crate::block_library::previews(
                &self.state,
                &self.app,
                arg(&args, "keys")?,
            )),
            "block_library_pack" => to_json(crate::block_library::pack_view(
                &self.state,
                &self.app,
                arg(&args, "key")?,
                arg::<Option<String>>(&args, "sha256")?.unwrap_or_default(),
            )?),
            "community_pack" => to_json(crate::community::pack_detail(
                &self.state,
                &self.app,
                &arg::<String>(&args, "packId")?,
                arg::<Option<bool>>(&args, "refresh")?.unwrap_or(false),
            )),
            "community_sign_in" => to_json(crate::community::sign_in(&self.state, &self.app)?),
            "community_cancel_sign_in" => {
                to_json(crate::community::cancel_sign_in(&self.state, &self.app)?)
            }
            "community_sign_out" => {
                let (state, app) = (self.state.clone(), self.app.clone());
                to_json(
                    tokio::task::spawn_blocking(move || crate::community::sign_out(&state, &app))
                        .await
                        .map_err(|e| e.to_string())??,
                )
            }
            "community_post_comment" => to_json(crate::community::post_comment(
                &self.state,
                &self.app,
                arg(&args, "packId")?,
                arg(&args, "body")?,
            )?),
            "community_delete_comment" => to_json(crate::community::delete_comment(
                &self.state,
                &self.app,
                arg(&args, "packId")?,
                arg(&args, "id")?,
            )?),
            "community_my_packs" => to_json(crate::community::my_packs(
                &self.state,
                &self.app,
                arg::<Option<bool>>(&args, "refresh")?.unwrap_or(false),
            )),
            "community_publish_file" => to_json(crate::community::publish_file(
                &self.state,
                &self.app,
                arg(&args, "path")?,
            )?),
            "community_publish" => {
                let block_ids: Vec<String> = arg(&args, "blockIds")?;
                let info = blockwork_core::block_packs::PackInfo {
                    id: arg::<Option<String>>(&args, "id")?.unwrap_or_default(),
                    name: arg(&args, "name")?,
                    description: arg::<Option<String>>(&args, "description")?.unwrap_or_default(),
                    author: String::new(),
                    version: arg::<Option<String>>(&args, "version")?.unwrap_or_default(),
                };
                to_json(crate::community::publish_blocks(
                    &self.state,
                    &self.app,
                    block_ids,
                    info,
                    arg::<Option<Vec<blockwork_core::block_packs::VariableInterface>>>(
                        &args,
                        "variableRoles",
                    )?
                    .unwrap_or_default(),
                )?)
            }
            "set_browser_bridge" => {
                let (enabled, port): (bool, u16) = (arg(&args, "enabled")?, arg(&args, "port")?);
                to_json(crate::browser_bridge::configure(
                    &self.state,
                    &self.app,
                    enabled,
                    port,
                )?)
            }
            "new_browser_key" => to_json(crate::browser_bridge::new_key(&self.state, &self.app)?),
            "browser_key" => to_json(blockwork_core::browser::key()),
            "new_webhook_token" => to_json(crate::webhook::new_token(&self.state, &self.app)?),
            "webhook_token" => to_json(crate::webhook::token()),
            "set_macro_max_run_secs" => {
                let seconds: u32 = arg(&args, "seconds")?;
                to_json(commands::set_macro_max_run_secs(
                    &self.state,
                    &self.app,
                    seconds,
                )?)
            }
            "set_macro_max_loop_iterations" => {
                let iterations: u32 = arg(&args, "iterations")?;
                to_json(commands::set_macro_max_loop_iterations(
                    &self.state,
                    &self.app,
                    iterations,
                )?)
            }
            "set_macro_conditions" => {
                let window: String = arg(&args, "onlyInWindow")?;
                let power = arg(&args, "power")?;
                let (from, until): (String, String) =
                    (arg(&args, "activeFrom")?, arg(&args, "activeUntil")?);
                let days: u8 = arg(&args, "activeDays")?;
                to_json(commands::set_macro_conditions(
                    &self.state,
                    &self.app,
                    window,
                    power,
                    &from,
                    &until,
                    days,
                )?)
            }
            "list_macros" => to_json(crate::macro_runs::list(&self.state)?),
            // For the region picker: where a window is, to make a pick window-relative.
            "window_bounds" => {
                let pattern: String = arg(&args, "pattern")?;
                let by: blockwork_core::window::MatchBy = arg::<String>(&args, "by")?.parse()?;
                to_json(
                    tokio::task::spawn_blocking(move || {
                        blockwork_core::window::bounds(&pattern, by)
                    })
                    .await
                    .map_err(|e| e.to_string())??,
                )
            }
            "screen_grab" => {
                let (x, y): (f64, f64) = (arg(&args, "x")?, arg(&args, "y")?);
                let (width, height): (f64, f64) = (arg(&args, "width")?, arg(&args, "height")?);
                to_json(
                    tokio::task::spawn_blocking(move || {
                        crate::screen_pick::grab(x, y, width, height)
                    })
                    .await
                    .map_err(|e| e.to_string())??,
                )
            }
            // For the USB device picker.
            "list_usb_devices" => to_json(
                tokio::task::spawn_blocking(blockwork_core::system::usb_devices)
                    .await
                    .map_err(|e| e.to_string())??,
            ),
            "list_models" => to_json(blockwork_core::detect::models::catalog()?),
            "download_model" => {
                let id: String = arg(&args, "id")?;
                to_json(
                    tokio::task::spawn_blocking(move || {
                        blockwork_core::detect::models::install(&id)
                    })
                    .await
                    .map_err(|e| e.to_string())??,
                )
            }
            "import_model" => {
                let path: String = arg(&args, "path")?;
                to_json(blockwork_core::detect::models::import(&path)?)
            }
            "image_from_file" => {
                let path: String = arg(&args, "path")?;
                to_json(crate::screen_pick::image_from_file(&path)?)
            }
            "run_macro_named" => {
                let name: String = arg(&args, "name")?;
                to_json(crate::macro_runs::run_named(&self.state, &self.app, &name)?)
            }
            "stop_macro_named" => {
                let name: String = arg(&args, "name")?;
                to_json(crate::macro_runs::stop_named(&self.state, &name)?)
            }
            "create_global" => {
                let name: String = arg(&args, "name")?;
                to_json(commands::create_global(&self.state, &self.app, &name)?)
            }
            "delete_global" => {
                let name: String = arg(&args, "name")?;
                to_json(commands::delete_global(&self.state, &self.app, &name)?)
            }
            "emergency_stop" => {
                commands::emergency_stop(&self.state, &self.app);
                to_json(())
            }
            "resume_triggers" => to_json(commands::resume_triggers(&self.state, &self.app)?),
            "clear_run_log" => to_json(commands::clear_run_log(&self.state, &self.app)?),
            "save_macro" => to_json(commands::save_macro(&self.state, &self.app)?),
            "export_macro_file_name" => {
                let macro_id: String = arg(&args, "macroId")?;
                to_json(commands::export_macro_file_name(macro_id)?)
            }
            "export_macro" => {
                let macro_id: String = arg(&args, "macroId")?;
                let path: String = arg(&args, "path")?;
                to_json(commands::export_macro(macro_id, path)?)
            }
            "import_macro" => {
                let path: String = arg(&args, "path")?;
                to_json(commands::import_macro(&self.state, &self.app, path)?)
            }
            "confirm_import_macro" => {
                let keep_settings: HashMap<String, bool> = arg(&args, "keepSettings")?;
                let allow: Vec<String> = args
                    .get("allow")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                to_json(
                    commands::confirm_import_macro(&self.state, &self.app, keep_settings, allow)
                        .await?,
                )
            }
            "cancel_import_macro" => {
                to_json(commands::cancel_import_macro(&self.state, &self.app)?)
            }
            "preview_value" => {
                let value: BlockValue = arg(&args, "value")?;
                // Reporters may block (files, windows, the keyring over D-Bus).
                let state = self.state.clone();
                to_json(
                    tokio::task::spawn_blocking(move || commands::preview_value(&state, value))
                        .await
                        .map_err(|e| e.to_string())??,
                )
            }
            "start_key_capture" => {
                let strand_id: String = arg(&args, "strandId")?;
                let path: Vec<PathStep> = arg(&args, "path")?;
                to_json(commands::start_key_capture(
                    &self.state,
                    &self.app,
                    strand_id,
                    path,
                )?)
            }
            "start_standalone_key_capture" => to_json(commands::start_standalone_key_capture(
                &self.state,
                &self.app,
            )?),
            "key_capture_event" => {
                let keys: Vec<(String, String)> = arg(&args, "keys")?;
                to_json(commands::key_capture_event(&self.state, &self.app, keys)?)
            }
            "clear_standalone_key_capture" => to_json(commands::clear_standalone_key_capture(
                &self.state,
                &self.app,
            )?),
            "run_macro" => to_json(commands::run_macro(&self.state, &self.app)?),
            "run_strand" => {
                let strand_id: String = arg(&args, "strandId")?;
                to_json(commands::run_strand(&self.state, &self.app, strand_id)?)
            }
            "set_global_speed_multiplier" => {
                let multiplier: f64 = arg(&args, "multiplier")?;
                to_json(commands::set_global_speed_multiplier(
                    &self.state,
                    &self.app,
                    multiplier,
                )?)
            }
            "start_recording" => to_json(commands::start_recording(&self.state, &self.app)?),
            "stop_recording" => to_json(commands::stop_recording(&self.state, &self.app)?),
            "set_recording_countdown" => {
                let seconds: u8 = arg(&args, "seconds")?;
                to_json(commands::set_recording_countdown(
                    &self.state,
                    &self.app,
                    seconds,
                )?)
            }
            "toggle_record_mouse_relative" => {
                let relative: bool = arg(&args, "relative")?;
                to_json(
                    commands::toggle_record_mouse_relative(&self.state, &self.app, relative)
                        .await?,
                )
            }
            "toggle_record_mouse_movement" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::toggle_record_mouse_movement(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "open_settings" => to_json(commands::open_settings(&self.state, &self.app)?),
            "close_settings" => to_json(commands::close_settings(&self.state, &self.app)?),
            "start_combo_capture" => {
                let action: HotkeyActionDto = arg(&args, "action")?;
                to_json(commands::start_combo_capture(
                    &self.state,
                    &self.app,
                    action,
                )?)
            }
            "start_binding_combo_capture" => {
                let index: usize = arg(&args, "index")?;
                to_json(commands::start_binding_combo_capture(
                    &self.state,
                    &self.app,
                    index,
                )?)
            }
            "start_pending_combo_capture" => to_json(commands::start_pending_combo_capture(
                &self.state,
                &self.app,
            )?),
            "combo_capture_event" => {
                let code: String = arg(&args, "code")?;
                let modifiers: u8 = arg(&args, "modifiers")?;
                to_json(commands::combo_capture_event(
                    &self.state,
                    &self.app,
                    code,
                    modifiers,
                )?)
            }
            "cancel_combo_capture" => {
                to_json(commands::cancel_combo_capture(&self.state, &self.app)?)
            }
            "set_pending_macro_idx" => {
                let index: Option<usize> = arg(&args, "index")?;
                to_json(commands::set_pending_macro_idx(
                    &self.state,
                    &self.app,
                    index,
                )?)
            }
            "add_macro_hotkey" => to_json(commands::add_macro_hotkey(&self.state, &self.app)?),
            "remove_hotkey_binding" => {
                let index: usize = arg(&args, "index")?;
                to_json(commands::remove_hotkey_binding(
                    &self.state,
                    &self.app,
                    index,
                )?)
            }
            "clear_named_hotkey" => {
                let action: HotkeyActionDto = arg(&args, "action")?;
                to_json(commands::clear_named_hotkey(
                    &self.state,
                    &self.app,
                    action,
                )?)
            }
            "reset_hotkey_to_default" => {
                let action: HotkeyActionDto = arg(&args, "action")?;
                to_json(commands::reset_hotkey_to_default(
                    &self.state,
                    &self.app,
                    action,
                )?)
            }
            "set_ipc_port_text" => {
                let text: String = arg(&args, "text")?;
                to_json(commands::set_ipc_port_text(&self.state, &self.app, text)?)
            }
            "start_ipc_server" => {
                to_json(commands::start_ipc_server(&self.state, &self.app).await?)
            }
            "stop_ipc_server" => to_json(commands::stop_ipc_server(&self.state, &self.app)?),
            "set_ipc_auto_start" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_ipc_auto_start(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "set_close_to_tray" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_close_to_tray(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "set_language" => {
                let language: String = arg(&args, "language")?;
                to_json(commands::set_language(&self.state, &self.app, language)?)
            }
            "set_color_scheme" => {
                let scheme: String = arg(&args, "scheme")?;
                to_json(commands::set_color_scheme(&self.state, &self.app, scheme)?)
            }
            "set_clock_format" => {
                let format: String = arg(&args, "format")?;
                to_json(commands::set_clock_format(&self.state, &self.app, format)?)
            }
            "set_animation_speed" => {
                let speed: f64 = arg(&args, "speed")?;
                to_json(commands::set_animation_speed(
                    &self.state,
                    &self.app,
                    speed,
                )?)
            }
            "set_palette_layout" => {
                let layout: String = arg(&args, "layout")?;
                to_json(commands::set_palette_layout(
                    &self.state,
                    &self.app,
                    layout,
                )?)
            }
            "set_start_at_startup" => {
                let enabled: bool = arg(&args, "enabled")?;
                to_json(commands::set_start_at_startup(
                    &self.state,
                    &self.app,
                    enabled,
                )?)
            }
            "set_onboarding_done" => {
                let done: bool = arg(&args, "done")?;
                to_json(commands::set_onboarding_done(&self.state, &self.app, done)?)
            }
            "check_for_updates" => to_json(commands::check_for_updates(&self.state, &self.app)?),
            "apply_update" => to_json(commands::apply_update(&self.state, &self.app)?),
            "list_installed_apps" => to_json(commands::list_installed_apps().await),
            other => Err(format!("unknown command: {other}")),
        }
    }
}
