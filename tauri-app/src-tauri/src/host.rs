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

//! [`HostIntegration`] backed by Tauri plugins and webview windows.

use micyou_core::host::{HostIntegration, HotkeyCallback};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

pub struct TauriHost(pub AppHandle);

impl HostIntegration for TauriHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.0
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| format!("open_url: {e}"))
    }

    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| format!("notify: {e}"))
    }

    fn register_hotkey(&self, shortcut: &str, on_press: HotkeyCallback) -> Result<(), String> {
        self.0
            .global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    on_press();
                }
            })
            .map_err(|e| format!("hotkey register: {e}"))
    }

    fn open_plugin_panel(&self, plugin_id: &str, panel_id: &str, title: &str) -> Result<(), String> {
        let label = format!("plugin-window-{}", plugin_id.replace('.', "-"));
        if let Some(window) = self.0.get_webview_window(&label) {
            return window.set_focus().map_err(|e| e.to_string());
        }
        let builder = WebviewWindowBuilder::new(
            &self.0,
            &label,
            WebviewUrl::App(format!("index.html#/plugin/{plugin_id}/{panel_id}").into()),
        )
        .title(title)
        .inner_size(520.0, 720.0)
        .min_inner_size(360.0, 480.0)
        .decorations(false)
        .transparent(true);
        #[cfg(target_os = "macos")]
        let builder = builder.visible(false);
        builder
            .build()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
