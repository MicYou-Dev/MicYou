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

use rcgen::{CertificateParams, KeyPair, SanType};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_WEB_PORT: u16 = 8443;

pub struct WebServer {
    cancel_token: std::sync::Mutex<CancellationToken>,
    client_count: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Best-effort IPv6 companion listener task (see `start`). The IPv4
    /// listener in `task` remains the primary one for lifecycle state.
    task_v6: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

struct GeneratedCert {
    cert_pem: String,
    key_pem: String,
}

/// Where the self-signed certificate lives. It sits next to the shared config
/// rather than in the world-readable temp directory because it holds the
/// TLS private key.
fn cert_cache_dir() -> PathBuf {
    crate::config::config_dir().join("web_cert")
}

/// LAN IPv4 addresses a browser may use to reach the web mode.
pub fn get_lan_ips() -> Vec<IpAddr> {
    let Ok(interfaces) = local_ip_address::list_afinet_netifas() else {
        return Vec::new();
    };
    interfaces
        .into_iter()
        .map(|(_, ip)| ip)
        .filter(|ip| match ip {
            IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_link_local() && !is_benchmark_v4(v4),
            IpAddr::V6(_) => false,
        })
        .collect()
}

/// 198.18.0.0/15 is used by proxy tools (Clash TUN) for fake IPs.
fn is_benchmark_v4(ip: &std::net::Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 198 && (b & 0xfe) == 18
}

/// Bindable LAN IPv6 addresses (ULA/GUA, best first). Additive companion to
/// `get_lan_ips`, used for the WebSocket origin check and certificate SANs.
pub fn get_lan_ipv6s() -> Vec<IpAddr> {
    crate::transport::net_bind::collect_ipv6_interfaces(&[])
        .into_iter()
        .map(|(ip, _)| IpAddr::V6(ip))
        .collect()
}

/// Every address the certificate must cover, loopback first.
fn certificate_ips() -> Vec<IpAddr> {
    let mut ips = vec![
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
    ];
    ips.extend(get_lan_ips());
    ips.extend(get_lan_ipv6s());
    ips
}

fn generate_self_signed_cert_pem(ips: &[IpAddr]) -> Result<GeneratedCert, String> {
    let mut params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| format!("Failed to create cert params: {}", e))?;
    params
        .subject_alt_names
        .extend(ips.iter().copied().map(SanType::IpAddress));

    let key_pair =
        KeyPair::generate().map_err(|e| format!("Failed to generate key pair: {}", e))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("Failed to sign certificate: {}", e))?;

    Ok(GeneratedCert {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    })
}

/// Write a file readable by the current user only.
fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    // `mode` only applies on creation; tighten a key file left by an older build.
    #[cfg(unix)]
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    std::io::Write::write_all(&mut file, contents.as_bytes())
}

/// Older versions kept the certificate and its private key in the temp
/// directory; nothing reads them any more.
fn remove_legacy_cert_dir() {
    let legacy = std::env::temp_dir().join("micyou_web_cert");
    if let Err(e) = std::fs::remove_dir_all(&legacy) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("[Web] could not remove {}: {e}", legacy.display());
        }
    }
}

