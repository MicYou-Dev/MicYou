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

use micyou_core::config::{self, ServerPrefs};
use micyou_core::micyou_audio::dsp::AudioDspSettings;
use micyou_core::platform::{blackhole, vbcable};
use micyou_core::server::ServerState;
use micyou_core::settings::PipeWireStatus;
#[cfg(feature = "vbcable")]
use std::sync::Arc;
use tauri::{AppHandle, State};

// Device enumeration can take seconds on Windows (Bluetooth endpoints), so it
// runs off the main thread, where sync commands would freeze the UI.
#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<String>, String> {
    super::blocking(|| Ok(micyou_core::settings::audio_devices())).await
}

#[tauri::command]
pub fn update_audio_settings(
    state: State<'_, ServerState>,
    settings: AudioDspSettings,
) -> Result<String, String> {
    state.controls().apply_dsp_settings(settings)?;
    Ok("Settings updated".to_string())
}

/// Current AEC availability and state.
///
/// Every frontend reads this instead of deciding platform support locally, so the
/// GUI, the CLI and the TUI cannot disagree about whether AEC can run.
#[tauri::command]
pub fn get_aec_status(state: State<'_, ServerState>) -> micyou_core::events::AecStatus {
    let availability = micyou_core::micyou_audio::aec_reference_availability();
    let enabled = state.current_dsp_settings().aec_enabled;
    micyou_core::events::AecStatus {
        available: availability.available,
        enabled: enabled && availability.available,
        reason: availability.reason,
    }
}

#[tauri::command]
pub fn get_audio_settings(state: State<'_, ServerState>) -> AudioDspSettings {
    state.current_dsp_settings()
}

#[tauri::command]
pub fn get_server_prefs() -> ServerPrefs {
    config::load_server_prefs()
}

#[tauri::command]
pub fn save_server_prefs(prefs: ServerPrefs) -> Result<String, String> {
    config::save_server_prefs(&prefs)?;
    Ok("Server prefs saved".to_string())
}

#[tauri::command]
pub fn check_pipewire() -> PipeWireStatus {
    micyou_core::settings::pipewire_status()
}

#[tauri::command]
pub async fn check_vbcable() -> Result<bool, String> {
    super::blocking(|| Ok(vbcable::is_installed())).await
}

#[cfg(feature = "vbcable")]
#[tauri::command]
pub async fn install_vbcable(app: AppHandle) -> Result<vbcable::VBCableResult, String> {
    Ok(vbcable::install(Arc::new(crate::events::TauriEventSink(app))).await)
}

#[cfg(not(feature = "vbcable"))]
#[tauri::command]
pub fn install_vbcable(_app: AppHandle) -> Result<vbcable::VBCableResult, String> {
    Ok(vbcable::VBCableResult {
        success: false,
        error_type: Some("feature_disabled".to_string()),
        message: Some("VB-Cable installation feature not enabled".to_string()),
    })
}

#[tauri::command]
pub async fn check_blackhole() -> Result<blackhole::BlackHoleStatus, String> {
    blackhole::check_blackhole().await
}

#[tauri::command]
pub async fn set_blackhole_as_input() -> Result<blackhole::BlackHoleResult, String> {
    blackhole::set_blackhole_as_input().await
}

#[tauri::command]
pub async fn restore_input_device() -> Result<blackhole::BlackHoleResult, String> {
    blackhole::restore_input_device().await
}
