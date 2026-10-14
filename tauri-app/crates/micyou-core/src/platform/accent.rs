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

//! Desktop accent color that seeds the "system" theme.

use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemAccentColor {
    pub hex: String,
    pub source: String,
    pub supported: bool,
}

#[cfg_attr(not(any(windows, target_os = "macos", target_os = "linux")), allow(dead_code))]
fn accent_color(hex: impl Into<String>, source: impl Into<String>) -> SystemAccentColor {
    SystemAccentColor {
        hex: hex.into(),
        source: source.into(),
        supported: true,
    }
}

fn fallback_accent_color() -> SystemAccentColor {
    SystemAccentColor {
        hex: "#5b7cfa".to_string(),
        source: "fallback".to_string(),
        supported: false,
    }
}

#[cfg(windows)]
fn platform_accent_color() -> Option<SystemAccentColor> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let dwm = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\DWM")
        .ok()?;
    let value: u32 = dwm.get_value("AccentColor").ok()?;
    Some(accent_color(abgr_to_hex(value), "windows-dwm"))
}

/// Windows stores AccentColor as AABBGGRR.
#[cfg(windows)]
fn abgr_to_hex(value: u32) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        value & 0xff,
        (value >> 8) & 0xff,
        (value >> 16) & 0xff
    )
}

#[cfg(target_os = "macos")]
fn platform_accent_color() -> Option<SystemAccentColor> {
    macos_appkit_accent_color().or_else(macos_defaults_accent_color)
}

/// Asks AppKit for the resolved accent instead of reading the `AppleAccentColor`
/// default: that key does not exist at all while the user keeps the default
/// "Multicolor" choice, so reading it alone reported the feature as unsupported on
/// a stock Mac. `controlAccentColor` answers on every supported version and already
/// resolves Multicolor and Graphite to the colour macOS actually draws.
///
/// The classes are looked up rather than named with `class!`, which panics when
/// AppKit is not loaded (the CLI and TUI do not link it); those fall back to the
/// preference key.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn macos_appkit_accent_color() -> Option<SystemAccentColor> {
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    let color_class = Class::get("NSColor")?;
    let space_class = Class::get("NSColorSpace")?;
    let (red, green, blue) = unsafe {
        let color: *mut Object = msg_send![color_class, controlAccentColor];
        if color.is_null() {
            return None;
        }
        // Catalog colours raise when asked for components directly, so resolve into
        // sRGB first (and it is what the hex below is expressed in).
        let space: *mut Object = msg_send![space_class, sRGBColorSpace];
        if space.is_null() {
            return None;
        }
        let converted: *mut Object = msg_send![color, colorUsingColorSpace: space];
        if converted.is_null() {
            return None;
        }
        let red: f64 = msg_send![converted, redComponent];
        let green: f64 = msg_send![converted, greenComponent];
        let blue: f64 = msg_send![converted, blueComponent];
        (red, green, blue)
    };

    Some(accent_color(components_to_hex(red, green, blue), "macos-accent"))
}

/// Rounds AppKit's 0..=1 colour components into `#rrggbb`, clamping values outside
/// that range because extended-range colour spaces can exceed it.
#[cfg(target_os = "macos")]
fn components_to_hex(red: f64, green: f64, blue: f64) -> String {
    fn channel(value: f64) -> u8 {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(red),
        channel(green),
        channel(blue)
    )
}

/// Fallback for when AppKit cannot resolve a colour: the preference key only
/// exists once the user picks a named accent, so this misses the default.
#[cfg(target_os = "macos")]
fn macos_defaults_accent_color() -> Option<SystemAccentColor> {
    let output = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleAccentColor"])
        .output()
        .ok()?;
    let value = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i32>()
        .ok()?;
    let hex = match value {
        0 => "#ff3b30",
        1 => "#ff9500",
        2 => "#ffcc00",
        3 => "#34c759",
        4 => "#007aff",
        5 => "#af52de",
        6 => "#ff2d55",
        _ => return None,
    };
    Some(accent_color(hex, "macos-accent"))
}

#[cfg(target_os = "linux")]
fn platform_accent_color() -> Option<SystemAccentColor> {
    let is_kde = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.split(':').any(|part| part.eq_ignore_ascii_case("KDE")))
        .unwrap_or(false);
    if is_kde {
        kde_accent_color().or_else(gnome_accent_color)
    } else {
        gnome_accent_color().or_else(kde_accent_color)
    }
}

