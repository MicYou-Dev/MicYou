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

//! Valve's Steam Streaming audio driver, used as MicYou's Windows virtual
//! microphone path.
//!
//! The driver package ships as a WHQL-signed INF/SYS/CAT set. It is bundled
//! under `src-tauri/resources/steam-audio/x64` so a default install needs no
//! download and no extra software (VB-CABLE used to be a separate download).
//! `SteamStreamingMicrophone` provides the render/capture loopback pair MicYou
//! uses as its virtual microphone; `SteamStreamingSpeakers` is Valve's playback
//! sibling, installed for Steam Link compatibility and hidden from MicYou's
//! device picker.
//!
//! Drivers are installed through `pnputil /add-driver <inf> /install`, which
//! needs elevation. The in-app path raises one UAC prompt; the Inno Setup
//! installer already runs elevated and calls `pnputil` directly.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct VirtualAudioResult {
    pub success: bool,
    pub error_type: Option<String>,
    pub message: Option<String>,
}

/// Device names exposed by the Steam Streaming audio driver. Windows appends
/// the endpoint index in some locales, so match on the stable prefix.
pub fn is_steam_audio_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("steam streaming microphone") || lower.contains("steam streaming speakers")
}

/// Check whether the Steam Streaming Microphone render endpoint is usable.
/// The driver package can be present while the endpoint is disabled, or while
/// it still has the driver defaults (8 channels, +30 dB capture gain, render
/// and capture formats that disagree); those states must not be reported as
/// "installed" or the UI never offers the repair path that runs
/// `configure.ps1`.
#[cfg(target_os = "windows")]
pub fn is_installed() -> bool {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Some(name) = micyou_audio::device_name(&dev) {
                if name.to_lowercase().contains("steam streaming microphone") {
                    if let Ok(config) = dev.default_output_config() {
                        return config.channels() == 2 && config.sample_rate() == 48_000;
                    }
                    return false;
                }
            }
        }
    }
    false
}

/// Whether the Valve driver package is present, even if its endpoints are
/// disabled or still need `configure.ps1`. Used for actionable diagnostics.
#[cfg(target_os = "windows")]
pub fn driver_present() -> bool {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey_with_flags(
        "SYSTEM\\CurrentControlSet\\Services\\SteamStreamingMicrophone",
        KEY_READ,
    )
    .is_ok()
}

#[cfg(not(target_os = "windows"))]
pub fn is_installed() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn driver_present() -> bool {
    false
}

/// File names of the bundled package, in install order: playback endpoint
/// first so the recording endpoint MicYou advertises has a sink right away.
const DRIVER_INFS: [&str; 2] = ["SteamStreamingSpeakers.inf", "SteamStreamingMicrophone.inf"];

fn has_driver_package(dir: &Path) -> bool {
    DRIVER_INFS.iter().all(|inf| dir.join(inf).is_file())
}

/// Locate a driver package directory: the copy bundled with MicYou first, then
/// the one shipped by an installed Steam client. The Steam fallback keeps
/// development builds and pre-driver install trees working without a rebundle.
pub fn driver_dir(resource_hint: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(hint) = resource_hint {
        candidates.push(hint.join("steam-audio").join("x64"));
        candidates.push(hint.join("resources").join("steam-audio").join("x64"));
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        candidates.push(exe_dir.join("resources").join("steam-audio").join("x64"));
        candidates.push(exe_dir.join("steam-audio").join("x64"));
    }
    #[cfg(debug_assertions)]
    candidates.push(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src-tauri/resources/steam-audio/x64"
        ))
        .to_path_buf(),
    );

    for root in steam_install_roots() {
        candidates.push(root.join("drivers").join("Windows10").join("x64"));
        candidates.push(root.join("drivers").join("Windows8.1").join("x64"));
    }

    candidates.into_iter().find(|dir| has_driver_package(dir))
}

fn steam_install_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for variable in [
        "CommonProgramFiles(x86)",
        "ProgramFiles(x86)",
        "ProgramFiles",
    ] {
        if let Some(base) = std::env::var_os(variable) {
            roots.push(PathBuf::from(base).join("Steam"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;

        for (root, path) in [
            (HKEY_CURRENT_USER, "Software\\Valve\\Steam"),
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Valve\\Steam"),
        ] {
            if let Ok(key) = RegKey::predef(root).open_subkey_with_flags(path, KEY_READ) {
                if let Ok(install) = key.get_value::<String, _>("SteamPath") {
                    roots.push(PathBuf::from(install));
                }
            }
        }
    }

    roots
}

#[cfg(target_os = "windows")]
static IS_INSTALLING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "windows")]
fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("micyou_steam_audio")
}

