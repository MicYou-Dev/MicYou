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

//! Runtime controls (mute, monitoring, DSP settings) shared by the frontends
//! and by the plugin host API, so every caller gets the same validation,
//! persistence and notifications.

use crate::events::SharedEvents;
use crate::plugins::PluginHost;
use crate::server::output::AudioOutputHandle;
use crate::server::ServerState;
use crate::stats::NetworkStats;
use crate::transport::tcp::SharedActiveConnection;
use crate::transport::udp::ActiveAudioSession;
use cpal::traits::HostTrait;
use micyou_audio::dsp::AudioDspSettings;
use micyou_plugin::PluginEvent;
use micyou_protocol::micyou::{MessageWrapper, MuteMessage};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// Names of the output devices cpal can open, sorted and deduplicated.
pub fn audio_devices() -> Vec<String> {
    let mut names: Vec<String> = match cpal::default_host().output_devices() {
        Ok(devices) => devices.filter_map(|dev| micyou_audio::device_name(&dev)).collect(),
        Err(e) => {
            log::warn!("[Audio] listing output devices failed: {e}");
            Vec::new()
        }
    };
    names.sort();
    names.dedup();
    names
}

#[derive(Serialize)]
pub struct PipeWireStatus {
    pub available: bool,
    pub setup: bool,
    pub device_exists: bool,
    pub install_command: String,
    pub distro: String,
}

