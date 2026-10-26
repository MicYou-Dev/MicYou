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

/// Stable reason codes reported when AEC becomes unavailable at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AecFailure {
    InferenceFailed,
    ModelLoadFailed,
    PermissionDenied,
    PipeWireUnavailable,
    ReferenceLost,
    UnsupportedOs,
    VirtualSourceMissing,
}

impl std::fmt::Display for AecFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AecFailure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InferenceFailed => "inference_failed",
            Self::ModelLoadFailed => "model_load_failed",
            Self::PermissionDenied => "permission_denied",
            Self::PipeWireUnavailable => "pipewire_unavailable",
            Self::ReferenceLost => "reference_lost",
            Self::UnsupportedOs => "unsupported_os",
            Self::VirtualSourceMissing => "virtual_source_missing",
        }
    }
}

/// Runtime capability of the platform's AEC far-end reference capture.
///
/// `available` is the gate the GUI, CLI and TUI all read; `reason` explains a
/// negative answer so a frontend can tell "this system cannot do it" apart from
/// "the capture stopped" without knowing anything about the platform itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AecAvailability {
    pub available: bool,
    pub reason: Option<AecFailure>,
}

impl AecAvailability {
    /// The platform can supply a far-end reference.
    pub const fn ready() -> Self {
        Self {
            available: true,
            reason: None,
        }
    }

    /// The running system has no supported capture path.
    pub const fn unsupported() -> Self {
        Self {
            available: false,
            reason: Some(AecFailure::UnsupportedOs),
        }
    }

    /// The system could capture, but this process is not allowed to.
    pub const fn permission_denied() -> Self {
        Self {
            available: false,
            reason: Some(AecFailure::PermissionDenied),
        }
    }
}

/// Whether this platform can capture an AEC far-end reference right now.
///
/// This is the single source of truth for every frontend. The answer is detected
/// at run time instead of being compiled in, so a system lacking the required
/// capture API reports [`AecAvailability::unsupported`] rather than failing once
/// AEC is switched on.
pub fn aec_reference_availability() -> AecAvailability {
    static CACHED: std::sync::OnceLock<AecAvailability> = std::sync::OnceLock::new();
    *CACHED.get_or_init(detect_reference_availability)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn detect_reference_availability() -> AecAvailability {
    // WASAPI render loopback and the PipeWire sink monitor always exist.
    AecAvailability::ready()
}

#[cfg(target_os = "macos")]
fn detect_reference_availability() -> AecAvailability {
    // Core Audio process taps exist from macOS 14.2, and only a bundle declaring
    // NSAudioCaptureUsageDescription gets real audio from them. Both are fixed for
    // the life of the process, so they belong here rather than in a runtime
    // failure that every new session would reset.
    if !crate::macos_tap::process_tap_available() {
        // Without taps the capture falls back to a BlackHole virtual device.
        // Whether that driver is installed can change while the app runs, so it
        // stays a runtime `VirtualSourceMissing` failure the user can retry after
        // installing it, instead of a verdict cached for the process lifetime.
        return AecAvailability::ready();
    }
    if !crate::macos_tap::bundle_declares_audio_capture() {
        // A mis-packaged bundle gets silence from its taps forever, but BlackHole
        // still works, so only report this when there is nothing to fall back to.
        if !crate::loopback::blackhole_device_present() {
            return AecAvailability::permission_denied();
        }
    }
    AecAvailability::ready()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn detect_reference_availability() -> AecAvailability {
    AecAvailability::unsupported()
}

#[cfg(test)]
mod tests {
    use super::{aec_reference_availability, AecAvailability, AecFailure};

    #[test]
    fn failure_codes_remain_stable_for_frontends() {
        assert_eq!(AecFailure::InferenceFailed.as_str(), "inference_failed");
        assert_eq!(AecFailure::ModelLoadFailed.as_str(), "model_load_failed");
        assert_eq!(
            AecFailure::PipeWireUnavailable.as_str(),
            "pipewire_unavailable"
        );
        assert_eq!(AecFailure::ReferenceLost.as_str(), "reference_lost");
        assert_eq!(
            AecFailure::VirtualSourceMissing.as_str(),
            "virtual_source_missing"
        );
        assert_eq!(AecFailure::PermissionDenied.as_str(), "permission_denied");
        assert_eq!(AecFailure::UnsupportedOs.as_str(), "unsupported_os");
    }

    #[test]
    fn availability_explains_every_unavailable_answer() {
        let availability = aec_reference_availability();
        assert_eq!(availability.available, availability.reason.is_none());
    }

    #[test]
    fn availability_detection_is_cached_and_consistent() {
        assert_eq!(aec_reference_availability(), aec_reference_availability());
    }

    #[test]
    fn ready_and_unsupported_differ_only_in_outcome() {
        assert!(AecAvailability::ready().available);
        assert_eq!(AecAvailability::ready().reason, None);
        assert!(!AecAvailability::unsupported().available);
        assert_eq!(
            AecAvailability::unsupported().reason,
            Some(AecFailure::UnsupportedOs)
        );
        assert!(!AecAvailability::permission_denied().available);
        assert_eq!(
            AecAvailability::permission_denied().reason,
            Some(AecFailure::PermissionDenied)
        );
    }
}
