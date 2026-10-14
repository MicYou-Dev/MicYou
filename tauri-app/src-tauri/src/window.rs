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

//! Main window management and the custom (frameless) window chrome.

use micyou_core::server::ServerState;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
pub fn apply_macos_vibrancy(win: &tauri::WebviewWindow) {
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

    let _ = apply_vibrancy(
        win,
        NSVisualEffectMaterial::Sidebar,
        Some(NSVisualEffectState::Active),
        // Matches the CSS `rounded-2xl` window chrome.
        Some(16.0),
    );

    use objc::runtime::{Class, Object, NO};
    use objc::{msg_send, sel, sel_impl};

    if let Ok(ptr) = win.ns_window() {
        #[allow(unexpected_cfgs)]
        unsafe {
            let ns_window = ptr as *mut Object;
            if let Some(ns_color) = Class::get("NSColor") {
                let clear: *mut Object = msg_send![ns_color, clearColor];
                let _: () = msg_send![ns_window, setOpaque: NO];
                let _: () = msg_send![ns_window, setBackgroundColor: clear];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_macos_vibrancy(_: &tauri::WebviewWindow) {}

/// Windows 11: let DWM round the frameless window so the acrylic backdrop
/// follows the rounded content. Windows 10 rejects the attribute; that is fine.
#[cfg(windows)]
pub fn apply_rounded_corners(win: &tauri::WebviewWindow) {
    use winapi::um::dwmapi::DwmSetWindowAttribute;
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;

    let Ok(hwnd) = win.hwnd() else { return };
    let preference = DWMWCP_ROUND;
    // SAFETY: hwnd is a live window handle and the attribute value is a u32.
    unsafe {
        DwmSetWindowAttribute(
            hwnd.0 as _,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn apply_rounded_corners(_: &tauri::WebviewWindow) {}

fn main_window<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri::WebviewWindow<R>, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())
}

#[tauri::command]
pub fn set_window_effects(app: AppHandle, enabled: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    if enabled {
        window
            .set_effects(EffectsBuilder::new().effect(Effect::Acrylic).build())
            .map_err(|e| e.to_string())?;
    } else {
        window
            .set_effects(None::<tauri::utils::config::WindowEffectsConfig>)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Windows-specific: custom window drag using raw Win32 API.
#[cfg(windows)]
#[tauri::command]
pub async fn start_window_drag(app: AppHandle) -> Result<(), String> {
    use winapi::um::winuser::{
        GetAsyncKeyState, GetCursorPos, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE,
        SWP_NOZORDER, VK_LBUTTON,
    };

    let mut cursor_pos: winapi::shared::windef::POINT = unsafe { std::mem::zeroed() };
    if unsafe { GetCursorPos(&mut cursor_pos as *mut _) } == 0 {
        return Err("GetCursorPos failed".to_string());
    }
    let start_cursor = (cursor_pos.x, cursor_pos.y);

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let start_win = (pos.x, pos.y);

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;

    let _ = window.set_effects(None::<tauri::utils::config::WindowEffectsConfig>);
    drop(window);

    let app_clone = app.clone();
    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
    let hwnd_raw = hwnd.0 as isize;

    std::thread::spawn(move || {
        loop {
            unsafe {
                if GetAsyncKeyState(VK_LBUTTON) as i16 >= 0 {
                    break;
                }

                let mut cur: winapi::shared::windef::POINT = std::mem::zeroed();
                if GetCursorPos(&mut cur as *mut _) == 0 {
                    break;
                }

                let dx = cur.x - start_cursor.0;
                let dy = cur.y - start_cursor.1;

                SetWindowPos(
                    hwnd_raw as *mut _,
                    std::ptr::null_mut(),
                    start_win.0 + dx as i32,
                    start_win.1 + dy as i32,
                    0,
                    0,
                    flags,
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }

        if let Some(win) = app_clone.get_webview_window("main") {
            let _ = restore_acrylic(&win);
        }
    });

    Ok(())
}

#[cfg(not(windows))]
#[tauri::command]
pub async fn start_window_drag(_app: AppHandle) -> Result<(), String> {
    Err("Window drag is only supported on Windows".to_string())
}

#[cfg(windows)]
fn restore_acrylic(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .set_effects(EffectsBuilder::new().effect(Effect::Acrylic).build())
        .map_err(|e| e.to_string())
}

/// A rounded rectangle in window-local logical pixels.
#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct BlurRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub radius: f64,
}

/// Sets the behind-window blur region (logical pixels, rounded rects); an empty
/// list removes it. Returns whether the compositor blur is in effect, which is
/// only the case on KDE Plasma under Wayland.
#[tauri::command]
pub fn set_window_blur(
    window: tauri::WebviewWindow,
    regions: Vec<BlurRect>,
) -> Result<bool, String> {
    #[cfg(target_os = "linux")]
    {
        let gtk_window = window.gtk_window().map_err(|e| e.to_string())?;
        crate::kwin_effects::set_window_blur(window.label(), &gtk_window, &regions)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, regions);
        Ok(false)
    }
}

/// Adds a native drop shadow with the given corner radius around the window,
/// or removes it (`None`). Returns whether the shadow is in effect, which is
/// only the case on KDE Plasma under Wayland.
#[tauri::command]
pub fn set_window_shadow(window: tauri::WebviewWindow, radius: Option<u32>) -> Result<bool, String> {
    #[cfg(target_os = "linux")]
    {
        let gtk_window = window.gtk_window().map_err(|e| e.to_string())?;
        crate::kwin_effects::set_window_shadow(window.label(), &gtk_window, radius)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, radius);
        Ok(false)
    }
}

pub const SETTINGS_WINDOW_LABEL: &str = "settings";

/// Opens the settings window, or focuses it when it is already open. It uses
/// the same frameless, transparent chrome as the main window.
///
/// Async on purpose: sync commands run on the main thread, and building a
/// webview window there deadlocks on Windows (tauri#4121).
#[tauri::command]
pub async fn open_settings_window(app: AppHandle, title: String) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        let _ = win.unminimize();
        win.show().map_err(|e| e.to_string())?;
        // Closing only hid the window; its page animates back in on this.
        let _ = win.emit_to(SETTINGS_WINDOW_LABEL, "settings-window-shown", ());
        return win.set_focus().map_err(|e| e.to_string());
    }

    let win = tauri::WebviewWindowBuilder::new(
        &app,
        SETTINGS_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html#/settings".into()),
    )
    .title(title)
    .inner_size(1000.0, 680.0)
    .min_inner_size(820.0, 560.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .effects(EffectsBuilder::new().effect(Effect::Acrylic).build())
    .center()
    .build()
    .map_err(|e| e.to_string())?;
    // AppKit calls must run on the main thread.
    let styled = win.clone();
    app.run_on_main_thread(move || {
        apply_macos_vibrancy(&styled);
        apply_rounded_corners(&styled);
    })
    .map_err(|e| e.to_string())?;

    // The settings page streams the spectrum while it is visible; a closed
    // window cannot turn that off itself.
    let handle = app.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            handle.state::<ServerState>().set_spectrum_streaming(false);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    let win = main_window(&app)?;
    let _ = win.unminimize();
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn minimize_main_window(app: AppHandle) -> Result<(), String> {
    // A real minimize keeps the window in the taskbar; hiding to the tray is
    // the close action's job.
    main_window(&app)?.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) -> Result<(), String> {
    let win = main_window(&app)?;
    win.hide().map_err(|e| e.to_string())?;
    Ok(())
}

/// Places the macOS traffic lights for the active header layout. No-op elsewhere.
#[tauri::command]
pub fn apply_macos_window_layout(app: AppHandle, mode: String) -> Result<(), String> {
    let mode = crate::macos_window::Mode::parse(&mode);
    let win = main_window(&app)?;
    crate::macos_window::apply(&win.as_ref().window(), mode)
}

/// Sets the macOS Dock badge label (`None` clears it). No-op elsewhere.
#[tauri::command]
pub fn set_dock_badge(app: AppHandle, label: Option<String>) -> Result<(), String> {
    let win = main_window(&app)?;
    crate::macos_dock::set_badge(&win.as_ref().window(), label)
}

/// Replaces the app-wide menu with the descriptor the frontend sent. No-op elsewhere.
#[tauri::command]
pub fn set_app_menu(app: AppHandle, menu: Vec<crate::menubar::MenuNode>) -> Result<(), String> {
    crate::menubar::apply(&app, &menu)
}