/// Reuse the cached certificate while it still covers the current LAN
/// addresses, so browsers only re-accept it when the network changed.
fn load_or_generate_cert_pem() -> Result<GeneratedCert, String> {
    let cache_dir = cert_cache_dir();
    let cert_path = cache_dir.join("cert.pem");
    let key_path = cache_dir.join("key.pem");
    let sans_path = cache_dir.join("sans.txt");
    remove_legacy_cert_dir();
    let ips = certificate_ips();
    // Sorted, so a different interface enumeration order alone does not make
    // browsers accept a new certificate.
    let mut sans = ips.iter().map(IpAddr::to_string).collect::<Vec<_>>();
    sans.sort_unstable();
    sans.dedup();
    let sans = sans.join("\n");

    let cached = (
        std::fs::read_to_string(&cert_path),
        std::fs::read_to_string(&key_path),
        std::fs::read_to_string(&sans_path),
    );
    if let (Ok(cert_pem), Ok(key_pem), Ok(cached_sans)) = cached {
        if !cert_pem.is_empty() && !key_pem.is_empty() && cached_sans == sans {
            return Ok(GeneratedCert { cert_pem, key_pem });
        }
    }

    let cert = generate_self_signed_cert_pem(&ips)?;
    let persisted = std::fs::create_dir_all(&cache_dir)
        .and_then(|()| write_private(&key_path, &cert.key_pem))
        .and_then(|()| std::fs::write(&cert_path, &cert.cert_pem))
        .and_then(|()| std::fs::write(&sans_path, &sans));
    if let Err(e) = persisted {
        log::warn!("[Web] could not cache the TLS certificate: {e}");
    }
    Ok(cert)
}

