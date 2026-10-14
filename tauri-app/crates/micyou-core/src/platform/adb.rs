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

use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AdbDevice {
    pub serial: String,
    pub state: String,
    pub description: String,
}

#[derive(serde::Serialize)]
#[serde(tag = "type")]
pub enum UsbModeResult {
    Success,
    NoDevices,
    MultipleDevices { devices: Vec<AdbDevice> },
}

pub fn find_adb() -> Option<PathBuf> {
    if let Ok(path) = env::var("PATH") {
        let separator = if cfg!(windows) { ";" } else { ":" };
        let executable = if cfg!(windows) { "adb.exe" } else { "adb" };

        for dir in path.split(separator) {
            let mut adb_path = PathBuf::from(dir);
            adb_path.push(executable);
            if adb_path.exists() {
                return Some(adb_path);
            }
        }
    }

    let common_paths = if cfg!(windows) {
        vec![
            format!(
                "{}\\Android\\Sdk\\platform-tools\\adb.exe",
                env::var("LOCALAPPDATA").unwrap_or_default()
            ),
            format!(
                "{}\\AppData\\Local\\Android\\Sdk\\platform-tools\\adb.exe",
                env::var("USERPROFILE").unwrap_or_default()
            ),
            "C:\\Android\\sdk\\platform-tools\\adb.exe".to_string(),
        ]
    } else {
        vec![
            format!(
                "{}/Android/Sdk/platform-tools/adb",
                env::var("HOME").unwrap_or_default()
            ),
            "/usr/bin/adb".to_string(),
            "/usr/local/bin/adb".to_string(),
            "/opt/android-sdk/platform-tools/adb".to_string(),
            "/opt/homebrew/bin/adb".to_string(),
        ]
    };

    for path in common_paths {
        if !path.is_empty() {
            let adb_path = PathBuf::from(path);
            if adb_path.exists() {
                return Some(adb_path);
            }
        }
    }

    None
}

fn parse_adb_devices(output: &str) -> Vec<AdbDevice> {
    let mut devices = Vec::new();

    for line in output.lines() {
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() || line.starts_with("List of") || line.starts_with("*") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let serial = parts[0].to_string();
            let state = parts[1].to_string();

            if state == "offline" {
                continue;
            }

            let mut description = serial.clone();
            for part in &parts[2..] {
                if let Some(val) = part.strip_prefix("model:") {
                    description = val.to_string();
                    break;
                }
            }

            devices.push(AdbDevice {
                serial,
                state,
                description,
            });
        }
    }

    devices
}

pub fn list_adb_devices() -> Result<Vec<AdbDevice>, String> {
    let adb = find_adb().ok_or("ADB not found in PATH or common locations")?;

    let output = Command::new(adb)
        .arg("devices")
        .arg("-l")
        .output()
        .map_err(|e| format!("Failed to execute adb command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() && stdout.trim().is_empty() {
        return Err(format!("ADB devices command failed: {}", stderr));
    }

    let devices = parse_adb_devices(&stdout);
    Ok(devices)
}

pub fn enable_usb_mode(port: u16, device_serial: Option<&str>) -> Result<UsbModeResult, String> {
    // iPhones need no adb: the server's usbmux bridge reaches them once it runs.
    let Some(adb) = find_adb() else {
        return if super::usbmux::has_usb_device() {
            Ok(UsbModeResult::Success)
        } else {
            Err("ADB not found in PATH or common locations".to_string())
        };
    };

    if let Some(serial) = device_serial {
        run_adb_reverse(&adb, port, Some(serial))?;
        return Ok(UsbModeResult::Success);
    }

    let output = Command::new(&adb)
        .arg("devices")
        .arg("-l")
        .output()
        .map_err(|e| format!("Failed to execute adb command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = parse_adb_devices(&stdout);

    match devices.len() {
        0 if super::usbmux::has_usb_device() => Ok(UsbModeResult::Success),
        0 => Ok(UsbModeResult::NoDevices),
        1 => {
            run_adb_reverse(&adb, port, Some(&devices[0].serial))?;
            Ok(UsbModeResult::Success)
        }
        _ => Ok(UsbModeResult::MultipleDevices { devices }),
    }
}

fn run_adb_reverse(adb: &PathBuf, port: u16, serial: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new(adb);

    if let Some(s) = serial {
        cmd.arg("-s").arg(s);
    }

    let output = cmd
        .arg("reverse")
        .arg(format!("tcp:{}", port))
        .arg(format!("tcp:{}", port))
        .output()
        .map_err(|e| format!("Failed to execute adb command: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        Err(format!("ADB reverse failed: {}", err_msg))
    }
}
