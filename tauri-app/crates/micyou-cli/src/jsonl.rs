// Copyright (C) 2026 TimmySheep
// Third-party frontend JSON Lines control bridge, added 2026-10-08.
// SPDX-License-Identifier: GPL-3.0-or-later

use micyou_core::events::{AecStatus, ServerEvents, SpectrumPayload};
use micyou_core::micyou_audio::dsp::AudioDspSettings;
use micyou_core::server::ServerState;
use micyou_core::stats::AudioMetrics;
use micyou_core::transport::tcp::DeviceInfo;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Instant;

const MAX_LINE_BYTES: usize = 64 * 1024;
const EVENT_QUEUE_CAPACITY: usize = 128;

/// A bounded, single-writer stdout channel. Best-effort telemetry uses
/// `try_send` so a slow UI never blocks audio telemetry callbacks.
#[derive(Clone)]
pub struct Output {
    sender: SyncSender<Value>,
}

impl Output {
    pub fn start() -> (Self, JoinHandle<()>) {
        let (sender, receiver) = mpsc::sync_channel::<Value>(EVENT_QUEUE_CAPACITY);
        let writer = std::thread::Builder::new()
            .name("micyou-jsonl-stdout".to_string())
            .spawn(move || {
                let stdout = io::stdout();
                let mut stream = stdout.lock();
                while let Ok(frame) = receiver.recv() {
                    if serde_json::to_writer(&mut stream, &frame).is_err()
                        || stream.write_all(b"\n").is_err()
                        || stream.flush().is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to start JSONL stdout writer");
        (Self { sender }, writer)
    }

    /// Responses and lifecycle frames must not be silently discarded.
    pub fn send(&self, frame: Value) -> Result<(), String> {
        self.sender
            .send(frame)
            .map_err(|_| "JSONL output pipe is closed".to_string())
    }

    fn event(&self, frame: Value, best_effort: bool) {
        if best_effort {
            // A slow consumer must never block the audio telemetry callback.
            let _ = self.sender.try_send(frame);
        } else {
            // Preserve connection, mute, and lifecycle transitions.
            let _ = self.sender.send(frame);
        }
    }
}

pub fn write_standalone_error(message: &str) {
    let frame = json!({
        "v": 1,
        "type": "error",
        "error": { "code": "backend_start_failed", "message": message }
    });
    let stdout = io::stdout();
    let mut stream = stdout.lock();
    let _ = serde_json::to_writer(&mut stream, &frame);
    let _ = stream.write_all(b"\n");
    let _ = stream.flush();
}

pub struct JsonlEventSink {
    output: Output,
    gate: Mutex<PreReadyEvents>,
    epoch: Instant,
    last_metrics_ms: AtomicU64,
    last_level_ms: AtomicU64,
    last_spectrum_ms: AtomicU64,
}

struct PreReadyEvents {
    ready: bool,
    pending: VecDeque<(Value, bool)>,
}

impl JsonlEventSink {
    pub fn new(output: Output) -> Self {
        Self {
            output,
            gate: Mutex::new(PreReadyEvents {
                ready: false,
                pending: VecDeque::new(),
            }),
            epoch: Instant::now(),
            last_metrics_ms: AtomicU64::new(u64::MAX),
            last_level_ms: AtomicU64::new(u64::MAX),
            last_spectrum_ms: AtomicU64::new(u64::MAX),
        }
    }

    pub fn send_ready(&self, frame: Value) -> Result<(), String> {
        let mut gate = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.output.send(frame)?;
        while let Some((event, best_effort)) = gate.pending.pop_front() {
            self.output.event(event, best_effort);
        }
        gate.ready = true;
        Ok(())
    }

    pub fn output(&self) -> &Output {
        &self.output
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis().min(u64::MAX as u128) as u64
    }

    fn allow_rate(slot: &AtomicU64, now_ms: u64, interval_ms: u64) -> bool {
        let mut previous = slot.load(Ordering::Relaxed);
        loop {
            if previous != u64::MAX && now_ms.saturating_sub(previous) < interval_ms {
                return false;
            }
            match slot.compare_exchange_weak(previous, now_ms, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return true,
                Err(observed) => previous = observed,
            }
        }
    }

    fn emit(&self, name: &str, payload: Value, best_effort: bool) {
        let frame = json!({
            "v": 1,
            "type": "event",
            "name": name,
            "payload": payload,
        });
        let mut gate = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !gate.ready {
            if gate.pending.len() >= EVENT_QUEUE_CAPACITY {
                if best_effort {
                    return;
                }
                if let Some(index) = gate.pending.iter().position(|(_, telemetry)| *telemetry) {
                    gate.pending.remove(index);
                } else {
                    gate.pending.pop_front();
                }
            }
            gate.pending.push_back((frame, best_effort));
            return;
        }
        // Do not hold the gate while sending. Audio telemetry callbacks can
        // still acquire it even when a critical frame is back-pressured.
        drop(gate);
        self.output.event(frame, best_effort);
    }
}

impl ServerEvents for JsonlEventSink {
    fn device_connected(&self, info: DeviceInfo) {
        self.emit("device_connected", json!(info), false);
    }

    fn device_disconnected(&self) {
        self.emit("device_disconnected", json!({}), false);
    }

    fn audio_metrics(&self, metrics: AudioMetrics) {
        if Self::allow_rate(&self.last_metrics_ms, self.now_ms(), 500) {
            self.emit("audio_metrics", json!(metrics), true);
        }
    }

    fn udp_audio_warning(&self) {
        self.emit("udp_audio_warning", json!({}), false);
    }

    fn mute_state_changed(&self, is_muted: bool) {
        self.emit("mute_state_changed", json!({ "isMuted": is_muted }), false);
    }

    fn audio_level(&self, level: u32) {
        if Self::allow_rate(&self.last_level_ms, self.now_ms(), 50) {
            self.emit("audio_level", json!({ "level": level }), true);
        }
    }

    fn audio_spectrum(&self, spectrum: SpectrumPayload) {
        if Self::allow_rate(&self.last_spectrum_ms, self.now_ms(), 100) {
            self.emit("audio_spectrum", json!(spectrum), true);
        }
    }

    fn server_stopped(&self) {
        self.emit("server_stopped", json!({}), false);
    }

    fn web_client_count(&self, count: u32) {
        self.emit("web_client_count", json!({ "count": count }), false);
    }

    fn install_progress(&self, message: String) {
        self.emit("install_progress", json!({ "message": message }), false);
    }

    fn aec_status_changed(&self, status: AecStatus) {
        self.emit("aec_status_changed", json!(status), false);
    }

    fn monitoring_state_changed(&self, enabled: bool) {
        self.emit(
            "monitoring_state_changed",
            json!({ "enabled": enabled }),
            false,
        );
    }

    fn plugin_download_progress(&self, progress: micyou_core::events::DownloadProgress) {
        self.emit("plugin_download_progress", json!(progress), false);
    }
}

struct Request {
    v: u32,
    kind: String,
    id: Value,
    name: String,
    payload: Value,
}

fn parse_request(line: &str) -> Result<Request, serde_json::Error> {
    let value = serde_json::from_str::<Value>(line)?;
    Ok(Request {
        v: value
            .get("v")
            .and_then(Value::as_u64)
            .and_then(|version| u32::try_from(version).ok())
            .unwrap_or_default(),
        kind: value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        id: value.get("id").cloned().unwrap_or(Value::Null),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        payload: value.get("payload").cloned().unwrap_or(Value::Null),
    })
}

/// Read one UTF-8 line while enforcing a hard allocation limit.
fn read_bounded_line<R>(reader: &mut R) -> io::Result<Option<String>>
where
    R: BufRead,
{
    let mut bytes = Vec::with_capacity(256);
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let count = newline.unwrap_or(available.len());
        if bytes.len().saturating_add(count) > MAX_LINE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSONL input line exceeds 64 KiB",
            ));
        }
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub async fn run_control(state: &ServerState, output: &Output) -> Result<(), String> {
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel(1);
    // Tokio's stdin wrapper uses an uncancellable blocking read that can stall
    // runtime shutdown. Keep blocking stdin on a detached, dedicated thread.
    std::thread::Builder::new()
        .name("micyou-jsonl-stdin".to_string())
        .spawn(move || {
            let stdin = io::stdin();
            let mut input = BufReader::new(stdin.lock());
            loop {
                let line = read_bounded_line(&mut input);
                let finished = !matches!(&line, Ok(Some(_)));
                if input_tx.blocking_send(line).is_err() || finished {
                    break;
                }
            }
        })
        .map_err(|error| format!("failed to start JSONL stdin reader: {error}"))?;
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);

