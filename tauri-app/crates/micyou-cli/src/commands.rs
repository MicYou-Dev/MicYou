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

use micyou_core::config;
use micyou_core::micyou_audio::dsp::AudioDspSettings;
#[cfg(target_os = "windows")]
use std::process::exit;
use micyou_core::mode_lock as lock;

pub fn cmd_devices() {
    let devices = micyou_core::settings::audio_devices();
    if devices.is_empty() {
        println!("no audio output devices found");
        return;
    }
    println!("audio output devices:");
    for (i, name) in devices.iter().enumerate() {
        println!("  {}. {name}", i + 1);
    }
}

pub fn cmd_status() {
    match lock::read_lock() {
        Some(lock_info) => {
            let mode = match lock_info.mode {
                lock::RunMode::Gui => "GUI",
                lock::RunMode::Cli => "CLI",
                lock::RunMode::Tui => "TUI",
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let started_ago = now.saturating_sub(lock_info.started_at);
            println!(
                "mode: {mode}\npid: {}\nstarted {}s ago\nlock file: {}",
                lock_info.pid,
                started_ago,
                lock::lock_path().display()
            );
        }
        None => {
            println!("no server running");
        }
    }
    println!("settings file: {}", config::settings_path().display());
    println!("data dir: {}", lock::data_dir().display());
}

pub fn cmd_stop() {
    match lock::read_lock() {
        Some(lock_info) if lock_info.mode == lock::RunMode::Cli => {
            println!(
                "the CLI server (pid {}) manages its own lifetime - stop it by pressing Ctrl+C in its terminal",
                lock_info.pid
            );
        }
        Some(lock_info) if lock_info.mode == lock::RunMode::Tui => {
            println!(
                "the TUI server (pid {}) manages its own lifetime - stop it by pressing q or Ctrl+C in its terminal",
                lock_info.pid
            );
        }
        Some(lock_info) if lock_info.mode == lock::RunMode::Gui => {
            println!(
                "the GUI (pid {}) is running - stop the server from the app window or tray",
                lock_info.pid
            );
        }
        Some(_) => {
            println!("unknown lock state");
        }
        None => {
            println!("no server running");
        }
    }
}

fn print_settings(settings: &AudioDspSettings) {
    let raw = serde_json::to_string_pretty(settings).unwrap_or_default();
    println!("{raw}");
}

pub fn cmd_settings_get(key: Option<String>) -> Result<(), String> {
    let settings = config::load_dsp_settings();
    match key {
        None => print_settings(&settings),
        Some(k) => {
            let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
            match value.get(&k) {
                Some(v) => println!("{k} = {v}"),
                None => {
                    eprintln!("unknown setting: {k}");
                    return Err(format!("unknown setting: {k}"));
                }
            }
        }
    }
    Ok(())
}

pub fn cmd_settings_set(key: String, value: String) -> Result<(), String> {
    let mut settings = config::load_dsp_settings();
    let mut current = serde_json::to_value(&settings).map_err(|e| e.to_string())?;

    let parsed = parse_value(&value);
    match current.get_mut(&key) {
        Some(holder) => {
            *holder = parsed;
        }
        None => {
            return Err(format!("unknown setting: {key}"));
        }
    }
    settings = serde_json::from_value(current).map_err(|e| e.to_string())?;
    if key == "aecEnabled" && settings.aec_enabled {
        if let Some(reason) = micyou_core::micyou_audio::aec_reference_availability().reason {
            return Err(format!("AEC is unavailable: {}", reason.as_str()));
        }
    }
    config::save_dsp_settings(&settings)?;
    println!("{key} = {value}");
    Ok(())
}

fn parse_value(value: &str) -> serde_json::Value {
    if let Ok(n) = value.parse::<i64>() {
        return serde_json::json!(n);
    }
    if let Ok(f) = value.parse::<f64>() {
        return serde_json::json!(f);
    }
    if value == "true" {
        return serde_json::json!(true);
    }
    if value == "false" {
        return serde_json::json!(false);
    }
    serde_json::Value::String(value.to_string())
}

pub fn cmd_chain_list() {
    let settings = config::load_dsp_settings();
    if settings.processing_chain.is_empty() {
        println!("processing chain: (empty)");
        return;
    }
    println!("processing chain:");
    for (i, stage) in settings.processing_chain.iter().enumerate() {
        println!("  {}. {stage}", i + 1);
    }
}

pub fn cmd_chain_set(chain: Vec<String>) -> Result<(), String> {
    let mut settings = config::load_dsp_settings();
    let mut normalized: Vec<String> = Vec::new();
    for item in chain {
        if item == "AEC" && !normalized.is_empty() && normalized[0] == "AEC" {
            continue; // dedupe
        }
        normalized.push(item);
    }
    settings.processing_chain = normalized;
    config::save_dsp_settings(&settings)?;
    cmd_chain_list();
    Ok(())
}

pub fn cmd_mics() {
    #[cfg(target_os = "linux")]
    {
        let status = micyou_core::settings::pipewire_status();
        println!("PipeWire status:");
        println!("  available: {}", status.available);
        println!("  virtual sink: {}", status.device_exists);
        if status.available && !status.device_exists {
            println!("  run `micyou-cli serve` to auto-setup the virtual sink, or use the GUI");
        }
        if !status.available {
            println!("  PipeWire not detected (is pipewire-pulse running?)");
            if !status.install_command.is_empty() {
                println!("  Install command: {}", status.install_command);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let installed = micyou_core::platform::blackhole::is_installed();
        println!("BlackHole status:");
        println!("  installed: {installed}");
        if !installed {
            println!("  install BlackHole from https://existential.audio/blackhole/");
        }
    }
    #[cfg(target_os = "windows")]
    {
        let installed = micyou_core::platform::vbcable::is_installed();
        println!("VB-CABLE status:");
        println!("  installed: {installed}");
        if !installed {
            println!("  run `micyou-cli mics install` to install VB-CABLE");
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        println!("unsupported platform");
    }
}

#[cfg(target_os = "windows")]
pub async fn cmd_mics_install() -> Result<(), String> {
    let events: micyou_core::events::SharedEvents =
        std::sync::Arc::new(crate::events::CliEventSink::new(false));
    let result = micyou_core::platform::vbcable::install(events).await;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).unwrap_or_default()
    );
    if result.success {
        Ok(())
    } else {
        exit(1)
    }
}

pub fn cmd_adb_devices() {
    match micyou_core::platform::adb::list_adb_devices() {
        Ok(devices) if devices.is_empty() => {
            println!("no ADB devices found");
        }
        Ok(devices) => {
            println!("ADB devices:");
            for device in devices {
                println!(
                    "  {} ({}) - {}",
                    device.serial, device.state, device.description
                );
            }
        }
        Err(e) => {
            eprintln!("failed to list ADB devices: {e}");
        }
    }
}

pub fn cmd_config_path() {
    println!("config dir: {}", config::config_dir().display());
    println!("settings: {}", config::settings_path().display());
    println!("lock: {}", lock::lock_path().display());
}

pub fn cmd_server_get() {
    let prefs = config::load_server_prefs();
    println!("mode: {}", prefs.mode);
    println!("port: {}", prefs.port);
    println!("webPort: {}", prefs.web_port);
    println!("bindAddress: {}", prefs.bind_address);
    println!("autoBind: {}", prefs.auto_bind);
    println!("outputDevice: {}", prefs.output_device);
    println!(
        "file: {}",
        config::server_prefs_path().display()
    );
}

pub fn cmd_server_set(key: &str, value: &str) -> Result<(), String> {
    let mut prefs = config::load_server_prefs();
    match key {
        "port" => {
            let v: u16 = value
                .parse()
                .map_err(|_| format!("invalid port '{value}'"))?;
            if v == 0 {
                return Err("port must be > 0".to_string());
            }
            prefs.port = v;
        }
        "webPort" => {
            let v: u16 = value
                .parse()
                .map_err(|_| format!("invalid webPort '{value}'"))?;
            prefs.web_port = v;
        }
        "mode" => {
            if !["wifi", "usb", "web"].contains(&value) {
                return Err(format!(
                    "invalid mode '{value}' (expected wifi, usb or web)"
                ));
            }
            prefs.mode = value.to_string();
        }
        "bindAddress" => {
            prefs.bind_address = value.to_string();
        }
        "autoBind" => match value {
            "true" | "1" | "yes" | "on" => prefs.auto_bind = true,
            "false" | "0" | "no" | "off" => prefs.auto_bind = false,
            _ => return Err(format!("invalid boolean '{value}'")),
        },
        "outputDevice" => {
            prefs.output_device = value.to_string();
        }
        _ => {
            return Err(format!(
                "unknown key '{key}' (expected port, webPort, mode, bindAddress, autoBind, outputDevice)"
            ));
        }
    }
    config::save_server_prefs(&prefs)?;
    println!("{key} = {value}");
    Ok(())
}
