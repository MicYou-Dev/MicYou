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

//! Forwards core server events to the webview.

use micyou_core::events::{AecStatus, DownloadProgress, ServerEvents, SpectrumPayload};
use micyou_core::stats::AudioMetrics;
use micyou_core::transport::tcp::DeviceInfo;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub struct TauriEventSink(pub AppHandle);

impl TauriEventSink {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Err(e) = self.0.emit(event, payload) {
            log::warn!("failed to emit {event}: {e}");
        }
    }
}

impl ServerEvents for TauriEventSink {
    fn device_connected(&self, info: DeviceInfo) {
        self.emit("device-connected", info);
    }
    fn device_disconnected(&self) {
        self.emit("device-disconnected", ());
    }
    fn audio_metrics(&self, metrics: AudioMetrics) {
        self.emit("audio-metrics", metrics);
    }
    fn udp_audio_warning(&self) {
        self.emit("udp_audio_warning", ());
    }
    fn mute_state_changed(&self, is_muted: bool) {
        self.emit("mute-state-changed", is_muted);
    }
    fn audio_level(&self, level: u32) {
        self.emit("audio-level", level);
    }
    fn audio_spectrum(&self, spectrum: SpectrumPayload) {
        // High-rate payload: only the main window draws the spectrum.
        if let Some(main) = self.0.get_webview_window("main") {
            if let Err(e) = main.emit("audio-spectrum", spectrum) {
                log::warn!("failed to emit audio-spectrum: {e}");
            }
        }
    }
    fn server_stopped(&self) {
        self.emit("server-stopped", ());
    }
    fn web_client_count(&self, count: u32) {
        self.emit("web-client-count", count);
    }
    fn install_progress(&self, message: String) {
        self.emit("virtual-audio-install-progress", message);
    }
    fn aec_status_changed(&self, status: AecStatus) {
        self.emit("aec-status-changed", status);
    }
    fn monitoring_state_changed(&self, enabled: bool) {
        self.emit("monitoring-enabled-changed", enabled);
    }
    fn plugin_download_progress(&self, progress: DownloadProgress) {
        self.emit("plugin-download-progress", progress);
    }
}
