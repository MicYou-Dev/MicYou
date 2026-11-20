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

use micyou_core::about::UpdateCheckResult;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Consumes the marker written by the Inno Setup installer. The desktop app
/// cannot edit WebView2's localStorage from the installer, so the installer
/// asks the app to forget the "onboarding completed" flag on its next start.
#[tauri::command]
pub fn take_onboarding_reset() -> bool {
    let marker = micyou_core::config::config_dir().join("onboarding-reset");
    match std::fs::remove_file(&marker) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            log::warn!(
                "removing the onboarding reset marker {} failed: {error}",
                marker.display()
            );
            false
        }
    }
}

#[tauri::command]
pub async fn check_app_update(cdk: Option<String>) -> Result<UpdateCheckResult, String> {
    micyou_core::about::check_app_update(cdk).await
}

#[tauri::command]
pub async fn get_sponsors() -> Result<String, String> {
    micyou_core::about::get_sponsors().await
}

fn log_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_log_dir().map_err(|e| e.to_string())?.join("micyou.log"))
}

#[tauri::command]
pub fn export_log(app: AppHandle) -> Result<(), String> {
    let log_file = log_file(&app)?;
    if !log_file.exists() {
        return Err("Log file not found".to_string());
    }
    app.dialog().file().save_file(move |target| {
        let Some(target) = target.and_then(|path| path.into_path().ok()) else {
            return;
        };
        if let Err(e) = std::fs::copy(&log_file, &target) {
            log::error!("exporting the log to {} failed: {e}", target.display());
        }
    });
    Ok(())
}

#[tauri::command]
pub fn get_log_path(app: AppHandle) -> Result<String, String> {
    Ok(log_file(&app)?.display().to_string())
}

#[tauri::command]
pub fn get_log_content(app: AppHandle) -> Result<String, String> {
    let log_file = log_file(&app)?;
    if !log_file.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&log_file).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> Result<String, String> {
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let dir = log_dir.display().to_string();
    app.opener()
        .open_path(&dir, None::<&str>)
        .map_err(|e| format!("open log dir: {e}"))?;
    Ok(dir)
}
