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

use crate::events::SharedEvents;
use crate::server::service::ConnectionMode;
use micyou_protocol::micyou::MessageWrapper;
use micyou_protocol::{HANDSHAKE_CLIENT_STR, HANDSHAKE_SERVER_STR, PACKET_MAGIC};
use prost::Message;
use serde::Serialize;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::transport::session::{
    audio_payload_len, validate_audio_packet, AudioStreamEvent, ExpectedAudioSession,
};
use crate::transport::udp::{
    try_accept_audio_packet, ActiveAudioSession, AudioPacketAcceptance, SharedActiveAudioSession,
};

const FRAME_HEADER_LEN: usize = 8;
// Audio protobuf frames are normally only a few KiB. One MiB leaves ample codec and
// control-message headroom while bounding allocations from an untrusted peer.
const MAX_CONTROL_PAYLOAD_LEN: usize = 1024 * 1024;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONCURRENT_CLIENTS: usize = 64;
const FRAME_READ_TIMEOUT: Duration = Duration::from_secs(10);
const FRAME_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const CLIENT_SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_millis(250);
static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(windows)]
pub type RawSocketHandle = std::os::windows::io::RawSocket;
#[cfg(unix)]
pub type RawSocketHandle = std::os::unix::io::RawFd;

pub struct ActiveConnection {
    pub sender: tokio::sync::mpsc::Sender<MessageWrapper>,
    pub raw_socket: RawSocketHandle,
    pub connection_id: u64,
    pub takeover_token: CancellationToken,
    pub platform: ClientPlatform,
}

pub type SharedActiveConnection = Arc<Mutex<Option<ActiveConnection>>>;
pub type SharedTakeoverLock = Arc<Mutex<()>>;

struct TaskGuard {
    tasks: Vec<JoinHandle<()>>,
}

impl TaskGuard {
    fn new(tasks: Vec<JoinHandle<()>>) -> Self {
        Self { tasks }
    }

    async fn abort_and_wait(mut self) {
        for task in &self.tasks {
            task.abort();
        }
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub ip: String,
    pub latency: u32,
    pub platform: ClientPlatform,
}

/// Platform the phone reports in its Connect message, normalized at the wire boundary.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClientPlatform {
    Android,
    Ios,
    /// Clients that predate the field (every Android release before it) or send an unknown value.
    #[default]
    Unknown,
}

impl ClientPlatform {
    pub fn from_wire(value: &str) -> Self {
        match value {
            "android" => Self::Android,
            "ios" => Self::Ios,
            _ => Self::Unknown,
        }
    }

    /// Label shown by the CLI and TUI and passed to plugins.
    pub fn device_label(self) -> &'static str {
        match self {
            Self::Android => "MicYou Android",
            Self::Ios => "MicYou iOS",
            Self::Unknown => "MicYou Mobile",
        }
    }
}

pub struct TcpServerConfig {
    pub events: SharedEvents,
    pub port: u16,
    pub bind_address: String,
    pub cancel_token: CancellationToken,
    pub audio_tx: tokio::sync::mpsc::Sender<AudioStreamEvent>,
    pub stats: Arc<crate::stats::NetworkStats>,
    pub mode: ConnectionMode,
    pub active_connection: SharedActiveConnection,
    pub takeover_lock: SharedTakeoverLock,
    pub active_audio_session: SharedActiveAudioSession,
    pub plugins: Arc<crate::plugins::PluginHost>,
}

/// Run the TCP control server until cancelled; startup success or failure
/// is reported through `ready`.
pub async fn run_tcp_server(
    config: TcpServerConfig,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    if let Err(e) = serve(config, ready).await {
        log::error!("[Server] TCP server error: {e}");
    }
}