fn float32_to_pcm16(float32_bytes: &[u8]) -> Vec<u8> {
    float32_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|chunk| {
            let sample = f32::from_le_bytes(*chunk).clamp(-1.0, 1.0);
            ((sample * 32767.0) as i16).to_le_bytes()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connection_check_reports_origin_rejection() {
        let missing = check_connection(HeaderMap::new()).await.into_response();
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);

        let mut headers = HeaderMap::new();
        headers.insert("x-micyou-origin", "https://evil.example".parse().unwrap());
        assert_eq!(
            check_connection(headers).await.into_response().status(),
            StatusCode::FORBIDDEN
        );

        let mut headers = HeaderMap::new();
        headers.insert("x-micyou-origin", "https://127.0.0.1:8443".parse().unwrap());
        assert_eq!(
            check_connection(headers).await.into_response().status(),
            StatusCode::OK
        );
    }

    #[test]
    fn test_generate_self_signed_cert_pem() {
        let cert = generate_self_signed_cert_pem(&certificate_ips());
        assert!(
            cert.is_ok(),
            "Cert generation should succeed: {:?}",
            cert.err()
        );
        let c = cert.unwrap();
        assert!(c.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(c.key_pem.contains("PRIVATE KEY"));
    }

    #[test]
    fn test_get_lan_ips() {
        for ip in get_lan_ips() {
            assert!(ip.is_ipv4() && !ip.is_loopback(), "Invalid IP: {}", ip);
        }
    }

    #[test]
    fn test_get_lan_ipv6s_are_bindable() {
        for ip in get_lan_ipv6s() {
            match ip {
                IpAddr::V6(v6) => {
                    assert!(crate::transport::net_bind::is_bindable_v6(&v6), "Not bindable: {}", ip)
                }
                IpAddr::V4(_) => panic!("get_lan_ipv6s returned IPv4: {}", ip),
            }
        }
    }

    #[test]
    fn test_is_valid_origin_v4_behavior_unchanged() {
        assert!(is_valid_origin(None));
        assert!(is_valid_origin(Some("http://localhost:8443")));
        assert!(is_valid_origin(Some("http://127.0.0.1:8443")));
        assert!(!is_valid_origin(Some("https://evil.example.com")));
        assert!(!is_valid_origin(Some("https://localhost.evil.example.com")));
        assert!(!is_valid_origin(Some("https://127.0.0.1.evil.example.com")));
        assert!(!is_valid_origin(Some("null")));
        for ip in get_lan_ips() {
            assert!(is_valid_origin(Some(&format!("https://{}:8443", ip))));
        }
    }

    #[test]
    fn test_is_valid_origin_accepts_ipv6() {
        // Browsers bracket IPv6 hosts in the Origin header.
        assert!(is_valid_origin(Some("https://[::1]:8443")));
        for ip in get_lan_ipv6s() {
            assert!(
                is_valid_origin(Some(&format!("https://[{}]:8443", ip))),
                "origin with [{}] rejected",
                ip
            );
        }
    }

    #[test]
    fn origin_host_strips_scheme_port_and_brackets() {
        assert_eq!(origin_host("https://192.168.1.2:8443"), Some("192.168.1.2"));
        assert_eq!(origin_host("https://[fd00::1]:8443"), Some("fd00::1"));
        assert_eq!(origin_host("https://[fd00::1]x"), None);
        assert_eq!(origin_host("localhost"), None);
    }

    #[test]
    fn test_float32_to_pcm16_one() {
        let input = 1.0f32.to_le_bytes();
        let pcm = float32_to_pcm16(&input);
        assert_eq!(pcm.len(), 2);
        let sample = i16::from_le_bytes([pcm[0], pcm[1]]);
        assert_eq!(sample, 32767);
    }

    #[test]
    fn test_float32_to_pcm16_neg_one() {
        let input = (-1.0f32).to_le_bytes();
        let pcm = float32_to_pcm16(&input);
        let sample = i16::from_le_bytes([pcm[0], pcm[1]]);
        assert_eq!(sample, -32767);
    }

    #[test]
    fn test_float32_to_pcm16_zero() {
        let input = 0.0f32.to_le_bytes();
        let pcm = float32_to_pcm16(&input);
        let sample = i16::from_le_bytes([pcm[0], pcm[1]]);
        assert_eq!(sample, 0);
    }

    #[test]
    fn decrement_client_count_does_not_underflow_after_stop_reset() {
        let count = AtomicUsize::new(0);
        assert_eq!(decrement_client_count(&count), 0);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn decrement_client_count_returns_the_remaining_clients() {
        let count = AtomicUsize::new(2);
        assert_eq!(decrement_client_count(&count), 1);
        assert_eq!(decrement_client_count(&count), 0);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn new_web_sender_closes_replaced_sender() {
        let senders = ActiveWebSender::default();
        let shutdown = CancellationToken::new();
        let (first_generation, first_cancel, first_replaced) = senders.activate(&shutdown);
        let (second_generation, second_cancel, second_replaced) = senders.activate(&shutdown);

        assert!(!first_replaced);
        assert!(second_replaced);
        assert!(first_cancel.is_cancelled());
        assert!(!second_cancel.is_cancelled());
        assert!(!senders.is_current(first_generation));
        assert!(senders.is_current(second_generation));
    }

    #[test]
    fn replacement_disconnect_does_not_restore_old_sender() {
        let senders = ActiveWebSender::default();
        let shutdown = CancellationToken::new();
        let (first_generation, first_cancel, _) = senders.activate(&shutdown);
        let (second_generation, _, _) = senders.activate(&shutdown);

        assert!(senders.deactivate(second_generation));
        assert!(!senders.is_current(first_generation));
        assert!(first_cancel.is_cancelled());
        assert!(!senders.deactivate(first_generation));
    }

    #[test]
    fn server_shutdown_closes_the_active_sender() {
        let senders = ActiveWebSender::default();
        let shutdown = CancellationToken::new();
        let (_, cancel, _) = senders.activate(&shutdown);
        shutdown.cancel();
        assert!(cancel.is_cancelled());
    }
}

use crate::events::SharedEvents;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::serve::Listener;
use axum::Router;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

const MAX_TLS_HANDSHAKES: usize = 32;
const MAX_WEBSOCKET_CONNECTIONS: usize = 8;
const TLS_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const WEB_CLIENT_HTML: &str = include_str!("../../assets/web_client.html");
const ALPINE_JS: &str = include_str!("../../assets/alpine.min.js");

/// Host part of an `Origin` header (`scheme://host[:port]`), with IPv6
/// brackets removed.
fn origin_host(origin: &str) -> Option<&str> {
    let (_, authority) = origin.split_once("://")?;
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        return (tail.is_empty() || tail.starts_with(':')).then_some(host);
    }
    let host = authority.split(':').next()?;
    (!host.is_empty()).then_some(host)
}

/// Only pages served by this machine may open the audio socket; any other
/// site could otherwise inject audio into the virtual microphone from the
/// user's browser.
fn is_valid_origin(origin: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true;
    };
    let Some(host) = origin_host(origin) else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let Ok(ip) = host.parse::<IpAddr>() else {
        return false;
    };
    ip.is_loopback() || get_lan_ips().contains(&ip) || get_lan_ipv6s().contains(&ip)
}

// A normal HTTP request can expose diagnostics that a browser hides during a
// failed WebSocket handshake. This does not authorize the later upgrade.
async fn check_connection(headers: HeaderMap) -> impl IntoResponse {
    let origin = headers.get("x-micyou-origin").and_then(|v| v.to_str().ok());
    if origin.is_some() && is_valid_origin(origin) {
        (StatusCode::OK, "Ready")
    } else {
        (StatusCode::FORBIDDEN, "Invalid origin")
    }
}

async fn handle_websocket(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<WebServerState>,
) -> impl IntoResponse {
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    if !is_valid_origin(origin) {
        return (StatusCode::FORBIDDEN, "Invalid origin").into_response();
    }
    let permit = match state.websocket_slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Too many WebSocket connections",
            )
                .into_response()
        }
    };
    ws.on_upgrade(move |socket| handle_ws_socket(socket, state, permit))
}

