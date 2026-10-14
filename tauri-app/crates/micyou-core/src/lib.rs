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

//! MicYou's desktop audio server, independent of any UI toolkit.
//!
//! Frontends construct a [`server::ServerState`] with their own
//! [`events::ServerEvents`] sink and [`host::HostIntegration`], then drive it
//! through [`server::start_server`] / [`server::stop_server`] and the
//! runtime controls in [`settings`].

pub mod about;
pub mod config;
pub mod discovery;
pub mod events;
pub mod host;
pub mod logging;
pub mod mode_lock;
pub mod modes;
pub mod platform;
pub mod plugins;
pub mod server;
pub mod settings;
pub mod stats;
pub mod themes;
pub mod transport;

pub use micyou_audio;
pub use micyou_plugin;
pub use micyou_protocol;
