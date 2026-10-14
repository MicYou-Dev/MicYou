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

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AudioMetrics {
    pub bitrate: i32,
    pub sample_rate: i32,
    pub latency_ms: i64,
    pub network_latency_ms: i64,
    pub packet_loss_rate: f64,
    pub jitter_ms: f64,
    pub buffer_duration_ms: i64,
}

pub struct NetworkStats {
    pub rtt_ms: AtomicI64,
    pub jitter_bits: AtomicU64,
    pub loss_rate_bits: AtomicU64,
    pub last_udp_packet_time_ms: AtomicU64,
    pub tcp_connected_time_ms: AtomicU64,
    pub bitrate: AtomicU32,
    pub sample_rate: AtomicU32,
    /// Shared hard-mute flag. The audio output engine watches this very
    /// AtomicBool, so flipping the mute state silences the cpal stream (and
    /// drops everything queued) within one device callback period.
    /// (NetworkStats itself is never serialized — the frontend receives
    /// AudioMetrics via `to_metrics` — so no serde attribute is needed here.)
    pub is_muted: Arc<AtomicBool>,
    pub channels: AtomicU32,
    pub input_level_bits: AtomicU32,
    pub processed_level_bits: AtomicU32,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            rtt_ms: AtomicI64::new(0),
            jitter_bits: AtomicU64::new(0f64.to_bits()),
            loss_rate_bits: AtomicU64::new(0f64.to_bits()),
            last_udp_packet_time_ms: AtomicU64::new(0),
            tcp_connected_time_ms: AtomicU64::new(0),
            bitrate: AtomicU32::new(0),
            sample_rate: AtomicU32::new(0),
            is_muted: Arc::new(AtomicBool::new(false)),
            channels: AtomicU32::new(0),
            input_level_bits: AtomicU32::new(0f32.to_bits()),
            processed_level_bits: AtomicU32::new(0f32.to_bits()),
        }
    }
}

impl NetworkStats {
    pub fn set_rtt(&self, rtt: i64) {
        let prev = self.rtt_ms.load(Ordering::Relaxed);
        let smoothed = if prev > 0 {
            (prev as f64 * 0.5 + rtt as f64 * 0.5).round() as i64
        } else {
            rtt
        };
        self.rtt_ms.store(smoothed, Ordering::Relaxed);
    }
    pub fn get_rtt(&self) -> i64 {
        self.rtt_ms.load(Ordering::Relaxed)
    }

    pub fn set_jitter(&self, jitter: f64) {
        self.jitter_bits.store(jitter.to_bits(), Ordering::Relaxed);
    }
    pub fn get_jitter(&self) -> f64 {
        f64::from_bits(self.jitter_bits.load(Ordering::Relaxed))
    }

    pub fn set_loss_rate(&self, loss: f64) {
        self.loss_rate_bits.store(loss.to_bits(), Ordering::Relaxed);
    }
    pub fn get_loss_rate(&self) -> f64 {
        f64::from_bits(self.loss_rate_bits.load(Ordering::Relaxed))
    }

    pub fn mark_udp_received(&self, time_ms: u64) {
        self.last_udp_packet_time_ms
            .store(time_ms, Ordering::Relaxed);
    }
    pub fn get_last_udp_time(&self) -> u64 {
        self.last_udp_packet_time_ms.load(Ordering::Relaxed)
    }

