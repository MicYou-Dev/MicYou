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

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// objc's msg_send! expands to cfgs rustc does not know about.
#![allow(unexpected_cfgs)]

mod app;
mod commands;
mod events;
mod host;
#[cfg(target_os = "linux")]
mod kwin_effects;
mod macos_dock;
mod macos_window;
mod menubar;
mod tray;
mod window;

/// Picks the WebKitGTK rendering path before GTK starts.
///
/// The accelerated DMA-BUF renderer is the default, including on NVIDIA:
/// with explicit sync disabled (`__NV_DISABLE_EXPLICIT_SYNC=1`) current
/// drivers render through it on KWin. `MICYOU_RENDERER=software` or
/// `--software-rendering` switches to CPU rendering. Variables already set
/// in the environment always win.
#[cfg(target_os = "linux")]
fn configure_renderer() {
    let software = std::env::args_os().any(|arg| arg == "--software-rendering")
        || std::env::var("MICYOU_RENDERER")
            .is_ok_and(|value| value.eq_ignore_ascii_case("software"));
    let set_default = |key: &str, value: &str| {
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    };

    if software {
        set_default("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        set_default("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        eprintln!("[Renderer] Software rendering fallback enabled");
    } else if std::path::Path::new("/proc/driver/nvidia/version").exists() {
        set_default("__NV_DISABLE_EXPLICIT_SYNC", "1");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    configure_renderer();

    app::run()
}
