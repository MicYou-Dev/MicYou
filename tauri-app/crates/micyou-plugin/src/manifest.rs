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

//! Plugin manifest: a unified, platform-independent description shared by the
//! desktop (Tauri), CLI/TUI and the future Android runtime.
//!
//! A plugin directory contains a `plugin.json` plus the entry artifact
//! (native cdylib or WASM module). The manifest is the single source of truth
//! for identity, runtime type, capabilities, DSP wiring and UI registration.

use crate::error::{PluginError, PluginResult};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Minimum supported Host API version.
pub const MIN_SUPPORTED_API_VERSION: u32 = 1;

/// Host API version this plugin system speaks. Plugins declare the version
/// they were built against; the host rejects incompatible ones.
pub const HOST_API_VERSION: u32 = 2;

/// Plugin directory layout: the manifest file name.
pub const MANIFEST_FILE_NAME: &str = "plugin.json";

/// Reverse-DNS plugin id (e.g. `dev.micyou.eq`). Allowed charset:
/// lowercase alphanumerics plus `.` and `-`, at least one dot.
pub fn validate_plugin_id(id: &str) -> bool {
    // The id names the plugin's directory, so every dot-separated label must
    // be non-empty: "..", ".x" or "x." would not be reverse-DNS anyway.
    id.len() <= 128
        && id.contains('.')
        && id.split('.').all(|label| {
            !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

/// Runtime type. `Native` loads a platform cdylib (`.so` / `.dylib` / `.dll`),
/// `Wasm` loads a WebAssembly module into the sandboxed interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    #[default]
    Native,
    Wasm,
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeKind::Native => write!(f, "native"),
            RuntimeKind::Wasm => write!(f, "wasm"),
        }
    }
}

/// Functional category of a plugin, used by the host to decide lifecycle and
/// scheduling policy (e.g. real-time DSP plugins never block the audio thread).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Background logic / automation / networking.
    #[default]
    Utility,
    /// Real-time audio processor inserted into the DSP chain.
    Dsp,
    /// Provides a frontend configuration panel.
    Ui,
    /// Dedicated to cross-device state synchronization.
    Bridge,
}

impl fmt::Display for PluginKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginKind::Dsp => write!(f, "dsp"),
            PluginKind::Utility => write!(f, "utility"),
            PluginKind::Ui => write!(f, "ui"),
            PluginKind::Bridge => write!(f, "bridge"),
        }
    }
}

/// Capability identifiers the host understands. Plugins must declare the
/// capabilities they need; the host grants them after policy checks.
pub mod capabilities {
    /// Insert a processing node into the real-time DSP chain.
    pub const DSP_NODE: &str = "dsp.node";
    /// Read host configuration (settings.json etc).
    pub const CONFIG_READ: &str = "config.read";
    /// Write host configuration.
    pub const CONFIG_WRITE: &str = "config.write";
    /// Emit events on the plugin bus (broadcast / subscribe model).
    pub const EVENT_EMIT: &str = "event.emit";
    /// Send messages to other plugins or to a remote device plugin.
    pub const MESSAGE_SEND: &str = "message.send";
    /// Query live audio stream state (levels, format, latency).
    pub const AUDIO_STATE: &str = "audio.state";
    /// Play an audio file (wav) through the host output device.
    pub const AUDIO_PLAY: &str = "audio.play";
    /// Read files inside the plugin's install directory (sandboxed).
    pub const FS_READ: &str = "fs.read";
    /// Write files inside the plugin's install directory (sandboxed).
    pub const FS_WRITE: &str = "fs.write";
    /// Enumerate connected devices (phones, web clients).
    pub const DEVICE_LIST: &str = "device.list";
    /// Open outbound network connections.
    pub const NETWORK_IO: &str = "network.io";
    /// Open URLs in the system default browser.
    pub const OPEN_URL: &str = "open.url";
    /// Read the system clipboard.
    pub const CLIPBOARD_READ: &str = "clipboard.read";
    /// Write to the system clipboard.
    pub const CLIPBOARD_WRITE: &str = "clipboard.write";
    /// Observe host control plane signals (read-only: mute, devices, dsp changes).
    pub const CONTROL_OBSERVE: &str = "control.observe";
    /// Intercept or mutate host control plane signals.
    pub const CONTROL_INTERCEPT: &str = "control.intercept";
}

