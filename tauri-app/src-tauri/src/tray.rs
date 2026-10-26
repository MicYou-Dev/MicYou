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

use serde::Deserialize;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuStrings {
    pub tooltip: String,
    pub show: String,
    pub hide: String,
    pub start: String,
    pub stop: String,
    pub exit: String,
    #[serde(default)]
    pub switch_cli: String,
    #[serde(default)]
    pub switch_tui: String,
}

impl TrayMenuStrings {
    pub fn english_defaults() -> Self {
        Self {
            tooltip: "MicYou Desktop".to_string(),
            show: "Show App".to_string(),
            hide: "Hide App".to_string(),
            start: "Start Streaming".to_string(),
            stop: "Stop Streaming".to_string(),
            exit: "Exit".to_string(),
            switch_cli: "Switch to CLI Mode".to_string(),
            switch_tui: "Switch to TUI Mode".to_string(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrayState {
    pub window_visible: bool,
    pub is_streaming: bool,
}

/// Returns the localized label for the show/hide menu item given the current
/// window visibility. The click id stays the same regardless of label.
pub fn show_hide_label(state: TrayState, strings: &TrayMenuStrings) -> &str {
    if state.window_visible {
        &strings.hide
    } else {
        &strings.show
    }
}

/// Returns the localized label for the streaming toggle menu item.
pub fn stream_toggle_label(state: TrayState, strings: &TrayMenuStrings) -> &str {
    if state.is_streaming {
        &strings.stop
    } else {
        &strings.start
    }
}

pub const MENU_ID_SHOW: &str = "show";
pub const MENU_ID_TOGGLE_STREAM: &str = "toggle_stream";
pub const MENU_ID_EXIT: &str = "exit";
pub const MENU_ID_SWITCH_CLI: &str = "switch_cli";
pub const MENU_ID_SWITCH_TUI: &str = "switch_tui";

pub struct TrayContext {
    pub strings: Mutex<TrayMenuStrings>,
    pub state: Mutex<TrayState>,
}

impl Default for TrayContext {
    fn default() -> Self {
        Self {
            strings: Mutex::new(TrayMenuStrings::english_defaults()),
            state: Mutex::new(TrayState::default()),
        }
    }
}

const TRAY_ID: &str = "micyou-main-tray";

pub fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let ctx = app.state::<TrayContext>();
    let strings = ctx.strings.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let state = *ctx.state.lock().unwrap_or_else(|p| p.into_inner());

    let menu = build_menu(app, &strings, state)?;

    // macOS wants a monochrome template image in the menu bar so the icon takes on the
    // bar's tint, while the other platforms keep the coloured application icon.
    #[cfg(target_os = "macos")]
    let (icon, icon_is_template) = (
        tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon-template.png"))?,
        true,
    );
    #[cfg(not(target_os = "macos"))]
    let (icon, icon_is_template) = (
        app.default_window_icon()
            .cloned()
            .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?,
        false,
    );

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(icon_is_template)
        .tooltip(&strings.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            log::info!(target: "tray", "menu event: {id}");
            match id {
                MENU_ID_SHOW
                | MENU_ID_TOGGLE_STREAM
                | MENU_ID_EXIT
                | MENU_ID_SWITCH_CLI
                | MENU_ID_SWITCH_TUI => {
                    if let Err(e) = app.emit("tray-action", id) {
                        log::error!(target: "tray", "failed to forward tray action {id}: {e}");
                    }
                }
                // The app menu bar shares this handler (Tauri installs one global
                // menu listener), so its `menu:` ids land here too.
                other => log::debug!(target: "tray", "unknown menu id: {other}"),
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                if let Err(e) = tray.app_handle().emit("tray-action", MENU_ID_SHOW) {
                    log::error!(target: "tray", "failed to forward tray double click: {e}");
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn rebuild_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let ctx = app.state::<TrayContext>();
    let strings = ctx.strings.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let state = *ctx.state.lock().unwrap_or_else(|p| p.into_inner());
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    tray.set_menu(Some(build_menu(app, &strings, state)?))?;
    tray.set_tooltip(Some(&strings.tooltip))
}

#[tauri::command]
pub fn set_tray_strings(app: AppHandle, strings: TrayMenuStrings) -> Result<(), String> {
    *app.state::<TrayContext>()
        .strings
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = strings;
    rebuild_menu(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_tray_state(app: AppHandle, state: TrayState) -> Result<(), String> {
    *app.state::<TrayContext>()
        .state
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = state;
    rebuild_menu(&app).map_err(|e| e.to_string())
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    strings: &TrayMenuStrings,
    state: TrayState,
) -> tauri::Result<Menu<R>> {
    let show_hide = MenuItem::with_id(
        app,
        MENU_ID_SHOW,
        show_hide_label(state, strings),
        true,
        None::<&str>,
    )?;
    let toggle_stream = MenuItem::with_id(
        app,
        MENU_ID_TOGGLE_STREAM,
        stream_toggle_label(state, strings),
        true,
        None::<&str>,
    )?;
    let switch_cli = MenuItem::with_id(
        app,
        MENU_ID_SWITCH_CLI,
        &strings.switch_cli,
        true,
        None::<&str>,
    )?;
    let switch_tui = MenuItem::with_id(
        app,
        MENU_ID_SWITCH_TUI,
        &strings.switch_tui,
        true,
        None::<&str>,
    )?;
    let exit = MenuItem::with_id(app, MENU_ID_EXIT, &strings.exit, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    Menu::with_items(
        app,
        &[
            &show_hide,
            &toggle_stream,
            &separator,
            &switch_cli,
            &switch_tui,
            &separator,
            &exit,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> TrayMenuStrings {
        TrayMenuStrings {
            tooltip: "T".into(),
            show: "Show".into(),
            hide: "Hide".into(),
            start: "Start".into(),
            stop: "Stop".into(),
            exit: "Exit".into(),
            switch_cli: "Switch".into(),
            switch_tui: "Switch TUI".into(),
        }
    }

    #[test]
    fn show_hide_label_uses_hide_when_visible() {
        assert_eq!(
            show_hide_label(
                TrayState {
                    window_visible: true,
                    is_streaming: false
                },
                &s()
            ),
            "Hide"
        );
    }

    #[test]
    fn show_hide_label_uses_show_when_hidden() {
        assert_eq!(
            show_hide_label(
                TrayState {
                    window_visible: false,
                    is_streaming: false
                },
                &s()
            ),
            "Show"
        );
    }

    #[test]
    fn stream_toggle_label_uses_stop_when_streaming() {
        assert_eq!(
            stream_toggle_label(
                TrayState {
                    window_visible: true,
                    is_streaming: true
                },
                &s()
            ),
            "Stop"
        );
    }

    #[test]
    fn stream_toggle_label_uses_start_when_idle() {
        assert_eq!(
            stream_toggle_label(
                TrayState {
                    window_visible: true,
                    is_streaming: false
                },
                &s()
            ),
            "Start"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_menu_bar_icon_is_a_square_retina_template() {
        // The tray layer gives every tray image an 18pt height and keeps the aspect
        // ratio, so a non-square asset would render distorted, and the pixel count is
        // the only thing standing in for Retina detail (18pt needs 36px at 2x).
        let icon = tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray-icon-template.png"
        ))
        .expect("the committed menu bar icon must decode");
        assert_eq!(icon.width(), icon.height());
        assert!(icon.width() >= 36, "got {}px", icon.width());
    }

    #[test]
    fn english_defaults_are_non_empty() {
        let d = TrayMenuStrings::english_defaults();
        assert!(!d.tooltip.is_empty());
        assert!(!d.show.is_empty() && !d.hide.is_empty());
        assert!(!d.start.is_empty() && !d.stop.is_empty());
        assert!(!d.exit.is_empty());
        assert!(!d.switch_cli.is_empty());
        assert!(!d.switch_tui.is_empty());
    }

    #[test]
    fn tray_switch_labels_deserialize_from_frontend_camel_case() {
        let strings: TrayMenuStrings = serde_json::from_value(serde_json::json!({
            "tooltip": "tooltip",
            "show": "show",
            "hide": "hide",
            "start": "start",
            "stop": "stop",
            "exit": "exit",
            "switchCli": "CLI label",
            "switchTui": "TUI label"
        }))
        .unwrap();
        assert_eq!(strings.switch_cli, "CLI label");
        assert_eq!(strings.switch_tui, "TUI label");
    }
}