async fn serve(
    config: TcpServerConfig,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let TcpServerConfig {
        events,
        port,
        bind_address,
        cancel_token,
        audio_tx,
        stats,
        mode,
        active_connection,
        takeover_lock,
        active_audio_session,
        plugins,
    } = config;
    // IPv6-aware bind: for IPv4 literals and hostnames this is exactly the
    // legacy `TcpListener::bind(format!("{}:{}", bind_address, port))` path.
    let bind_target = crate::transport::net_bind::normalize_socket_addr(&bind_address, port);
    let listener = match crate::transport::net_bind::bind_tcp_listener(&bind_address, port).await {
        Ok(listener) => listener,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return Err(Box::new(error));
        }
    };

    // Additive IPv6 companion listener for the legacy IPv4 auto-bind
    // ("0.0.0.0"): also listen on [::] (v6-only) so IPv6 clients can connect
    // without the user picking an address. The IPv4 listener above is not
    // touched; a companion failure (no IPv6 stack, port unavailable) is
    // non-fatal and only logged. Bound BEFORE signalling ready so clients
    // can never observe a half-started server.
    let v6_listener = if crate::transport::net_bind::wants_v6_companion(&bind_address) {
        match crate::transport::net_bind::bind_tcp_listener_v6only(port) {
            Ok(listener) => {
                log::info!(
                    "TCP Control Server also listening on [::]:{} (IPv6 companion)",
                    port
                );
                Some(listener)
            }
            Err(error) => {
                log::warn!(
                    "IPv6 companion TCP listener not started: {}{}",
                    error,
                    crate::transport::net_bind::companion_failure_hint(&error)
                );
                None
            }
        }
    } else {
        None
    };

    let _ = ready.send(Ok(()));
    log::info!("TCP Control Server listening on {}", bind_target);

    let mut clients = JoinSet::new();
    let client_slots = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CLIENTS));

    // Per-connection setup shared by the IPv4 listener and the IPv6
    // companion. The body is the legacy accept arm verbatim; both listeners
    // produce the same (TcpStream, SocketAddr) pair, and handle_client is
    // address-family agnostic (it only ever uses addr.ip()).
    macro_rules! accept_client {
        ($accept_result:expr) => {
            match $accept_result {
                Ok((socket, addr)) => {
                    // Control frames (ping/pong) are ~60 bytes; without
                    // TCP_NODELAY, Nagle aggregation adds up to ~40ms of
                    // jitter to the RTT reading. On USB mode the real
                    // latency is a few ms, so this jitter is very visible.
                    if let Err(e) = socket.set_nodelay(true) {
                        log::warn!("Failed to set TCP_NODELAY on client socket: {}", e);
                    }
                    let permit = match client_slots.clone().try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            log::warn!("TCP client limit reached; rejecting {}", addr);
                            continue;
                        }
                    };
                    log::info!("New client connected: {}", addr);
                    let events = events.clone();
                    let audio_tx = audio_tx.clone();
                    let stats = stats.clone();
                    let active_connection = active_connection.clone();
                    let takeover_lock = takeover_lock.clone();
                    let active_audio_session = active_audio_session.clone();
                    let plugins = plugins.clone();
                    let client_cancel = cancel_token.clone();
                    clients.spawn(async move {
                        let _permit = permit;
                        if let Err(e) = handle_client(
                            socket,
                            addr,
                            events,
                            audio_tx,
                            stats,
                            mode,
                            active_connection,
                            takeover_lock,
                            active_audio_session,
                            plugins,
                            client_cancel,
                        )
                        .await
                        {
                            log::warn!("Client {} error: {}", addr, e);
                        }
                        log::info!("Client {} disconnected", addr);
                    });
                }
                Err(e) => log::warn!("Failed to accept TCP connection: {}", e),
            }
        };
    }

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("TCP Server cancelled");
                break;
            }
            Some(result) = clients.join_next(), if !clients.is_empty() => {
                if let Err(e) = result {
                    log::warn!("TCP client task failed: {}", e);
                }
            }
            accept_result = listener.accept() => {
                accept_client!(accept_result);
            }
            accept_result = accept_v6_companion(v6_listener.as_ref()) => {
                accept_client!(accept_result);
            }
        }
    }

    // Take ownership of the published raw socket while handle_client still owns the
    // corresponding TcpStream, then shut it down so client tasks can exit cooperatively.
    cleanup_session_state(&active_connection, &active_audio_session).await;
    if timeout(CLIENT_SHUTDOWN_GRACE_PERIOD, async {
        while clients.join_next().await.is_some() {}
    })
    .await
    .is_err()
    {
        clients.abort_all();
        while clients.join_next().await.is_some() {}
    }
    Ok(())
}