/// The endpoint configuration script shipped beside the driver package.
#[cfg(target_os = "windows")]
fn configure_path(resource_hint: Option<&Path>, driver_dir: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(hint) = resource_hint {
        candidates.push(hint.join("steam-audio").join("configure.ps1"));
        candidates.push(
            hint.join("resources")
                .join("steam-audio")
                .join("configure.ps1"),
        );
    }
    if let Some(parent) = driver_dir.and_then(Path::parent) {
        candidates.push(parent.join("configure.ps1"));
    }
    #[cfg(debug_assertions)]
    candidates.push(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src-tauri/resources/steam-audio/configure.ps1"
        ))
        .to_path_buf(),
    );
    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn pnputil_path() -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join("pnputil.exe")
}

#[cfg(target_os = "windows")]
fn ps_quote(value: &str) -> String {
    value.replace('\'', "''")
}

/// A UAC prompt can stay open indefinitely, so elevated setup is bounded.
#[cfg(target_os = "windows")]
const INSTALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

#[cfg(target_os = "windows")]
fn pending_marker() -> PathBuf {
    crate::config::config_dir().join("steam_audio_pending")
}

#[cfg(target_os = "windows")]
fn mark_pending_configuration() {
    let path = pending_marker();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        "restart required to configure the Steam Streaming Microphone\n",
    );
}

#[cfg(target_os = "windows")]
fn clear_pending_configuration() {
    let _ = std::fs::remove_file(pending_marker());
}

