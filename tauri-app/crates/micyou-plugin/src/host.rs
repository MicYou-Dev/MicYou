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

//! Host API: the services the host exposes to plugins.
//!
//! The trait here is the *logical* contract. Both runtimes translate it:
//! - Native plugins receive a C ABI function table (see `native.rs`).
//! - WASM plugins receive host functions registered in the linker (see `wasm.rs`).
//!
//! Keeping one logical contract means a plugin written against it behaves the
//! same on desktop and (later) on Android.

use std::path::{Component, Path, PathBuf};

use crate::{error::PluginError, PluginResult};
use serde::{Deserialize, Serialize};

/// Resolve `rel` inside `dir`, rejecting absolute paths and `..` traversal.
/// The result stays inside `dir`; symlinks inside the sandbox are not
/// followed by the caller (documented limitation for plugin-owned files).
pub fn sandbox_path(dir: &Path, rel: &str) -> PluginResult<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() {
        return Err(PluginError::Validation(format!(
            "absolute path not allowed: {rel}"
        )));
    }
    let mut out = dir.to_path_buf();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                return Err(PluginError::Validation(format!(
                    "path traversal not allowed: {rel}"
                )))
            }
            Component::CurDir => {}
            Component::Normal(c) => out.push(c),
            _ => return Err(PluginError::Validation(format!("invalid path: {rel}"))),
        }
    }
    Ok(out)
}

/// Log levels a plugin can emit through the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Snapshot of the live audio stream state, returned by `HostApi::audio_state`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStateSnapshot {
    /// Whether a transport session is currently streaming audio.
    pub streaming: bool,
    /// Input sample rate in Hz (0 when idle).
    pub sample_rate: u32,
    /// Channel count of the incoming stream.
    pub channels: u32,
    /// Raw input level (RMS, 0..1).
    pub input_level: f32,
    /// Level after the DSP chain (RMS, 0..1).
    pub processed_level: f32,
    /// Output queue latency in milliseconds.
    pub queued_ms: f64,
    /// Whether the mute button is engaged.
    pub muted: bool,
}

/// Snapshot of a connected device (phone / web client).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSnapshot {
    /// Connection mode: wifi | usb | web.
    pub mode: String,
    /// Peer address / device label.
    pub label: String,
    /// Whether the device audio session is active.
    pub audio_active: bool,
}

/// Target of a cross-device plugin message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessageTarget {
    /// A plugin on the same host.
    #[serde(rename_all = "camelCase")]
    Local { plugin_id: String },
    /// A plugin on the connected remote device.
    #[serde(rename_all = "camelCase")]
    Remote { plugin_id: String },
    /// Broadcast to all hosts (local + remote) subscribed to the topic.
    Broadcast,
}

/// The services plugins can call. Implemented by the host and handed to each
/// plugin instance; methods must be cheap and never block the real-time audio
/// thread unless explicitly documented.
pub trait HostApi: Send + Sync {
    /// Emit a structured log line attributed to the plugin.
    fn log(&self, level: PluginLogLevel, message: &str);

    /// Read a plugin-scoped configuration value (merged defaults + overrides).
    fn get_config(&self, key: &str) -> Option<serde_json::Value>;

    /// Write a plugin-scoped configuration value (persisted by the host).
    fn set_config(&self, key: &str, value: serde_json::Value) -> PluginResult<()>;

    /// Publish an event on the plugin bus; local subscribers receive it.
    fn emit_event(&self, topic: &str, payload: serde_json::Value) -> PluginResult<()>;

    /// Send a binary message to a local or remote plugin.
    fn send_message(&self, target: MessageTarget, payload: Vec<u8>) -> PluginResult<()>;

    /// Absolute path of the plugin's install directory (read-only).
    fn plugin_dir(&self) -> String;

    /// Register a global system hotkey (e.g. "ctrl+shift+p").
    /// Returns a numeric handle; pressing the hotkey delivers a message to
    /// the plugin on topic `hotkey:<handle>` with a JSON payload.
    fn register_hotkey(&self, shortcut: &str) -> PluginResult<u64>;

    /// Open one of the plugin's own panels (ui.panels entry) in an
    /// independent host window. The plugin decides when a window is needed.
    fn open_window(&self, panel_id: &str) -> PluginResult<()>;

    /// Live audio stream state (requires `audio.state` capability).
    fn audio_state(&self) -> AudioStateSnapshot;

    /// Play a WAV file through the host audio output (requires `audio.play`
    /// capability). Returns once the file is queued; playback is asynchronous
    /// on a host-owned thread and never real-time safe.
    fn play_sound(&self, path: &str) -> PluginResult<()>;

    /// Read a UTF-8 text file inside the plugin's own install directory
    /// (requires `fs.read`). Paths are sandboxed: `..` traversal and absolute
    /// paths are rejected.
    fn fs_read(&self, path: &str) -> PluginResult<String>;

    /// Write a UTF-8 text file inside the plugin's own install directory
    /// (requires `fs.write`). Parent directories are created as needed and
    /// the same sandbox rules as `fs_read` apply.
    fn fs_write(&self, path: &str, content: &str) -> PluginResult<()>;