pub async fn cleanup_session_state(
    active_connection: &SharedActiveConnection,
    active_audio_session: &SharedActiveAudioSession,
) {
    cleanup_session_state_with(active_connection, active_audio_session, force_close_socket).await;
}

/// Accepts from the optional IPv6 companion listener. When no companion is
/// configured this future stays pending forever, so the `select!` arm simply
/// never fires and the IPv4-only behaviour is unchanged.
async fn accept_v6_companion(
    listener: Option<&tokio::net::TcpListener>,
) -> std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)> {
    match listener {
        Some(listener) => listener.accept().await,
        None => std::future::pending().await,
    }
}

async fn cleanup_session_state_with<F>(
    active_connection: &SharedActiveConnection,
    active_audio_session: &SharedActiveAudioSession,
    mut shutdown_socket: F,
) where
    F: FnMut(RawSocketHandle),
{
    // Remove the handle from shared state before cancellation or shutdown. Later cleanup
    // callers must not be able to act on a descriptor that its TcpStream may have closed.
    let connection = {
        let mut active = active_connection.lock().await;
        active.take()
    };
    if let Some(connection) = connection {
        connection.takeover_token.cancel();
        shutdown_socket(connection.raw_socket);
        drop(connection.sender);
    }
    match active_audio_session.write() {
        Ok(mut active) => *active = ActiveAudioSession::default(),
        Err(poisoned) => *poisoned.into_inner() = ActiveAudioSession::default(),
    }
}

#[cfg(windows)]
fn safe_shutdown_socket(raw: RawSocketHandle) -> std::io::Result<()> {
    let status = unsafe {
        winapi::um::winsock2::shutdown(
            raw as winapi::um::winsock2::SOCKET,
            winapi::um::winsock2::SD_BOTH,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn safe_shutdown_socket(raw: RawSocketHandle) -> std::io::Result<()> {
    let status = unsafe { libc::shutdown(raw, libc::SHUT_RDWR) };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn force_close_socket(raw: RawSocketHandle) {
    if let Err(e) = safe_shutdown_socket(raw) {
        log::warn!("Force close socket error: {}", e);
    }
}

fn parse_frame_header(header: &[u8; FRAME_HEADER_LEN]) -> Result<usize, IoError> {
    let magic = i32::from_be_bytes(header[0..4].try_into().unwrap());
    if magic != PACKET_MAGIC {
        return Err(IoError::new(ErrorKind::InvalidData, "invalid packet magic"));
    }

    let payload_len_i32 = i32::from_be_bytes(header[4..8].try_into().unwrap());
    if payload_len_i32 < 0 {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "negative payload length",
        ));
    }
    let payload_len = usize::try_from(payload_len_i32)
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "invalid payload length"))?;
    if payload_len > MAX_CONTROL_PAYLOAD_LEN {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "payload length exceeds limit",
        ));
    }
    FRAME_HEADER_LEN
        .checked_add(payload_len)
        .ok_or_else(|| IoError::new(ErrorKind::InvalidData, "frame length overflow"))?;
    Ok(payload_len)
}

async fn run_if_active<F>(
    active: &SharedActiveConnection,
    takeover_token: &CancellationToken,
    connection_id: u64,
    action: F,
) -> bool
where
    F: FnOnce(),
{
    if takeover_token.is_cancelled() {
        return false;
    }
    let active = active.lock().await;
    if takeover_token.is_cancelled()
        || active
            .as_ref()
            .is_none_or(|connection| connection.connection_id != connection_id)
    {
        return false;
    }
    action();
    true
}

/// Whether `connection_id` is still the published connection. Callers that
/// notify plugins use this instead of [`run_if_active`] so plugin callbacks
/// never run while the connection lock is held.
async fn is_active(
    active: &SharedActiveConnection,
    takeover_token: &CancellationToken,
    connection_id: u64,
) -> bool {
    run_if_active(active, takeover_token, connection_id, || {}).await
}