    pub fn mark_tcp_connected(&self, time_ms: u64) {
        self.tcp_connected_time_ms.store(time_ms, Ordering::Relaxed);
        self.last_udp_packet_time_ms.store(0, Ordering::Relaxed);
    }
    pub fn mark_tcp_disconnected(&self) {
        self.tcp_connected_time_ms.store(0, Ordering::Relaxed);
        self.last_udp_packet_time_ms.store(0, Ordering::Relaxed);
        self.loss_rate_bits.store(0f64.to_bits(), Ordering::Relaxed);
        self.jitter_bits.store(0f64.to_bits(), Ordering::Relaxed);
        self.rtt_ms.store(0, Ordering::Relaxed);
    }
    pub fn get_tcp_connected_time(&self) -> u64 {
        self.tcp_connected_time_ms.load(Ordering::Relaxed)
    }

    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Relaxed);
    }
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }
    /// Clone of the shared mute flag, for wiring into the audio output engine.
    pub fn mute_flag(&self) -> Arc<AtomicBool> {
        self.is_muted.clone()
    }

    pub fn set_audio_info(&self, sample_rate: u32, channels: u32) {
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.channels.store(channels, Ordering::Relaxed);
    }

    pub fn set_bitrate(&self, bitrate: u32) {
        self.bitrate.store(bitrate, Ordering::Relaxed);
    }

    pub fn set_levels(&self, input: f32, processed: f32) {
        self.input_level_bits.store(input.to_bits(), Ordering::Relaxed);
        self.processed_level_bits.store(processed.to_bits(), Ordering::Relaxed);
    }

    pub fn to_metrics(&self, buffer_duration: i64) -> AudioMetrics {
        let rtt = self.get_rtt();
        AudioMetrics {
            bitrate: self.bitrate.load(Ordering::Relaxed) as i32,
            sample_rate: self.sample_rate.load(Ordering::Relaxed) as i32,
            latency_ms: buffer_duration + rtt,
            network_latency_ms: rtt,
            packet_loss_rate: self.get_loss_rate(),
            jitter_ms: self.get_jitter(),
            buffer_duration_ms: buffer_duration,
        }
    }
}

/// Milliseconds since the Unix epoch; the phone stamps packets with the same
/// clock, so transit times are comparable.
pub fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// Receive-side statistics of one transport (loss, jitter, bitrate),
/// published into [`NetworkStats`]. Restarts whenever a new audio session
/// epoch begins so a reconnect does not inherit the previous stream's
/// sequence numbers.
#[derive(Default)]
pub struct StreamMeter {
    epoch: u64,
    highest_seq: Option<i64>,
    /// Bit `n` is set when `highest_seq - n` has arrived, so a duplicate can
    /// be told apart from a late packet that fills a gap.
    seen: u64,
    received: u64,
    lost: u64,
    jitter: f64,
    last_transit: Option<i64>,
    window_start_ms: u64,
    window_bytes: u64,
}

impl StreamMeter {
    const BITRATE_WINDOW_MS: u64 = 1000;

