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

//! Unified plugin abstraction.
//!
//! Both runtimes (native cdylib and WASM) implement the same logical plugin
//! contract. A loaded plugin is either a `NativePlugin` or a `WasmPlugin`;
//! callers go through `PluginInstance`, so the audio engine, the bus and the
//! GUI never care which runtime backs a plugin.

use crate::error::{PluginError, PluginResult};
use crate::host::HostApi;
use crate::manifest::{PluginKind, PluginManifest, RuntimeKind};
use serde::{Deserialize, Serialize};

/// A plugin's lifecycle state, persisted by the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    /// Installed but not started by the manager.
    #[default]
    Disabled,
    /// Started and receiving runtime calls.
    Enabled,
}

impl PluginState {
    pub fn is_enabled(&self) -> bool {
        matches!(self, PluginState::Enabled)
    }
}

/// Events delivered to plugins (other than audio frames and cross-device
/// messages, which have dedicated entry points).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginEvent {
    /// A device (phone/web client) connected or disconnected.
    DeviceConnected {
        mode: String,
        label: String,
    },
    DeviceDisconnected,
    /// Mute state changed by the user.
    MuteChanged {
        muted: bool,
    },
    /// Monitoring (ear-return) state changed.
    MonitoringChanged {
        enabled: bool,
    },
    /// The DSP settings changed (chain reordered, a node toggled).
    DspSettingsChanged,
    /// The plugin was enabled or disabled while the host kept running.
    StateChanged {
        enabled: bool,
    },
}

/// An audio frame handed to a DSP plugin node.
/// The plugin may mutate `data` in place (frame size is host-defined).
#[derive(Debug)]
pub struct AudioFrameCtx<'a> {
    pub data: &'a mut Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
    pub queued_ms: f64,
    /// 0-based occurrence of this plugin's chain node among entries with the
    /// same id. `Some` only when the host invokes
    /// `micyou_plugin_process_node`; a single-node host leaves it `None`.
    pub node_index: Option<usize>,
}

/// What a plugin did with an audio frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    /// Output replaced input (normal).
    Ok,
    /// Plugin is not ready yet; host keeps the input untouched.
    Bypass,
}

/// The runtime-agnostic contract implemented by native and WASM plugins.
pub trait PluginRuntime: Send {
    fn manifest(&self) -> &PluginManifest;

    /// Called once after loading, before any audio/event traffic.
    fn init(&mut self, host: &dyn HostApi) -> PluginResult<()>;

    /// Called on unload / host shutdown.
    fn deinit(&mut self);

    /// Called for every audio frame when the plugin is a DSP node.
    fn process_audio(&mut self, ctx: &mut AudioFrameCtx<'_>) -> PluginResult<ProcessStatus>;

    /// Called for local bus events the plugin subscribed to.
    fn handle_event(&mut self, event: &PluginEvent) -> PluginResult<()>;

    /// Called for messages addressed to this plugin (local or remote).
    fn handle_message(&mut self, source: &str, topic: &str, payload: &[u8]) -> PluginResult<()>;
}

/// A loaded plugin instance (either runtime). `PluginInstance` derefs to the
/// runtime trait so callers can invoke it uniformly.
pub enum PluginInstance {
    Native(Box<dyn PluginRuntime>),
    Wasm(Box<dyn PluginRuntime>),
}

impl PluginInstance {
    pub fn manifest(&self) -> &PluginManifest {
        match self {
            PluginInstance::Native(p) => p.manifest(),
            PluginInstance::Wasm(p) => p.manifest(),
        }
    }

    pub fn runtime_kind(&self) -> RuntimeKind {
        self.manifest().runtime
    }

    pub fn kind(&self) -> PluginKind {
        self.manifest().kind
    }

    pub fn id(&self) -> &str {
        &self.manifest().id
    }

    pub fn runtime(&mut self) -> &mut dyn PluginRuntime {
        match self {
            PluginInstance::Native(p) => p.as_mut(),
            PluginInstance::Wasm(p) => p.as_mut(),
        }
    }
}

impl PluginRuntime for PluginInstance {
    fn manifest(&self) -> &PluginManifest {
        PluginInstance::manifest(self)
    }

    fn init(&mut self, host: &dyn HostApi) -> PluginResult<()> {
        self.runtime().init(host)
    }

    fn deinit(&mut self) {
        self.runtime().deinit();
    }

    fn process_audio(&mut self, ctx: &mut AudioFrameCtx<'_>) -> PluginResult<ProcessStatus> {
        self.runtime().process_audio(ctx)
    }

    fn handle_event(&mut self, event: &PluginEvent) -> PluginResult<()> {
        self.runtime().handle_event(event)
    }

    fn handle_message(&mut self, source: &str, topic: &str, payload: &[u8]) -> PluginResult<()> {
        self.runtime().handle_message(source, topic, payload)
    }
}

/// Helper: capability checks shared by the native and WASM bridges.
pub fn require_capability(manifest: &PluginManifest, capability: &str) -> PluginResult<()> {
    if manifest.capabilities.iter().any(|c| c == capability) {
        Ok(())
    } else {
        Err(PluginError::PermissionDenied(format!(
            "plugin {} lacks capability {capability}",
            manifest.id
        )))
    }
}

/// Helper: default topic for a plugin's cross-device messages.
pub fn message_topic(plugin_id: &str) -> String {
    format!("plugin:{plugin_id}")
}

pub use crate::host::MessageTarget as _MessageTargetReexport;
