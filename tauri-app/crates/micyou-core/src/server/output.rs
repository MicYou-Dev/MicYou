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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often the idle device thread checks for a failed stream.
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(500);
/// Delay before reopening a failed stream, doubled per failed attempt.
const REOPEN_BACKOFF_MIN: Duration = Duration::from_secs(1);
const REOPEN_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// An open stream whose data callback has not run for this long is treated
/// as failed. Device periods are tens of milliseconds, so this only trips
/// when the backend stopped calling back.
const STALL_TIMEOUT: Duration = Duration::from_secs(2);

/// Persistent audio output device thread.
///
/// `cpal::Stream` is deliberately `!Send + !Sync`, so the
/// `AudioOutputManager` can never live inside the shared `ServerState`.
/// Instead a dedicated thread owns it for the whole process lifetime and
/// receives commands over an mpsc channel. The device is opened at app
/// startup (GUI) or on the first server start (CLI/TUI) and only closed when
/// the process exits — server stop and phone connect/disconnect never tear it
/// down.
enum AudioOutputCommand {
    Open(Option<String>, usize, Sender<bool>),
    Push(Vec<f32>, usize),
    PushSound(Vec<f32>, f32),
    SetMonitoring(bool),
    Shutdown,
}

/// Watches the output callback counter and reports when it stops advancing.
struct StallDetector {
    last_ticks: u64,
    since: Instant,
}

impl StallDetector {
    fn new(ticks: u64, now: Instant) -> Self {
        Self {
            last_ticks: ticks,
            since: now,
        }
    }

    /// Whether `ticks` has not changed for longer than `STALL_TIMEOUT`.
    fn stalled(&mut self, ticks: u64, now: Instant) -> bool {
        if ticks != self.last_ticks {
            self.last_ticks = ticks;
            self.since = now;
            return false;
        }
        now.duration_since(self.since) > STALL_TIMEOUT
    }
}

/// Reopens the output stream after the device failed (e.g. the PipeWire sink
/// disappeared), so a broken device neither spins cpal's error loop nor needs
/// an app restart. Failures are either reported by cpal's error callback or
/// detected as a stalled data callback.
#[derive(Default)]
struct Recovery {
    /// Arguments of the last successful open; `None` until the first open.
    target: Option<(Option<String>, usize)>,
    retry_at: Option<Instant>,
    backoff: Duration,
    /// Present while the stream is open.
    stall: Option<StallDetector>,
}

impl Recovery {
    fn opened(
        &mut self,
        manager: &micyou_audio::AudioOutputManager,
        device: Option<String>,
        buffer_ms: usize,
    ) {
        self.target = Some((device, buffer_ms));
        self.retry_at = None;
        self.backoff = REOPEN_BACKOFF_MIN;
        self.stall = Some(StallDetector::new(manager.callback_ticks(), Instant::now()));
    }

    fn poll(&mut self, manager: &mut micyou_audio::AudioOutputManager) {
        let failed = manager.take_stream_failure();
        let stalled = match &mut self.stall {
            Some(stall) if manager.is_open() => {
                stall.stalled(manager.callback_ticks(), Instant::now())
            }
            _ => false,
        };
        if (failed || stalled) && manager.is_open() {
            if stalled {
                log::warn!("[Audio] Output stream stopped calling back, closing it and retrying");
            } else {
                log::warn!("[Audio] Output stream failed, closing it and retrying");
            }
            manager.close();
            self.stall = None;
            self.backoff = REOPEN_BACKOFF_MIN;
            self.retry_at = Some(Instant::now() + self.backoff);
        }
        let (Some(retry_at), Some((device, buffer_ms))) = (self.retry_at, &self.target) else {
            return;
        };
        if Instant::now() < retry_at || manager.is_open() {
            return;
        }
        match manager.start(device.clone(), *buffer_ms) {
            Ok(()) => {
                log::info!("[Audio] Output device reopened");
                self.retry_at = None;
                self.stall = Some(StallDetector::new(manager.callback_ticks(), Instant::now()));
            }
            Err(e) => {
                self.backoff = (self.backoff * 2).min(REOPEN_BACKOFF_MAX);
                log::debug!(
                    "[Audio] Reopen failed, next attempt in {:?}: {e}",
                    self.backoff
                );
                self.retry_at = Some(Instant::now() + self.backoff);
            }
        }
    }
}