/// Run a generated PowerShell script through one elevated PowerShell process
/// and return its exit code (0 or 3010 are success for pnputil).
#[cfg(target_os = "windows")]
async fn run_elevated(script: String) -> Result<i32, String> {
    let dir = temp_dir();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("create temp dir: {e}"))?;

    let script_path = dir.join("install-driver.ps1");
    tokio::fs::write(&script_path, script)
        .await
        .map_err(|e| format!("write install script: {e}"))?;

    // Start-Process joins the -ArgumentList array with spaces, so the script
    // path has to carry its own quotes for profiles such as "C:\Users\John Doe".
    let script_argument = format!("\"{}\"", script_path.to_string_lossy());
    let command = format!(
        "$p = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File','{}') -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
        ps_quote(&script_argument)
    );

    let output = tokio::time::timeout(
        INSTALL_TIMEOUT,
        tokio::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .output(),
    )
    .await
    .map_err(|_| "timeout".to_string())?
    .map_err(|e| format!("run powershell: {e}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("canceled")
        || stderr.contains("cancelled")
        || stderr.contains("elevation")
        || stderr.contains("administrator")
        || stderr.contains("拒绝")
        || stderr.contains("取消")
    {
        return Err("uac_denied".to_string());
    }

    // pnputil reports 3010 (ERROR_SUCCESS_REBOOT_REQUIRED) as a failed process
    // exit status, but the driver was staged successfully.
    let code = output.status.code().unwrap_or(-1);
    if code == 0 || code == 3010 {
        return Ok(code);
    }
    Err(format!(
        "installer exit code {code}: {}",
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

/// Run the driver installs in one elevated PowerShell process. `pnputil`
/// returns 3010 when a reboot is required, which is still a successful stage.
#[cfg(target_os = "windows")]
async fn run_installer(driver_dir: &Path, configure: Option<&Path>) -> Result<bool, String> {
    let mut script = String::from("$ErrorActionPreference = 'Stop'\r\n$reboot = $false\r\n");
    for inf in DRIVER_INFS {
        let path = driver_dir.join(inf);
        script.push_str(&format!(
            "& '{}' /add-driver '{}' /install\r\n$code = $LASTEXITCODE\r\nif ($code -eq 3010) {{ $reboot = $true }} elseif ($code -ne 0) {{ exit $code }}\r\n",
            ps_quote(&pnputil_path().to_string_lossy()),
            ps_quote(&path.to_string_lossy())
        ));
    }
    let configure_script = configure
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    script.push_str(&format!(
        "$cfg = '{}'\r\nif (-not $reboot -and $cfg -and (Test-Path -LiteralPath $cfg)) {{\r\n  & $cfg\r\n  if ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}\r\n}}\r\nif ($reboot) {{ exit 3010 }}\r\nexit 0\r\n",
        ps_quote(&configure_script)
    ));
    let code = run_elevated(script).await?;
    Ok(code == 3010)
}

/// Apply the endpoint format/gain without reinstalling the driver. This is the
/// repair path for machines where Steam Link / Remote Play already installed
/// the package but never ran `configure.ps1`.
#[cfg(target_os = "windows")]
async fn run_configure(configure: &Path) -> Result<(), String> {
    let script = format!(
        "$cfg = '{}'\r\nif (-not (Test-Path -LiteralPath $cfg)) {{ exit 2 }}\r\n& $cfg\r\nexit $LASTEXITCODE\r\n",
        ps_quote(&configure.to_string_lossy())
    );
    let code = run_elevated(script).await?;
    if code == 0 {
        Ok(())
    } else {
        Err(format!("configure script exit code {code}"))
    }
}

#[cfg(target_os = "windows")]
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

/// Install the bundled Steam Streaming audio driver.
#[cfg(target_os = "windows")]
pub async fn install(
    resource_hint: Option<&Path>,
    events: crate::events::SharedEvents,
) -> VirtualAudioResult {
    use std::sync::atomic::Ordering;

    if IS_INSTALLING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return VirtualAudioResult {
            success: false,
            error_type: Some("already_installing".to_string()),
            message: Some("Installation already in progress".to_string()),
        };
    }

    let result = install_inner(resource_hint, &events).await;
    IS_INSTALLING.store(false, Ordering::SeqCst);

    match &result {
        VirtualAudioResult { success: true, .. } => {
            events.install_progress("Installation complete".to_string());
        }
        VirtualAudioResult {
            error_type: Some(error_type),
            ..
        } => {
            events.install_progress(format!("Failed: {error_type}"));
        }
        _ => {}
    }

    let _ = std::fs::remove_dir_all(temp_dir());
    result
}

#[cfg(target_os = "windows")]
async fn install_inner(
    resource_hint: Option<&Path>,
    events: &crate::events::SharedEvents,
) -> VirtualAudioResult {
    let driver_dir = driver_dir(resource_hint);
    let configure = configure_path(resource_hint, driver_dir.as_deref());

    // Steam Link / Remote Play may have installed the package without the
    // MicYou endpoint configuration. Run the idempotent configure script even
    // when the driver is already present, instead of returning "Already
    // installed" and leaving the driver defaults (8 channels, +30 dB capture
    // gain, mismatched render/capture formats) in place.
    if driver_present() {
        let Some(configure) = configure.as_deref() else {
            return VirtualAudioResult {
                success: false,
                error_type: Some("driver_missing".to_string()),
                message: Some(
                    "Steam Streaming Microphone is installed but configure.ps1 is missing from this install; reinstall MicYou to repair it".to_string(),
                ),
            };
        };
        events.install_progress(
            "Configuring Steam Streaming Microphone (Windows audio may restart briefly)..."
                .to_string(),
        );
        if let Err(error) = run_configure(configure).await {
            let error_type = if error == "uac_denied" || error == "timeout" {
                error.as_str()
            } else {
                "config_failed"
            };
            return VirtualAudioResult {
                success: false,
                error_type: Some(error_type.to_string()),
                message: Some(error),
            };
        }
        if !wait_for_device(20).await {
            return VirtualAudioResult {
                success: false,
                error_type: Some("endpoint_unavailable".to_string()),
                message: Some(
                    "Steam Streaming Microphone is installed but its endpoint is not available; enable it in Windows Sound settings or reinstall the driver".to_string(),
                ),
            };
        }
        clear_pending_configuration();
        return VirtualAudioResult {
            success: true,
            error_type: None,
            message: Some("Steam Streaming Microphone is ready".to_string()),
        };
    }

    let Some(driver_dir) = driver_dir else {
        return VirtualAudioResult {
            success: false,
            error_type: Some("driver_missing".to_string()),
            message: Some(
                "Steam Streaming audio driver files are missing from this install".to_string(),
            ),
        };
    };

    events.install_progress("Installing Steam Streaming Microphone driver...".to_string());

    let reboot_required = match run_installer(&driver_dir, configure.as_deref()).await {
        Ok(reboot) => reboot,
        Err(error) => {
            let error_type = if error == "uac_denied" || error == "timeout" {
                error.as_str()
            } else {
                "install_failed"
            };
            return VirtualAudioResult {
                success: false,
                error_type: Some(error_type.to_string()),
                message: Some(error),
            };
        }
    };

    if reboot_required {
        mark_pending_configuration();
        return VirtualAudioResult {
            success: true,
            error_type: None,
            message: Some(
                "Driver installed; restart Windows, then run MicYou setup again to finish the endpoint configuration"
                    .to_string(),
            ),
        };
    }

    events.install_progress("Waiting for audio devices...".to_string());

    if !wait_for_device(30).await {
        return VirtualAudioResult {
            success: false,
            error_type: Some("timeout".to_string()),
            message: Some(
                "Steam Streaming Microphone did not appear within 30 seconds".to_string(),
            ),
        };
    }

    clear_pending_configuration();
    VirtualAudioResult {
        success: true,
        error_type: None,
        message: Some("Steam Streaming Microphone is ready".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
pub async fn install(
    _resource_hint: Option<&Path>,
    _events: crate::events::SharedEvents,
) -> VirtualAudioResult {
    VirtualAudioResult {
        success: false,
        error_type: Some("unsupported_platform".to_string()),
        message: Some("Steam Streaming audio is only used on Windows".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_steam_endpoint_names() {
        assert!(is_steam_audio_device("Steam Streaming Microphone"));
        assert!(is_steam_audio_device(
            "Steam Streaming Speakers (Steam Streaming Speakers)"
        ));
        assert!(!is_steam_audio_device(
            "CABLE Input (VB-Audio Virtual Cable)"
        ));
    }

    #[test]
    fn bundled_driver_package_is_present() {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/steam-audio/x64");
        assert!(
            has_driver_package(&dir),
            "bundled Steam audio driver package is missing: {}",
            dir.display()
        );
    }

    #[test]
    fn is_installed_runs_without_panic() {
        let _ = is_installed();
    }
}
