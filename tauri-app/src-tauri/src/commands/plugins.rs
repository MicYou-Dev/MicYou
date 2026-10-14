/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

//! Plugin management commands.

use super::blocking;
use micyou_core::host::HostIntegration;
use micyou_core::plugins::install::{PluginPreview, PluginSyncStatus, PluginUpdate, PluginView};
use micyou_core::server::ServerState;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn list_plugins(state: State<'_, ServerState>) -> Result<Vec<PluginView>, String> {
    state.plugins.list()
}

#[tauri::command]
pub fn set_plugin_enabled(state: State<'_, ServerState>, id: String, enabled: bool) -> Result<(), String> {
    state.plugins.set_enabled(&id, enabled)
}

#[tauri::command]
pub fn uninstall_plugin(state: State<'_, ServerState>, id: String) -> Result<(), String> {
    state.plugins.uninstall(&id)
}

#[tauri::command]
pub fn get_plugin_config(state: State<'_, ServerState>, id: String) -> Result<serde_json::Value, String> {
    state.plugins.config(&id)
}

#[tauri::command]
pub fn set_plugin_config(
    state: State<'_, ServerState>,
    id: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    state.plugins.set_config(&id, &key, value)
}

#[tauri::command]
pub fn get_plugin_logs(state: State<'_, ServerState>, id: String) -> Vec<String> {
    state.plugins.logs.lines(&id)
}

#[tauri::command]
pub fn get_plugin_sync_status(state: State<'_, ServerState>) -> PluginSyncStatus {
    state.plugins.sync_status()
}

#[tauri::command]
pub fn get_plugin_panel_icons(state: State<'_, ServerState>, id: String) -> HashMap<String, String> {
    state.plugins.panel_icons(&id)
}

#[tauri::command]
pub fn get_app_locale() -> String {
    micyou_core::config::load_ui_prefs().language
}

/// Open the plugin directory in the system file manager.
#[tauri::command]
pub fn open_plugins_dir(app: AppHandle, state: State<'_, ServerState>) -> Result<String, String> {
    let dir = state.plugins.plugin_dir_path()?.display().to_string();
    app.opener()
        .open_path(&dir, None::<&str>)
        .map_err(|e| format!("open plugins dir: {e}"))?;
    Ok(dir)
}

/// Async for the same reason as `open_settings_window`: building a window in
/// a sync command deadlocks on Windows.
#[tauri::command]
pub async fn open_plugin_window(
    app: AppHandle,
    state: State<'_, ServerState>,
    plugin_id: String,
    panel_id: String,
) -> Result<(), String> {
    let title = state
        .plugins
        .list()?
        .into_iter()
        .find(|plugin| plugin.id == plugin_id)
        .map(|plugin| {
            let panel = plugin
                .ui
                .as_ref()
                .and_then(|ui| ui.panels.iter().find(|p| p.id == panel_id))
                .map(|p| p.label.clone())
                .unwrap_or_else(|| panel_id.clone());
            format!("{} · {panel}", plugin.name)
        })
        .ok_or_else(|| format!("unknown plugin {plugin_id}"))?;
    crate::host::TauriHost(app).open_plugin_panel(&plugin_id, &panel_id, &title)
}

#[tauri::command]
pub fn get_plugin_panel(state: State<'_, ServerState>, plugin_id: String, panel_id: String) -> Result<String, String> {
    state.plugins.panel_html(&plugin_id, &panel_id)
}

#[tauri::command]
pub fn plugin_trigger(
    state: State<'_, ServerState>,
    plugin_id: String,
    action: String,
    payload: Option<String>,
) {
    state
        .plugins
        .trigger(&plugin_id, &action, payload.unwrap_or_default().as_bytes());
}

#[tauri::command]
pub fn preview_plugin_zip(zip_path: PathBuf) -> Result<PluginPreview, String> {
    micyou_core::plugins::install::preview_zip(&zip_path)
}

#[tauri::command]
pub async fn preview_plugin_from_url(manifest_url: String) -> Result<PluginPreview, String> {
    blocking(move || micyou_core::plugins::install::preview_url(&manifest_url)).await
}

#[tauri::command]
pub fn cancel_plugin_download(id: String) {
    micyou_core::plugins::install::cancel_download(&id);
}

/// Download a plugin zip from the market and install it (the frontend asks
/// for permission first via `preview_plugin_from_url`).
#[tauri::command]
pub async fn install_plugin_from_url(app: AppHandle, id: String, zip_url: String) -> Result<String, String> {
    let plugins = app.state::<ServerState>().plugins.clone();
    blocking(move || plugins.install_from_url(&id, &zip_url)).await
}

#[tauri::command]
pub async fn check_plugin_updates(app: AppHandle) -> Result<Vec<PluginUpdate>, String> {
    let plugins = app.state::<ServerState>().plugins.clone();
    blocking(move || plugins.check_updates()).await
}

#[tauri::command]
pub async fn update_plugin(app: AppHandle, id: String) -> Result<String, String> {
    let plugins = app.state::<ServerState>().plugins.clone();
    blocking(move || plugins.update(&id)).await
}

#[tauri::command]
pub fn import_plugin(state: State<'_, ServerState>, source: PathBuf) -> Result<String, String> {
    state.plugins.import(&source)
}
