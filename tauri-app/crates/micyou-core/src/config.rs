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

use micyou_audio::dsp::AudioDspSettings;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Shared config directory (Windows: %APPDATA%\micyou, unix: XDG_CONFIG_HOME or ~/.config + micyou).
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("micyou");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let dir = PathBuf::from(xdg).join("micyou");
        if !dir.as_os_str().is_empty() {
            return dir;
        }
    }
    std::env::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("micyou")
}

/// settings.json: the DSP settings shared by GUI, CLI and TUI.
pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// ui.json: GUI UI preferences (language, theme color) that the TUI reads.
pub fn ui_prefs_path() -> PathBuf {
    config_dir().join("ui.json")
}

/// theme.json: current GUI theme colors exported for the TUI.
pub fn theme_path() -> PathBuf {
    config_dir().join("theme.json")
}

/// server.json: connection-level settings shared by GUI, CLI and TUI
/// (port, mode, bind address, output device).
pub fn server_prefs_path() -> PathBuf {
    config_dir().join("server.json")
}

fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text)
        .inspect_err(|e| log::warn!("[Config] ignoring malformed {}: {e}", path.display()))
        .ok()
}

/// Write a config file atomically: the GUI, CLI and TUI read these files
/// concurrently, and a half-written file would parse as defaults.
fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(|e| format!("create config dir failed: {e}"))?;
    let json =
        serde_json::to_string_pretty(value).map_err(|e| format!("serialize {name} failed: {e}"))?;
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    fs::write(&tmp, json)
        .and_then(|()| fs::rename(&tmp, path))
        .inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
        .map_err(|e| format!("write {name} failed: {e}"))
}

/// Load DSP settings from settings.json, falling back to defaults.
pub fn load_dsp_settings() -> AudioDspSettings {
    load_json::<AudioDspSettings>(&settings_path())
        .map(|mut settings| {
            settings.normalize();
            settings
        })
        .unwrap_or_default()
}

/// Persist DSP settings to settings.json (GUI, CLI and TUI share this file).
pub fn save_dsp_settings(settings: &AudioDspSettings) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.normalize();
    save_json(&settings_path(), &normalized)
}

/// Raw settings.json as a JSON value (for the CLI `settings get`).
pub fn settings_json() -> serde_json::Value {
    load_json(&settings_path())
        .unwrap_or_else(|| serde_json::to_value(AudioDspSettings::default()).unwrap_or_default())
}

/// GUI UI preferences persisted to ui.json.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UiPrefs {
    pub language: String,
    pub theme_color: String,
}

pub fn load_ui_prefs() -> UiPrefs {
    load_json(&ui_prefs_path()).unwrap_or_default()
}

pub fn save_ui_prefs(prefs: &UiPrefs) -> Result<(), String> {
    save_json(&ui_prefs_path(), prefs)
}

/// Theme colors exported from the GUI for the TUI.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub surface: String,
    pub surface_variant: String,
    pub on_surface: String,
    pub error: String,
}

pub fn load_theme_colors() -> ThemeColors {
    load_json(&theme_path()).unwrap_or_default()
}

pub fn save_theme_colors(colors: &ThemeColors) -> Result<(), String> {
    save_json(&theme_path(), colors)
}

/// Connection-level settings shared between the GUI, CLI and TUI.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerPrefs {
    /// Streaming port for wifi/usb modes.
    pub port: u16,
    /// Port for the web (https) mode.
    pub web_port: u16,
    /// Connection mode: wifi | usb | web.
    pub mode: String,
    /// Bind address ("0.0.0.0" when auto-bind).
    pub bind_address: String,
    /// Whether to listen on all interfaces.
    pub auto_bind: bool,
    /// Selected output audio device name.
    pub output_device: String,
    /// Whether mute state is synchronized with the mobile client in both
    /// directions. When false, the desktop neither sends its mute state to
    /// the phone nor applies mute state received from it. Defaults to true,
    /// including for server.json files written before this field existed.
    pub mute_sync: bool,
}

impl Default for ServerPrefs {
    fn default() -> Self {
        Self {
            port: 8554,
            web_port: 8443,
            mode: "wifi".to_string(),
            bind_address: "0.0.0.0".to_string(),
            auto_bind: true,
            output_device: String::new(),
            mute_sync: true,
        }
    }
}

/// Load connection settings from server.json, falling back to defaults.
pub fn load_server_prefs() -> ServerPrefs {
    load_json(&server_prefs_path()).unwrap_or_default()
}

/// Persist connection settings to server.json (GUI, CLI and TUI share this file).
pub fn save_server_prefs(prefs: &ServerPrefs) -> Result<(), String> {
    save_json(&server_prefs_path(), prefs)
}
