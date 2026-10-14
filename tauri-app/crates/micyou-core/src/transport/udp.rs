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

use tokio::net::UdpSocket;

use micyou_protocol::micyou::MessageWrapper;
use micyou_protocol::UDP_PACKET_MAGIC;
use prost::Message;
use std::error::Error;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

use crate::transport::session::{can_bind_legacy_packet, validate_audio_packet, AudioStreamEvent};
use micyou_protocol::micyou::AudioPacketMessageOrdered;

const UDP_HEADER_LEN: usize = 8;
// A protobuf wrapper around Android's <=1,400-byte PCM/FEC chunks. Keep datagrams
// below the UDP protocol maximum; nested audio buffers are validated separately.
pub const MAX_AUDIO_PAYLOAD_LEN: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActiveAudioSession {
    #[default]
    Inactive,
    UnboundLegacy {
        peer_ip: IpAddr,
        epoch: u64,
    },
    Bound {
        peer_ip: IpAddr,
        session_id: i64,
        epoch: u64,
    },
}

pub type SharedActiveAudioSession = Arc<RwLock<ActiveAudioSession>>;

fn parse_datagram(datagram: &[u8]) -> Option<&[u8]> {
    if datagram.len() < UDP_HEADER_LEN {
        return None;
    }
    let magic = i32::from_be_bytes(datagram[0..4].try_into().ok()?);
    if magic != UDP_PACKET_MAGIC {
        return None;
    }
    let payload_len_i32 = i32::from_be_bytes(datagram[4..8].try_into().ok()?);
    if payload_len_i32 < 0 {
        return None;
    }
    let payload_len = usize::try_from(payload_len_i32).ok()?;
    if payload_len > MAX_AUDIO_PAYLOAD_LEN {
        return None;
    }
    let end = UDP_HEADER_LEN.checked_add(payload_len)?;
    if end > datagram.len() {
        return None;
    }
    Some(&datagram[UDP_HEADER_LEN..end])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioPacketAcceptance {
    Rejected,
    Accepted { epoch: u64 },
}

pub fn try_accept_audio_packet(
    active_audio_session: &SharedActiveAudioSession,
    source_ip: IpAddr,
    packet: &AudioPacketMessageOrdered,
) -> AudioPacketAcceptance {
    // Every audio packet passes through here, so the common bound case only
    // takes the read lock; the write lock is reserved for binding a legacy
    // stream to its first packet's session ID.
    let current = *active_audio_session
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match current {
        ActiveAudioSession::Inactive => AudioPacketAcceptance::Rejected,
        ActiveAudioSession::Bound {
            peer_ip,
            session_id,
            epoch,
        } => {
            if peer_ip == source_ip && session_id == packet.session_id {
                AudioPacketAcceptance::Accepted { epoch }
            } else {
                AudioPacketAcceptance::Rejected
            }
        }
        ActiveAudioSession::UnboundLegacy { peer_ip, .. } => {
            if peer_ip != source_ip || !can_bind_legacy_packet(packet) {
                return AudioPacketAcceptance::Rejected;
            }
            let mut active = active_audio_session
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // Another packet may have bound the stream (or a new connection
            // replaced it) between the two locks: re-check under the write lock.
            match *active {
                ActiveAudioSession::UnboundLegacy { peer_ip, epoch } if peer_ip == source_ip => {
                    *active = ActiveAudioSession::Bound {
                        peer_ip,
                        session_id: packet.session_id,
                        epoch,
                    };
                    AudioPacketAcceptance::Accepted { epoch }
                }
                ActiveAudioSession::Bound {
                    peer_ip,
                    session_id,
                    epoch,
                } if peer_ip == source_ip && session_id == packet.session_id => {
                    AudioPacketAcceptance::Accepted { epoch }
                }
                _ => AudioPacketAcceptance::Rejected,
            }
        }
    }
}

pub async fn start_udp_server(
    tx: Sender<AudioStreamEvent>,
    port: u16,
    bind_address: String,
    cancel_token: CancellationToken,
    stats: std::sync::Arc<crate::stats::NetworkStats>,
    active_audio_session: SharedActiveAudioSession,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let bound = crate::transport::net_bind::bind_udp_socket(&bind_address, port)
        .and_then(UdpSocket::from_std);
    let socket = match bound {
        Ok(socket) => socket,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return Err(error.into());
        }
    };
    // Additive IPv6 companion socket for the legacy IPv4 auto-bind
    // ("0.0.0.0"): v6-only on the same port, so IPv4 and IPv6 clients are
    // each served by their own socket and peer addresses never mix families
    // (the audio-session gate compares TCP peer IP against UDP source IP).
    // Failure is non-fatal and only logged. Bound BEFORE signalling ready so
    // no datagram window is missed.
    let v6_socket = if crate::transport::net_bind::wants_v6_companion(&bind_address) {
        match crate::transport::net_bind::bind_udp_socket_v6only(port).and_then(UdpSocket::from_std) {
            Ok(socket) => {
                log::info!(
                    "UDP Audio Server also listening on [::]:{} (IPv6 companion)",
                    port
                );
                Some(socket)
            }
            Err(error) => {
                log::warn!(
                    "IPv6 companion UDP socket not started: {}{}",
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
    log::info!("UDP Audio Server listening on {}", port);

    let mut buf = vec![0u8; 65535];
    // Separate receive buffer for the IPv6 companion socket; stays empty
    // (never written) when there is no companion.
    let mut buf_v6 = if v6_socket.is_some() {
        vec![0u8; 65535]
    } else {
        Vec::new()
    };

    let mut meter = crate::stats::StreamMeter::default();

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("UDP Server cancelled");
                break;
            }
            recv_result = crate::transport::net_bind::recv_from_either(&socket, v6_socket.as_ref(), &mut buf, &mut buf_v6) => {
                let (len, addr, from_v6) = match recv_result {
                    Ok(res) => res,
                    Err(e) => {
                        log::warn!("UDP recv error: {}", e);
                        continue;
                    }
                };

                // Each socket received into its own buffer (see
                // net_bind::recv_from_either); everything below is the
                // untouched legacy processing, family-agnostic via addr.ip().
                let recv_buf = if from_v6 { &buf_v6[..len] } else { &buf[..len] };
                let Some(payload) = parse_datagram(recv_buf) else {
                    continue;
                };
                match MessageWrapper::decode(payload) {
                    Ok(msg) => {
                        if let Some(audio_packet_ordered) = msg.audio_packet {
                            if !validate_audio_packet(&audio_packet_ordered) {
                                continue;
                            }
                            let AudioPacketAcceptance::Accepted { epoch } = try_accept_audio_packet(
                                &active_audio_session,
                                addr.ip(),
                                &audio_packet_ordered,
                            ) else {
                                continue;
                            };
                            stats.mark_udp_received(crate::stats::unix_millis());
                            meter.observe(&stats, epoch, &audio_packet_ordered, payload.len());

                            match tx.try_send(AudioStreamEvent::Packet {
                                packet: audio_packet_ordered,
                                epoch,
                            }) {
                                Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to decode UDP payload from {}: {}", addr, e);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn datagram(payload_len: i32, actual_payload: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(UDP_HEADER_LEN + actual_payload);
        bytes.extend_from_slice(&UDP_PACKET_MAGIC.to_be_bytes());
        bytes.extend_from_slice(&payload_len.to_be_bytes());
        bytes.resize(UDP_HEADER_LEN + actual_payload, 7);
        bytes
    }

    #[test]
    fn parser_rejects_negative_length() {
        assert!(parse_datagram(&datagram(-1, 0)).is_none());
    }

    #[test]
    fn parser_rejects_over_limit_length() {
        assert!(parse_datagram(&datagram(MAX_AUDIO_PAYLOAD_LEN as i32 + 1, 0)).is_none());
    }

    #[test]
    fn parser_rejects_truncated_payload() {
        assert!(parse_datagram(&datagram(4, 3)).is_none());
    }

    #[test]
    fn parser_rejects_bad_magic() {
        let mut bytes = datagram(0, 0);
        bytes[0] ^= 1;
        assert!(parse_datagram(&bytes).is_none());
    }

    #[test]
    fn parser_accepts_maximum_payload_boundary() {
        let bytes = datagram(MAX_AUDIO_PAYLOAD_LEN as i32, MAX_AUDIO_PAYLOAD_LEN);
        assert_eq!(parse_datagram(&bytes).unwrap().len(), MAX_AUDIO_PAYLOAD_LEN);
    }

    fn accepted(result: AudioPacketAcceptance) -> bool {
        matches!(result, AudioPacketAcceptance::Accepted { .. })
    }

    fn packet(
        sequence_number: i32,
        fec_sequence_number: i32,
        session_id: i64,
    ) -> AudioPacketMessageOrdered {
        AudioPacketMessageOrdered {
            sequence_number,
            audio_packet: None,
            timestamp: 0,
            fec_buffer: Vec::new(),
            fec_sequence_number,
            session_id,
            fec_packet_lengths: Vec::new(),
        }
    }

    #[test]
    fn modern_and_inactive_filters_are_strict() {
        let ip: IpAddr = "192.168.1.2".parse().unwrap();
        let other: IpAddr = "192.168.1.3".parse().unwrap();
        let active = Arc::new(RwLock::new(ActiveAudioSession::Bound {
            peer_ip: ip,
            session_id: 202,
            epoch: 7,
        }));
        assert_eq!(
            try_accept_audio_packet(&active, ip, &packet(99, -1, 202)),
            AudioPacketAcceptance::Accepted { epoch: 7 }
        );
        assert!(!accepted(try_accept_audio_packet(
            &active,
            ip,
            &packet(0, -1, 101)
        )));
        assert!(!accepted(try_accept_audio_packet(
            &active,
            other,
            &packet(0, -1, 202)
        )));

        *active.write().unwrap() = ActiveAudioSession::Inactive;
        assert!(!accepted(try_accept_audio_packet(
            &active,
            ip,
            &packet(0, -1, 202)
        )));
    }

    #[test]
    fn legacy_first_low_packet_binds_non_zero_id_and_preserves_epoch() {
        let ip: IpAddr = "192.168.1.2".parse().unwrap();
        let active = Arc::new(RwLock::new(ActiveAudioSession::UnboundLegacy {
            peer_ip: ip,
            epoch: 9,
        }));

        assert_eq!(
            try_accept_audio_packet(&active, ip, &packet(0, -1, 202)),
            AudioPacketAcceptance::Accepted { epoch: 9 }
        );
        assert_eq!(
            *active.read().unwrap(),
            ActiveAudioSession::Bound {
                peer_ip: ip,
                session_id: 202,
                epoch: 9
            }
        );
        assert!(!accepted(try_accept_audio_packet(
            &active,
            ip,
            &packet(1, -1, 101)
        )));
    }

    #[test]
    fn legacy_high_or_fec_packet_cannot_bind_but_session_zero_can() {
        let ip: IpAddr = "192.168.1.2".parse().unwrap();
        let active = Arc::new(RwLock::new(ActiveAudioSession::UnboundLegacy {
            peer_ip: ip,
            epoch: 3,
        }));

        assert!(!accepted(try_accept_audio_packet(
            &active,
            ip,
            &packet(99, -1, 101)
        )));
        assert!(!accepted(try_accept_audio_packet(
            &active,
            ip,
            &packet(0, 12, 101)
        )));
        assert_eq!(
            *active.read().unwrap(),
            ActiveAudioSession::UnboundLegacy {
                peer_ip: ip,
                epoch: 3
            }
        );
        assert_eq!(
            try_accept_audio_packet(&active, ip, &packet(0, -1, 0)),
            AudioPacketAcceptance::Accepted { epoch: 3 }
        );
    }
}
