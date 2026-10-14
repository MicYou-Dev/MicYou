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

//! The per-server audio thread: reorders and FEC-recovers packets, decodes
//! PCM/Opus, resamples to 48 kHz, runs the DSP chain and feeds the output.

use crate::events::{AecStatus, SharedEvents, SpectrumPayload};
use crate::plugins::PluginHost;
use crate::server::output::{self, AudioOutputHandle};
use crate::stats::NetworkStats;
use crate::transport::jitter_buffer::JitterBuffer;
use crate::transport::opus;
use crate::transport::session::AudioStreamEvent;
use crate::transport::udp::{ActiveAudioSession, SharedActiveAudioSession};
use micyou_audio::dsp::{AudioDspSettings, DspProcessor};
use micyou_audio::AecFailure;
use std::path::PathBuf;
use micyou_protocol::micyou::AudioPacketMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Output sample rate of the whole pipeline.
const OUTPUT_RATE: u32 = 48_000;

pub(super) struct Pipeline {
    pub audio_rx: mpsc::Receiver<AudioStreamEvent>,
    pub ready_tx: oneshot::Sender<Result<(), String>>,
    pub audio_output: Arc<AudioOutputHandle>,
    pub output_device: Option<String>,
    pub output_buffer_ms: usize,
    pub resource_dir: Option<PathBuf>,
    pub dsp_settings: Arc<RwLock<AudioDspSettings>>,
    pub plugins: Arc<PluginHost>,
    pub events: SharedEvents,
    pub active_audio_session: SharedActiveAudioSession,
    pub is_monitoring: Arc<AtomicBool>,
    pub spectrum_streaming_enabled: Arc<AtomicBool>,
    pub stats: Arc<NetworkStats>,
    /// Web clients send already-clean browser audio: skip the DSP chain.
    pub bypass_dsp: bool,
}

pub(super) fn spawn(pipeline: Pipeline) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("micyou-audio".into())
        .spawn(move || run(pipeline))
        .expect("spawning the audio thread only fails on resource exhaustion")
}

fn disable_aec_runtime(runtime_available: &mut bool, events: &SharedEvents, reason: AecFailure) {
    if !std::mem::replace(runtime_available, false) {
        return;
    }
    events.aec_status_changed(AecStatus {
        available: false,
        enabled: false,
        reason: Some(reason),
    });
}

fn restore_aec_runtime(
    runtime_available: &mut bool,
    settings: &RwLock<AudioDspSettings>,
    events: &SharedEvents,
) {
    // A platform without a reference source can never become available, so the
    // restored state reports the capability instead of assuming success.
    let availability = micyou_audio::aec_reference_availability();
    *runtime_available = availability.available;
    let enabled = settings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .aec_enabled;
    events.aec_status_changed(AecStatus {
        available: availability.available,
        enabled: enabled && availability.available,
        reason: availability.reason,
    });
}

fn should_capture_loopback(
    transport_active: bool,
    audio_received: bool,
    aec_enabled: bool,
    runtime_available: bool,
) -> bool {
    transport_active && audio_received && aec_enabled && runtime_available
}

/// Decode little-endian PCM into f32 samples. Returns false for an unknown
/// `audio_format` (Android AudioFormat encoding constant).
fn decode_pcm(audio_format: i32, buffer: &[u8], out: &mut Vec<f32>) -> bool {
    out.clear();
    match audio_format {
        // ENCODING_PCM_16BIT
        2 => out.extend(
            buffer
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c) as f32 / 32768.0),
        ),
        // ENCODING_PCM_8BIT (unsigned)
        3 => out.extend(buffer.iter().map(|&b| (b as f32 - 128.0) / 128.0)),
        // ENCODING_PCM_FLOAT. A NaN or infinity would stick in every
        // recursive filter of the DSP chain and silence the stream for good,
        // so non-finite samples from the peer become silence.
        4 => out.extend(buffer.as_chunks::<4>().0.iter().map(|c| {
            let sample = f32::from_le_bytes(*c);
            if sample.is_finite() {
                sample
            } else {
                0.0
            }
        })),
        // ENCODING_PCM_24BIT_PACKED
        6 => out.extend(buffer.as_chunks::<3>().0.iter().map(|c| {
            let sample = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i8 as i32) << 16);
            sample as f32 / 8_388_608.0
        })),
        _ => return false,
    }
    true
}

