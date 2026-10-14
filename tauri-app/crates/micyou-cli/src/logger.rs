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

//! Minimal stderr backend for the `log` facade used by micyou-core.

use log::{LevelFilter, Log, Metadata, Record};
use micyou_core::logging::LogFilter;
use std::io::Write;

struct StderrLogger {
    filter: LogFilter,
}

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = record.level().as_str().to_ascii_lowercase();
        let _ = writeln!(std::io::stderr().lock(), "[{level}] {}", record.args());
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

/// Install the logger. Call once, before any other work.
pub fn init(default: LevelFilter) {
    let filter = LogFilter::from_env(default);
    let logger: &'static StderrLogger = Box::leak(Box::new(StderrLogger { filter }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(filter.max_level());
    }
}
