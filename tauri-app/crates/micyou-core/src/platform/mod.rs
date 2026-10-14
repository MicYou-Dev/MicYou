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

pub mod accent;
#[cfg(target_os = "linux")]
pub mod alsa;
pub mod adb;
pub mod blackhole;
pub mod firewall;
#[cfg(target_os = "linux")]
pub mod pipewire;
pub mod resources;
pub mod terminal;
pub mod usbmux;
pub mod vbcable;
