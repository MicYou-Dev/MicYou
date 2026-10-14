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

use crate::events::CliEventSink;
use micyou_core::host::headless::HeadlessHost;
use micyou_core::mode_lock::{self, RunMode};
use micyou_core::server::{self, ServerState, StartRequest};
use std::sync::Arc;

pub struct ServeArgs {
    pub port: Option<u16>,
    pub mode: Option<String>,
    pub device: Option<String>,
    pub bind: Option<String>,
    pub quiet: bool,
}

/// Run the audio server in the foreground. CLI flags override the shared
/// server.json; otherwise GUI and CLI start with the same values.
pub async fn run(args: ServeArgs) -> Result<(), String> {
    let request = StartRequest::resolve(args.port, args.mode.as_deref(), args.device, args.bind)?;
    mode_lock::acquire(RunMode::Cli)?;
    let result = run_locked(request, args.quiet).await;
    mode_lock::release();
    result
}

async fn run_locked(request: StartRequest, quiet: bool) -> Result<(), String> {
    if request.mode == server::service::ConnectionMode::Usb {
        println!("Setting up USB mode (adb for Android, usbmuxd for iOS) on port {}", request.port);
        micyou_core::platform::adb::enable_usb_mode(request.port, None)
            .map_err(|e| format!("enable_usb_mode failed: {e}"))?;
    }

    let state = ServerState::new(
        Arc::new(CliEventSink::new(quiet)),
        Arc::new(HeadlessHost::new()),
        None,
    );
    println!("{}", server::start_server(&state, request).await?);

    println!("Press Ctrl+C to stop");
    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("cannot listen for Ctrl+C ({e}), stopping");
    }
    println!("Stopping server...");
    if let Err(e) = server::stop_server(&state).await {
        eprintln!("error while stopping: {e}");
    }
    server::service::close_output_device(&state);
    Ok(())
}
