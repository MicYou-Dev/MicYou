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

//! Plugin message bus: local pub/sub, RPC, and cross-device relay.
//!
//! One `PluginBus` per host. It connects three worlds:
//! - **Local plugins** (native/wasm) exchanging events through `publish` and
//!   topic subscriptions,
//! - **The remote device** (phone) through a `PluginSyncTransport`,
//! - **RPC callers** (plugins or host code) waiting on correlation ids.
//!
//! The message shape is the logical counterpart of the wire format in
//! `micyou-protocol` (`micyou::PluginMessage`); `sync.rs` maps between them.
//! Android reuses the same wire format and the same `PluginBus` semantics,
//! only swapping the transport implementation.

use crate::error::{PluginError, PluginResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// Logical plugin message (transport-agnostic).
#[derive(Debug, Clone, PartialEq)]
pub struct PluginMessage {
    pub source: String,
    /// Receiving plugin id; empty = broadcast.
    pub target: String,
    pub topic: String,
    pub payload: Vec<u8>,
    /// Pairs a request with its response; 0 = one-way.
    pub correlation_id: u64,
    pub is_response: bool,
    /// Mirrors `PluginError` codes; 0 = ok.
    pub error_code: i32,
    pub error_message: String,
}

impl PluginMessage {
    pub fn new(source: &str, target: &str, topic: &str, payload: Vec<u8>) -> Self {
        Self {
            source: source.to_string(),
            target: target.to_string(),
            topic: topic.to_string(),
            payload,
            correlation_id: 0,
            is_response: false,
            error_code: 0,
            error_message: String::new(),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error_code == 0
    }
}

/// Transport to the remote device. The desktop implementation rides the TCP
/// control channel (`MessageWrapper.pluginMessage`); a future Android
/// implementation uses the same channel from the phone side.
pub trait PluginSyncTransport: Send + Sync {
    /// Push a message to the connected device. Errors when offline.
    fn send(&self, msg: &PluginMessage) -> PluginResult<()>;
    /// Whether a device session is currently connected.
    fn is_connected(&self) -> bool;
}

/// Transport used before any device connects.
pub struct NullTransport;

impl PluginSyncTransport for NullTransport {
    fn send(&self, _msg: &PluginMessage) -> PluginResult<()> {
        Err(PluginError::MessageDelivery(
            "no device connected".to_string(),
        ))
    }
    fn is_connected(&self) -> bool {
        false
    }
}

type TopicHandler = dyn Fn(&PluginMessage) + Send + Sync;
type LocalDispatcher = dyn Fn(&PluginMessage) -> PluginResult<()> + Send + Sync;

/// The host-side message bus.
pub struct PluginBus {
    transport: Arc<dyn PluginSyncTransport>,
    /// Routes incoming messages to local plugins (set by the host).
    local_dispatcher: Arc<LocalDispatcher>,
    subscriptions: RwLock<HashMap<String, Vec<Arc<TopicHandler>>>>,
    pending: Mutex<HashMap<u64, std::sync::mpsc::Sender<PluginResult<Vec<u8>>>>>,
    next_correlation: AtomicU64,
}

impl PluginBus {
    pub fn new(
        transport: Arc<dyn PluginSyncTransport>,
        local_dispatcher: Arc<LocalDispatcher>,
    ) -> Self {
        Self {
            transport,
            local_dispatcher,
            subscriptions: RwLock::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
            next_correlation: AtomicU64::new(1),
        }
    }

    pub fn transport(&self) -> &Arc<dyn PluginSyncTransport> {
        &self.transport
    }

    /// Replace the transport (e.g. when the device session changes).
    pub fn set_transport(&self, transport: Arc<dyn PluginSyncTransport>) {
        // Swapping an `Arc<dyn>` field requires interior mutability; the bus is
        // shared, so we box the old one as-is and rely on a Mutex<Arc> wrapper.
        // (Callers should prefer constructing the bus once with the final
        // transport and letting the adapter itself point at the live session.)
        let _ = transport;
        log::warn!("[plugins] set_transport is a no-op placeholder");
    }

    // ── Local pub/sub ──────────────────────────────────────────────────────

    /// Subscribe to a topic. Returns an id usable with `unsubscribe`.
    pub fn subscribe(&self, topic: &str, handler: Arc<TopicHandler>) -> PluginResult<()> {
        let mut subs = self
            .subscriptions
            .write()
            .map_err(|_| PluginError::Runtime("bus subscriptions poisoned".into()))?;
        subs.entry(topic.to_string()).or_default().push(handler);
        Ok(())
    }

    /// Publish an event: local subscribers run, then the remote device
    /// receives it as a broadcast (empty target) if connected.
    pub fn publish(&self, topic: &str, payload: Vec<u8>) -> PluginResult<()> {
        let msg = PluginMessage::new("host", "", topic, payload);
        // Local subscribers (run under a read guard snapshot to avoid holding
        // the lock across handler calls).
        let handlers: Vec<Arc<TopicHandler>> = self
            .subscriptions
            .read()
            .map_err(|_| PluginError::Runtime("bus subscriptions poisoned".into()))?
            .get(topic)
            .cloned()
            .unwrap_or_default();
        for handler in handlers {
            handler(&msg);
        }
        // Relay to the remote device.
        if self.transport.is_connected() {
            self.transport.send(&msg)?;
        }
        Ok(())
    }

    // ── RPC ────────────────────────────────────────────────────────────────

    /// Send a request to `target` (local or remote) and wait for its response.
    /// Blocks the calling thread up to `timeout`; never call this from the
    /// real-time audio thread.
    pub fn request(
        &self,
        target: &str,
        topic: &str,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> PluginResult<Vec<u8>> {
        let correlation_id = self.next_correlation.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| PluginError::Runtime("bus pending poisoned".into()))?
            .insert(correlation_id, tx);

        let msg = PluginMessage {
            correlation_id,
            ..PluginMessage::new("host", target, topic, payload)
        };
        if let Err(e) = self.transport.send(&msg) {
            self.pending
                .lock()
                .map_err(|_| PluginError::Runtime("bus pending poisoned".into()))?
                .remove(&correlation_id);
            return Err(e);
        }
        // Also deliver locally when the target is a local plugin (the host
        // dispatcher routes by plugin id; response comes back through
        // `complete_request` from the plugin's own reply).
        let _ = (self.local_dispatcher)(&msg);

        match rx.recv_timeout(timeout) {
            Ok(Ok(payload)) => Ok(payload),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                self.pending
                    .lock()
                    .map_err(|_| PluginError::Runtime("bus pending poisoned".into()))?
                    .remove(&correlation_id);
                Err(PluginError::MessageDelivery(format!(
                    "request to {target} timed out after {timeout:?}"
                )))
            }
        }
    }

    /// Complete a pending RPC (called by the host when a plugin responds).
    pub fn complete_request(&self, correlation_id: u64, result: PluginResult<Vec<u8>>) {
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(tx) = pending.remove(&correlation_id) {
                let _ = tx.send(result);
            }
        }
    }

    /// Build a response message for a request.
    pub fn respond(&self, request: &PluginMessage, result: PluginResult<Vec<u8>>) -> PluginMessage {
        match result {
            Ok(payload) => PluginMessage {
                source: request.target.clone(),
                target: request.source.clone(),
                topic: request.topic.clone(),
                payload,
                correlation_id: request.correlation_id,
                is_response: true,
                error_code: 0,
                error_message: String::new(),
            },
            Err(e) => PluginMessage {
                source: request.target.clone(),
                target: request.source.clone(),
                topic: request.topic.clone(),
                payload: Vec::new(),
                correlation_id: request.correlation_id,
                is_response: true,
                error_code: error_code(&e),
                error_message: e.to_string(),
            },
        }
    }

    // ── Incoming routing ───────────────────────────────────────────────────

    /// Handle a message that arrived from the remote device (or a local
    /// plugin's reply). Responses complete pending RPCs; requests/events are
    /// routed to the local dispatcher and topic subscribers.
    pub fn handle_incoming(&self, msg: &PluginMessage) {
        if msg.is_response {
            let result = if msg.is_ok() {
                Ok(msg.payload.clone())
            } else {
                Err(PluginError::Runtime(msg.error_message.clone()))
            };
            self.complete_request(msg.correlation_id, result);
            return;
        }
        // Request or one-way event: route to local plugins.
        let _ = (self.local_dispatcher)(msg);
        // And to topic subscribers.
        if let Ok(subs) = self.subscriptions.read() {
            if let Some(handlers) = subs.get(&msg.topic) {
                let handlers = handlers.clone();
                drop(subs);
                for handler in handlers {
                    handler(msg);
                }
            }
        }
    }

    /// Pending RPC count (diagnostics/tests).
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }
}

