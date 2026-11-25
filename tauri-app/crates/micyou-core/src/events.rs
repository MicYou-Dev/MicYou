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

use crate::stats::{AudioMetrics, BatteryStatus};
use crate::transport::tcp::DeviceInfo;
use std::sync::Arc;

/// Events emitted by the audio server core.
///
/// Each frontend supplies its own sink: the GUI forwards them to the webview,
/// the CLI writes log lines and the TUI updates its state. The core never
/// talks to a UI toolkit directly.
pub trait ServerEvents: Send + Sync + 'static {
    fn device_connected(&self, info: DeviceInfo);
    fn device_disconnected(&self);
    /// Battery reported by the connected phone. `None` clears it (device
    /// disconnected, or the phone could not read its charge).
    fn client_battery(&self, battery: Option<BatteryStatus>);
    fn audio_metrics(&self, metrics: AudioMetrics);
    fn udp_audio_warning(&self);
    fn mute_state_changed(&self, is_muted: bool);
    fn audio_level(&self, level: u32);
    fn audio_spectrum(&self, spectrum: SpectrumPayload);
    fn server_stopped(&self);
    fn web_client_count(&self, count: u32);
    fn install_progress(&self, message: String);
    fn aec_status_changed(&self, status: AecStatus);
    fn monitoring_state_changed(&self, enabled: bool);
    fn plugin_download_progress(&self, progress: DownloadProgress);
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct AecStatus {
    pub available: bool,
    pub enabled: bool,
    pub reason: Option<micyou_audio::AecFailure>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct SpectrumPayload {
    pub raw: Vec<f32>,
    pub processed: Vec<f32>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct DownloadProgress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
    pub done: bool,
}

pub type SharedEvents = Arc<dyn ServerEvents>;
