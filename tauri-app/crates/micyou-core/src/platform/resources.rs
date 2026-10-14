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

//! Lookup of files bundled with the desktop app (ALSA/PipeWire config, model
//! licenses) across dev trees, installed packages and AppImages.

use std::path::{Path, PathBuf};

/// The GUI crate owns `resources/`; `cargo run` falls back to it.
const DEV_APP_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri");

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Locate the directory containing MicYou's bundled runtime resources. Linux
/// packages use /usr/bin + /usr/lib/micyou/resources, AppImage exposes the
/// same tree below its mount point. `hint` is the frontend's own idea of the
/// resource directory, if it has one.
pub fn find_resource_dir(hint: Option<&Path>) -> Option<PathBuf> {
    // The ALSA/PipeWire config is the canonical marker; the legacy .onnx
    // markers keep pre-port install trees (which still ship the model files)
    // resolvable after an in-place upgrade.
    const MARKERS: [&str; 3] = [
        "alsa/micyou-pipewire.conf",
        "purevox6.onnx",
        "aec7_ep0185.onnx",
    ];

    let mut candidates = Vec::new();
    if let Some(dir) = hint {
        candidates.push(dir.to_path_buf());
        candidates.push(dir.join("resources"));
    }
    if let Some(exe_dir) = exe_dir() {
        candidates.push(exe_dir.join("resources"));
        if let Some(prefix) = exe_dir.parent() {
            candidates.push(prefix.join("lib").join("micyou").join("resources"));
        }
        candidates.push(exe_dir);
    }
    candidates.push(Path::new(DEV_APP_DIR).join("resources"));

    candidates
        .into_iter()
        .find(|directory| MARKERS.iter().any(|model| directory.join(model).exists()))
}
