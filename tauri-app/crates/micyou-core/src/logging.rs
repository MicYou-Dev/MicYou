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

//! Level filtering shared by the CLI and TUI `log` backends (the GUI uses
//! `tauri-plugin-log`).

use log::{Level, LevelFilter};

/// MicYou crates log at a per-frontend default while third-party crates are
/// capped at `warn`, so mdns and audio backends do not drown the output.
/// `RUST_LOG=<level>` overrides both.
#[derive(Debug, Clone, Copy)]
pub struct LogFilter {
    own: LevelFilter,
    deps: LevelFilter,
}

impl LogFilter {
    pub fn from_env(default: LevelFilter) -> Self {
        let level = std::env::var("RUST_LOG")
            .ok()
            .and_then(|value| value.trim().parse::<LevelFilter>().ok());
        match level {
            Some(level) => Self {
                own: level,
                deps: level,
            },
            None => Self {
                own: default,
                deps: LevelFilter::Warn.min(default),
            },
        }
    }

    pub fn enabled(&self, target: &str, level: Level) -> bool {
        let filter = if target.starts_with("micyou") {
            self.own
        } else {
            self.deps
        };
        level <= filter
    }

    /// Value for `log::set_max_level`.
    pub fn max_level(&self) -> LevelFilter {
        self.own.max(self.deps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_party_crates_are_capped_at_warn() {
        let filter = LogFilter {
            own: LevelFilter::Info,
            deps: LevelFilter::Warn,
        };
        assert!(filter.enabled("micyou_core::server::output", Level::Info));
        assert!(!filter.enabled("micyou_core::server::output", Level::Debug));
        assert!(!filter.enabled("mdns_sd::service_daemon", Level::Info));
        assert!(filter.enabled("mdns_sd::service_daemon", Level::Warn));
        assert_eq!(filter.max_level(), LevelFilter::Info);
    }
}
