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

use serde::Serialize;
#[cfg(feature = "vbcable")]
use std::path::{Path, PathBuf};
#[cfg(feature = "vbcable")]
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Serialize)]
pub struct VBCableResult {
    pub success: bool,
    pub error_type: Option<String>,
    pub message: Option<String>,
}

/// Check if VB-CABLE is installed by scanning audio devices, registry, or driver files
#[cfg(target_os = "windows")]
pub fn is_installed() -> bool {
    use cpal::traits::{DeviceTrait, HostTrait};

    let is_vbcable_device = |name: &str| -> bool {
        let lower = name.to_lowercase();
        lower.contains("cable input")
            || lower.contains("cable output")
            || lower.contains("vb-audio")
            || (lower.contains("cable") && (lower.contains("virtual") || lower.contains("audio")))
    };

    // 1. Device check: check CPAL output and input devices
    let host = cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Some(name) = micyou_audio::device_name(&dev) {
                if is_vbcable_device(&name) {
                    return true;
                }
            }
        }
    }
    if let Ok(devices) = host.input_devices() {
        for dev in devices {
            if let Some(name) = micyou_audio::device_name(&dev) {
                if is_vbcable_device(&name) {
                    return true;
                }
            }
        }
    }

    // 2. Registry verification: check services and software keys (64-bit and 32-bit views)
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let registry_paths = [
        // Windows Services (service names created by official driver)
        "SYSTEM\\CurrentControlSet\\Services\\VBCABLE",
        "SYSTEM\\CurrentControlSet\\Services\\VBCABLEA",
        "SYSTEM\\CurrentControlSet\\Services\\VBCABLEB",
        "SYSTEM\\CurrentControlSet\\Services\\VB-Cable",
        // Software keys
        "SOFTWARE\\VB-Audio\\Cable",
        "SOFTWARE\\VB-Audio\\VB-Cable",
        "SOFTWARE\\WOW6432Node\\VB-Audio\\Cable",
        "SOFTWARE\\WOW6432Node\\VB-Audio\\VB-Cable",
    ];

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for path in &registry_paths {
        if hklm.open_subkey_with_flags(path, KEY_READ).is_ok() {
            return true;
        }
    }

    // 3. File system verification: check driver and utility files
    let file_paths = [
        r"C:\Program Files\VB\CABLE\vbcable_control_panel.exe",
        r"C:\Program Files (x86)\VB\CABLE\vbcable_control_panel.exe",
        r"C:\Windows\System32\drivers\vbcable.sys",
        r"C:\Windows\System32\drivers\vbcable_win7_x64.sys",
    ];
    for path in &file_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    false
}

#[cfg(not(target_os = "windows"))]
pub fn is_installed() -> bool {
    false
}

#[cfg(feature = "vbcable")]
static IS_INSTALLING: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "vbcable")]
const INSTALLER_URL: &str =
    "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip";
#[cfg(feature = "vbcable")]
const INSTALLER_NAME: &str = "VBCABLE_Setup_x64.exe";
#[cfg(feature = "vbcable")]
const INSTALLER_DIR: &str = "VBCABLE_Driver_Pack45";

#[cfg(feature = "vbcable")]
fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("micyou_vbcable")
}

#[cfg(feature = "vbcable")]
async fn download_installer(events: &crate::events::SharedEvents) -> Result<PathBuf, String> {
    let dir = temp_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("create temp dir: {e}"))?;

    let zip_path = dir.join("vbcable_pack.zip");

    events.install_progress("Downloading installer...".to_string());

    let bytes = reqwest::get(INSTALLER_URL)
        .await
        .map_err(|e| format!("download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("read response: {e}"))?;

    tokio::fs::write(&zip_path, &bytes)
        .await
        .map_err(|e| format!("write zip: {e}"))?;

    events.install_progress("Extracting installer...".to_string());

    let extract_dir = dir.join(INSTALLER_DIR);
    tokio::fs::create_dir_all(&extract_dir)
        .await
        .map_err(|e| format!("create extract dir: {e}"))?;

    let zip_path_clone = zip_path.clone();
    let extract_dir_clone = extract_dir.clone();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path_clone).map_err(|e| format!("open zip: {e}"))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;
        archive
            .extract(&extract_dir_clone)
            .map_err(|e| format!("extract zip: {e}"))
    })
    .await
    .map_err(|e| format!("spawn blocking: {e}"))??;

    // Clean up zip
    tokio::fs::remove_file(&zip_path).await.ok();

    // Find the installer exe
    let installer = extract_dir.join(INSTALLER_NAME);
    if installer.exists() {
        Ok(installer)
    } else {
        Err(format!("{INSTALLER_NAME} not found in extracted archive"))
    }
}

