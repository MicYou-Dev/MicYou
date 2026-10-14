/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/LanRhyme/MicYou>
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

//! macOS Dock integration: the badge label on the app's Dock tile. The entry
//! point is a no-op on other platforms, so callers do not have to guard by
//! target.

use tauri::{Runtime, Window};

/// Sets the Dock badge label; `None` clears it.
#[cfg(target_os = "macos")]
pub fn set_badge<R: Runtime>(window: &Window<R>, label: Option<String>) -> Result<(), String> {
    window.set_badge_label(label).map_err(|e| e.to_string())
}

/// Sets the Dock badge label; `None` clears it.
#[cfg(not(target_os = "macos"))]
pub fn set_badge<R: Runtime>(_window: &Window<R>, _label: Option<String>) -> Result<(), String> {
    Ok(())
}