/// Map a `PluginError` to the wire error code.
pub fn error_code(error: &PluginError) -> i32 {
    match error {
        PluginError::NotFound(_) => 1,
        PluginError::InvalidManifest(_) => 2,
        PluginError::Validation(_) => 3,
        PluginError::UnknownPlugin(_) => 4,
        PluginError::NotLoaded(_) => 5,
        PluginError::LoadFailed(_) => 6,
        PluginError::ApiVersionMismatch { .. } => 7,
        PluginError::PermissionDenied(_) => 8,
        PluginError::AlreadyExists(_) => 9,
        PluginError::Runtime(_) => 10,
        PluginError::MessageDelivery(_) => 11,
        PluginError::Io(_) => 12,
    }
}

/// Reverse-map a wire error code to a human message (best effort).
pub fn error_message_for(code: i32) -> String {
    match code {
        0 => "ok".to_string(),
        1 => "not found".to_string(),
        2 => "invalid manifest".to_string(),
        3 => "validation failed".to_string(),
        4 => "unknown plugin".to_string(),
        5 => "not loaded".to_string(),
        6 => "load failed".to_string(),
        7 => "api version mismatch".to_string(),
        8 => "permission denied".to_string(),
        9 => "already exists".to_string(),
        10 => "runtime error".to_string(),
        11 => "message delivery failed".to_string(),
        12 => "io error".to_string(),
        _ => format!("unknown error {code}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[derive(Default)]
    struct FakeTransport {
        sent: Mutex<Vec<PluginMessage>>,
        connected: AtomicUsize,
    }

    impl PluginSyncTransport for FakeTransport {
        fn send(&self, msg: &PluginMessage) -> PluginResult<()> {
            self.sent.lock().unwrap().push(msg.clone());
            Ok(())
        }
        fn is_connected(&self) -> bool {
            self.connected.load(AtomicOrdering::SeqCst) == 1
        }
    }

    fn noop_dispatcher() -> Arc<LocalDispatcher> {
        Arc::new(|_| Ok(()))
    }

    #[test]
    fn publish_runs_local_subscribers_and_relays_to_transport() {
        let transport = Arc::new(FakeTransport::default());
        transport.connected.store(1, AtomicOrdering::SeqCst);
        let bus = PluginBus::new(transport.clone(), noop_dispatcher());

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        bus.subscribe(
            "sensor.data",
            Arc::new(move |msg: &PluginMessage| {
                assert_eq!(msg.topic, "sensor.data");
                assert!(msg.target.is_empty(), "publish is a broadcast");
                calls_clone.fetch_add(1, AtomicOrdering::SeqCst);
            }),
        )
        .unwrap();

        bus.publish("sensor.data", vec![1, 2, 3]).unwrap();
        assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
        let sent = transport.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].payload, vec![1, 2, 3]);
    }

    #[test]
    fn request_completes_when_remote_responds() {
        let transport = Arc::new(FakeTransport::default());
        transport.connected.store(1, AtomicOrdering::SeqCst);
        let bus = Arc::new(PluginBus::new(transport.clone(), noop_dispatcher()));

        let bus_clone = bus.clone();
        let responder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            // The remote echoes: response with same correlation id.
            let sent = transport.sent.lock().unwrap().clone();
            let request = sent.into_iter().next().expect("request sent");
            let response = bus_clone.respond(&request, Ok(vec![9, 9]));
            bus_clone.handle_incoming(&response);
        });

        let result = bus
            .request(
                "dev.micyou.peer",
                "calc",
                vec![1, 2],
                Duration::from_secs(1),
            )
            .expect("RPC must complete");
        assert_eq!(result, vec![9, 9]);
        responder.join().unwrap();
        assert_eq!(bus.pending_count(), 0);
    }

    #[test]
    fn request_times_out_when_remote_silent() {
        let transport = Arc::new(FakeTransport::default());
        transport.connected.store(1, AtomicOrdering::SeqCst);
        let bus = PluginBus::new(transport.clone(), noop_dispatcher());

        let result = bus.request(
            "dev.micyou.silent",
            "ping",
            Vec::new(),
            Duration::from_millis(50),
        );
        assert!(matches!(result, Err(PluginError::MessageDelivery(_))));
        assert_eq!(bus.pending_count(), 0, "timed-out RPC must be cleaned up");
    }

    #[test]
    fn error_response_carries_message() {
        let transport = Arc::new(FakeTransport::default());
        transport.connected.store(1, AtomicOrdering::SeqCst);
        let bus = PluginBus::new(transport.clone(), noop_dispatcher());

        let request = PluginMessage::new("a", "b", "t", vec![]);
        let response = bus.respond(&request, Err(PluginError::PermissionDenied("nope".into())));
        assert!(response.is_response);
        assert!(!response.is_ok());
        assert_eq!(
            response.error_code,
            error_code(&PluginError::PermissionDenied("nope".into()))
        );
        assert_eq!(response.error_message, "capability not granted: nope");
    }

    #[test]
    fn error_code_mapping_is_stable() {
        // These codes are part of the wire protocol — changing them breaks
        // compatibility with already-shipped peers.
        assert_eq!(error_code(&PluginError::NotFound("".into())), 1);
        assert_eq!(error_code(&PluginError::Validation("".into())), 3);
        assert_eq!(error_code(&PluginError::PermissionDenied("".into())), 8);
        assert_eq!(error_code(&PluginError::Runtime("".into())), 10);
        assert_eq!(error_code(&PluginError::MessageDelivery("".into())), 11);
    }
}
