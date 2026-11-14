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
use crate::jsonl::{JsonlEventSink, Output};
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
    pub jsonl: bool,
}

/// Run the audio server in the foreground. CLI flags override the shared
/// server.json; otherwise GUI and CLI start with the same values.
pub async fn run(args: ServeArgs) -> Result<(), String> {
    let request = StartRequest::resolve(args.port, args.mode.as_deref(), args.device, args.bind)?;
    mode_lock::acquire(RunMode::Cli)?;
    // Modified 2026-10-08: JSONL controls use a bounded
    // stdout writer; the upstream human-readable mode remains unchanged.
    let (output, writer) = if args.jsonl {
        let (output, writer) = Output::start();
        (Some(output), Some(writer))
    } else {
        (None, None)
    };
    let jsonl_sink = output
        .as_ref()
        .map(|output| Arc::new(JsonlEventSink::new(output.clone())));
    let events: micyou_core::events::SharedEvents = match jsonl_sink.as_ref() {
        Some(sink) => sink.clone(),
        None => Arc::new(CliEventSink::new(args.quiet)),
    };
    let result = run_locked(request, args.jsonl, events.clone(), jsonl_sink.as_deref()).await;
    drop(events);
    drop(output);
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    mode_lock::release();
    result
}

async fn run_locked(
    request: StartRequest,
    jsonl: bool,
    events: micyou_core::events::SharedEvents,
    jsonl_sink: Option<&JsonlEventSink>,
) -> Result<(), String> {
    if request.mode == server::service::ConnectionMode::Usb {
        if jsonl {
            eprintln!("Setting up USB mode (adb for Android, usbmuxd for iOS) on port {}", request.port);
        } else {
            println!("Setting up USB mode (adb for Android, usbmuxd for iOS) on port {}", request.port);
        }
        micyou_core::platform::adb::enable_usb_mode(request.port, None)
            .map_err(|e| format!("enable_usb_mode failed: {e}"))?;
    }

    let state = ServerState::new(events, Arc::new(HeadlessHost::new()), None);
    let started = server::start_server(&state, request).await?;
    let control_result = if let Some(sink) = jsonl_sink {
        match sink.send_ready(serde_json::json!({
            "v": 1,
            "type": "ready",
            "payload": {
                "protocolVersion": 1,
                "backendVersion": env!("CARGO_PKG_VERSION"),
                "message": started
            }
        })) {
            Ok(()) => crate::jsonl::run_control(&state, sink.output()).await,
            Err(error) => Err(error),
        }
    } else {
        println!("{started}");
        println!("Press Ctrl+C to stop");
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("cannot listen for Ctrl+C ({e}), stopping");
        }
        Ok(())
    };

    if !jsonl {
        println!("Stopping server...");
    }
    let stop_result = server::stop_server(&state)
        .await
        .map_err(|error| format!("error while stopping: {error}"));
    if let Err(error) = &stop_result {
        eprintln!("{error}");
    }
    server::service::close_output_device(&state);
    control_result?;
    if jsonl {
        stop_result?;
    }
    Ok(())
}
