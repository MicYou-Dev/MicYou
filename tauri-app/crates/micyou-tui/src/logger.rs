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

//! `log` backend that feeds records into the TUI logs page.
//!
//! Writing to stderr would tear the alternate screen, so records travel over
//! the event channel instead. Once the UI has exited (the receiver is gone)
//! they fall back to stderr so shutdown errors are still visible.

use crate::events::Event;
use log::{Level, LevelFilter, Log, Metadata, Record};
use micyou_core::logging::LogFilter;
use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

struct TuiLogger {
    filter: LogFilter,
    tx: Mutex<Sender<Event>>,
}

impl Log for TuiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let tag = match record.level() {
            Level::Error => "[err]",
            Level::Warn => "[warn]",
            Level::Info | Level::Debug | Level::Trace => "[inf]",
        };
        let line = format!("{tag} {}", record.args());
        let sent = self
            .tx
            .lock()
            .map(|tx| tx.send(Event::Log(line.clone())).is_ok())
            .unwrap_or(false);
        if !sent {
            let _ = writeln!(std::io::stderr().lock(), "{line}");
        }
    }

    fn flush(&self) {}
}

/// Install the logger. Call once, before the server starts.
pub fn init(tx: Sender<Event>) {
    let filter = LogFilter::from_env(LevelFilter::Info);
    let logger: &'static TuiLogger = Box::leak(Box::new(TuiLogger {
        filter,
        tx: Mutex::new(tx),
    }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(filter.max_level());
    }
}