/// Plasma writes the accent (custom or wallpaper-derived) to kdeglobals; without
/// one, the color scheme's selection color is what Plasma uses as the accent.
#[cfg(target_os = "linux")]
fn kde_accent_color() -> Option<SystemAccentColor> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    let content = std::fs::read_to_string(config_home.join("kdeglobals")).ok()?;
    let hex = kde_accent_from_globals(&content)?;
    Some(accent_color(hex, "kde-accent"))
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn kde_accent_from_globals(content: &str) -> Option<String> {
    ini_value(content, "General", "AccentColor")
        .or_else(|| ini_value(content, "Colors:Selection", "BackgroundNormal"))
        .and_then(kde_color_to_hex)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn ini_value<'a>(content: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut in_section = false;
    for line in content.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name == section;
        } else if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim());
                }
            }
        }
    }
    None
}

/// KDE stores colors as "r,g,b" (optionally ",a"), occasionally as "#rrggbb".
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn kde_color_to_hex(value: &str) -> Option<String> {
    if let Some(hex) = value.strip_prefix('#') {
        return (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| format!("#{}", hex.to_ascii_lowercase()));
    }
    let parts: Vec<u8> = value
        .split(',')
        .map(|p| p.trim().parse::<u8>().ok())
        .collect::<Option<_>>()?;
    match parts.as_slice() {
        [r, g, b] | [r, g, b, _] => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn gnome_accent_color() -> Option<SystemAccentColor> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "accent-color"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('\'')
        .to_ascii_lowercase();
    let hex = match value.as_str() {
        "red" => "#f44336",
        "orange" => "#ff9800",
        "yellow" => "#fbc02d",
        "green" => "#4caf50",
        "blue" => "#2196f3",
        "purple" => "#9c27b0",
        "pink" => "#e91e63",
        "slate" => "#607d8b",
        _ => return None,
    };
    Some(accent_color(hex, "gnome-accent"))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_accent_color() -> Option<SystemAccentColor> {
    None
}

pub fn system_accent_color() -> SystemAccentColor {
    platform_accent_color().unwrap_or_else(fallback_accent_color)
}

#[cfg(test)]
mod tests {
    use super::{fallback_accent_color, kde_accent_from_globals, kde_color_to_hex};

    #[test]
    fn fallback_is_stable_and_marked_unsupported() {
        let color = fallback_accent_color();
        assert_eq!(color.hex, "#5b7cfa");
        assert_eq!(color.source, "fallback");
        assert!(!color.supported);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn appkit_components_become_a_clamped_hex_string() {
        use super::components_to_hex;
        assert_eq!(components_to_hex(0.0, 0.0, 0.0), "#000000");
        assert_eq!(components_to_hex(1.0, 1.0, 1.0), "#ffffff");
        // The macOS default accent, which is what a stock Mac should report.
        assert_eq!(components_to_hex(0.0, 122.0 / 255.0, 1.0), "#007aff");
        // Extended-range colour spaces can exceed 0..=1.
        assert_eq!(components_to_hex(-0.5, 2.0, 0.5), "#00ff80");
    }

    #[test]
    fn reads_kde_accent_before_selection_color() {
        let globals = "[Colors:Selection]\nBackgroundNormal=81,80,133\n\n[General]\nAccentColor=108,106,180\n";
        assert_eq!(kde_accent_from_globals(globals).as_deref(), Some("#6c6ab4"));
    }

    #[test]
    fn falls_back_to_kde_selection_color() {
        let globals = "[General]\nColorScheme=BreezeDark\n\n[Colors:Selection]\nBackgroundNormal=61,174,233\n";
        assert_eq!(kde_accent_from_globals(globals).as_deref(), Some("#3daee9"));
    }

    #[test]
    fn parses_kde_color_formats() {
        assert_eq!(kde_color_to_hex("61,174,233,255").as_deref(), Some("#3daee9"));
        assert_eq!(kde_color_to_hex("#3DAEE9").as_deref(), Some("#3daee9"));
        assert_eq!(kde_color_to_hex("300,0,0"), None);
        assert_eq!(kde_color_to_hex("1,2"), None);
    }

    #[cfg(windows)]
    #[test]
    fn converts_windows_abgr_accent_color() {
        assert_eq!(super::abgr_to_hex(0xff3366cc), "#cc6633");
    }
}