    loop {
        tokio::select! {
            signal_result = &mut signal => {
                signal_result.map_err(|error| format!("cannot listen for Ctrl+C: {error}"))?;
                break;
            }
            line_result = input_rx.recv() => {
                let Some(line_result) = line_result else { break };
                let line = line_result.map_err(|error| format!("invalid JSONL input: {error}"))?;
                let Some(line) = line else { break };
                let request = match parse_request(&line) {
                    Ok(request) => request,
                    Err(error) => {
                        output.send(json!({
                            "v": 1,
                            "type": "response",
                            "id": Value::Null,
                            "ok": false,
                            "error": { "code": "invalid_request", "message": error.to_string() }
                        }))?;
                        continue;
                    }
                };
                let id = request.id.clone();
                let valid_id = request.id.as_str().is_some_and(|id| !id.is_empty() && id.len() <= 128);
                let result = if !valid_id {
                    Err("id must be a non-empty string no longer than 128 bytes".to_string())
                } else if request.v != 1 || request.kind != "command" {
                    Err("unsupported protocol version or frame type".to_string())
                } else {
                    execute_command(state, &request.name, request.payload)
                };
                let should_stop = result.is_ok() && request.name == "stop";
                let response = match result {
                    Ok(payload) => json!({ "v": 1, "type": "response", "id": id, "ok": true, "payload": payload }),
                    Err(message) => json!({ "v": 1, "type": "response", "id": id, "ok": false, "error": { "code": "command_failed", "message": message } }),
                };
                output.send(response)?;
                if should_stop { break; }
            }
        }
    }
    Ok(())
}

