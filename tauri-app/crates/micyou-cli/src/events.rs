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

use micyou_core::events::{AecStatus, DownloadProgress, ServerEvents, SpectrumPayload};
use micyou_core::stats::{AudioMetrics, BatteryStatus};
use micyou_core::transport::tcp::DeviceInfo;

/// Log-mode events: print a compact line per event.
pub struct CliEventSink {
    /// If true, suppresses the frequent per-second logging (audio levels and
    /// [stats] network metrics) to prevent terminal spam.
    quiet: bool,
}

impl CliEventSink {
    /// Create a new CLI event sink.
    pub fn new(quiet: bool) -> Self {
        Self { quiet }
    }
}

impl ServerEvents for CliEventSink {
    fn device_connected(&self, info: DeviceInfo) {
        println!("[mic] connected: {} ({})", info.name, info.ip);
    }

    fn device_disconnected(&self) {
        println!("[mic] disconnected");
    }

    fn client_battery(&self, battery: Option<BatteryStatus>) {
        // Only the reported state is worth a line; clearing on disconnect is
        // already covered by [mic] disconnected.
        if let Some(battery) = battery {
            println!(
                "[mic] battery {}%{}",
                battery.level,
                if battery.charging { " (charging)" } else { "" }
            );
        }
    }

    fn audio_metrics(&self, metrics: AudioMetrics) {
        // 每秒触发一次的高频统计：静默模式（-q）下与 [level] 一并屏蔽
        if self.quiet {
            return;
        }
        println!(
            "[stats] latency {} ms (network {} ms) buffer {} ms jitter {:.1} ms loss {:.2}%",
            metrics.latency_ms,
            metrics.network_latency_ms,
            metrics.buffer_duration_ms,
            metrics.jitter_ms,
            metrics.packet_loss_rate * 100.0
        );
    }

    fn udp_audio_warning(&self) {
        println!("[warn] no UDP audio for a while - check network connection");
    }

    fn mute_state_changed(&self, is_muted: bool) {
        println!("[mic] muted: {is_muted}");
    }

    fn audio_level(&self, level: u32) {
        // 仅在非静默模式下才打印音频电平
        if !self.quiet {
            println!("[level] {level}");
        }
    }

    fn audio_spectrum(&self, _spectrum: SpectrumPayload) {}

    fn server_stopped(&self) {
        println!("[server] stopped");
    }

    fn web_client_count(&self, count: u32) {
        println!("[web] clients: {count}");
    }

    fn install_progress(&self, message: String) {
        println!("[install] {message}");
    }

    fn aec_status_changed(&self, status: AecStatus) {
        if status.available && status.enabled {
            println!("[aec] enabled");
        } else if let Some(reason) = status.reason {
            println!("[warn] AEC disabled: {reason}");
        }
    }

    fn monitoring_state_changed(&self, enabled: bool) {
        println!("[monitor] {}", if enabled { "on" } else { "off" });
    }

    fn plugin_download_progress(&self, progress: DownloadProgress) {
        if progress.done {
            println!("[plugins] downloaded {} ({} bytes)", progress.id, progress.downloaded);
        }
    }
}
