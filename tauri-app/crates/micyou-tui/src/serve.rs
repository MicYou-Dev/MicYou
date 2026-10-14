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

use crate::events::{Event, TuiEventSink};
use micyou_core::host::headless::HeadlessHost;
use micyou_core::mode_lock::{self, RunMode};
use micyou_core::server::{self, ServerState, StartRequest};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

pub struct ServeArgs {
    pub port: Option<u16>,
    pub mode: Option<String>,
    pub device: Option<String>,
    pub bind: Option<String>,
}

/// Start the audio server and own it for the lifetime of the terminal UI.
pub async fn run(args: ServeArgs) -> Result<(), String> {
    let (tx, rx) = channel::<Event>();
    crate::logger::init(tx.clone());
    let request = StartRequest::resolve(args.port, args.mode.as_deref(), args.device, args.bind)?;
    mode_lock::acquire(RunMode::Tui)?;
    let result = run_locked(request, tx, rx).await;
    mode_lock::release();
    result
}

async fn run_locked(
    request: StartRequest,
    tx: Sender<Event>,
    rx: Receiver<Event>,
) -> Result<(), String> {
    if request.mode == server::service::ConnectionMode::Usb {
        micyou_core::platform::adb::enable_usb_mode(request.port, None)
            .map_err(|e| format!("enable_usb_mode failed: {e}"))?;
    }

    let state = Arc::new(ServerState::new(
        Arc::new(TuiEventSink::new(tx)),
        Arc::new(HeadlessHost::new()),
        None,
    ));
    state.set_spectrum_streaming(true);

    let port = request.port;
    let mode = request.mode.as_str().to_string();
    server::start_server(&state, request).await?;
    // The UI loop blocks; hand this worker's queued tasks to the others so
    // the network tasks keep running.
    let tui_result =
        tokio::task::block_in_place(|| crate::tui::run_tui(rx, state.clone(), port, mode));
    if let Err(e) = server::stop_server(&state).await {
        eprintln!("error while stopping: {e}");
    }
    server::service::close_output_device(&state);
    tui_result
}