/// All capability identifiers the host currently recognizes.
pub const KNOWN_CAPABILITIES: &[&str] = &[
    capabilities::DSP_NODE,
    capabilities::CONFIG_READ,
    capabilities::CONFIG_WRITE,
    capabilities::EVENT_EMIT,
    capabilities::MESSAGE_SEND,
    capabilities::AUDIO_STATE,
    capabilities::AUDIO_PLAY,
    capabilities::DEVICE_LIST,
    capabilities::NETWORK_IO,
    capabilities::FS_READ,
    capabilities::FS_WRITE,
    capabilities::OPEN_URL,
    capabilities::CLIPBOARD_READ,
    capabilities::CLIPBOARD_WRITE,
    capabilities::CONTROL_OBSERVE,
    capabilities::CONTROL_INTERCEPT,
];

/// Native platform tags used in `PluginManifest.platforms`.
pub mod platforms {
    pub const WINDOWS: &str = "windows";
    pub const LINUX: &str = "linux";
    pub const MACOS: &str = "macos";
    pub const ANDROID: &str = "android";
}

fn default_api_version() -> u32 {
    HOST_API_VERSION
}

/// Optional UI registration: buttons panel (`route=buttons`) or custom
/// pages (`panels`), rendered by the frontend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiDescriptor {
    /// Frontend route / component identifier (e.g. `plugin-panel`,
    /// `buttons` for the generic soundpad button grid).
    pub route: String,
    /// Display name of the panel.
    pub label: String,
    /// Relative path to a bundled JS entry (advanced; default: generic form).
    #[serde(default)]
    pub entry: Option<String>,
    /// Custom pages shown in the settings sidebar; each is a self-contained
    /// single-file HTML document inside the plugin directory that talks to
    /// the host through the postMessage bridge (`get_plugin_panel`).
    #[serde(default)]
    pub panels: Vec<UiPanel>,
}

/// A plugin-authored settings page: id + label + HTML file name relative to
/// the plugin directory. The HTML must be self-contained (inline CSS/JS).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPanel {
    /// Stable id used by the frontend routing (`panel:<pluginId>:<id>`).
    pub id: String,
    /// Sidebar entry label.
    pub label: String,
    /// HTML file name inside the plugin directory.
    pub entry: String,
    /// Whether this panel gets a sidebar entry in the settings dialog.
    /// Panels meant only for standalone windows (opened by the plugin's own
    /// actions) should set `sidebar: false`.
    #[serde(default = "default_true")]
    pub sidebar: bool,
}

fn default_true() -> bool {
    true
}

/// Optional DSP registration: where the node is inserted in the chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DspDescriptor {
    /// Insert before or after a built-in node name (e.g. `Equalizer`).
    #[serde(default)]
    pub insert_after: Option<String>,
    /// Insert at the head of the chain (before AEC) when true.
    #[serde(default)]
    pub first: bool,
    /// Preferred processing block size in samples (native plugins only;
    /// the host may fall back to its own frame size).
    #[serde(default)]
    pub frame_size: Option<usize>,
    /// DSP plugins are granted an additional real-time slot check.
    #[serde(default)]
    pub realtime_safe: bool,
}

/// A single configurable field; the host renders an automatic form from
/// `configSchema` so plugins do not need a hand-written settings page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigField {
    /// Config key (used with get_config / set_config).
    pub key: String,
    /// number | boolean | string | select
    #[serde(default = "default_field_type")]
    pub field_type: String,
    /// UI label (falls back to the key).
    #[serde(default)]
    pub label: Option<String>,
    /// Help text.
    #[serde(default)]
    pub description: Option<String>,
    /// Default value (JSON).
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// number: min / max / step
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub step: Option<f64>,
    /// select: choices [{value, label}]
    #[serde(default)]
    pub options: Vec<ConfigOption>,
}

fn default_field_type() -> String {
    "string".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOption {
    pub value: String,
    #[serde(default)]
    pub label: Option<String>,
}

/// Declarative settings schema for automatic form generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSchema {
    #[serde(default)]
    pub fields: Vec<ConfigField>,
}