    /// Connected devices (requires `device.list` capability).
    fn connected_devices(&self) -> Vec<DeviceSnapshot>;

    /// Arm a one-shot timer. After `ms` milliseconds the host delivers a
    /// message on topic `timer:expired` whose JSON payload is
    /// `{"timer":<id>,"payload":"<payload>"}`. Returns the timer id, usable
    /// with `clear_timeout`.
    fn set_timeout(&self, ms: u64, payload: &str) -> PluginResult<u64>;

    /// Cancel a timer previously returned by `set_timeout`. No-op for
    /// unknown/expired ids.
    fn clear_timeout(&self, id: u64) -> PluginResult<()>;

    /// Issue an outbound HTTP request (requires `network.io`). The request
    /// runs on a host-owned thread; the plugin is notified asynchronously via
    /// a message on topic `http:response` whose JSON payload is
    /// `{"request":<id>,"ok":bool,"status":u16,"body":"...","error":"..."}`.
    /// Returns the request id.
    fn http_request(
        &self,
        method: &str,
        url: &str,
        headers_json: &str,
        body: &str,
    ) -> PluginResult<u64>;

    /// Arm a repeating timer. Every `ms` milliseconds the host delivers a
    /// message on topic `interval:tick` whose JSON payload is
    /// `{"interval":<id>,"payload":"<payload>"}`. Use `clear_interval` to stop.
    fn set_interval(&self, ms: u64, payload: &str) -> PluginResult<u64>;

    /// Stop a repeating timer previously returned by `set_interval`.
    fn clear_interval(&self, id: u64) -> PluginResult<()>;

    /// Open a URL in the system default browser (requires `open.url`).
    fn open_url(&self, url: &str) -> PluginResult<()>;

    /// Show a system notification (no capability required).
    fn notify(&self, title: &str, body: &str) -> PluginResult<()>;

    /// Current host UI locale, e.g. "zh-CN" or "en" (no capability).
    fn locale(&self) -> String;

    /// Host identity and API version as a JSON string, e.g.
    /// `{"name":"micyou","version":"2.1.1","apiVersion":2}`: `version` is the
    /// running host app version, `apiVersion` is the host API version (`2`).
    /// No capability required.
    fn host_info(&self) -> String;

    /// Read the current clipboard text (requires `clipboard.read`).
    fn clipboard_read(&self) -> PluginResult<String>;

    /// Replace the clipboard text (requires `clipboard.write`).
    fn clipboard_write(&self, text: &str) -> PluginResult<()>;

    /// Set the icon of a settings-sidebar panel. `icon` is either a file name
    /// relative to the plugin directory (PNG/SVG) or a short text/emoji.
    /// No capability required; the host only renders what the plugin declares.
    fn set_panel_icon(&self, panel_id: &str, icon: &str) -> PluginResult<()>;

    /// Get host mute state (requires `control.observe` capability).
    fn get_muted(&self) -> PluginResult<bool> {
        Err(PluginError::Runtime("get_muted not supported".into()))
    }

    /// Set host mute state (requires `control.intercept` capability).
    fn set_muted(&self, _muted: bool) -> PluginResult<()> {
        Err(PluginError::Runtime("set_muted not supported".into()))
    }

    /// Get host audio monitoring / ear-return state (requires `control.observe` capability).
    fn get_monitoring(&self) -> PluginResult<bool> {
        Err(PluginError::Runtime("get_monitoring not supported".into()))
    }

    /// Set host audio monitoring / ear-return state (requires `control.intercept` capability).
    fn set_monitoring(&self, _enabled: bool) -> PluginResult<()> {
        Err(PluginError::Runtime("set_monitoring not supported".into()))
    }

    /// Get current DSP settings as a JSON string (requires `control.observe` capability).
    fn get_dsp_settings(&self) -> PluginResult<String> {
        Err(PluginError::Runtime("get_dsp_settings not supported".into()))
    }

    /// Update DSP settings from a JSON string (requires `control.intercept` capability).
    fn set_dsp_settings(&self, _settings_json: &str) -> PluginResult<()> {
        Err(PluginError::Runtime("set_dsp_settings not supported".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::sandbox_path;
    use std::path::Path;

    fn dir() -> &'static Path {
        Path::new("/tmp/plugin-dir")
    }

    #[test]
    fn sandbox_accepts_normal_relative_paths() {
        let p = sandbox_path(dir(), "data/file.txt").expect("ok");
        assert_eq!(p, Path::new("/tmp/plugin-dir/data/file.txt"));
    }

    #[test]
    fn sandbox_rejects_parent_traversal() {
        assert!(sandbox_path(dir(), "../evil.txt").is_err());
        assert!(sandbox_path(dir(), "a/../../evil.txt").is_err());
    }

    #[test]
    fn sandbox_rejects_absolute_paths() {
        assert!(sandbox_path(dir(), "/etc/passwd").is_err());
    }

    #[test]
    fn sandbox_allows_curdir_and_nested() {
        let p = sandbox_path(dir(), "./sub/./file.txt").expect("ok");
        assert_eq!(p, Path::new("/tmp/plugin-dir/sub/file.txt"));
    }
}