/// Turns wire packets into 48 kHz interleaved f32. Both codecs and the
/// resampler are stateful, so they are rebuilt whenever the stream format
/// changes and dropped when a new transport session starts.
#[derive(Default)]
struct StreamDecoder {
    /// Keyed by (sample_rate, channels).
    opus: Option<((u32, usize), opus::Decoder)>,
    opus_buf: Vec<f32>,
    /// Keyed by (sample_rate, channels); `None` inside means creating the
    /// resampler failed for that format, which is not retried per packet.
    resampler: Option<((u32, usize), Option<micyou_audio::RubatoResampler>)>,
    resample_buf: Vec<f32>,
    /// Decode failures repeat for every packet of a broken stream; log the
    /// first one per session only.
    reported_failure: bool,
}

impl StreamDecoder {
    /// Longest Opus frame (120 ms), the buffer size the decoder may need.
    const MAX_OPUS_FRAME_MS: usize = 120;

    fn reset(&mut self) {
        self.opus = None;
        self.resampler = None;
        self.reported_failure = false;
    }

    fn report_failure(&mut self, message: std::fmt::Arguments) {
        if !std::mem::replace(&mut self.reported_failure, true) {
            log::warn!("[Audio] {message} (further errors this session are not logged)");
        }
    }

    /// Decode `audio` into `out`; leaves `out` empty when nothing decodable.
    fn decode(&mut self, audio: &AudioPacketMessage, out: &mut Vec<f32>) {
        out.clear();
        // validate_audio_packet bounds both fields to positive values.
        let sample_rate = audio.sample_rate as u32;
        let channels = audio.channel_count as usize;
        if audio.codec == micyou_protocol::CODEC_OPUS {
            self.decode_opus(&audio.buffer, sample_rate, channels, out);
        } else if !decode_pcm(audio.audio_format, &audio.buffer, out) {
            self.report_failure(format_args!("Unsupported audio format: {}", audio.audio_format));
        }
        if !out.is_empty() && sample_rate != OUTPUT_RATE {
            self.resample(sample_rate, channels, out);
        }
    }

    fn decode_opus(&mut self, buffer: &[u8], sample_rate: u32, channels: usize, out: &mut Vec<f32>) {
        let key = (sample_rate, channels);
        if self.opus.as_ref().is_none_or(|(current, _)| *current != key) {
            self.opus = opus::Channels::from_channel_count(channels)
                .and_then(|ch| opus::Decoder::new(sample_rate, ch).ok())
                .map(|decoder| (key, decoder));
            if self.opus.is_none() {
                self.report_failure(format_args!(
                    "Failed to create Opus decoder for {sample_rate}Hz/{channels}ch"
                ));
            }
        }
        let Some((_, decoder)) = self.opus.as_mut() else {
            return;
        };
        let capacity = sample_rate as usize * Self::MAX_OPUS_FRAME_MS / 1000 * channels;
        self.opus_buf.resize(capacity, 0.0);
        match decoder.decode_float(buffer, &mut self.opus_buf) {
            Ok(frames) => out.extend_from_slice(&self.opus_buf[..frames * channels]),
            Err(e) => self.report_failure(format_args!("Opus decode error: {e}")),
        }
    }

    fn resample(&mut self, sample_rate: u32, channels: usize, samples: &mut Vec<f32>) {
        let key = (sample_rate, channels);
        if self.resampler.as_ref().is_none_or(|(current, _)| *current != key) {
            let resampler = micyou_audio::RubatoResampler::new(sample_rate, OUTPUT_RATE, channels)
                .inspect_err(|e| log::error!("[Audio] Failed to create resampler: {e}"))
                .ok();
            self.resampler = Some((key, resampler));
        }
        if let Some((_, Some(resampler))) = self.resampler.as_mut() {
            resampler.resample(samples, channels, &mut self.resample_buf);
            std::mem::swap(samples, &mut self.resample_buf);
        }
    }
}

/// Mean square root of a block, the level shown in meters.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

/// How long the audio thread waits for the next event before re-checking the
/// session (loopback capture start/stop). Packets wake it immediately.
const ACTIVE_POLL: Duration = Duration::from_millis(100);
const IDLE_POLL: Duration = Duration::from_millis(500);

/// Block the audio thread until the next event or `wait` elapses. The timer
/// is created inside `block_on`, the only place the runtime is current.
fn recv_within<T>(
    runtime: &tokio::runtime::Runtime,
    rx: &mut mpsc::Receiver<T>,
    wait: Duration,
) -> Result<Option<T>, tokio::time::error::Elapsed> {
    runtime.block_on(async { tokio::time::timeout(wait, rx.recv()).await })
}