/// A dependency on another plugin: the dependency must be installed,
/// enabled and satisfy the semver requirement before this plugin can be
/// enabled. Enables plugin-to-plugin composition (forward-declared links).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDependency {
    /// Plugin id of the dependency.
    pub id: String,
    /// Semver requirement, e.g. "^1.0.0" or ">=1.2, <2".
    #[serde(default)]
    pub version: String,
    /// When true, a missing/unmet dependency only warns instead of blocking.
    #[serde(default)]
    pub optional: bool,
}

/// The unified plugin manifest. Field names use camelCase to mirror the
/// wire/protocol style used across MicYou.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Reverse-DNS id, e.g. `dev.micyou.eq`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Semver version.
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// native | wasm
    pub runtime: RuntimeKind,
    /// File name of the entry artifact relative to the plugin directory.
    pub entry: String,
    /// Supported platforms; empty means all. Tags: linux, windows, macos, android.
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Host API version this plugin was built against.
    #[serde(default = "default_api_version")]
    pub api_version: u32,
    /// Capability identifiers requested from the host.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Functional category.
    #[serde(default)]
    pub kind: PluginKind,
    #[serde(default)]
    pub ui: Option<UiDescriptor>,
    #[serde(default)]
    pub dsp: Option<DspDescriptor>,
    /// Default configuration (merged into plugin state on first enable).
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    /// SPDX license identifier, e.g. "MIT" or "GPL-3.0-only".
    #[serde(default)]
    pub license: Option<String>,
    /// Plugin homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Source repository URL.
    #[serde(default)]
    pub repository: Option<String>,
    /// Search keywords for the plugin store / import.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Minimum host API version required (major must not exceed the host's).
    /// Example: "1.0.0".
    #[serde(default)]
    pub min_host_version: Option<String>,
    /// Icon file name relative to the plugin directory (PNG recommended).
    #[serde(default)]
    pub icon: Option<String>,
    /// CPU architectures this entry artifact supports; empty means all.
    /// Tags: x86_64, aarch64, i686, armv7, riscv64.
    #[serde(default)]
    pub arches: Vec<String>,
    /// Localized names, keyed by BCP-47 locale tag, e.g. {"zh-CN": "变声器"}.
    /// Wire format is camelCase (`nameI18n`, matching the marketplace manifest
    /// convention and the frontend view model); the legacy snake_case key is
    /// still accepted as an alias.
    #[serde(default, rename = "nameI18n", alias = "name_i18n")]
    pub name_i18n: std::collections::HashMap<String, String>,
    /// Localized descriptions, keyed by locale tag (wire: `descriptionI18n`).
    #[serde(default, rename = "descriptionI18n", alias = "description_i18n")]
    pub description_i18n: std::collections::HashMap<String, String>,
    /// Dependencies on other plugins (installed, enabled, version-satisfied
    /// before this plugin can be enabled).
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,
    /// Declarative settings schema; the host renders an automatic form.
    #[serde(default)]
    pub config_schema: Option<ConfigSchema>,
    /// Direct download URL of the plugin package (zip). Used by in-app update
    /// when present; otherwise the zip URL is derived from `update_url`
    /// (`.json` → `.zip`). Market catalogs also surface this field.
    #[serde(default, rename = "downloadUrl", alias = "download_url")]
    pub download_url: Option<String>,
    /// URL of a remote manifest (JSON) used for update checks. The host
    /// compares the remote version against the installed one.
    #[serde(default)]
    pub update_url: Option<String>,
}

