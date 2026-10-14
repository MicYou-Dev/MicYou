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

use micyou_protocol::micyou::AudioPacketMessageOrdered;

// Legacy clients may send larger audio buffers, so retain the compatibility ceiling
// for the primary payload while bounding FEC metadata to Android's current PCM chunk.
pub const MAX_AUDIO_BUFFER_LEN: usize = 64 * 1024;
pub const MAX_PCM_BLOCK_LEN: usize = 1_320;
pub const FEC_GROUP_SIZE: usize = 12;
const MIN_SAMPLE_RATE: i32 = 8_000;
const MAX_SAMPLE_RATE: i32 = 192_000;
const MAX_CHANNEL_COUNT: i32 = 2;

pub fn validate_audio_packet(packet: &AudioPacketMessageOrdered) -> bool {
    let Some(audio) = packet.audio_packet.as_ref() else {
        return false;
    };
    if !(1..=MAX_CHANNEL_COUNT).contains(&audio.channel_count) {
        return false;
    }
    if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&audio.sample_rate) {
        return false;
    }
    if audio.buffer.is_empty() || audio.buffer.len() > MAX_AUDIO_BUFFER_LEN {
        return false;
    }

    let is_opus = audio.codec == micyou_protocol::CODEC_OPUS;

    let is_fec = !packet.fec_buffer.is_empty();
    if packet.fec_buffer.len() > MAX_PCM_BLOCK_LEN
        || (!is_fec && !packet.fec_packet_lengths.is_empty())
    {
        return false;
    }
    if is_fec && !packet.fec_packet_lengths.is_empty() {
        if packet.fec_packet_lengths.len() != FEC_GROUP_SIZE
            || packet.fec_packet_lengths.iter().any(|&len| {
                let len = len as usize;
                len == 0 || len > MAX_PCM_BLOCK_LEN
            })
        {
            return false;
        }
        // PCM source lengths must be frame-aligned for XOR recovery; Opus
        // payloads vary arbitrarily in size, so only the size bounds apply.
        if !is_opus {
            let Some(frame_size) = pcm_frame_size(audio) else {
                return false;
            };
            if packet
                .fec_packet_lengths
                .iter()
                .any(|&len| !(len as usize).is_multiple_of(frame_size))
            {
                return false;
            }
        }
    }

    if is_opus {
        true
    } else {
        matches!(pcm_frame_size(audio), Some(frame_size) if audio.buffer.len() % frame_size == 0)
    }
}

/// Bytes per interleaved PCM frame for an audio packet, if its codec uses PCM.
pub(crate) fn pcm_frame_size(audio: &micyou_protocol::micyou::AudioPacketMessage) -> Option<usize> {
    let bytes_per_sample = match audio.audio_format {
        2 => 2_usize, // PCM 16-bit
        3 => 1_usize, // PCM 8-bit
        4 => 4_usize, // PCM float
        6 => 3_usize, // PCM 24-bit
        _ => return None,
    };
    bytes_per_sample.checked_mul(audio.channel_count as usize)
}

pub fn audio_payload_len(packet: &AudioPacketMessageOrdered) -> usize {
    packet
        .audio_packet
        .as_ref()
        .map_or(0, |audio| audio.buffer.len())
        .saturating_add(packet.fec_buffer.len())
        .saturating_add(
            packet
                .fec_packet_lengths
                .len()
                .saturating_mul(std::mem::size_of::<u32>()),
        )
}

/// Transport-level audio session state. `UnboundLegacy` is distinct from both an
/// inactive transport and a session explicitly bound to ID zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExpectedAudioSession {
    #[default]
    Inactive,
    UnboundLegacy,
    Bound(i64),
}

/// Legacy streams restart their sequence counter at zero. Keeping this window
/// below the first normal FEC group boundary prevents delayed packets (including
/// the group-zero parity packet) from claiming a newly connected legacy stream.
pub const MAX_LEGACY_START_SEQUENCE: i32 = 4;

pub fn can_bind_legacy_packet(packet: &AudioPacketMessageOrdered) -> bool {
    (0..=MAX_LEGACY_START_SEQUENCE).contains(&packet.sequence_number)
        && packet.fec_sequence_number <= 0
}

