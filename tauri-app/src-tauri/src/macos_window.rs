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

// Only the macOS build drives these types; elsewhere the commands are no-ops.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

/// Where the native window controls go: the close button's distance from the
/// window's left edge and how far its title bar container grows downwards.
///
/// The frontend reserves the matching room with the `--macos-titlebar-safe-area`
/// variables in `src/shared/assets/index.css`.
#[derive(Clone, Copy, Debug)]
pub struct LayoutMetrics {
    pub inset_x: f64,
    pub inset_y: f64,
}

pub const FULL: LayoutMetrics = LayoutMetrics { inset_x: 32.0, inset_y: 37.0 };
pub const POCKET: LayoutMetrics = LayoutMetrics { inset_x: 24.0, inset_y: 24.0 };

/// Which header layout the window currently shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Full,
    Pocket,
}

impl Mode {
    pub fn metrics(self) -> LayoutMetrics {
        match self {
            Mode::Full => FULL,
            Mode::Pocket => POCKET,
        }
    }

    /// Unknown values fall back to the full layout, but deserve a warning: they
    /// mean the frontend and the backend disagree about the layout names.
    pub fn parse(value: &str) -> Mode {
        match value {
            "full" => Mode::Full,
            "pocket" => Mode::Pocket,
            other => {
                log::warn!(
                    target: "window",
                    "unknown window layout \"{other}\", using the full one"
                );
                Mode::Full
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{LayoutMetrics, Mode};
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use std::sync::Mutex;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    const CLOSE_BUTTON: usize = 0;
    const MINIATURIZE_BUTTON: usize = 1;
    const ZOOM_BUTTON: usize = 2;

    /// AppKit snaps to physical pixels; anything under half a point is noise.
    const EPSILON: f64 = 0.5;

    /// Whether the buttons sit far enough apart to be re-anchored. Re-anchoring
    /// by a zero pitch would stack all three on the same spot.
    pub(super) fn spacing_is_usable(pitch: f64) -> bool {
        pitch > EPSILON
    }

    /// Layout the UI last asked for, replayed after AppKit relayouts.
    static CURRENT: Mutex<Option<Mode>> = Mutex::new(None);

    /// Records `mode` as the layout to keep, then schedules the placement. The
    /// placement itself runs on the main thread after this returns, so an `Err`
    /// only reports that it could not be scheduled; a placement that fails there
    /// is logged instead of returned.
    pub fn apply(window: &tauri::Window, mode: Mode) -> Result<(), String> {
        *CURRENT.lock().map_err(|e| e.to_string())? = Some(mode);
        run_on_main(window, mode.metrics())
    }

    /// Re-applies the last requested layout. Resizing the window makes AppKit
    /// restore the default title bar container and button origins, so the
    /// layout has to be written again afterwards.
    pub fn reapply(window: &tauri::Window) {
        let mode = CURRENT.lock().ok().and_then(|current| *current);
        if let Some(mode) = mode {
            let _ = run_on_main(window, mode.metrics());
        }
    }

    fn run_on_main(window: &tauri::Window, metrics: LayoutMetrics) -> Result<(), String> {
        // The NSWindow pointer is not `Send`, so re-acquire it on the main
        // thread. The window handle is cloned twice: one copy is borrowed to
        // schedule the work, the other is moved into the closure.
        let handle = window.clone();
        let inner = handle.clone();
        handle
            .run_on_main_thread(move || match inner.ns_window() {
                Ok(ptr) if !ptr.is_null() => {
                    if let Err(e) = unsafe { set_inset(ptr as *mut Object, metrics) } {
                        log::warn!(target: "window", "window control placement failed: {e}");
                    }
                }
                Ok(_) => log::warn!(target: "window", "window control placement: no NSWindow"),
                Err(e) => log::warn!(target: "window", "window control placement: {e}"),
            })
            .map_err(|e| e.to_string())
    }

    /// Moves the traffic lights by growing the title bar container and then
    /// re-anchoring each button horizontally. The windowing layer applies the
    /// same layout when a traffic light position is configured, but only while
    /// building the window; doing it here makes it available at runtime.
    unsafe fn set_inset(ns_window: *mut Object, metrics: LayoutMetrics) -> Result<(), String> {
        // Everything below crosses into AppKit through raw pointers; keep it in
        // one explicit unsafe block so the boundary is visible.
        unsafe {
            let close: *mut Object = msg_send![ns_window, standardWindowButton: CLOSE_BUTTON];
            let miniaturize: *mut Object = msg_send![ns_window, standardWindowButton: MINIATURIZE_BUTTON];
            let zoom: *mut Object = msg_send![ns_window, standardWindowButton: ZOOM_BUTTON];
            if close.is_null() || miniaturize.is_null() || zoom.is_null() {
                return Err("standard window buttons unavailable".to_string());
            }

            // close -> title bar container -> theme frame
            let close_superview: *mut Object = msg_send![close, superview];
            if close_superview.is_null() {
                return Err("title bar container unavailable".to_string());
            }
            let container: *mut Object = msg_send![close_superview, superview];
            if container.is_null() {
                return Err("title bar container unavailable".to_string());
            }

            let close_rect: CGRect = msg_send![close, frame];
            let miniaturize_rect: CGRect = msg_send![miniaturize, frame];
            let pitch = miniaturize_rect.origin.x - close_rect.origin.x;
            if !spacing_is_usable(pitch) {
                // The buttons are not laid out yet, or came back in an unexpected
                // order. Spacing them by a zero pitch would stack all three, and the
                // idempotency check below would then keep them stacked, so leave the
                // layout alone and let the next relayout try again.
                return Err(format!("unexpected traffic light spacing ({pitch})"));
            }

            let target_height = close_rect.size.height + metrics.inset_y;
            let window_rect: CGRect = msg_send![ns_window, frame];
            let target_container_y = window_rect.size.height - target_height;

            let container_rect: CGRect = msg_send![container, frame];
            let container_is_off = (container_rect.size.height - target_height).abs() > EPSILON
                || (container_rect.origin.y - target_container_y).abs() > EPSILON;
            if container_is_off {
                let mut rect = container_rect;
                rect.size.height = target_height;
                rect.origin.y = target_container_y;
                let _: () = msg_send![container, setFrame: rect];
            }

            for (index, button) in [close, miniaturize, zoom].into_iter().enumerate() {
                let rect: CGRect = msg_send![button, frame];
                let target_x = metrics.inset_x + index as f64 * pitch;
                if (rect.origin.x - target_x).abs() <= EPSILON {
                    continue;
                }
                let origin = CGPoint {
                    x: target_x,
                    y: rect.origin.y,
                };
                let _: () = msg_send![button, setFrameOrigin: origin];
            }

            Ok(())
        }
    }}

#[cfg(target_os = "macos")]
pub use imp::{apply, reapply};

/// Other platforms already place the controls correctly.
#[cfg(not(target_os = "macos"))]
pub fn apply(_window: &tauri::Window, _mode: Mode) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn reapply(_window: &tauri::Window) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Distance from the window's top edge to the centre of the controls.
    ///
    /// AppKit keeps each button's own origin fixed inside the title bar container,
    /// so growing the container is what moves the controls down. The `+ 2` is the
    /// button's fixed offset inside it: half of the 16px button height minus its
    /// constant 6px bottom origin. Only the tests need the centre; the placement
    /// itself works in container heights.
    const fn center_from_top(inset_y: f64) -> f64 {
        inset_y + 2.0
    }

    #[test]
    fn inset_centres_the_controls_on_the_header() {
        assert_eq!(center_from_top(FULL.inset_y), 39.0);
        assert_eq!(center_from_top(POCKET.inset_y), 26.0);
    }

    #[test]
    fn mode_parse_falls_back_to_full() {
        assert_eq!(Mode::parse("pocket"), Mode::Pocket);
        assert_eq!(Mode::parse("full"), Mode::Full);
        assert_eq!(Mode::parse(""), Mode::Full);
        assert_eq!(Mode::parse("POCKET"), Mode::Full);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unusable_button_spacing_is_rejected() {
        assert!(!super::imp::spacing_is_usable(0.0));
        assert!(!super::imp::spacing_is_usable(-20.0));
        assert!(super::imp::spacing_is_usable(20.0));
    }
}