pub fn pipewire_status() -> PipeWireStatus {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::pipewire;
        let (distro, install_command) = pipewire::detect_install_info();
        PipeWireStatus {
            available: pipewire::is_available(),
            setup: pipewire::is_setup(),
            device_exists: pipewire::device_exists(),
            install_command,
            distro,
        }
    }
    #[cfg(not(target_os = "linux"))]
    PipeWireStatus {
        available: false,
        setup: false,
        device_exists: false,
        install_command: String::new(),
        distro: String::new(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingStatus {
    pub is_server_running: bool,
    pub is_connected: bool,
    pub is_muted: bool,
    pub platform: crate::transport::tcp::ClientPlatform,
}

/// Handles on the live server state that runtime controls act on.
#[derive(Clone)]
pub struct Controls {
    pub(crate) events: SharedEvents,
    pub(crate) stats: Arc<NetworkStats>,
    pub(crate) active_connection: SharedActiveConnection,
    pub(crate) plugins: Arc<PluginHost>,
    pub(crate) is_monitoring: Arc<AtomicBool>,
    pub(crate) audio_output: Arc<AudioOutputHandle>,
    pub(crate) dsp_settings: Arc<RwLock<AudioDspSettings>>,
}

impl Controls {
    pub fn is_muted(&self) -> bool {
        self.stats.is_muted()
    }

    /// Mute locally and, unless mute sync is disabled in server.json, tell
    /// the connected phone.
    pub fn set_muted(&self, muted: bool) {
        self.stats.set_muted(muted);
        self.events.mute_state_changed(muted);
        self.plugins
            .broadcast_event(&PluginEvent::MuteChanged { muted });
        if !crate::config::load_server_prefs().mute_sync {
            return;
        }
        self.send_mute_to_phone(muted);
    }

    /// Push the mute state to the connected phone without dropping it when
    /// the connection lock is briefly held by the TCP handler. Inside a tokio
    /// runtime the send runs as a task that reads the state at send time, so
    /// rapid toggles still converge on the latest value; elsewhere (the GUI
    /// main thread, plugin threads) it waits for the lock directly.
    fn send_mute_to_phone(&self, muted: bool) {
        fn message(muted: bool) -> MessageWrapper {
            MessageWrapper {
                mute: Some(MuteMessage {
                    is_muted: Some(muted),
                }),
                ..Default::default()
            }
        }

        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let stats = self.stats.clone();
            let active_connection = self.active_connection.clone();
            runtime.spawn(async move {
                let sender = active_connection
                    .lock()
                    .await
                    .as_ref()
                    .map(|connection| connection.sender.clone());
                if let Some(sender) = sender {
                    if let Err(e) = sender.send(message(stats.is_muted())).await {
                        log::warn!("[Mute] failed to send mute state to the phone: {e}");
                    }
                }
            });
            return;
        }
        let connection = self.active_connection.blocking_lock();
        if let Some(connection) = connection.as_ref() {
            if let Err(e) = connection.sender.try_send(message(muted)) {
                log::warn!("[Mute] failed to send mute state to the phone: {e}");
            }
        }
    }

    pub fn is_monitoring(&self) -> bool {
        self.is_monitoring.load(Ordering::Relaxed)
    }

    pub fn set_monitoring(&self, enabled: bool) {
        self.is_monitoring.store(enabled, Ordering::Relaxed);
        self.audio_output.set_monitoring(enabled);
        self.events.monitoring_state_changed(enabled);
        self.plugins
            .broadcast_event(&PluginEvent::MonitoringChanged { enabled });
    }

    pub fn dsp_settings(&self) -> AudioDspSettings {
        self.dsp_settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Validate, persist to settings.json and apply a full settings value.
    pub fn apply_dsp_settings(&self, mut settings: AudioDspSettings) -> Result<(), String> {
        settings.normalize();
        // AEC must always run first in the processing chain.
        if let Some(pos) = settings.processing_chain.iter().position(|s| s == "AEC") {
            let stage = settings.processing_chain.remove(pos);
            settings.processing_chain.insert(0, stage);
        }
        // Keep the per-plugin chain nodes in sync with the live registry
        // (#347): a full write must neither drop active nor keep stale nodes.
        self.plugins.reconcile_settings_chain(&mut settings);

        let mut current = self
            .dsp_settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if settings.aec_enabled && !current.aec_enabled {
            if let Some(reason) = micyou_audio::aec_reference_availability().reason {
                return Err(reason.as_str().to_string());
            }
        }
        crate::config::save_dsp_settings(&settings)
            .map_err(|e| format!("Failed to persist settings: {e}"))?;
        *current = settings;
        drop(current);
        self.plugins.broadcast_event(&PluginEvent::DspSettingsChanged);
        Ok(())
    }

    /// Apply a partial JSON object (or a complete settings object) on top of
    /// the current settings.
    pub fn patch_dsp_settings(&self, json: &str) -> Result<(), String> {
        let mut value = serde_json::to_value(self.dsp_settings())
            .map_err(|e| format!("serialize current dsp settings: {e}"))?;
        let patch: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(json).map_err(|e| format!("invalid dsp json: {e}"))?;
        let object = value
            .as_object_mut()
            .expect("AudioDspSettings serializes to a JSON object");
        object.extend(patch);
        let updated = serde_json::from_value(value)
            .map_err(|e| format!("invalid dsp settings: {e}"))?;
        self.apply_dsp_settings(updated)
    }
}

impl ServerState {
    pub fn controls(&self) -> Controls {
        Controls {
            events: self.events.clone(),
            stats: self.network_stats.clone(),
            active_connection: self.active_connection.clone(),
            plugins: self.plugins.clone(),
            is_monitoring: self.is_monitoring.clone(),
            audio_output: self.audio_output.clone(),
            dsp_settings: self.dsp_settings.clone(),
        }
    }

    pub fn set_spectrum_streaming(&self, enabled: bool) {
        self.spectrum_streaming_enabled
            .store(enabled, Ordering::Release);
    }

    /// Current DSP settings. Reads the shared settings.json when present so
    /// edits made by another frontend show up.
    pub fn current_dsp_settings(&self) -> AudioDspSettings {
        if crate::config::settings_path().exists() {
            crate::config::load_dsp_settings()
        } else {
            self.controls().dsp_settings()
        }
    }

    pub async fn streaming_status(&self) -> StreamingStatus {
        let is_server_running = matches!(
            self.lifecycle.lock().await.phase(),
            crate::server::lifecycle::ServerLifecyclePhase::Running
        );
        let control_platform = self
            .active_connection
            .lock()
            .await
            .as_ref()
            .map(|connection| connection.platform);
        let audio_active = !matches!(
            *self
                .active_audio_session
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            ActiveAudioSession::Inactive
        );
        #[cfg(feature = "web-server")]
        let web_connected = self
            .web_server
            .lock()
            .await
            .as_ref()
            .is_some_and(|web| web.client_count() > 0);
        #[cfg(not(feature = "web-server"))]
        let web_connected = false;

        StreamingStatus {
            is_server_running,
            is_connected: control_platform.is_some() || audio_active || web_connected,
            is_muted: self.network_stats.is_muted(),
            platform: control_platform.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::events::{AecStatus, DownloadProgress, ServerEvents, SpectrumPayload};
    use crate::host::{HostIntegration, HotkeyCallback};
    use crate::server::ServerState;
    use crate::stats::AudioMetrics;
    use crate::transport::tcp::{ActiveConnection, DeviceInfo};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    struct NoEvents;

    impl ServerEvents for NoEvents {
        fn device_connected(&self, _: DeviceInfo) {}
        fn device_disconnected(&self) {}
        fn audio_metrics(&self, _: AudioMetrics) {}
        fn udp_audio_warning(&self) {}
        fn mute_state_changed(&self, _: bool) {}
        fn audio_level(&self, _: u32) {}
        fn audio_spectrum(&self, _: SpectrumPayload) {}
        fn server_stopped(&self) {}
        fn web_client_count(&self, _: u32) {}
        fn install_progress(&self, _: String) {}
        fn aec_status_changed(&self, _: AecStatus) {}
        fn monitoring_state_changed(&self, _: bool) {}
        fn plugin_download_progress(&self, _: DownloadProgress) {}
    }

    struct NoHost;

    impl HostIntegration for NoHost {
        fn open_url(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn notify(&self, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn register_hotkey(&self, _: &str, _: HotkeyCallback) -> Result<(), String> {
            Ok(())
        }
        fn open_plugin_panel(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn mute_reaches_the_phone_while_the_connection_lock_is_busy() {
        let state = ServerState::new(Arc::new(NoEvents), Arc::new(NoHost), None);
        let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
        *state.active_connection.lock().await = Some(ActiveConnection {
            sender,
            raw_socket: Default::default(),
            connection_id: 1,
            takeover_token: CancellationToken::new(),
            platform: Default::default(),
        });

        let busy = state.active_connection.lock().await;
        let controls = state.controls();
        controls.stats.set_muted(true);
        controls.send_mute_to_phone(true);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(receiver.try_recv().is_err(), "sent while the lock was held");
        drop(busy);

        let message = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("mute state was dropped")
            .expect("channel closed");
        assert_eq!(message.mute.and_then(|m| m.is_muted), Some(true));
    }
}
