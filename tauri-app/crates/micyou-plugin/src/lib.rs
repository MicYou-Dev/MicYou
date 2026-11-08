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

//! MicYou plugin framework.
//!
//! Unified plugin abstraction with a dual runtime:
//! - **Native** plugins: platform cdylibs (`.so` / `.dylib` / `.dll`) loaded
//!   through a versioned C ABI, for real-time DSP and deep system integration.
//! - **WASM** plugins: WebAssembly modules executed in a sandboxed interpreter,
//!   for utilities, UI logic and lightweight processing.
//!
//! The manifest, Host API contract and cross-device message protocol are
//! platform-neutral so the same plugin works on the desktop app, the CLI/TUI
//! and (in a future phase) Android with a different loading implementation.

pub mod abi;
pub mod bus;
pub mod dsp;
pub mod error;
pub mod host;
pub mod manager;
pub mod manifest;
pub mod native;
pub mod plugin;
pub mod sync;
pub mod wasm;

pub use bus::{error_code, error_message_for, PluginBus, PluginMessage, PluginSyncTransport};
pub use dsp::{DspNode, PluginDspBridge, PluginDspRegistry};
pub use error::{PluginError, PluginResult};
pub use host::{sandbox_path, AudioStateSnapshot, DeviceSnapshot, HostApi, MessageTarget, PluginLogLevel};
pub use manager::{PluginEntry, PluginManager, PluginPersistedState, ScanReport};
pub use manifest::{
    capabilities, platforms, ConfigField, ConfigSchema, DspDescriptor, PluginDependency, PluginKind,
    PluginManifest, UiPanel, RuntimeKind, UiDescriptor, HOST_API_VERSION, HOST_VERSION,
    KNOWN_CAPABILITIES, MANIFEST_FILE_NAME,
};
pub use plugin::{
    message_topic, require_capability, AudioFrameCtx, PluginEvent, PluginInstance, PluginRuntime,
    PluginState, ProcessStatus,
};