#[derive(Debug)]
pub enum AudioStreamEvent {
    SessionStarting {
        expected: ExpectedAudioSession,
        epoch: u64,
    },
    Packet {
        packet: AudioPacketMessageOrdered,
        epoch: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use micyou_protocol::micyou::{AudioPacketMessage, MessageWrapper};
    use prost::Message;

    fn packet(buffer_len: usize) -> AudioPacketMessageOrdered {
        AudioPacketMessageOrdered {
            sequence_number: 0,
            audio_packet: Some(AudioPacketMessage {
                buffer: vec![0; buffer_len],
                sample_rate: 48_000,
                channel_count: 1,
                audio_format: 2,
                codec: micyou_protocol::CODEC_PCM,
            }),
            timestamp: 0,
            fec_buffer: Vec::new(),
            fec_sequence_number: -1,
            session_id: 1,
            fec_packet_lengths: Vec::new(),
        }
    }

    #[test]
    fn decoded_protobuf_with_oversized_nested_audio_buffer_is_rejected() {
        let encoded = MessageWrapper {
            audio_packet: Some(packet(MAX_AUDIO_BUFFER_LEN + 2)),
            ..Default::default()
        }
        .encode_to_vec();
        let decoded = MessageWrapper::decode(encoded.as_slice()).unwrap();
        assert!(!validate_audio_packet(
            decoded.audio_packet.as_ref().unwrap()
        ));
    }

    #[test]
    fn normal_audio_and_legacy_or_current_fec_payloads_are_accepted() {
        let normal = packet(1_400);
        assert!(validate_audio_packet(&normal));

        let mut legacy_fec = packet(1_320);
        legacy_fec.fec_sequence_number = 12;
        legacy_fec.fec_buffer = vec![1];
        assert!(validate_audio_packet(&legacy_fec));

        let mut current_fec = legacy_fec;
        current_fec.fec_packet_lengths = vec![1_320; FEC_GROUP_SIZE];
        assert!(validate_audio_packet(&current_fec));
    }

    #[test]
    fn fec_length_metadata_shape_and_values_are_strictly_validated() {
        let mut fec = packet(1_320);
        fec.fec_buffer = vec![1];

        fec.fec_packet_lengths = vec![2; FEC_GROUP_SIZE + 1];
        assert!(!validate_audio_packet(&fec));
        fec.fec_packet_lengths = vec![2; FEC_GROUP_SIZE - 1];
        assert!(!validate_audio_packet(&fec));

        fec.fec_packet_lengths = vec![2; FEC_GROUP_SIZE];
        fec.fec_packet_lengths[0] = 0;
        assert!(!validate_audio_packet(&fec));
        fec.fec_packet_lengths[0] = MAX_PCM_BLOCK_LEN as u32 + 2;
        assert!(!validate_audio_packet(&fec));
        fec.fec_packet_lengths[0] = 3;
        assert!(!validate_audio_packet(&fec));
    }

    #[test]
    fn pcm8_mono_fec_accepts_odd_source_lengths() {
        let mut fec = packet(5);
        fec.audio_packet.as_mut().unwrap().audio_format = 3;
        fec.fec_buffer = vec![1];
        fec.fec_packet_lengths = vec![5; FEC_GROUP_SIZE];

        assert!(validate_audio_packet(&fec));
    }

    #[test]
    fn pcm16_mono_fec_rejects_odd_source_lengths() {
        let mut fec = packet(4);
        fec.fec_buffer = vec![1];
        fec.fec_packet_lengths = vec![2; FEC_GROUP_SIZE];
        fec.fec_packet_lengths[0] = 3;

        assert!(!validate_audio_packet(&fec));
    }

    #[test]
    fn oversized_fec_list_and_buffer_are_rejected() {
        let mut fec = packet(2);
        fec.fec_buffer = vec![1];
        fec.fec_packet_lengths = vec![2; 100_000];
        assert!(!validate_audio_packet(&fec));

        fec.fec_packet_lengths.clear();
        fec.fec_buffer = vec![0; MAX_PCM_BLOCK_LEN + 1];
        assert!(!validate_audio_packet(&fec));
    }

    #[test]
    fn regular_packet_cannot_carry_fec_lengths() {
        let mut regular = packet(2);
        regular.fec_packet_lengths = vec![2; FEC_GROUP_SIZE];
        assert!(!validate_audio_packet(&regular));
    }

    #[test]
    fn payload_budget_includes_fec_length_allocation() {
        let mut fec = packet(100);
        fec.fec_buffer = vec![1; 10];
        fec.fec_packet_lengths = vec![2; FEC_GROUP_SIZE];
        assert_eq!(
            audio_payload_len(&fec),
            100 + 10 + FEC_GROUP_SIZE * std::mem::size_of::<u32>()
        );
    }

    #[test]
    fn omitted_proto3_fec_sequence_decodes_as_zero() {
        let encoded = MessageWrapper {
            audio_packet: Some(AudioPacketMessageOrdered {
                sequence_number: 12,
                fec_sequence_number: 0,
                ..packet(2)
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let decoded = MessageWrapper::decode(encoded.as_slice()).unwrap();

        assert_eq!(decoded.audio_packet.unwrap().fec_sequence_number, 0);
    }

    #[test]
    fn opus_packets_skip_pcm_frame_alignment() {
        // Opus payloads are arbitrarily sized; only size bounds apply.
        let mut opus = packet(100);
        opus.audio_packet.as_mut().unwrap().codec = micyou_protocol::CODEC_OPUS;
        opus.audio_packet.as_mut().unwrap().buffer = vec![0x9c; 63];

        assert!(validate_audio_packet(&opus));

        let mut fec = opus.clone();
        fec.fec_buffer = vec![1];
        fec.fec_packet_lengths = vec![63; FEC_GROUP_SIZE];
        assert!(validate_audio_packet(&fec));

        // Odd lengths that aren't frame multiples are valid for Opus FEC groups.
        fec.fec_packet_lengths = vec![61; FEC_GROUP_SIZE];
        assert!(validate_audio_packet(&fec));

        // Size bounds still enforced.
        fec.fec_packet_lengths[0] = MAX_PCM_BLOCK_LEN as u32 + 1;
        assert!(!validate_audio_packet(&fec));

        fec.fec_packet_lengths = vec![63; FEC_GROUP_SIZE];
        fec.fec_buffer = vec![0; MAX_PCM_BLOCK_LEN + 1];
        assert!(!validate_audio_packet(&fec));
    }

    #[test]
    fn invalid_audio_metadata_or_alignment_is_rejected() {
        let mut invalid = packet(1_400);
        invalid.audio_packet.as_mut().unwrap().channel_count = 99;
        assert!(!validate_audio_packet(&invalid));
        invalid.audio_packet.as_mut().unwrap().channel_count = 1;
        invalid.audio_packet.as_mut().unwrap().buffer.push(0);
        assert!(!validate_audio_packet(&invalid));
    }
}