fn execute_command(state: &ServerState, name: &str, payload: Value) -> Result<Value, String> {
    let controls = state.controls();
    match name {
        "stop" => Ok(json!({ "stopping": true })),
        "setMuted" => {
            let muted = payload
                .get("isMuted")
                .and_then(Value::as_bool)
                .ok_or_else(|| "payload.isMuted must be a boolean".to_string())?;
            controls.set_muted(muted);
            Ok(json!({ "isMuted": muted }))
        }
        "setMonitoring" => {
            let enabled = payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| "payload.enabled must be a boolean".to_string())?;
            controls.set_monitoring(enabled);
            Ok(json!({ "enabled": enabled }))
        }
        "getDspSettings" => {
            serde_json::to_value(controls.dsp_settings()).map_err(|error| error.to_string())
        }
        "applyDspSettings" => {
            let settings: AudioDspSettings = serde_json::from_value(payload)
                .map_err(|error| format!("invalid DSP settings: {error}"))?;
            controls.apply_dsp_settings(settings)?;
            serde_json::to_value(controls.dsp_settings()).map_err(|error| error.to_string())
        }
        "setSpectrumStreaming" => {
            let enabled = payload
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| "payload.enabled must be a boolean".to_string())?;
            state.set_spectrum_streaming(enabled);
            Ok(json!({ "enabled": enabled }))
        }
        "getConnectionSettings" => serde_json::to_value(micyou_core::config::load_server_prefs())
            .map_err(|error| error.to_string()),
        "applyConnectionSettings" => {
            let settings: micyou_core::config::ServerPrefs = serde_json::from_value(payload)
                .map_err(|error| format!("invalid connection settings: {error}"))?;
            if !matches!(settings.mode.as_str(), "wifi" | "usb" | "web") {
                return Err("mode must be wifi, usb, or web".to_string());
            }
            if settings.web_port == 0 {
                return Err("webPort must be between 1 and 65535".to_string());
            }
            if settings.mode != "web" && !(1..=65534).contains(&settings.port) {
                return Err("port must be between 1 and 65534 for Wi-Fi/USB".to_string());
            }
            micyou_core::config::save_server_prefs(&settings)?;
            Ok(json!({ "requiresRestart": true, "settings": settings }))
        }
        "listAudioDevices" => Ok(json!({ "devices": micyou_core::settings::audio_devices() })),
        "listAdbDevices" => {
            let devices = micyou_core::platform::adb::list_adb_devices()?;
            Ok(json!({ "devices": devices }))
        }
        "getVirtualAudioStatus" => Ok(virtual_audio_status()),
        _ => Err(format!("unknown command: {name}")),
    }
}

#[cfg(target_os = "macos")]
fn virtual_audio_status() -> Value {
    json!({ "device": "BlackHole", "installed": micyou_core::platform::blackhole::is_installed(), "manualSetupUrl": "https://existential.audio/blackhole/" })
}

#[cfg(target_os = "windows")]
fn virtual_audio_status() -> Value {
    json!({ "device": "VB-CABLE", "installed": micyou_core::platform::vbcable::is_installed(), "manualSetupUrl": "https://vb-audio.com/Cable/" })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn virtual_audio_status() -> Value {
    json!({ "device": "unsupported", "installed": false, "manualSetupUrl": Value::Null })
}

#[cfg(test)]
mod tests {
    use super::{parse_request, read_bounded_line, MAX_LINE_BYTES};
    use std::io::BufReader;

    #[test]
    fn accepts_a_line_within_the_limit() {
        let input = b"{\"v\":1}\n";
        let mut reader = BufReader::new(std::io::Cursor::new(input));
        assert_eq!(
            read_bounded_line(&mut reader).unwrap().as_deref(),
            Some("{\"v\":1}")
        );
    }

    #[test]
    fn rejects_an_oversized_line() {
        let input = vec![b'x'; MAX_LINE_BYTES + 1];
        let mut reader = BufReader::new(std::io::Cursor::new(input));
        assert!(read_bounded_line(&mut reader).is_err());
    }

    #[test]
    fn parses_a_versioned_command_envelope() {
        let request = parse_request(
            r#"{"v":1,"type":"command","id":"request-1","name":"setMuted","payload":{"isMuted":true}}"#,
        )
        .unwrap();
        assert_eq!(request.v, 1);
        assert_eq!(request.kind, "command");
        assert_eq!(request.id, "request-1");
        assert_eq!(request.name, "setMuted");
        assert_eq!(request.payload["isMuted"], true);
    }
}