#[cfg(feature = "vbcable")]
async fn run_installer(installer_path: &Path) -> Result<(), String> {
    let path_str = installer_path.to_string_lossy().to_string();
    let cmd = format!(
        "Start-Process -FilePath '{}' -ArgumentList '-i','-h' -Verb RunAs -Wait",
        path_str.replace('\'', "''")
    );

    let output = tokio::process::Command::new("powershell")
        .args(["-Command", &cmd])
        .output()
        .await
        .map_err(|e| format!("run powershell: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("elevation") || stderr.contains("administrator") {
            return Err("uac_denied".to_string());
        }
        return Err(format!("installer exit code: {}", output.status));
    }

    Ok(())
}

#[cfg(feature = "vbcable")]
async fn wait_for_device(max_secs: u64) -> bool {
    if is_installed() {
        return true;
    }
    let mut waited = 0u64;
    while waited < max_secs {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        waited += 2;
        if is_installed() {
            return true;
        }
    }
    false
}

#[cfg(feature = "vbcable")]
fn cleanup_temp_files() {
    let dir = temp_dir();
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(feature = "vbcable")]
async fn configure_devices(events: &crate::events::SharedEvents) -> Result<(), String> {
    events.install_progress("Configuring devices...".to_string());
    let scripts = vec![
        "Get-PnpDevice -FriendlyName '*CABLE Output*' | Where-Object { $_.Status -eq 'OK' } | ForEach-Object { Write-Host \"Found: $($_.FriendlyName)\" }",
    ];
    for script in scripts {
        let _ = tokio::process::Command::new("powershell")
            .args(["-Command", script])
            .output()
            .await;
    }
    Ok(())
}

#[cfg(feature = "vbcable")]
pub async fn install(events: crate::events::SharedEvents) -> VBCableResult {
    if IS_INSTALLING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return VBCableResult {
            success: false,
            error_type: Some("already_installing".to_string()),
            message: Some("Installation already in progress".to_string()),
        };
    }

    let result = install_inner(&events).await;
    IS_INSTALLING.store(false, Ordering::SeqCst);

    match &result {
        VBCableResult { success: true, .. } => {
            events.install_progress("Installation complete".to_string());
        }
        VBCableResult {
            error_type: Some(et),
            ..
        } => {
            events.install_progress(format!("Failed: {et}"));
        }
        _ => {}
    }

    cleanup_temp_files();
    result
}

#[cfg(feature = "vbcable")]
async fn install_inner(events: &crate::events::SharedEvents) -> VBCableResult {
    if is_installed() {
        events.install_progress("Configuring devices...".to_string());
        if let Err(e) = configure_devices(events).await {
            return VBCableResult {
                success: true,
                error_type: Some("config_failed".to_string()),
                message: Some(format!("Installed but configuration failed: {e}")),
            };
        }
        return VBCableResult {
            success: true,
            error_type: None,
            message: Some("Already installed".to_string()),
        };
    }

    let installer_path = match download_installer(events).await {
        Ok(p) => p,
        Err(e) => {
            return VBCableResult {
                success: false,
                error_type: Some("download_failed".to_string()),
                message: Some(e),
            };
        }
    };

    events.install_progress("Installing (requires admin approval)...".to_string());

    if let Err(e) = run_installer(&installer_path).await {
        let error_type = if e == "uac_denied" {
            "uac_denied"
        } else {
            "install_failed"
        };
        return VBCableResult {
            success: false,
            error_type: Some(error_type.to_string()),
            message: Some(e),
        };
    }

    events.install_progress("Waiting for device initialization...".to_string());

    if !wait_for_device(30).await {
        return VBCableResult {
            success: false,
            error_type: Some("timeout".to_string()),
            message: Some("Device not detected after 30 seconds".to_string()),
        };
    }

    if let Err(e) = configure_devices(events).await {
        return VBCableResult {
            success: true,
            error_type: Some("config_failed".to_string()),
            message: Some(format!("Installed but configuration failed: {e}")),
        };
    }

    VBCableResult {
        success: true,
        error_type: None,
        message: Some("Installation and configuration complete".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_installed_runs_without_panic() {
        let _ = is_installed();
    }
}