async fn clear_if_active(active: &SharedActiveConnection, connection_id: u64) -> bool {
    let mut lock = active.lock().await;
    if lock
        .as_ref()
        .is_some_and(|connection| connection.connection_id == connection_id)
    {
        *lock = None;
        true
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_client(
    mut socket: TcpStream,
    addr: SocketAddr,
    events: SharedEvents,
    audio_tx: tokio::sync::mpsc::Sender<AudioStreamEvent>,
    stats: Arc<crate::stats::NetworkStats>,
    mode: ConnectionMode,
    active_connection: SharedActiveConnection,
    takeover_lock: SharedTakeoverLock,
    active_audio_session: SharedActiveAudioSession,
    plugins: Arc<crate::plugins::PluginHost>,
    cancel_token: CancellationToken,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut handshake_buf = vec![0u8; HANDSHAKE_CLIENT_STR.len()];
    tokio::select! {
        _ = cancel_token.cancelled() => return Ok(()),
        result = timeout(HANDSHAKE_TIMEOUT, socket.read_exact(&mut handshake_buf)) => {
            result.map_err(|_| IoError::new(ErrorKind::TimedOut, "handshake read timed out"))??;
        }
    }
    if handshake_buf != HANDSHAKE_CLIENT_STR {
        return Err("Invalid handshake from client".into());
    }

    // Do not touch global connection/audio state until the complete response is visible
    // to the peer. A failed or stalled candidate handshake cannot disturb the active device.
    tokio::select! {
        _ = cancel_token.cancelled() => return Ok(()),
        result = timeout(HANDSHAKE_TIMEOUT, async {
            socket.write_all(HANDSHAKE_SERVER_STR).await?;
            socket.flush().await
        }) => {
            result.map_err(|_| IoError::new(ErrorKind::TimedOut, "handshake write timed out"))??;
        }
    }

    // The first framed message completes the protocol handshake. Modern clients send Connect with
    // their audio session ID; legacy clients' first control/audio frame decodes with no Connect.
    let first_message = tokio::select! {
        _ = cancel_token.cancelled() => return Ok(()),
        result = timeout(FRAME_READ_TIMEOUT, async {
            let mut header = [0u8; FRAME_HEADER_LEN];
            socket.read_exact(&mut header).await?;
            let payload_len = parse_frame_header(&header)?;
            let mut payload = vec![0u8; payload_len];
            socket.read_exact(&mut payload).await?;
            MessageWrapper::decode(payload.as_slice())
                .map_err(|e| IoError::new(ErrorKind::InvalidData, e))
        }) => result.map_err(|_| IoError::new(ErrorKind::TimedOut, "initial control frame timed out"))??,
    };
    let expected_session = match first_message.connect.as_ref() {
        Some(connect) => ExpectedAudioSession::Bound(connect.session_id),
        None => ExpectedAudioSession::UnboundLegacy,
    };
    let platform = first_message
        .connect
        .as_ref()
        .map_or(ClientPlatform::Unknown, |connect| {
            ClientPlatform::from_wire(&connect.platform)
        });

    #[cfg(windows)]
    let raw: RawSocketHandle = std::os::windows::io::AsRawSocket::as_raw_socket(&socket);
    #[cfg(unix)]
    let raw: RawSocketHandle = std::os::unix::io::AsRawFd::as_raw_fd(&socket);

    let connection_id = NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<MessageWrapper>(100);
    let takeover_token = CancellationToken::new();

    // Serialize reservation and publication so acknowledged candidates cannot reorder
    // SessionStarting relative to the active connection they publish.
    let _takeover_guard = takeover_lock.lock().await;
    let session_start_permit = tokio::select! {
        _ = cancel_token.cancelled() => return Ok(()),
        result = timeout(HANDSHAKE_TIMEOUT, audio_tx.clone().reserve_owned()) => {
            result
                .map_err(|_| IoError::new(ErrorKind::TimedOut, "audio pipeline reservation timed out"))?
                .map_err(|_| IoError::new(ErrorKind::BrokenPipe, "audio pipeline stopped"))?
        }
    };
    let old = {
        let mut active = active_connection.lock().await;
        let old = active.replace(ActiveConnection {
            sender: tx.clone(),
            raw_socket: raw,
            connection_id,
            takeover_token: takeover_token.clone(),
            platform,
        });
        if let Some(old) = old.as_ref() {
            old.takeover_token.cancel();
            force_close_socket(old.raw_socket);
        }
        let epoch = {
            let mut active_audio = active_audio_session
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let previous_epoch = match *active_audio {
                ActiveAudioSession::Inactive => 0,
                ActiveAudioSession::UnboundLegacy { epoch, .. }
                | ActiveAudioSession::Bound { epoch, .. } => epoch,
            };
            let epoch = previous_epoch.wrapping_add(1).max(1);
            *active_audio = match expected_session {
                ExpectedAudioSession::Inactive => ActiveAudioSession::Inactive,
                ExpectedAudioSession::UnboundLegacy => ActiveAudioSession::UnboundLegacy {
                    peer_ip: addr.ip(),
                    epoch,
                },
                ExpectedAudioSession::Bound(session_id) => ActiveAudioSession::Bound {
                    peer_ip: addr.ip(),
                    session_id,
                    epoch,
                },
            };
            epoch
        };
        session_start_permit.send(AudioStreamEvent::SessionStarting {
            expected: expected_session,
            epoch,
        });
        // Point the plugin sync adapter at this client's control channel so
        // plugin messages can flow to the phone (cleared when it exits).
        plugins.sync.set_sender(Some(tx.clone()));
        old
    };
    drop(old);
    drop(_takeover_guard);

    log::info!("Handshake successful with {} ({:?})", addr, platform);
    let device_info = DeviceInfo {
        name: platform.device_label().to_string(),
        ip: addr.ip().to_string(),
        latency: 12,
        platform,
    };
    let current_time = crate::stats::unix_millis();
    if is_active(&active_connection, &takeover_token, connection_id).await {
        stats.mark_tcp_connected(current_time);
        events.device_connected(device_info.clone());
        plugins.broadcast_event(&micyou_plugin::PluginEvent::DeviceConnected {
            mode: mode.as_str().to_string(),
            label: device_info.name,
        });
    }

    let mut meter = crate::stats::StreamMeter::default();
    handle_message(
        first_message,
        &mut meter,
        &tx,
        &audio_tx,
        &stats,
        &events,
        &takeover_token,
        connection_id,
        &active_connection,
        &active_audio_session,
        plugins.clone(),
        addr.ip(),
    )
    .await;

    let (mut read_half, mut write_half) = socket.into_split();
    let writer_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let payload = msg.encode_to_vec();
            if payload.len() > MAX_CONTROL_PAYLOAD_LEN {
                log::warn!("Outgoing control payload exceeds protocol limit");
                break;
            }
            let Ok(payload_len) = i32::try_from(payload.len()) else {
                break;
            };
            let frame_len = match FRAME_HEADER_LEN.checked_add(payload.len()) {
                Some(len) => len,
                None => break,
            };
            let mut frame = Vec::with_capacity(frame_len);
            frame.extend_from_slice(&PACKET_MAGIC.to_be_bytes());
            frame.extend_from_slice(&payload_len.to_be_bytes());
            frame.extend_from_slice(&payload);
            match timeout(FRAME_WRITE_TIMEOUT, write_half.write_all(&frame)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    log::warn!("Write failed: {}", e);
                    break;
                }
                Err(_) => {
                    log::warn!("Control frame write timed out");
                    break;
                }
            }
        }
    });

    let tx_ping = tx.clone();
    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            let ping_msg = MessageWrapper {
                audio_packet: None,
                connect: None,
                mute: None,
                ping: Some(micyou_protocol::micyou::PingMessage {
                    timestamp: crate::stats::unix_millis() as i64,
                }),
                pong: None,
                plugin_message: None,
                stats: None,
            };
            if tx_ping.send(ping_msg).await.is_err() {
                break;
            }
        }
    });

    let stats_emit = stats.clone();
    let events_emit = events.clone();
    let tx_stats = tx.clone();
    let monitor_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        let mut warning_fired = false;
        loop {
            interval.tick().await;
            let buffer_duration = if mode == ConnectionMode::Usb { 5 } else { 30 };
            let metrics = stats_emit.to_metrics(buffer_duration);
            // Best effort: a full control queue only skips this second's report.
            let _ = tx_stats.try_send(stats_message(&metrics));
            events_emit.audio_metrics(metrics);
            if cfg!(target_os = "windows")
                && mode == ConnectionMode::Wifi
                && !warning_fired
                && !stats_emit.is_muted()
            {
                let tcp_time = stats_emit.get_tcp_connected_time();
                let last_udp = stats_emit.get_last_udp_time();
                // Any UDP packet this session proves the firewall lets the
                // audio port through; only warn after 10 s of TCP without one.
                let now = crate::stats::unix_millis();
                if tcp_time > 0 && last_udp == 0 && now.saturating_sub(tcp_time) > 10_000 {
                    events_emit.udp_audio_warning();
                    warning_fired = true;
                }
            }
        }
    });
    let task_guard = TaskGuard::new(vec![writer_task, ping_task, monitor_task]);

    let plugins_reader = plugins.clone();
    let reader = async {
        loop {
            let mut header = [0u8; FRAME_HEADER_LEN];
            let frame_result = timeout(FRAME_READ_TIMEOUT, async {
                read_half.read_exact(&mut header).await?;
                let payload_len = parse_frame_header(&header)?;
                // The parser guarantees this allocation never exceeds header + 1 MiB.
                let mut payload = vec![0u8; payload_len];
                read_half.read_exact(&mut payload).await?;
                Ok::<Vec<u8>, IoError>(payload)
            })
            .await
            .map_err(|_| IoError::new(ErrorKind::TimedOut, "control frame read timed out"))??;

            let message = MessageWrapper::decode(frame_result.as_slice())?;
            if takeover_token.is_cancelled() {
                break;
            }
            handle_message(
                message,
                &mut meter,
                &tx,
                &audio_tx,
                &stats,
                &events,
                &takeover_token,
                connection_id,
                &active_connection,
                &active_audio_session,
                plugins_reader.clone(),
                addr.ip(),
            )
            .await;
        }
        #[allow(unreachable_code)]
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    };

    let reader_result = tokio::select! {
        _ = cancel_token.cancelled() => Ok(()),
        result = reader => result,
    };
    task_guard.abort_and_wait().await;

    if clear_if_active(&active_connection, connection_id).await {
        *active_audio_session
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = ActiveAudioSession::default();
        stats.mark_tcp_disconnected();
        events.device_disconnected();
        plugins.broadcast_event(&micyou_plugin::PluginEvent::DeviceDisconnected);
    }
    // Release the plugin sync slot if it still points at this session.
    plugins.sync.clear_if(&tx);
    reader_result
}

