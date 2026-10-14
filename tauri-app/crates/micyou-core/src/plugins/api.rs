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

//! The [`HostApi`] each loaded plugin instance calls back into.

use super::{lock_err, PluginHost};
use crate::transport::udp::ActiveAudioSession;
use micyou_plugin::bus::PluginMessage;
use micyou_plugin::host::{
    AudioStateSnapshot, DeviceSnapshot, HostApi, MessageTarget, PluginLogLevel,
};
use micyou_plugin::{PluginError, PluginResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub(super) struct PluginHostApi {
    host: Arc<PluginHost>,
    plugin_id: String,
    dir: PathBuf,
    next_id: AtomicU64,
    /// Cancellation flags of running timeouts and intervals.
    timers: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl PluginHostApi {
    pub(super) fn new(host: Arc<PluginHost>, plugin_id: String, dir: PathBuf) -> Self {
        Self {
            host,
            plugin_id,
            dir,
            next_id: AtomicU64::new(1),
            timers: Mutex::new(HashMap::new()),
        }
    }

    /// Deliver `<topic>` to the plugin after `ms`, once or every `ms`.
    fn start_timer(&self, ms: u64, payload: &str, repeat: bool) -> PluginResult<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.timers
            .lock()
            .map_err(lock_err)?
            .insert(id, cancel.clone());
        let bus = self.host.bus.clone();
        let plugin_id = self.plugin_id.clone();
        let (topic, key) = if repeat {
            ("interval:tick", "interval")
        } else {
            ("timer:expired", "timer")
        };
        let body = serde_json::json!({ key: id, "payload": payload })
            .to_string()
            .into_bytes();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(ms.max(1)));
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            bus.handle_incoming(&PluginMessage::new("host", &plugin_id, topic, body.clone()));
            if !repeat {
                return;
            }
        });
        Ok(id)
    }

    fn clear_timer(&self, id: u64) -> PluginResult<()> {
        if let Some(cancel) = self.timers.lock().map_err(lock_err)?.remove(&id) {
            cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

fn http_request_blocking(
    method: &str,
    url: &str,
    headers_json: &str,
    body: String,
) -> Result<(u16, String), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut request = client.request(method, url);
    if !headers_json.trim().is_empty() {
        let headers: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(headers_json).map_err(|e| format!("invalid headers json: {e}"))?;
        for (name, value) in headers {
            let value = value
                .as_str()
                .ok_or_else(|| format!("header {name} must be a string"))?;
            request = request.header(&name, value);
        }
    }
    if !body.is_empty() {
        request = request.body(body);
    }
    let response = request.send().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let text = response.text().map_err(|e| e.to_string())?;
    Ok((status, text))
}

impl HostApi for PluginHostApi {
    fn log(&self, level: PluginLogLevel, message: &str) {
        self.host.logs.push(&self.plugin_id, level, message);
        log::info!(target: "plugin", "[{}] {}", self.plugin_id, message);
    }

    fn get_config(&self, key: &str) -> Option<serde_json::Value> {
        let manager = self.host.manager.lock().ok()?;
        manager.plugin_config(&self.plugin_id).ok()?.get(key).cloned()
    }

    fn set_config(&self, key: &str, value: serde_json::Value) -> PluginResult<()> {
        self.host
            .manager
            .lock()
            .map_err(lock_err)?
            .set_plugin_config(&self.plugin_id, key, value)
    }

    fn emit_event(&self, topic: &str, payload: serde_json::Value) -> PluginResult<()> {
        let bytes = serde_json::to_vec(&payload)
            .map_err(|e| PluginError::Runtime(format!("event serialization: {e}")))?;
        self.host.bus.publish(topic, bytes)
    }

    fn send_message(&self, target: MessageTarget, payload: Vec<u8>) -> PluginResult<()> {
        let bus = &self.host.bus;
        match target {
            MessageTarget::Local { plugin_id } => {
                let msg = PluginMessage::new(&self.plugin_id, &plugin_id, &plugin_id, payload);
                bus.handle_incoming(&msg);
                Ok(())
            }
            MessageTarget::Remote { plugin_id } => {
                let msg = PluginMessage::new(&self.plugin_id, &plugin_id, &plugin_id, payload);
                bus.transport().send(&msg)
            }
            MessageTarget::Broadcast => {
                let msg = PluginMessage::new(&self.plugin_id, "", "broadcast", payload);
                bus.handle_incoming(&msg);
                if bus.transport().is_connected() {
                    bus.transport().send(&msg)?;
                }
                Ok(())
            }
        }
    }

    fn audio_state(&self) -> AudioStateSnapshot {
        let server = &self.host.server;
        // try_lock: plugins call this from the audio thread and must never
        // block on a lifecycle transition.
        let is_server_running = server.lifecycle.try_lock().is_ok_and(|lifecycle| {
            matches!(
                lifecycle.phase(),
                crate::server::lifecycle::ServerLifecyclePhase::Running
            )
        });
        let control_connected = server
            .active_connection
            .try_lock()
            .is_ok_and(|connection| connection.is_some());
        let audio_active = server
            .active_audio_session
            .read()
            .is_ok_and(|session| !matches!(*session, ActiveAudioSession::Inactive));
        #[cfg(feature = "web-server")]
        let web_connected = server
            .web_server
            .try_lock()
            .is_ok_and(|web| web.as_ref().is_some_and(|w| w.client_count() > 0));
        #[cfg(not(feature = "web-server"))]
        let web_connected = false;

        let stats = &server.network_stats;
        let channels = stats.channels.load(Ordering::Relaxed);
        let queued_ms = server.audio_output.queued_ms();
        AudioStateSnapshot {
            streaming: is_server_running && (control_connected || audio_active || web_connected),
            sample_rate: stats.sample_rate.load(Ordering::Relaxed),
            channels,
            input_level: f32::from_bits(stats.input_level_bits.load(Ordering::Relaxed)),
            processed_level: f32::from_bits(stats.processed_level_bits.load(Ordering::Relaxed)),
            queued_ms,
            muted: stats.is_muted(),
        }
    }

    fn plugin_dir(&self) -> String {
        self.dir.display().to_string()
    }

    fn register_hotkey(&self, shortcut: &str) -> PluginResult<u64> {
        self.host.hotkeys.register(&self.plugin_id, shortcut)
    }

    fn open_window(&self, panel_id: &str) -> PluginResult<()> {
        let title = {
            let manager = self.host.manager.lock().map_err(lock_err)?;
            let entry = manager
                .entry(&self.plugin_id)?
                .ok_or_else(|| PluginError::UnknownPlugin(self.plugin_id.clone()))?;
            let panel = entry
                .manifest
                .ui
                .as_ref()
                .and_then(|ui| ui.panels.iter().find(|p| p.id == panel_id))
                .ok_or_else(|| PluginError::Validation(format!("unknown panel {panel_id}")))?;
            format!("{} · {}", entry.manifest.name, panel.label)
        };
        self.host
            .host
            .open_plugin_panel(&self.plugin_id, panel_id, &title)
            .map_err(PluginError::Runtime)
    }

    fn play_sound(&self, path: &str) -> PluginResult<()> {
        let full = if std::path::Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.dir.join(path)
        };
        self.host.sound.play_wav(&full.display().to_string())
    }

    fn fs_read(&self, path: &str) -> PluginResult<String> {
        let full = micyou_plugin::sandbox_path(&self.dir, path)?;
        std::fs::read_to_string(&full).map_err(PluginError::from)
    }

    fn fs_write(&self, path: &str, content: &str) -> PluginResult<()> {
        let full = micyou_plugin::sandbox_path(&self.dir, path)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PluginError::Runtime(format!("fs_write mkdir: {e}")))?;
        }
        std::fs::write(&full, content).map_err(PluginError::from)
    }

    fn set_timeout(&self, ms: u64, payload: &str) -> PluginResult<u64> {
        self.start_timer(ms, payload, false)
    }

    fn clear_timeout(&self, id: u64) -> PluginResult<()> {
        self.clear_timer(id)
    }

    fn set_interval(&self, ms: u64, payload: &str) -> PluginResult<u64> {
        self.start_timer(ms, payload, true)
    }

    fn clear_interval(&self, id: u64) -> PluginResult<()> {
        self.clear_timer(id)
    }

    fn http_request(
        &self,
        method: &str,
        url: &str,
        headers_json: &str,
        body: &str,
    ) -> PluginResult<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let method = method.to_string();
        let url = url.to_string();
        let headers_json = headers_json.to_string();
        let body = body.to_string();
        let bus = self.host.bus.clone();
        let plugin_id = self.plugin_id.clone();
        std::thread::spawn(move || {
            let payload = match http_request_blocking(&method, &url, &headers_json, body) {
                Ok((status, text)) => serde_json::json!({
                    "request": id, "ok": true, "status": status, "body": text, "error": null
                }),
                Err(e) => serde_json::json!({
                    "request": id, "ok": false, "status": 0, "body": "", "error": e
                }),
            };
            let msg = PluginMessage::new(
                "host",
                &plugin_id,
                "http:response",
                payload.to_string().into_bytes(),
            );
            bus.handle_incoming(&msg);
        });
        Ok(id)
    }

    fn open_url(&self, url: &str) -> PluginResult<()> {
        self.host.host.open_url(url).map_err(PluginError::Runtime)
    }

    fn notify(&self, title: &str, body: &str) -> PluginResult<()> {
        self.host.host.notify(title, body).map_err(PluginError::Runtime)
    }

    fn locale(&self) -> String {
        crate::config::load_ui_prefs().language
    }

    fn host_info(&self) -> String {
        serde_json::json!({
            "name": "micyou",
            "version": env!("CARGO_PKG_VERSION"),
            "apiVersion": micyou_plugin::manifest::HOST_API_VERSION,
        })
        .to_string()
    }

    fn clipboard_read(&self) -> PluginResult<String> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|e| PluginError::Runtime(format!("clipboard read: {e}")))
    }

    fn clipboard_write(&self, text: &str) -> PluginResult<()> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
            .map_err(|e| PluginError::Runtime(format!("clipboard write: {e}")))
    }

    fn set_panel_icon(&self, panel_id: &str, icon: &str) -> PluginResult<()> {
        self.host
            .panel_icons
            .lock()
            .map_err(lock_err)?
            .entry(self.plugin_id.clone())
            .or_default()
            .insert(panel_id.to_string(), icon.to_string());
        Ok(())
    }

    fn get_muted(&self) -> PluginResult<bool> {
        Ok(self.host.controls()?.is_muted())
    }

    fn set_muted(&self, muted: bool) -> PluginResult<()> {
        self.host.controls()?.set_muted(muted);
        Ok(())
    }

    fn get_monitoring(&self) -> PluginResult<bool> {
        Ok(self.host.controls()?.is_monitoring())
    }

    fn set_monitoring(&self, enabled: bool) -> PluginResult<()> {
        self.host.controls()?.set_monitoring(enabled);
        Ok(())
    }

    fn get_dsp_settings(&self) -> PluginResult<String> {
        serde_json::to_string(&self.host.controls()?.dsp_settings())
            .map_err(|e| PluginError::Runtime(format!("dsp serialize error: {e}")))
    }

    fn set_dsp_settings(&self, settings_json: &str) -> PluginResult<()> {
        self.host
            .controls()?
            .patch_dsp_settings(settings_json)
            .map_err(PluginError::Validation)
    }

    fn connected_devices(&self) -> Vec<DeviceSnapshot> {
        let server = &self.host.server;
        let mut devices = Vec::new();
        if self.host.bus.transport().is_connected() {
            let session = server
                .active_audio_session
                .read()
                .map(|session| *session)
                .unwrap_or(ActiveAudioSession::Inactive);
            let device = match session {
                ActiveAudioSession::Bound { peer_ip, .. }
                | ActiveAudioSession::UnboundLegacy { peer_ip, .. } => {
                    let mode = if peer_ip.is_loopback() { "usb" } else { "wifi" };
                    DeviceSnapshot {
                        mode: mode.to_string(),
                        label: peer_ip.to_string(),
                        audio_active: true,
                    }
                }
                ActiveAudioSession::Inactive => DeviceSnapshot {
                    mode: "wifi".to_string(),
                    label: "pending...".to_string(),
                    audio_active: false,
                },
            };
            devices.push(device);
        }

        #[cfg(feature = "web-server")]
        if let Ok(web) = server.web_server.try_lock() {
            let count = web.as_ref().map_or(0, |w| w.client_count());
            devices.extend((0..count).map(|_| DeviceSnapshot {
                mode: "web".to_string(),
                label: "web client".to_string(),
                audio_active: true,
            }));
        }
        devices
    }
}
