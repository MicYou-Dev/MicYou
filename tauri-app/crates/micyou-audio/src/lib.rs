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

pub mod aec;
#[cfg(feature = "dsp")]
pub mod dsp;
pub mod engine;
pub mod loopback;
#[cfg(target_os = "macos")]
mod macos_tap;
pub mod mixer;

pub use aec::{aec_reference_availability, AecAvailability, AecFailure};
#[cfg(feature = "dsp")]
pub use dsp::{AudioDspSettings, DspProcessor, EqualizerConfig};
pub use engine::{device_name, AudioOutputManager, RubatoResampler};
pub use loopback::LoopbackCapture;
pub use mixer::{SoundEffect, SoundMixer};
