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

use crate::commands;
use crate::events::TauriEventSink;
use crate::host::TauriHost;
use crate::tray::{self, TrayContext};
use crate::window;
use micyou_core::mode_lock::{self, RunMode};
use micyou_core::server::{service, ServerState};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::Emitter;
use tauri::Manager;

/// App menu items the frontend owns carry a prefix, because the tray registers
/// its own menu handler into the very same global menu event listeners: a bare
/// tray id such as `show` must never be mistaken for an app menu item.
#[cfg(target_os = "macos")]
const APP_MENU_ID_PREFIX: &str = "menu:";

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    app.manage(TrayContext::default());
    if let Err(e) = tray::build_tray(&handle) {
        log::warn!(target: "tray", "failed to build tray: {e}");
    }

    // The app menu only exists on macOS, and clicks on it have to reach the
    // frontend, which owns the labels and the behaviour. Only the ids the
    // frontend declared are forwarded; everything else (the tray's own bare
    // ids) is dropped here.
    #[cfg(target_os = "macos")]
    app.on_menu_event(|app, event| {
        let id = event.id().as_ref();
        if id.starts_with(APP_MENU_ID_PREFIX) {
            let _ = app.emit("app-menu-action", id.to_string());
        } else {
            // The tray shares this handler (Tauri installs one global menu
            // listener), so its own ids land here on every click.
            log::debug!(target: "menu", "ignoring foreign menu id: {id}");
        }
    });

    let state = ServerState::new(
        Arc::new(TauriEventSink(handle.clone())),
        Arc::new(TauriHost(handle.clone())),
        handle.path().resource_dir().ok(),
    );
    state.plugins.load_saved_plugins();
    app.manage(state);

    match mode_lock::acquire(RunMode::Gui) {
        Ok(()) => log::info!(target: "mode", "GUI mode lock acquired"),
        Err(e) => log::warn!(target: "mode", "GUI mode lock not acquired: {e}"),
    }

    if let Some(win) = app.get_webview_window("main") {
        window::apply_macos_vibrancy(&win);
        window::apply_rounded_corners(&win);
        // The window config leaves the control position unset so AppKit
        // cannot override the per-layout placement, which means the
        // initial layout has to be requested here.
        let main = win.as_ref().window();
        if let Err(e) = crate::macos_window::apply(&main, crate::macos_window::Mode::Full) {
            log::warn!(target: "window", "initial window control layout: {e}");
        }
    }

    // Opening the device can block on PipeWire setup; keep it off the UI thread.
    std::thread::spawn(move || {
        if service::open_output_device(&handle.state::<ServerState>()) {
            log::info!("[Audio] Virtual device ready at app startup");
        }
    });
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            let is_main_window = window.label() == "main";

            // This listener fires for every window the app creates, but only the
            // main window carries the in-app header these controls are aligned to,
            // so other windows must keep the placement AppKit gives them.
            if is_main_window
                && matches!(
                    event,
                    tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(_)
                )
            {
                crate::macos_window::reapply(window);
            }

            // The native close button has to keep the existing hide-to-tray /
            // confirm behaviour, which lives in the frontend. Other windows are
            // not intercepted: they have no handler for the forwarded request
            // and would be left unable to close.
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if is_main_window {
                    api.prevent_close();
                    let _ = window.emit("main-window-close-requested", ());
                }
            }
        })
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            window::set_window_effects,
            window::start_window_drag,
            window::set_window_blur,
            window::set_window_shadow,
            window::open_settings_window,
            window::show_main_window,
            window::minimize_main_window,
            window::hide_main_window,
            window::apply_macos_window_layout,
            window::set_dock_badge,
            window::set_app_menu,
            tray::set_tray_strings,
            tray::set_tray_state,
            commands::server::start_server,
            commands::server::stop_server,
            commands::server::get_streaming_status,
            commands::server::set_mute_state,
            commands::server::set_monitoring,
            commands::server::set_spectrum_streaming,
            commands::server::exit_app,
            commands::audio::get_audio_devices,
            commands::audio::update_audio_settings,
            commands::audio::get_audio_settings,
            commands::audio::get_aec_status,
            commands::audio::get_server_prefs,
            commands::audio::save_server_prefs,
            commands::audio::check_pipewire,
            commands::audio::check_vbcable,
            commands::audio::install_vbcable,
            commands::audio::check_blackhole,
            commands::audio::set_blackhole_as_input,
            commands::audio::restore_input_device,
            commands::network::enable_usb_mode,
            commands::network::list_adb_devices,
            commands::network::get_network_info,
            commands::network::get_network_interfaces,
            commands::network::get_web_status,
            commands::network::allow_firewall,
            commands::mode::get_mode_status,
            commands::mode::release_gui_lock,
            commands::mode::switch_to_cli,
            commands::mode::switch_to_tui,
            commands::mode::save_ui_prefs,
            commands::mode::save_theme_colors,
            commands::mode::get_theme_colors,
            commands::theme::get_system_accent_color,
            commands::theme::install_theme,
            commands::theme::list_installed_themes,
            commands::theme::get_installed_theme,
            commands::theme::remove_installed_theme,
            commands::about::get_app_version,
            commands::about::check_app_update,
            commands::about::get_sponsors,
            commands::about::export_log,
            commands::about::get_log_path,
            commands::about::get_log_content,
            commands::about::open_log_dir,
            commands::plugins::list_plugins,
            commands::plugins::set_plugin_enabled,
            commands::plugins::uninstall_plugin,
            commands::plugins::get_plugin_config,
            commands::plugins::set_plugin_config,
            commands::plugins::get_plugin_logs,
            commands::plugins::get_plugin_sync_status,
            commands::plugins::open_plugins_dir,
            commands::plugins::preview_plugin_zip,
            commands::plugins::preview_plugin_from_url,
            commands::plugins::install_plugin_from_url,
            commands::plugins::cancel_plugin_download,
            commands::plugins::check_plugin_updates,
            commands::plugins::get_plugin_panel_icons,
            commands::plugins::get_app_locale,
            commands::plugins::update_plugin,
            commands::plugins::import_plugin,
            commands::plugins::plugin_trigger,
            commands::plugins::get_plugin_panel,
            commands::plugins::open_plugin_window,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<ServerState>() {
                    service::close_output_device(&state);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    // Every InfoPlist.strings key has to match a usage description in Info.plist: a typo
    // or a missing locale makes macOS fall back to the English text without any warning,
    // which stays invisible until a localized system shows the prompt.
    #[test]
    fn macos_permission_prompts_are_localized_for_every_usage_description() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let plist = std::fs::read_to_string(manifest.join("Info.plist")).expect("Info.plist");
        let mut expected = plist
            .match_indices("<key>NS")
            .filter_map(|(start, _)| plist[start + 5..].split("</key>").next())
            .filter(|key| key.ends_with("UsageDescription"))
            .map(str::to_string)
            .collect::<Vec<_>>();
        expected.sort();
        assert!(!expected.is_empty(), "no usage descriptions found in Info.plist");

        let mut locales = 0;
        for entry in std::fs::read_dir(manifest.join("packaging/InfoPlist")).expect("locales") {
            let strings_path = entry.expect("locale entry").path().join("InfoPlist.strings");
            if !strings_path.is_file() {
                continue;
            }
            locales += 1;

            let strings = std::fs::read_to_string(&strings_path).expect("InfoPlist.strings");
            let mut found = Vec::new();
            for line in strings.lines().filter(|line| line.starts_with('"')) {
                let (key, value) = line.split_once(" = ").expect("a .strings entry needs ' = '");
                let value = value
                    .trim_start_matches('"')
                    .trim_end_matches(';')
                    .trim_end_matches('"');
                assert!(!value.is_empty(), "empty value in {}", strings_path.display());
                found.push(key.trim_matches('"').to_string());
            }
            found.sort();
            assert_eq!(
                found,
                expected,
                "{} does not match Info.plist",
                strings_path.display()
            );
        }
        assert_eq!(locales, 3, "expected one .lproj per localized language");
    }
}