    /// Account one accepted packet of `payload_len` wire bytes.
    pub fn observe(
        &mut self,
        stats: &NetworkStats,
        epoch: u64,
        packet: &micyou_protocol::micyou::AudioPacketMessageOrdered,
        payload_len: usize,
    ) {
        let now = unix_millis();
        if epoch != self.epoch {
            *self = Self {
                epoch,
                window_start_ms: now,
                ..Self::default()
            };
        }
        if let Some(audio) = &packet.audio_packet {
            stats.set_audio_info(audio.sample_rate.max(0) as u32, audio.channel_count.max(0) as u32);
        }
        self.window_bytes += payload_len as u64;
        let elapsed = now.saturating_sub(self.window_start_ms);
        if elapsed >= Self::BITRATE_WINDOW_MS {
            let bps = self.window_bytes * 8 * 1000 / elapsed;
            stats.set_bitrate(u32::try_from(bps).unwrap_or(u32::MAX));
            self.window_start_ms = now;
            self.window_bytes = 0;
        }

        // FEC parity packets reuse the next regular sequence number.
        if !packet.fec_buffer.is_empty() {
            return;
        }
        let seq = i64::from(packet.sequence_number);
        match self.highest_seq {
            Some(highest) if seq <= highest => {
                let bit = u32::try_from(highest - seq)
                    .ok()
                    .and_then(|offset| 1u64.checked_shl(offset));
                // Duplicates and packets older than the window change nothing.
                let Some(bit) = bit.filter(|bit| self.seen & bit == 0) else {
                    return;
                };
                // A late packet fills a gap counted as lost earlier.
                self.seen |= bit;
                self.lost = self.lost.saturating_sub(1);
            }
            Some(highest) => {
                let gap = seq - highest;
                self.lost += (gap - 1) as u64;
                self.seen = u32::try_from(gap)
                    .ok()
                    .and_then(|gap| self.seen.checked_shl(gap))
                    .unwrap_or(0)
                    | 1;
                self.highest_seq = Some(seq);
            }
            None => {
                self.seen = 1;
                self.highest_seq = Some(seq);
            }
        }
        self.received += 1;
        let expected = self.received + self.lost;
        stats.set_loss_rate(self.lost as f64 * 100.0 / expected as f64);

        // RFC 3550 interarrival jitter estimate.
        let transit = now as i64 - packet.timestamp;
        if let Some(last) = self.last_transit {
            let delta = (transit - last).abs() as f64;
            self.jitter += (delta - self.jitter) / 16.0;
            stats.set_jitter(self.jitter);
        }
        self.last_transit = Some(transit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use micyou_protocol::micyou::AudioPacketMessageOrdered;

    fn packet(sequence_number: i32) -> AudioPacketMessageOrdered {
        AudioPacketMessageOrdered {
            sequence_number,
            audio_packet: None,
            timestamp: 0,
            fec_buffer: Vec::new(),
            fec_sequence_number: -1,
            session_id: 0,
            fec_packet_lengths: Vec::new(),
        }
    }

    #[test]
    fn stream_meter_counts_gaps_and_late_packets() {
        let stats = NetworkStats::default();
        let mut meter = StreamMeter::default();
        for seq in [0, 1, 4] {
            meter.observe(&stats, 1, &packet(seq), 10);
        }
        // 2 and 3 missing out of 5.
        assert_eq!(stats.get_loss_rate(), 40.0);
        meter.observe(&stats, 1, &packet(2), 10);
        assert_eq!(stats.get_loss_rate(), 20.0);
    }

    #[test]
    fn stream_meter_ignores_duplicates() {
        let stats = NetworkStats::default();
        let mut meter = StreamMeter::default();
        for seq in [0, 2, 2, 0] {
            meter.observe(&stats, 1, &packet(seq), 10);
        }
        // Only 1 is missing; the repeats neither fill it nor count twice.
        assert!((stats.get_loss_rate() - 100.0 / 3.0).abs() < 1e-9);
        meter.observe(&stats, 1, &packet(1), 10);
        meter.observe(&stats, 1, &packet(1), 10);
        assert_eq!(stats.get_loss_rate(), 0.0);
    }

    #[test]
    fn stream_meter_restarts_with_a_new_epoch() {
        let stats = NetworkStats::default();
        let mut meter = StreamMeter::default();
        meter.observe(&stats, 1, &packet(0), 10);
        meter.observe(&stats, 1, &packet(i32::MAX), 10);
        meter.observe(&stats, 2, &packet(0), 10);
        meter.observe(&stats, 2, &packet(1), 10);
        assert_eq!(stats.get_loss_rate(), 0.0);
    }

    #[test]
    fn test_network_stats_connect_disconnect_lifecycle() {
        let stats = NetworkStats::default();
        stats.mark_udp_received(12345);
        stats.set_loss_rate(5.5);
        stats.set_jitter(12.0);

        assert_eq!(stats.get_last_udp_time(), 12345);
        assert_eq!(stats.get_loss_rate(), 5.5);

        stats.mark_tcp_connected(20000);
        assert_eq!(stats.get_tcp_connected_time(), 20000);
        assert_eq!(stats.get_last_udp_time(), 0);

        stats.mark_tcp_disconnected();
        assert_eq!(stats.get_tcp_connected_time(), 0);
        assert_eq!(stats.get_last_udp_time(), 0);
        assert_eq!(stats.get_loss_rate(), 0.0);
        assert_eq!(stats.get_jitter(), 0.0);
    }
}