async fn handle_ws_socket(
    mut socket: WebSocket,
    state: WebServerState,
    _permit: OwnedSemaphorePermit,
) {
    let (generation, cancel, replaced) = state.active_sender.activate(&state.shutdown);
    let count = if replaced {
        state.client_count.load(Ordering::SeqCst)
    } else {
        state.client_count.fetch_add(1, Ordering::SeqCst) + 1
    };
    state.events.web_client_count(count as u32);
    log::info!("Web client connected (total: {})", count);

    if count == 1 && !replaced {
        state
            .events
            .device_connected(crate::transport::tcp::DeviceInfo {
                name: "Web Browser".to_string(),
                ip: "browser".to_string(),
                latency: 0,
                platform: crate::transport::tcp::ClientPlatform::Unknown,
            });
    }

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            message = socket.recv() => match message {
            Some(Ok(Message::Binary(data))) => {
                if !state.active_sender.is_current(generation) {
                    break;
                }
                // The bundled client never sends these; drop the peer
                // instead of logging every malformed frame.
                if data.len() > 64 * 1024 || !data.len().is_multiple_of(4) {
                    log::warn!("Web audio frame of {} bytes is malformed, closing", data.len());
                    break;
                }

                let pcm = float32_to_pcm16(&data);
                let packet = micyou_protocol::micyou::AudioPacketMessage {
                    buffer: pcm,
                    sample_rate: 48000,
                    channel_count: 1,
                    audio_format: 2,
                    codec: micyou_protocol::CODEC_PCM,
                };
                match state.audio_tx.try_send((generation, packet)) {
                    Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            Some(Err(e)) => {
                log::warn!("WebSocket error: {}", e);
                break;
            }
            _ => {}
            }
        }
    }

    if state.active_sender.deactivate(generation) {
        let remaining = decrement_client_count(&state.client_count);
        state.events.web_client_count(remaining as u32);
        log::info!("Web client disconnected (remaining: {})", remaining);

        if remaining == 0 {
            state.events.device_disconnected();
        }
    } else {
        log::info!("Replaced Web client closed");
    }
}

/// Decrement without wrapping below zero; returns the new count.
fn decrement_client_count(client_count: &AtomicUsize) -> usize {
    client_count
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |count| count.checked_sub(1))
        .map_or(0, |previous| previous - 1)
}

async fn serve_html() -> impl IntoResponse {
    Html(WEB_CLIENT_HTML)
}

async fn serve_alpine_js() -> impl IntoResponse {
    ([("Content-Type", "application/javascript")], ALPINE_JS)
}

