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

//! Server start/stop, shared by every frontend.

use super::audio_pipeline::{self, Pipeline};
use super::lifecycle::{await_startup_ready, AUDIO_JOIN_TIMEOUT, STARTUP_TIMEOUT};
use super::output;
use super::ServerState;
use crate::transport::session::AudioStreamEvent;
use serde::Deserialize;
use std::sync::atomic::Ordering;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const NETWORK_TASK_JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Bounds the audio queue: Android packets are ~7 ms, so 128 slots give ample
/// scheduling headroom without retaining seconds of stale audio.
const AUDIO_CHANNEL_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    Wifi,
    Usb,
    Web,
}

impl ConnectionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wifi => "wifi",
            Self::Usb => "usb",
            Self::Web => "web",
        }
    }
}

impl std::str::FromStr for ConnectionMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "wifi" => Ok(Self::Wifi),
            "usb" => Ok(Self::Usb),
            "web" => Ok(Self::Web),
            other => Err(format!(
                "invalid mode '{other}' (expected wifi, usb or web)"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StartRequest {
    pub port: u16,
    pub mode: ConnectionMode,
    /// `None` binds every interface (dual-stack auto-bind).
    pub bind_address: Option<String>,
    /// `None` uses the platform default / virtual device.
    pub output_device: Option<String>,
}

impl StartRequest {
    /// Merge explicit overrides with the shared server.json, so terminal
    /// frontends start exactly like the GUI would.
    pub fn resolve(
        port: Option<u16>,
        mode: Option<&str>,
        output_device: Option<String>,
        bind_address: Option<String>,
    ) -> Result<Self, String> {
        let prefs = crate::config::load_server_prefs();
        let mode = mode.unwrap_or(&prefs.mode).parse()?;
        let bind_address = bind_address.or_else(|| saved_bind_address(&prefs, mode));
        Ok(Self {
            port: port.unwrap_or(prefs.port),
            mode,
            bind_address,
            output_device: output_device
                .or_else(|| output::normalize_output_device(&prefs.output_device)),
        })
    }
}

/// The interface picked in the GUI, if any. USB mode ignores it like the GUI
/// does: `adb reverse` delivers connections on loopback, which a server bound
/// to a LAN address would refuse. See [`listen_address`] for where USB binds.
/// Where the server listens. USB mode stays on loopback, where both `adb
/// reverse` and the iOS usbmux bridge arrive, so phones on Wi-Fi cannot
/// connect to a server the user started for a cable.
fn listen_address(mode: ConnectionMode, bind_address: Option<String>) -> String {
    match mode {
        ConnectionMode::Usb => "127.0.0.1".to_string(),
        _ => bind_address.unwrap_or_else(|| "0.0.0.0".to_string()),
    }
}

fn saved_bind_address(prefs: &crate::config::ServerPrefs, mode: ConnectionMode) -> Option<String> {
    let saved = prefs.bind_address.trim();
    (mode != ConnectionMode::Usb && !prefs.auto_bind && !saved.is_empty() && saved != "0.0.0.0")
        .then(|| saved.to_string())
}

/// Returns the UDP audio port (TCP port + 1) for non-web modes.
fn validate_server_port(port: u16, mode: ConnectionMode) -> Result<Option<u16>, String> {
    if mode == ConnectionMode::Web {
        return if port == 0 {
            Err("Web server port must be between 1 and 65535".to_string())
        } else {
            Ok(None)
        };
    }
    if port == 0 {
        return Err("Audio server port must be between 1 and 65534".to_string());
    }
    port.checked_add(1).map(Some).ok_or_else(|| {
        "Audio server port must be between 1 and 65534 so the following UDP port is valid"
            .to_string()
    })
}

async fn join_tasks_bounded(mut tasks: Vec<JoinHandle<()>>, timeout: std::time::Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    while let Some(mut task) = tasks.pop() {
        if tokio::time::timeout_at(deadline, &mut task).await.is_err() {
            log::warn!("[Server] network task did not stop in time, aborting the rest");
            task.abort();
            let _ = task.await;
            for task in &tasks {
                task.abort();
            }
            for task in tasks {
                let _ = task.await;
            }
            break;
        }
    }
}

async fn rollback_start(
    state: &ServerState,
    cancel_token: &CancellationToken,
    tasks: Vec<JoinHandle<()>>,
) -> Result<(), String> {
    cancel_token.cancel();
    crate::transport::tcp::cleanup_session_state(
        &state.active_connection,
        &state.active_audio_session,
    )
    .await;
    join_tasks_bounded(tasks, NETWORK_TASK_JOIN_TIMEOUT).await;
    state.cancel_token.lock().await.take();
    if let Some(mdns) = state.mdns_manager.lock().await.take() {
        mdns.stop_mdns();
    }
    let mut lifecycle = state.lifecycle.lock().await;
    lifecycle.begin_stopping();
    lifecycle.join_audio_bounded(AUDIO_JOIN_TIMEOUT).await
}

/// Undo a partial start and report both the original failure and any
/// cleanup failure.
async fn fail_start(
    state: &ServerState,
    cancel_token: &CancellationToken,
    tasks: Vec<JoinHandle<()>>,
    error: String,
) -> Result<String, String> {
    log::error!("[Server] start failed: {error}");
    Err(match rollback_start(state, cancel_token, tasks).await {
        Ok(()) => error,
        Err(cleanup) => format!("{error}; {cleanup}"),
    })
}

fn output_buffer_ms(state: &ServerState) -> usize {
    state
        .dsp_settings
        .read()
        .map(|s| (s.output_buffer_ms as usize).clamp(100, 1200))
        .unwrap_or_else(|poisoned| {
            (poisoned.into_inner().output_buffer_ms as usize).clamp(100, 1200)
        })
}

/// Open the persistent output device with the saved preferences. The GUI
/// calls this at launch so the virtual microphone exists before streaming.
pub fn open_output_device(state: &ServerState) -> bool {
    let prefs = crate::config::load_server_prefs();
    let resource_dir = state.resource_dir();
    output::ensure_started(
        &state.audio_output,
        output::normalize_output_device(&prefs.output_device),
        output_buffer_ms(state),
        resource_dir.as_deref(),
    )
}

/// Close the persistent output device. Only called when the process exits,
/// never on server stop.
pub fn close_output_device(state: &ServerState) {
    state.audio_output.shutdown();
    #[cfg(target_os = "linux")]
    crate::platform::pipewire::cleanup();
}

pub async fn start_server(state: &ServerState, request: StartRequest) -> Result<String, String> {
    let StartRequest {
        port,
        mode,
        bind_address,
        output_device,
    } = request;
    let udp_port = validate_server_port(port, mode)?;

    let _lifecycle_guard = state.lifecycle_gate.enter().await;
    // The lifecycle phase is the single source of truth for "running": a
    // token only exists between a successful begin_start and the next stop.
    state.lifecycle.lock().await.begin_start().await?;
    let bind_addr = listen_address(mode, bind_address);
    let cancel_token = CancellationToken::new();
    *state.cancel_token.lock().await = Some(cancel_token.clone());

    // Settings may have been edited by another frontend since launch.
    let file_settings = crate::config::load_dsp_settings();
    *state
        .dsp_settings
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = file_settings;

    // Web mode advertises its own HTTPS service type; announcing the TCP
    // control service there would point phones at a port that speaks TLS.
    // USB mode listens on loopback only, so there is nothing to announce.
    if mode == ConnectionMode::Wifi {
        match crate::discovery::NetworkManager::start_mdns(port, &bind_addr) {
            Ok(manager) => *state.mdns_manager.lock().await = Some(manager),
            Err(e) => {
                log::warn!("[Server] mDNS unavailable, phones must enter the IP manually: {e}")
            }
        }
    }

    // Pick up plugins installed by another frontend since the last start.
    state.plugins.load_saved_plugins();
    // Every registered DSP plugin gets its own `Plugin:<id>` chain node (#347).
    state.plugins.ensure_plugin_chain_node(&state.dsp_settings);

    let resource_dir = state.resource_dir();

    let (audio_tx, audio_rx) = tokio::sync::mpsc::channel(AUDIO_CHANNEL_CAPACITY);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let audio_thread = audio_pipeline::spawn(Pipeline {
        audio_rx,
        ready_tx,
        audio_output: state.audio_output.clone(),
        output_device,
        output_buffer_ms: output_buffer_ms(state),
        resource_dir,
        dsp_settings: state.dsp_settings.clone(),
        plugins: state.plugins.clone(),
        events: state.events.clone(),
        active_audio_session: state.active_audio_session.clone(),
        is_monitoring: state.is_monitoring.clone(),
        spectrum_streaming_enabled: state.spectrum_streaming_enabled.clone(),
        stats: state.network_stats.clone(),
        bypass_dsp: mode == ConnectionMode::Web,
    });
    state.lifecycle.lock().await.set_audio_thread(audio_thread);
    if let Err(error) = await_startup_ready(ready_rx, "Audio output", STARTUP_TIMEOUT).await {
        let error = format!("Failed to start audio output: {error}");
        return fail_start(state, &cancel_token, Vec::new(), error).await;
    }

    if mode == ConnectionMode::Web {
        return start_web(state, port, &bind_addr, audio_tx, &cancel_token).await;
    }

    let (tcp_ready_tx, tcp_ready_rx) = tokio::sync::oneshot::channel();
    let tcp_task = tokio::spawn(crate::transport::tcp::run_tcp_server(
        crate::transport::tcp::TcpServerConfig {
            events: state.events.clone(),
            port,
            bind_address: bind_addr.clone(),
            cancel_token: cancel_token.clone(),
            audio_tx: audio_tx.clone(),
            stats: state.network_stats.clone(),
            mode,
            active_connection: state.active_connection.clone(),
            takeover_lock: state.takeover_lock.clone(),
            active_audio_session: state.active_audio_session.clone(),
            plugins: state.plugins.clone(),
        },
        tcp_ready_tx,
    ));

    let udp_port = udp_port.expect("non-web port validation produces a UDP port");
    let (udp_ready_tx, udp_ready_rx) = tokio::sync::oneshot::channel();
    let udp_task = tokio::spawn({
        let stats = state.network_stats.clone();
        let session = state.active_audio_session.clone();
        let bind_addr = bind_addr.clone();
        let token = cancel_token.clone();
        async move {
            if let Err(e) = crate::transport::udp::start_udp_server(
                audio_tx,
                udp_port,
                bind_addr,
                token,
                stats,
                session,
                udp_ready_tx,
            )
            .await
            {
                log::error!("[Server] UDP server error: {e}");
            }
        }
    });

    let (tcp_ready, udp_ready) = tokio::join!(
        await_startup_ready(tcp_ready_rx, "TCP server", STARTUP_TIMEOUT),
        await_startup_ready(udp_ready_rx, "UDP server", STARTUP_TIMEOUT),
    );
    if let Err(error) = tcp_ready.and(udp_ready) {
        let error = format!("Failed to start network server: {error}");
        return fail_start(state, &cancel_token, vec![tcp_task, udp_task], error).await;
    }
    let mut tasks = vec![tcp_task, udp_task];
    // iOS reaches USB mode through usbmuxd instead of adb reverse.
    if mode == ConnectionMode::Usb {
        tasks.push(tokio::spawn(crate::platform::usbmux::run_bridge(
            port,
            cancel_token.clone(),
        )));
    }
    state.background_tasks.lock().await.extend(tasks);
    state.lifecycle.lock().await.mark_running();
    log::info!("[Server] started on port {port} ({})", mode.as_str());
    Ok(format!("Server started on port {port}"))
}

#[cfg(feature = "web-server")]
async fn start_web(
    state: &ServerState,
    port: u16,
    bind_addr: &str,
    audio_tx: tokio::sync::mpsc::Sender<AudioStreamEvent>,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    use crate::transport::session::{validate_audio_packet, ExpectedAudioSession};
    use micyou_protocol::micyou::{AudioPacketMessage, AudioPacketMessageOrdered};

    let web_server = crate::transport::web::WebServer::new();
    let (web_audio_tx, mut web_audio_rx) =
        tokio::sync::mpsc::channel::<(u64, AudioPacketMessage)>(AUDIO_CHANNEL_CAPACITY);
    if let Err(e) = web_server
        .start(port, bind_addr, state.events.clone(), web_audio_tx)
        .await
    {
        let error = format!("Failed to start web server: {e}");
        return fail_start(state, cancel_token, Vec::new(), error).await;
    }

    match crate::discovery::NetworkManager::start_web_mdns(port, bind_addr) {
        Ok(manager) => *state.web_mdns.lock().await = Some(manager),
        Err(e) => log::warn!("[Server] web mDNS unavailable: {e}"),
    }
    *state.web_server.lock().await = Some(web_server);

    // Browser clients send unsequenced chunks tagged with a connection
    // generation; wrap them into the ordered packets the pipeline expects.
    let web_audio_task = tokio::spawn(async move {
        let mut seq: i32 = 0;
        let mut active_generation = 0;
        while let Some((generation, packet)) = web_audio_rx.recv().await {
            if generation < active_generation {
                continue;
            }
            if generation > active_generation {
                active_generation = generation;
                seq = 0;
                let starting = AudioStreamEvent::SessionStarting {
                    expected: ExpectedAudioSession::Bound(0),
                    epoch: generation,
                };
                if audio_tx.send(starting).await.is_err() {
                    break;
                }
            }
            let ordered = AudioPacketMessageOrdered {
                sequence_number: seq,
                audio_packet: Some(packet),
                timestamp: 0,
                fec_buffer: Vec::new(),
                fec_sequence_number: -1,
                session_id: 0,
                fec_packet_lengths: Vec::new(),
            };
            seq = seq.wrapping_add(1);
            if !validate_audio_packet(&ordered) {
                continue;
            }
            let event = AudioStreamEvent::Packet {
                packet: ordered,
                epoch: generation,
            };
            if audio_tx.send(event).await.is_err() {
                break;
            }
        }
    });
    state.background_tasks.lock().await.push(web_audio_task);
    state.lifecycle.lock().await.mark_running();
    log::info!("[Server] web server started on port {port}");
    Ok(format!("Web server started on port {port}"))
}

#[cfg(not(feature = "web-server"))]
async fn start_web(
    state: &ServerState,
    _port: u16,
    _bind_addr: &str,
    _audio_tx: tokio::sync::mpsc::Sender<AudioStreamEvent>,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    let error = "Web mode is not available in this build".to_string();
    fail_start(state, cancel_token, Vec::new(), error).await
}

pub async fn stop_server(state: &ServerState) -> Result<String, String> {
    let _lifecycle_guard = state.lifecycle_gate.enter().await;
    state
        .spectrum_streaming_enabled
        .store(false, Ordering::Release);

    #[cfg(feature = "web-server")]
    {
        if let Some(web) = state.web_server.lock().await.take() {
            web.stop().await;
        }
        if let Some(web_mdns) = state.web_mdns.lock().await.take() {
            web_mdns.stop_mdns();
        }
    }
    if let Some(mdns) = state.mdns_manager.lock().await.take() {
        mdns.stop_mdns();
    }

    let token = state.cancel_token.lock().await.take();
    let had_token = token.is_some();
    if let Some(token) = token {
        token.cancel();
    }
    state.lifecycle.lock().await.begin_stopping();
    let tasks = std::mem::take(&mut *state.background_tasks.lock().await);
    crate::transport::tcp::cleanup_session_state(
        &state.active_connection,
        &state.active_audio_session,
    )
    .await;
    join_tasks_bounded(tasks, NETWORK_TASK_JOIN_TIMEOUT).await;
    let audio_result = state
        .lifecycle
        .lock()
        .await
        .join_audio_bounded(AUDIO_JOIN_TIMEOUT)
        .await;
    // Restore the original input device (BlackHole cleanup).
    #[cfg(target_os = "macos")]
    if let Err(e) = crate::platform::blackhole::do_restore_input_device().await {
        log::warn!("[Server] failed to restore the macOS input device: {e}");
    }
    audio_result?;
    if !had_token {
        return Err("Server is not running".to_string());
    }
    state.events.server_stopped();
    log::info!("[Server] stopped");
    Ok("Server stopped".to_string())
}

#[cfg(test)]
mod tests {
    use super::{listen_address, saved_bind_address, validate_server_port, ConnectionMode};
    use crate::config::ServerPrefs;

    #[test]
    fn usb_mode_listens_on_loopback_only() {
        assert_eq!(listen_address(ConnectionMode::Usb, Some("192.168.8.92".into())), "127.0.0.1");
        assert_eq!(listen_address(ConnectionMode::Usb, None), "127.0.0.1");
        assert_eq!(listen_address(ConnectionMode::Wifi, None), "0.0.0.0");
        assert_eq!(
            listen_address(ConnectionMode::Wifi, Some("192.168.8.92".into())),
            "192.168.8.92"
        );
    }

    #[test]
    fn usb_mode_ignores_the_saved_interface() {
        let prefs = ServerPrefs {
            auto_bind: false,
            bind_address: "192.168.8.92".to_string(),
            ..ServerPrefs::default()
        };
        assert_eq!(saved_bind_address(&prefs, ConnectionMode::Usb), None);
        assert_eq!(
            saved_bind_address(&prefs, ConnectionMode::Wifi).as_deref(),
            Some("192.168.8.92")
        );
    }

    #[test]
    fn non_web_port_zero_is_rejected() {
        assert!(validate_server_port(0, ConnectionMode::Wifi).is_err());
    }

    #[test]
    fn non_web_port_65534_produces_last_udp_port() {
        assert_eq!(
            validate_server_port(65534, ConnectionMode::Wifi),
            Ok(Some(65535))
        );
    }

    #[test]
    fn non_web_port_65535_is_rejected() {
        assert!(validate_server_port(65535, ConnectionMode::Wifi).is_err());
    }

    #[test]
    fn web_port_zero_is_rejected() {
        assert!(validate_server_port(0, ConnectionMode::Web).is_err());
    }

    #[test]
    fn connection_mode_rejects_unknown_values() {
        assert_eq!("usb".parse::<ConnectionMode>(), Ok(ConnectionMode::Usb));
        assert!("bluetooth".parse::<ConnectionMode>().is_err());
    }
}