impl PluginManifest {
    /// Parse + validate a manifest from raw JSON text.
    pub fn from_json(text: &str) -> PluginResult<Self> {
        let manifest: Self =
            serde_json::from_str(text).map_err(|e| PluginError::InvalidManifest(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Load + validate a manifest from `<plugin_dir>/plugin.json`.
    pub fn load_from_dir(dir: &Path) -> PluginResult<Self> {
        let path = dir.join(MANIFEST_FILE_NAME);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| PluginError::InvalidManifest(format!("{}: {e}", path.display())))?;
        Self::from_json(&text)
    }

    /// Semantic validation of a parsed manifest.
    pub fn validate(&self) -> PluginResult<()> {
        if !validate_plugin_id(&self.id) {
            return Err(PluginError::Validation(format!(
                "invalid plugin id {:?}: expect reverse-DNS lowercase alphanumeric with a dot",
                self.id
            )));
        }
        if self.name.is_empty() {
            return Err(PluginError::Validation("name must not be empty".into()));
        }
        semver::Version::parse(&self.version).map_err(|e| {
            PluginError::Validation(format!("invalid semver version {:?}: {e}", self.version))
        })?;
        if self.entry.is_empty() {
            return Err(PluginError::Validation("entry must not be empty".into()));
        }
        if self.api_version < MIN_SUPPORTED_API_VERSION || self.api_version > HOST_API_VERSION {
            return Err(PluginError::ApiVersionMismatch {
                plugin: self.api_version,
                host: HOST_API_VERSION,
            });
        }
        if let Some(schema) = &self.config_schema {
            for field in &schema.fields {
                if field.key.is_empty() {
                    return Err(PluginError::Validation(
                        "configSchema field key must not be empty".into(),
                    ));
                }
                match field.field_type.as_str() {
                    "number" | "boolean" | "string" | "select" => {}
                    other => {
                        return Err(PluginError::Validation(format!(
                            "configSchema field {:?} has unknown type {other:?}",
                            field.key
                        )));
                    }
                }
                if field.field_type == "select" && field.options.is_empty() {
                    return Err(PluginError::Validation(format!(
                        "configSchema select field {:?} needs options",
                        field.key
                    )));
                }
                if let (Some(lo), Some(hi)) = (field.min, field.max) {
                    if lo > hi {
                        return Err(PluginError::Validation(format!(
                            "configSchema field {:?} min > max",
                            field.key
                        )));
                    }
                }
            }
        }
        for dep in &self.dependencies {
            if !validate_plugin_id(&dep.id) {
                return Err(PluginError::Validation(format!(
                    "invalid dependency plugin id {:?}",
                    dep.id
                )));
            }
            if !dep.version.is_empty() {
                semver::VersionReq::parse(&dep.version).map_err(|e| {
                    PluginError::Validation(format!(
                        "invalid dependency version requirement {:?}: {e}",
                        dep.version
                    ))
                })?;
            }
        }
        if let Some(min) = &self.min_host_version {
            let parsed = semver::Version::parse(min).map_err(|e| {
                PluginError::Validation(format!("invalid min_host_version {min:?}: {e}"))
            })?;
            if parsed.major > HOST_API_VERSION as u64 {
                return Err(PluginError::ApiVersionMismatch {
                    plugin: self.api_version,
                    host: HOST_API_VERSION,
                });
            }
        }
        for cap in &self.capabilities {
            if !KNOWN_CAPABILITIES.contains(&cap.as_str()) {
                return Err(PluginError::Validation(format!(
                    "unknown capability {:?}",
                    cap
                )));
            }
        }
        if self.kind == PluginKind::Dsp && self.runtime == RuntimeKind::Wasm {
            // WASM DSP nodes are allowed but must declare realtime_safe, and the
            // host treats them as best-effort (interpreter latency is not
            // guaranteed real-time safe).
            if let Some(dsp) = &self.dsp {
                if dsp.realtime_safe {
                    return Err(PluginError::Validation(
                        "wasm dsp plugin must not claim realtime_safe; interpreter execution cannot guarantee real-time safety".into(),
                    ));
                }
            }
        }
        if self.kind == PluginKind::Ui && self.ui.is_none() {
            return Err(PluginError::Validation(
                "ui plugin must declare a ui descriptor".into(),
            ));
        }
        Ok(())
    }

    /// Entry artifact path resolved against the plugin directory.
    pub fn entry_path(&self, plugin_dir: &Path) -> PathBuf {
        let mut entry = self.entry.clone();

        if self.runtime == RuntimeKind::Native {
            let path = Path::new(&entry);
            if path.extension().is_none() {
                let ext = match () {
                    #[cfg(target_os = "linux")] _ => "so",
                    #[cfg(target_os = "macos")] _ => "dylib",
                    #[cfg(target_os = "windows")] _ => "dll",
                    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))] _ => "",
                };
                if !ext.is_empty() {
                    entry = format!("{}.{}", entry, ext);
                }
            }
        }
        
        plugin_dir.join(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_JSON: &str = r#"{
        "id": "dev.micyou.eq",
        "name": "Bass Boost",
        "version": "1.2.0",
        "author": "LanRhyme",
        "description": "10-band bass EQ",
        "runtime": "native",
        "entry": "libmicyou_eq.so",
        "platforms": ["linux", "windows", "macos"],
        "apiVersion": 1,
        "capabilities": ["dsp.node", "config.read"],
        "kind": "dsp",
        "dsp": { "insertAfter": "Equalizer", "realtimeSafe": true }
    }"#;