#[derive(Default)]
struct ActiveWebSender {
    generation: AtomicU64,
    cancel: std::sync::Mutex<Option<(u64, CancellationToken)>>,
}

impl ActiveWebSender {
    /// `shutdown` is the server's token: stopping the server must also end
    /// upgraded sockets, which axum's graceful shutdown no longer tracks.
    fn activate(&self, shutdown: &CancellationToken) -> (u64, CancellationToken, bool) {
        let cancel = shutdown.child_token();
        let mut active = self.cancel.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let replaced = if let Some((_, previous)) = active.replace((generation, cancel.clone())) {
            previous.cancel();
            true
        } else {
            false
        };
        (generation, cancel, replaced)
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    fn deactivate(&self, generation: u64) -> bool {
        let mut active = self.cancel.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.as_ref().is_some_and(|(current, _)| *current == generation) {
            active.take();
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct WebServerState {
    pub events: SharedEvents,
    pub audio_tx: tokio::sync::mpsc::Sender<(u64, micyou_protocol::micyou::AudioPacketMessage)>,
    pub client_count: Arc<AtomicUsize>,
    active_sender: Arc<ActiveWebSender>,
    pub websocket_slots: Arc<Semaphore>,
    shutdown: CancellationToken,
}

struct TlsListener {
    tcp: TcpListener,
    acceptor: TlsAcceptor,
    handshake_slots: Arc<Semaphore>,
    completed: tokio::sync::mpsc::Sender<(TlsStream<TcpStream>, SocketAddr)>,
    completed_rx: tokio::sync::mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
}

impl Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            tokio::select! {
                Some(accepted) = self.completed_rx.recv() => return accepted,
                accept_result = self.tcp.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            let permit = match self.handshake_slots.clone().try_acquire_owned() {
                                Ok(permit) => permit,
                                Err(_) => continue,
                            };
                            let acceptor = self.acceptor.clone();
                            let completed = self.completed.clone();
                            tokio::spawn(async move {
                                let _permit = permit;
                                match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
                                    Ok(Ok(tls)) => { let _ = completed.send((tls, addr)).await; }
                                    Ok(Err(e)) => log::debug!("TLS handshake failed: {}", e),
                                    Err(_) => log::debug!("TLS handshake timed out for {}", addr),
                                }
                            });
                        }
                        Err(e) => {
                            log::warn!("TCP accept error: {}", e);
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.tcp.local_addr()
    }
}

impl Default for WebServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WebServer {
    pub fn new() -> Self {
        Self {
            cancel_token: std::sync::Mutex::new(CancellationToken::new()),
            client_count: Arc::new(AtomicUsize::new(0)),
            running: Arc::new(AtomicBool::new(false)),
            task: std::sync::Mutex::new(None),
            task_v6: std::sync::Mutex::new(None),
        }
    }

    pub fn client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub async fn start(
        &self,
        port: u16,
        bind_address: &str,
        events: SharedEvents,
        audio_tx: tokio::sync::mpsc::Sender<(u64, micyou_protocol::micyou::AudioPacketMessage)>,
    ) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Web server is already running".to_string());
        }

        let new_token = CancellationToken::new();
        let state = WebServerState {
            events,
            audio_tx,
            client_count: self.client_count.clone(),
            active_sender: Arc::new(ActiveWebSender::default()),
            websocket_slots: Arc::new(Semaphore::new(MAX_WEBSOCKET_CONNECTIONS)),
            shutdown: new_token.clone(),
        };

        let app = Router::new()
            .route("/", get(serve_html))
            .route("/alpine.min.js", get(serve_alpine_js))
            .route("/connection-check", get(check_connection))
            .route("/ws", get(handle_websocket))
            .with_state(state);
        // Clone for the best-effort IPv6 listener started below; the IPv4
        // serve task keeps consuming the original exactly as before.
        let app_v6 = app.clone();

        // Load TLS certificate
        let cert = load_or_generate_cert_pem()?;
        let cert_chain = CertificateDer::pem_slice_iter(cert.cert_pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read certificate: {}", e))?;
        let private_key = PrivateKeyDer::from_pem_slice(cert.key_pem.as_bytes())
            .map_err(|e| format!("Failed to read private key: {}", e))?;

        let mut tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(|e| format!("TLS config error: {}", e))?;

        tls_config.alpn_protocols = vec![b"http/1.1".to_vec()];

        let acceptor = TlsAcceptor::from(Arc::new(tls_config));
        let acceptor_v6 = acceptor.clone();

        let tcp = crate::transport::net_bind::bind_tcp_listener(bind_address, port)
            .await
            .map_err(|e| format!("Web server bind error: {}", e))?;

        let (completed, completed_rx) = tokio::sync::mpsc::channel(MAX_TLS_HANDSHAKES);
        let tls_listener = TlsListener {
            tcp,
            acceptor,
            handshake_slots: Arc::new(Semaphore::new(MAX_TLS_HANDSHAKES)),
            completed,
            completed_rx,
        };

        log::info!(
            "Web server listening on https://{}",
            crate::transport::net_bind::normalize_socket_addr(bind_address, port)
        );

        *self
            .cancel_token
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = new_token.clone();
        let cancel_v6 = new_token.clone();
        let cancel = new_token;
        let running = self.running.clone();
        let client_count = self.client_count.clone();

        running.store(true, Ordering::SeqCst);

        let task = tokio::spawn(async move {
            axum::serve(tls_listener, app)
                .with_graceful_shutdown(async move {
                    cancel.cancelled().await;
                })
                .await
                .ok();

            running.store(false, Ordering::SeqCst);
            client_count.store(0, Ordering::SeqCst);
        });
        *self.task.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(task);

        // Best-effort IPv6 companion listener so phones on IPv6-only or
        // IPv6-preferring networks can reach web mode. It is strictly
        // additive: the IPv4 listener above is untouched, `only_v6(true)`
        // keeps the two sockets from conflicting, and a failure here (no
        // IPv6 stack, port unavailable) only logs and leaves web mode
        // working exactly as before over IPv4.
        if !crate::transport::net_bind::wants_v6_companion(bind_address) {
            return Ok(());
        }
        match crate::transport::net_bind::bind_tcp_listener_v6only(port) {
            Ok(tcp_v6) => {
                let (completed_v6, completed_rx_v6) =
                    tokio::sync::mpsc::channel(MAX_TLS_HANDSHAKES);
                let tls_listener_v6 = TlsListener {
                    tcp: tcp_v6,
                    acceptor: acceptor_v6,
                    handshake_slots: Arc::new(Semaphore::new(MAX_TLS_HANDSHAKES)),
                    completed: completed_v6,
                    completed_rx: completed_rx_v6,
                };
                log::info!("Web server listening on https://[::]:{} (IPv6)", port);
                let task_v6 = tokio::spawn(async move {
                    axum::serve(tls_listener_v6, app_v6)
                        .with_graceful_shutdown(async move {
                            cancel_v6.cancelled().await;
                        })
                        .await
                        .ok();
                });
                *self
                    .task_v6
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(task_v6);
            }
            Err(e) => {
                log::warn!(
                    "IPv6 web listener not started: {}{}",
                    e,
                    crate::transport::net_bind::companion_failure_hint(&e)
                );
            }
        }

        Ok(())
    }

    pub async fn stop(&self) {
        self.cancel_token
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .cancel();
        // The IPv6 companion listener shares the cancel token.
        for slot in [&self.task, &self.task_v6] {
            let task = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
            if let Some(mut task) = task {
                if tokio::time::timeout(std::time::Duration::from_secs(3), &mut task)
                    .await
                    .is_err()
                {
                    task.abort();
                    let _ = task.await;
                }
            }
        }
        self.running.store(false, Ordering::SeqCst);
        self.client_count.store(0, Ordering::SeqCst);
    }
}