fn run(pipeline: Pipeline) {
    let Pipeline {
        mut audio_rx,
        ready_tx,
        audio_output,
        output_device,
        output_buffer_ms,
        resource_dir,
        dsp_settings,
        plugins,
        events,
        active_audio_session,
        is_monitoring,
        spectrum_streaming_enabled,
        stats,
        bypass_dsp,
    } = pipeline;

    // The thread blocks on the async channel through a minimal runtime, so a
    // packet is processed the moment it arrives instead of on a poll tick.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("audio thread runtime: {e}")));
            return;
        }
    };

    // Ensure the virtual device is open. This is normally a no-op (already
    // opened at app startup); it also covers CLI/TUI first run and the rare
    // case where opening failed earlier and a later attempt succeeds.
    if !output::ensure_started(
        &audio_output,
        output_device,
        output_buffer_ms,
        resource_dir.as_deref(),
    ) {
        log::error!("[Audio] Output device unavailable; audio will be silent");
    }
    let _ = ready_tx.send(Ok(()));
    let mut dsp_processor = DspProcessor::new(dsp_settings.clone(), resource_dir);
    // Attach the plugin DSP stage (runs at the legacy "Plugins" node and
    // at per-plugin "Plugin:<id>" nodes, issue #347).
    dsp_processor.set_external_hook(Some(plugins.dsp_hook()));
    let mut jb = JitterBuffer::new(12);
    let mut decoder = StreamDecoder::default();
    let mut pcm_f32 = Vec::new();
    let mut frame_counter: u32 = 0;
    let mut monitoring: Option<bool> = None;

    // Speaker loopback capture for the AEC far-end reference. The audio crate
    // picks the platform source - WASAPI loopback, the PipeWire sink monitor, or
    // on macOS a Core Audio process tap falling back to BlackHole - so every
    // platform gets a capture and this layer only asks whether it is active.
    // It still starts lazily, once an AEC-enabled session sends audio.
    let loopback: Option<micyou_audio::LoopbackCapture> =
        Some(micyou_audio::LoopbackCapture::new());

    let mut audio_received_for_session = false;
    let mut aec_runtime_available = micyou_audio::aec_reference_availability().available;
    // A newly started server always begins with a fresh runtime state, even
    // before the first client session arrives.
    if loopback.is_some() {
        restore_aec_runtime(&mut aec_runtime_available, &dsp_settings, &events);
    }
    // Sync the AEC far-end capture with actual audio flow. A control session
    // alone is not enough: while waiting for the first valid audio packet,
    // there is no microphone stream that needs an echo reference.
    let sync_loopback = |audio_received: &mut bool, runtime_available: &mut bool| {
        let transport_active = !matches!(
            *active_audio_session
                .read()
                .unwrap_or_else(|p| p.into_inner()),
            ActiveAudioSession::Inactive
        );
        if !transport_active {
            *audio_received = false;
        }

        let Some(lb) = &loopback else {
            return transport_active;
        };
        let aec_enabled = dsp_settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .aec_enabled;
        let should_capture = should_capture_loopback(
            transport_active,
            *audio_received,
            aec_enabled,
            *runtime_available,
        );

        if !should_capture {
            if lb.is_active() {
                lb.stop();
            }
            return transport_active;
        }
        if lb.is_active() {
            return transport_active;
        }

        let failure = lb.take_failure_reason().or_else(|| lb.start().err());
        if let Some(reason) = failure {
            disable_aec_runtime(runtime_available, &events, reason);
        } else {
            log::info!("[Audio] Starting speaker loopback capture for AEC");
        }
        transport_active
    };

    let mut session_active = false;
    loop {
        let wait = if session_active { ACTIVE_POLL } else { IDLE_POLL };
        let event = match recv_within(&runtime, &mut audio_rx, wait) {
            Ok(Some(event)) => event,
            Ok(None) => break,
            Err(_) => {
                session_active =
                    sync_loopback(&mut audio_received_for_session, &mut aec_runtime_available);
                continue;
            }
        };

        let monitoring_now = is_monitoring.load(Ordering::Relaxed);
        if monitoring != Some(monitoring_now) {
            monitoring = Some(monitoring_now);
            audio_output.set_monitoring(monitoring_now);
        }
        match event {
            AudioStreamEvent::SessionStarting { expected, epoch } => {
                audio_received_for_session = false;
                if let Some(lb) = &loopback {
                    lb.reset_session();
                    restore_aec_runtime(&mut aec_runtime_available, &dsp_settings, &events);
                }
                dsp_processor.reset_aec_session();
                decoder.reset();
                jb.prepare_transport_session_epoch(expected, epoch);
                continue;
            }
            AudioStreamEvent::Packet { packet, epoch } => {
                audio_received_for_session = true;
                session_active =
                    sync_loopback(&mut audio_received_for_session, &mut aec_runtime_available);
                jb.push_epoch(packet, epoch);
            }
        }

        while let Some(ordered_packet) = jb.pop() {
            let Some(audio_data) = ordered_packet.audio_packet else {
                continue;
            };
            decoder.decode(&audio_data, &mut pcm_f32);
            if pcm_f32.is_empty() {
                continue;
            }
            let channels = (audio_data.channel_count as usize).max(1);

            let (input_rms, processed_rms) = if bypass_dsp {
                let level = rms(&pcm_f32);
                (level, level)
            } else {
                // The loopback capture is the true echo source the phone mic
                // picks up. Feed one mono reference sample per near-end frame
                // so packet sizes or input rates cannot make the two drift.
                let near_frames = pcm_f32.len() / channels;
                if let Some(far_data) = loopback
                    .as_ref()
                    .filter(|capture| capture.is_active())
                    .map(|capture| capture.read(near_frames))
                {
                    dsp_processor.set_far_end_audio(&far_data);
                }
                let levels =
                    dsp_processor.process(&mut pcm_f32, channels, audio_output.queued_ms());
                if let Some(reason) = dsp_processor.take_aec_failure() {
                    disable_aec_runtime(&mut aec_runtime_available, &events, reason);
                }
                levels
            };

            // Local hard-mute: the output engine drops the audio itself;
            // report silence too so meters and plugin snapshots read zero.
            let muted = stats.is_muted();
            if muted {
                stats.set_levels(0.0, 0.0);
            } else {
                stats.set_levels(input_rms, processed_rms);
            }

            audio_output.push(pcm_f32.clone(), channels);

            frame_counter = frame_counter.wrapping_add(1);
            if !frame_counter.is_multiple_of(6) {
                continue;
            }
            let level = if muted { 0 } else { (processed_rms * 500.0).min(100.0) as u32 };
            events.audio_level(level);
            if spectrum_streaming_enabled.load(Ordering::Acquire) {
                let (mut raw, mut processed) = dsp_processor.get_spectrums();
                if muted {
                    raw.fill(0.0);
                    processed.fill(0.0);
                }
                events.audio_spectrum(SpectrumPayload { raw, processed });
            }
        }
    }

    if let Some(lb) = &loopback {
        let was_active = lb.is_active();
        lb.stop();
        if was_active {
            log::info!("[Audio] Speaker loopback stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_pcm, recv_within, should_capture_loopback};
    use std::time::Duration;

    #[test]
    fn recv_within_works_on_a_plain_thread() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            let wait = Duration::from_millis(10);
            assert!(recv_within(&runtime, &mut rx, wait).is_err());
            tx.try_send(7).unwrap();
            assert_eq!(recv_within(&runtime, &mut rx, wait).unwrap(), Some(7));
            drop(tx);
            assert_eq!(recv_within(&runtime, &mut rx, wait).unwrap(), None);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn loopback_waits_for_first_audio_packet() {
        assert!(!should_capture_loopback(false, false, true, true));
        assert!(!should_capture_loopback(true, false, true, true));
        assert!(!should_capture_loopback(true, true, false, true));
        assert!(!should_capture_loopback(true, true, true, false));
        assert!(should_capture_loopback(true, true, true, true));
    }

    #[test]
    fn decodes_supported_pcm_formats() {
        let mut out = Vec::new();
        assert!(decode_pcm(2, &[0x00, 0x40], &mut out));
        assert_eq!(out, vec![0.5]);
        assert!(decode_pcm(3, &[128], &mut out));
        assert_eq!(out, vec![0.0]);
        assert!(decode_pcm(4, &1.0f32.to_le_bytes(), &mut out));
        assert_eq!(out, vec![1.0]);
        assert!(decode_pcm(6, &[0x00, 0x00, 0xc0], &mut out));
        assert_eq!(out, vec![-0.5]);
        assert!(decode_pcm(4, &f32::NAN.to_le_bytes(), &mut out));
        assert_eq!(out, vec![0.0]);
        assert!(!decode_pcm(9, &[1, 2], &mut out));
        assert!(out.is_empty());
    }
}