    #[test]
    fn parses_valid_manifest() {
        let manifest = PluginManifest::from_json(GOOD_JSON).unwrap();
        assert_eq!(manifest.id, "dev.micyou.eq");
        assert_eq!(manifest.runtime, RuntimeKind::Native);
        assert_eq!(manifest.kind, PluginKind::Dsp);
        assert_eq!(manifest.api_version, 1);
        assert_eq!(manifest.capabilities, vec!["dsp.node", "config.read"]);
        assert_eq!(
            manifest.dsp.as_ref().unwrap().insert_after.as_deref(),
            Some("Equalizer")
        );
        assert!(manifest.dsp.as_ref().unwrap().realtime_safe);
    }

    #[test]
    fn defaults_apply_when_fields_missing() {
        let json = r#"{
            "id": "dev.micyou.util",
            "name": "Logger",
            "version": "0.1.0",
            "runtime": "wasm",
            "entry": "logger.wasm"
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.api_version, HOST_API_VERSION);
        assert!(manifest.platforms.is_empty());
        assert!(manifest.capabilities.is_empty());
        assert_eq!(manifest.kind, PluginKind::Utility);
        assert!(manifest.ui.is_none());
        assert!(manifest.dsp.is_none());
    }

    #[test]
    fn rejects_invalid_plugin_id() {
        for bad in ["no-dot", "Uppercase.Id", "a/b", "", "sp ace", "中文.id", "..", ".a", "a.", "a..b"] {
            let json = format!(
                r#"{{"id":"{bad}","name":"x","version":"1.0.0","runtime":"wasm","entry":"x.wasm"}}"#
            );
            let result = PluginManifest::from_json(&json);
            assert!(result.is_err(), "id {bad:?} should be rejected");
        }
    }

    #[test]
    fn rejects_bad_semver() {
        let json = r#"{"id":"a.b","name":"x","version":"not-a-version","runtime":"wasm","entry":"x.wasm"}"#;
        assert!(PluginManifest::from_json(json).is_err());
    }

    #[test]
    fn rejects_api_version_mismatch() {
        let json = r#"{"id":"a.b","name":"x","version":"1.0.0","runtime":"wasm","entry":"x.wasm","apiVersion":99}"#;
        let result = PluginManifest::from_json(json).unwrap_err();
        assert!(matches!(
            result,
            PluginError::ApiVersionMismatch {
                plugin: 99,
                host: HOST_API_VERSION
            }
        ));
    }

    #[test]
    fn rejects_unknown_capability() {
        let json = r#"{"id":"a.b","name":"x","version":"1.0.0","runtime":"wasm","entry":"x.wasm","capabilities":["root"]}"#;
        let result = PluginManifest::from_json(json).unwrap_err();
        assert!(matches!(result, PluginError::Validation(_)));
    }

    #[test]
    fn rejects_wasm_dsp_claiming_realtime_safe() {
        let json = r#"{
            "id":"a.b.dsp","name":"x","version":"1.0.0","runtime":"wasm","entry":"x.wasm",
            "kind":"dsp","dsp":{"realtimeSafe":true}
        }"#;
        let result = PluginManifest::from_json(json).unwrap_err();
        assert!(matches!(result, PluginError::Validation(_)));
    }

    #[test]
    fn rejects_ui_plugin_without_ui_descriptor() {
        let json = r#"{"id":"a.b.ui","name":"x","version":"1.0.0","runtime":"wasm","entry":"x.wasm","kind":"ui"}"#;
        let result = PluginManifest::from_json(json).unwrap_err();
        assert!(matches!(result, PluginError::Validation(_)));
    }

    #[test]
    fn entry_path_resolves_relative_to_dir() {
        let manifest = PluginManifest::from_json(GOOD_JSON).unwrap();
        assert_eq!(
            manifest.entry_path(Path::new("/opt/micyou/plugins/dev.micyou.eq")),
            PathBuf::from("/opt/micyou/plugins/dev.micyou.eq/libmicyou_eq.so")
        );
    }

    #[test]
    fn entry_path_auto_appends_platform_extension_for_native() {
        let json = r#"{
            "id": "dev.micyou.cross", "name": "Cross", "version": "1.0.0",
            "runtime": "native", "entry": "my_plugin"
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        let path = manifest.entry_path(Path::new("/plugins/dev.micyou.cross"));
        
        #[cfg(target_os = "linux")]
        assert_eq!(path, PathBuf::from("/plugins/dev.micyou.cross/my_plugin.so"));
        #[cfg(target_os = "macos")]
        assert_eq!(path, PathBuf::from("/plugins/dev.micyou.cross/my_plugin.dylib"));
        #[cfg(target_os = "windows")]
        assert_eq!(path, PathBuf::from("/plugins/dev.micyou.cross/my_plugin.dll"));
    }

    #[test]
    fn entry_path_ignores_wasm_runtime() {
        let json = r#"{
            "id": "dev.micyou.wasm-test", "name": "Wasm", "version": "1.0.0",
            "runtime": "wasm", "entry": "my_plugin"
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        let path = manifest.entry_path(Path::new("/plugins/dev.micyou.wasm_test"));
        assert_eq!(path, PathBuf::from("/plugins/dev.micyou.wasm_test/my_plugin"));
    }

    #[test]
    fn serialize_roundtrip_preserves_camel_case() {
        let manifest = PluginManifest::from_json(GOOD_JSON).unwrap();
        let json = serde_json::to_value(&manifest).unwrap();
        assert!(json.get("apiVersion").is_some());
        assert!(json.get("insertAfter").is_none()); // nested struct not flattened
        assert_eq!(json["kind"], "dsp");
    }
}