pub struct AudioOutputHandle {
    tx: Sender<AudioOutputCommand>,
    /// `f64` bits of the milliseconds queued in the output ring, published by
    /// the device thread on every command and health tick. The audio thread reads it per
    /// packet, so it must never wait on a device thread busy reopening.
    queued_ms: Arc<AtomicU64>,
    thread: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// How long process exit waits for the device thread to close the stream
/// before the PipeWire virtual devices behind it are torn down.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(1);

impl AudioOutputHandle {
    /// Spawn the device thread with an externally owned hard-mute flag (the
    /// engine watches it directly, so mute toggles silence the output within
    /// one device callback period without any extra command round-trip).
    fn with_mute_flag(muted: Arc<AtomicBool>) -> Self {
        let (tx, rx) = mpsc::channel::<AudioOutputCommand>();
        let queued_ms = Arc::new(AtomicU64::new(0));
        let published = queued_ms.clone();
        let thread = std::thread::Builder::new()
            .name("micyou-output".into())
            .spawn(move || {
                let mut manager = micyou_audio::AudioOutputManager::with_mute_flag(muted);
                let mut recovery = Recovery::default();
                loop {
                    let cmd = rx.recv_timeout(HEALTH_CHECK_INTERVAL);
                    recovery.poll(&mut manager);
                    // Sampled before a Push lands, like the request/reply query
                    // this replaces: the DSP's underrun check compares the
                    // level the device has drained down to, not a full ring.
                    published.store(manager.queued_ms().to_bits(), Ordering::Relaxed);
                    match cmd {
                        Ok(AudioOutputCommand::Open(device, buffer_ms, reply)) => {
                            let ok = if manager.is_open() {
                                true
                            } else {
                                match manager.start(device.clone(), buffer_ms) {
                                    Ok(()) => {
                                        log::info!("[Audio] Output device opened");
                                        recovery.opened(&manager, device, buffer_ms);
                                        true
                                    }
                                    Err(e) => {
                                        log::error!("[Audio] Failed to open output device: {e}");
                                        false
                                    }
                                }
                            };
                            let _ = reply.send(ok);
                        }
                        Ok(AudioOutputCommand::Push(data, channels)) => {
                            manager.push_audio_data(&data, channels);
                        }
                        Ok(AudioOutputCommand::PushSound(samples, gain)) => {
                            manager.push_sound_effect(samples, gain);
                        }
                        Ok(AudioOutputCommand::SetMonitoring(enabled)) => {
                            manager.set_monitoring(enabled);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(AudioOutputCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                            manager.close();
                            published.store(0, Ordering::Relaxed);
                            break;
                        }
                    }
                }
            })
            .inspect_err(|e| log::error!("[Audio] Failed to spawn the output thread: {e}"))
            .ok();
        Self {
            tx,
            queued_ms,
            thread: std::sync::Mutex::new(thread),
        }
    }

    /// Spawn the persistent device thread whose hard-mute gate is driven by
    /// `muted` — pass `NetworkStats::mute_flag()` so every mute change
    /// (GUI, tray, plugins, phone) silences local output
    /// immediately.
    pub fn spawn_with_mute_flag(muted: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new(Self::with_mute_flag(muted))
    }

    /// Blocking open of the output device. Idempotent: returns immediately if
    /// the stream is already open.
    pub fn ensure_open(&self, device: Option<String>, buffer_ms: usize) -> bool {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .tx
            .send(AudioOutputCommand::Open(device, buffer_ms, reply_tx))
            .is_err()
        {
            return false;
        }
        reply_rx.recv().unwrap_or(false)
    }

    /// Push decoded PCM into the output ring buffer. The channel is unbounded,
    /// so this never blocks or drops audio while the device thread lives.
    pub fn push(&self, data: Vec<f32>, channels: usize) {
        let _ = self.tx.send(AudioOutputCommand::Push(data, channels));
    }

    /// Queue a plugin sound effect; mixed into the virtual mic output stream
    pub fn push_sound(&self, samples: Vec<f32>, gain: f32) {
        let _ = self.tx.send(AudioOutputCommand::PushSound(samples, gain));
    }

    pub fn set_monitoring(&self, enabled: bool) {
        let _ = self.tx.send(AudioOutputCommand::SetMonitoring(enabled));
    }

    /// Milliseconds of audio currently queued in the output ring buffer.
    pub fn queued_ms(&self) -> f64 {
        f64::from_bits(self.queued_ms.load(Ordering::Relaxed))
    }

    /// Close the output stream and stop the device thread. Only called when
    /// the process is exiting.
    pub fn shutdown(&self) {
        let _ = self.tx.send(AudioOutputCommand::Shutdown);
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let Some(thread) = thread else {
            return;
        };
        let deadline = Instant::now() + SHUTDOWN_WAIT;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if thread.is_finished() {
            let _ = thread.join();
        } else {
            log::warn!("[Audio] Output thread did not stop within {SHUTDOWN_WAIT:?}");
        }
    }
}

/// Normalize a persisted output-device value ("", "auto", "default" all mean
/// "no explicit device") to the form the audio engine expects.
pub fn normalize_output_device(raw: &str) -> Option<String> {
    let device = raw.trim();
    if device.is_empty() || device == "auto" || device == "default" {
        None
    } else {
        Some(device.to_string())
    }
}

/// Open the persistent output device (and on Linux the PipeWire virtual
/// sink/source it routes into). Idempotent, so app startup and every server
/// start may call it. `resource_dir` is the resolved bundle directory.
pub fn ensure_started(
    output: &AudioOutputHandle,
    device: Option<String>,
    buffer_ms: usize,
    resource_dir: Option<&std::path::Path>,
) -> bool {
    #[cfg(target_os = "linux")]
    if device.is_none()
        && crate::platform::pipewire::is_available()
        && !crate::platform::pipewire::is_setup()
    {
        if crate::platform::pipewire::setup(resource_dir) {
            log::info!("[PipeWire] Virtual device ready, ALSA will route to virtual sink");
        } else {
            log::warn!("[PipeWire] Setup failed, falling back to default device");
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = resource_dir;

    output.ensure_open(device, buffer_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stall_detector_trips_only_after_timeout_without_ticks() {
        let start = Instant::now();
        let mut stall = StallDetector::new(10, start);
        assert!(!stall.stalled(10, start + STALL_TIMEOUT / 2));
        assert!(!stall.stalled(11, start + STALL_TIMEOUT * 2));
        assert!(!stall.stalled(11, start + STALL_TIMEOUT * 3));
        assert!(stall.stalled(11, start + STALL_TIMEOUT * 3 + Duration::from_millis(1)));
    }
}