/// Link statistics for the phone, mirroring what the desktop UI shows.
fn stats_message(metrics: &crate::stats::AudioMetrics) -> MessageWrapper {
    let clamp = |value: i64| value.clamp(0, i32::MAX as i64) as i32;
    MessageWrapper {
        stats: Some(micyou_protocol::micyou::StatsMessage {
            rtt_ms: clamp(metrics.network_latency_ms),
            jitter_ms: metrics.jitter_ms as f32,
            packet_loss: metrics.packet_loss_rate.clamp(0.0, 1.0) as f32,
            latency_ms: clamp(metrics.latency_ms),
        }),
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_message(
    msg: MessageWrapper,
    meter: &mut crate::stats::StreamMeter,
    tx: &tokio::sync::mpsc::Sender<MessageWrapper>,
    audio_tx: &tokio::sync::mpsc::Sender<AudioStreamEvent>,
    stats: &Arc<crate::stats::NetworkStats>,
    events: &SharedEvents,
    takeover_token: &CancellationToken,
    connection_id: u64,
    active_connection: &SharedActiveConnection,
    active_audio_session: &SharedActiveAudioSession,
    plugins: Arc<crate::plugins::PluginHost>,
    peer_ip: std::net::IpAddr,
) {
    if let Some(audio) = msg.audio_packet {
        if !validate_audio_packet(&audio) {
            return;
        }
        let AudioPacketAcceptance::Accepted { epoch } =
            try_accept_audio_packet(active_audio_session, peer_ip, &audio)
        else {
            return;
        };
        meter.observe(stats, epoch, &audio, audio_payload_len(&audio));
        let permit = tokio::select! {
            biased;
            _ = takeover_token.cancelled() => return,
            result = audio_tx.clone().reserve_owned() => match result {
                Ok(permit) => permit,
                Err(_) => return,
            },
        };
        run_if_active(active_connection, takeover_token, connection_id, || {
            permit.send(AudioStreamEvent::Packet {
                packet: audio,
                epoch,
            });
        })
        .await;
    }
    if let Some(ping) = msg.ping {
        let pong_msg = MessageWrapper {
            audio_packet: None,
            connect: None,
            mute: None,
            ping: None,
            pong: Some(micyou_protocol::micyou::PongMessage {
                timestamp: ping.timestamp,
            }),
            plugin_message: None,
            stats: None,
        };
        let permit = tokio::select! {
            biased;
            _ = takeover_token.cancelled() => return,
            result = tx.clone().reserve_owned() => match result {
                Ok(permit) => permit,
                Err(_) => return,
            },
        };
        run_if_active(active_connection, takeover_token, connection_id, || {
            permit.send(pong_msg);
        })
        .await;
    }
    if let Some(pong) = msg.pong {
        let rtt = crate::stats::unix_millis() as i64 - pong.timestamp;
        if rtt >= 0 {
            run_if_active(active_connection, takeover_token, connection_id, || {
                stats.set_rtt(rtt)
            })
            .await;
        }
    }
    if let Some(mute) = msg.mute {
        // Mute sync disabled (server.json): ignore mute state coming from the
        // mobile client so it can neither change local state nor the UI.
        // A superseded connection must not flip the mute state either. The
        // plugin callbacks run outside the connection lock: plugins query
        // that lock (try_lock) and would misread a held one as disconnected.
        if crate::config::load_server_prefs().mute_sync
            && is_active(active_connection, takeover_token, connection_id).await
        {
            let is_muted = mute.is_muted.unwrap_or(false);
            log::info!("Received mute state: {}", is_muted);
            stats.set_muted(is_muted);
            events.mute_state_changed(is_muted);
            plugins.broadcast_event(&micyou_plugin::PluginEvent::MuteChanged { muted: is_muted });
        }
    }
    if let Some(plugin_message) = msg.plugin_message {
        // Cross-device plugin message: route to the bus (local plugins via the
        // dispatcher, pending RPCs via correlation id).
        if is_active(active_connection, takeover_token, connection_id).await {
            let logical = micyou_plugin::sync::from_wire(&plugin_message);
            plugins.bus.handle_incoming(&logical);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(payload_len: i32) -> [u8; FRAME_HEADER_LEN] {
        let mut header = [0; FRAME_HEADER_LEN];
        header[..4].copy_from_slice(&PACKET_MAGIC.to_be_bytes());
        header[4..].copy_from_slice(&payload_len.to_be_bytes());
        header
    }

    #[test]
    fn stats_message_clamps_out_of_range_values() {
        let metrics = crate::stats::AudioMetrics {
            bitrate: 0,
            sample_rate: 48_000,
            latency_ms: i64::MAX,
            network_latency_ms: -5,
            packet_loss_rate: 1.5,
            jitter_ms: 2.5,
            buffer_duration_ms: 30,
        };
        let stats = stats_message(&metrics).stats.unwrap();
        assert_eq!(stats.rtt_ms, 0);
        assert_eq!(stats.latency_ms, i32::MAX);
        assert_eq!(stats.packet_loss, 1.0);
        assert_eq!(stats.jitter_ms, 2.5);
    }

    #[test]
    fn connect_platform_is_normalized() {
        use micyou_protocol::micyou::ConnectMessage;

        let decode = |connect: ConnectMessage| {
            let payload = MessageWrapper {
                connect: Some(connect),
                ..Default::default()
            }
            .encode_to_vec();
            let wrapper = MessageWrapper::decode(payload.as_slice()).unwrap();
            ClientPlatform::from_wire(&wrapper.connect.unwrap().platform)
        };
        let connect = |platform: &str| ConnectMessage {
            session_id: 7,
            platform: platform.to_string(),
        };

        assert_eq!(decode(connect("ios")), ClientPlatform::Ios);
        assert_eq!(decode(connect("android")), ClientPlatform::Android);
        // Legacy clients leave the field empty; unexpected values are not trusted.
        assert_eq!(decode(connect("")), ClientPlatform::Unknown);
        assert_eq!(decode(connect("iOS ")), ClientPlatform::Unknown);
    }

    #[test]
    fn frame_header_rejects_negative_payload_length() {
        assert_eq!(
            parse_frame_header(&header(-1)).unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn frame_header_rejects_payload_over_limit() {
        assert_eq!(
            parse_frame_header(&header(MAX_CONTROL_PAYLOAD_LEN as i32 + 1))
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn frame_header_accepts_maximum_payload_length() {
        assert_eq!(
            parse_frame_header(&header(MAX_CONTROL_PAYLOAD_LEN as i32)).unwrap(),
            MAX_CONTROL_PAYLOAD_LEN
        );
    }

    #[tokio::test]
    async fn cancelled_takeover_token_rejects_side_effect() {
        let token = CancellationToken::new();
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        let active = Arc::new(Mutex::new(Some(ActiveConnection {
            sender,
            raw_socket: 0 as RawSocketHandle,
            connection_id: 1,
            takeover_token: token.clone(),
            platform: ClientPlatform::Unknown,
        })));
        token.cancel();
        let side_effect_ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let side_effect_ran_clone = side_effect_ran.clone();

        assert!(
            !run_if_active(&active, &token, 1, move || {
                side_effect_ran_clone.store(true, Ordering::SeqCst);
            })
            .await
        );
        assert!(!side_effect_ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn cleanup_session_state_takes_slot_before_cancel_and_only_uses_handle_once() {
        let token = CancellationToken::new();
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let raw = 42 as RawSocketHandle;
        let active_connection = Arc::new(Mutex::new(Some(ActiveConnection {
            sender,
            raw_socket: raw,
            connection_id: 1,
            takeover_token: token.clone(),
            platform: ClientPlatform::Unknown,
        })));
        let active_audio_session =
            Arc::new(std::sync::RwLock::new(ActiveAudioSession::UnboundLegacy {
                peer_ip: "127.0.0.1".parse().unwrap(),
                epoch: 1,
            }));
        let shutdown_handles = Arc::new(std::sync::Mutex::new(Vec::new()));

        let shutdown_handles_first = shutdown_handles.clone();
        let active_connection_at_shutdown = active_connection.clone();
        let token_at_shutdown = token.clone();
        cleanup_session_state_with(&active_connection, &active_audio_session, move |handle| {
            assert!(active_connection_at_shutdown.try_lock().unwrap().is_none());
            assert!(token_at_shutdown.is_cancelled());
            shutdown_handles_first.lock().unwrap().push(handle);
        })
        .await;

        let shutdown_handles_second = shutdown_handles.clone();
        cleanup_session_state_with(&active_connection, &active_audio_session, move |handle| {
            shutdown_handles_second.lock().unwrap().push(handle)
        })
        .await;

        assert_eq!(*shutdown_handles.lock().unwrap(), vec![raw]);
        assert!(token.is_cancelled());
        assert!(active_connection.lock().await.is_none());
        assert!(matches!(
            *active_audio_session.read().unwrap(),
            ActiveAudioSession::Inactive
        ));
        assert!(receiver.recv().await.is_none());
    }
}