#[test]
fn dependency_validation_accepts_valid_deps() {
    let m = serde_json::from_str::<PluginManifest>(
        r#"{"id":"a.b.c","name":"C","version":"1.0.0","runtime":"wasm",
            "entry":"x.wasm","apiVersion":1,
            "dependencies":[{"id":"a.b.dep","version":"^1.0.0"}]}"#,
    )
    .expect("parse");
    m.validate().expect("valid");
}

#[test]
fn dependency_validation_rejects_bad_id() {
    let m = serde_json::from_str::<PluginManifest>(
        r#"{"id":"a.b.c","name":"C","version":"1.0.0","runtime":"wasm",
            "entry":"x.wasm","apiVersion":1,
            "dependencies":[{"id":"NO_DOTS"}]}"#,
    )
    .expect("parse");
    assert!(m.validate().is_err());
}

#[test]
fn dependency_validation_rejects_bad_version_req() {
    let m = serde_json::from_str::<PluginManifest>(
        r#"{"id":"a.b.c","name":"C","version":"1.0.0","runtime":"wasm",
            "entry":"x.wasm","apiVersion":1,
            "dependencies":[{"id":"a.b.dep","version":"not-a-version"}]}"#,
    )
    .expect("parse");
    assert!(m.validate().is_err());
}

#[test]
fn config_schema_validation_accepts_valid_schema() {
    let m = serde_json::from_str::<PluginManifest>(
        r#"{"id":"a.b.c","name":"C","version":"1.0.0","runtime":"wasm",
            "entry":"x.wasm","apiVersion":1,
            "configSchema":{"fields":[
                {"key":"gain","fieldType":"number","min":0,"max":10,"step":0.1,"default":1},
                {"key":"enabled","fieldType":"boolean","default":true},
                {"key":"mode","fieldType":"select","options":[{"value":"a","label":"A"}]}
            ]}}"#,
    )
    .expect("parse");
    m.validate().expect("valid");
}

#[test]
fn config_schema_validation_rejects_unknown_type() {
    let m = serde_json::from_str::<PluginManifest>(
        r#"{"id":"a.b.c","name":"C","version":"1.0.0","runtime":"wasm",
            "entry":"x.wasm","apiVersion":1,
            "configSchema":{"fields":[{"key":"x","fieldType":"color"}]}}"#,
    )
    .expect("parse");
    assert!(m.validate().is_err());
}

#[test]
fn config_schema_validation_rejects_select_without_options() {
    let m = serde_json::from_str::<PluginManifest>(
        r#"{"id":"a.b.c","name":"C","version":"1.0.0","runtime":"wasm",
            "entry":"x.wasm","apiVersion":1,
            "configSchema":{"fields":[{"key":"mode","fieldType":"select"}]}}"#,
    )
    .expect("parse");
    assert!(m.validate().is_err());
}
