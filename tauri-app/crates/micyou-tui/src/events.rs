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
use micyou_core::stats::AudioMetrics;
use micyou_core::transport::tcp::DeviceInfo;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

/// Server events consumed by the interactive terminal UI.
#[derive(Debug, Clone)]
pub enum Event {
    DeviceConnected(DeviceInfo),
    DeviceDisconnected,
    Metrics(AudioMetrics),
    UdpWarning,
    MuteChanged(bool),
    Level(u32),
    Spectrum(Vec<f32>, Vec<f32>),
    Stopped,
    WebClientCount(u32),
    InstallProgress(String),
    AecStatus(AecStatus),
    /// A `log` record already formatted with its level tag.
    Log(String),
}

/// Forward server events to the TUI channel while throttling high-frequency
/// audio visualization updates to the terminal's useful refresh rate.
pub struct TuiEventSink {
    tx: Sender<Event>,
    level: Throttle,
    spectrum: Throttle,
}

impl TuiEventSink {
    pub fn new(tx: Sender<Event>) -> Self {
        Self {
            tx,
            level: Throttle::default(),
            spectrum: Throttle::default(),
        }
    }
}

/// Lets one update through per interval; called from the audio thread, so
/// it never blocks.
struct Throttle {
    epoch: Instant,
    /// Milliseconds after `epoch` when the next update may pass.
    next_ms: AtomicU64,
}

impl Default for Throttle {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            next_ms: AtomicU64::new(0),
        }
    }
}

impl Throttle {
    const INTERVAL_MS: u64 = 70;

    fn ready(&self) -> bool {
        let now = self.epoch.elapsed().as_millis() as u64;
        let next = self.next_ms.load(Ordering::Relaxed);
        now >= next
            && self
                .next_ms
                .compare_exchange(
                    next,
                    now + Self::INTERVAL_MS,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
    }
}

impl ServerEvents for TuiEventSink {
    fn device_connected(&self, info: DeviceInfo) {
        let _ = self.tx.send(Event::DeviceConnected(info));
    }

    fn device_disconnected(&self) {
        let _ = self.tx.send(Event::DeviceDisconnected);
    }

    fn audio_metrics(&self, metrics: AudioMetrics) {
        let _ = self.tx.send(Event::Metrics(metrics));
    }

    fn udp_audio_warning(&self) {
        let _ = self.tx.send(Event::UdpWarning);
    }

    fn mute_state_changed(&self, is_muted: bool) {
        let _ = self.tx.send(Event::MuteChanged(is_muted));
    }

    fn audio_level(&self, level: u32) {
        if self.level.ready() {
            let _ = self.tx.send(Event::Level(level));
        }
    }

    fn audio_spectrum(&self, spectrum: SpectrumPayload) {
        if self.spectrum.ready() {
            let _ = self
                .tx
                .send(Event::Spectrum(spectrum.raw, spectrum.processed));
        }
    }

    fn server_stopped(&self) {
        let _ = self.tx.send(Event::Stopped);
    }

    fn web_client_count(&self, count: u32) {
        let _ = self.tx.send(Event::WebClientCount(count));
    }

    fn install_progress(&self, message: String) {
        let _ = self.tx.send(Event::InstallProgress(message));
    }

    fn aec_status_changed(&self, status: AecStatus) {
        let _ = self.tx.send(Event::AecStatus(status));
    }

    fn monitoring_state_changed(&self, _enabled: bool) {}

    fn plugin_download_progress(&self, _progress: DownloadProgress) {}
}
