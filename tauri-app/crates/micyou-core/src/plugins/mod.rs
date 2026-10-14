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

//! Plugin host wiring: owns the plugin manager, the DSP node registry and the
//! cross-device message bus, shared by the audio thread (via
//! `DspProcessor::set_external_hook`), the TCP server (plugin message relay)
//! and the frontends.

mod api;
mod chain;
mod hotkeys;
pub mod install;
mod logs;
pub mod sound_player;
mod sync;

pub use chain::{reconcile_plugin_chain, PLUGIN_NODE_AFTER};
pub use logs::PluginLogs;
pub use sync::TcpPluginSyncAdapter;

use crate::host::SharedHost;
use crate::settings::Controls;
use api::PluginHostApi;
use hotkeys::HotkeyService;
use micyou_plugin::bus::{PluginBus, PluginMessage};
use micyou_plugin::host::HostApi;
use micyou_plugin::manifest::{PluginKind, RuntimeKind};
use micyou_plugin::{PluginError, PluginEvent, PluginResult, PluginRuntime};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Server state the host API reads to answer plugin queries.
#[derive(Clone)]
pub struct ServerHandles {
    pub network_stats: Arc<crate::stats::NetworkStats>,
    pub audio_output: Arc<crate::server::output::AudioOutputHandle>,
    pub active_connection: crate::transport::tcp::SharedActiveConnection,
    pub active_audio_session: crate::transport::udp::SharedActiveAudioSession,
    pub lifecycle: Arc<tokio::sync::Mutex<crate::server::lifecycle::ServerLifecycleState>>,
    #[cfg(feature = "web-server")]
    pub web_server: Arc<tokio::sync::Mutex<Option<crate::transport::web::WebServer>>>,
}

/// Panel id → icon, per plugin (set at runtime through the host API).
type PanelIcons = Mutex<HashMap<String, HashMap<String, String>>>;

pub struct PluginHost {
    pub manager: Arc<Mutex<micyou_plugin::PluginManager>>,
    pub dsp_registry: Arc<micyou_plugin::PluginDspRegistry>,
    pub sync: Arc<TcpPluginSyncAdapter>,
    pub bus: Arc<PluginBus>,
    pub logs: PluginLogs,
    sound: sound_player::SoundPlayer,
    hotkeys: HotkeyService,
    host: SharedHost,
    panel_icons: PanelIcons,
    /// Set once by [`crate::server::ServerState::new`].
    controls: OnceLock<Controls>,
    server: ServerHandles,
}

pub(crate) fn lock_err<T>(_: std::sync::PoisonError<T>) -> PluginError {
    PluginError::Runtime("plugin host lock poisoned".into())
}

impl PluginHost {
    pub fn new(server: ServerHandles, host: SharedHost) -> Arc<Self> {
        let config = crate::config::config_dir();
        let manager = Arc::new(Mutex::new(micyou_plugin::PluginManager::new(
            config.join("plugins"),
            config.join("plugin-state.json"),
        )));
        let sync = Arc::new(TcpPluginSyncAdapter::default());

        let manager_dispatch = manager.clone();
        // Routes bus messages to the addressed (or every) loaded plugin.
        let dispatcher = Arc::new(move |msg: &PluginMessage| -> PluginResult<()> {
                let targets: Vec<String> = {
                    let manager = manager_dispatch.lock().map_err(lock_err)?;
                    if msg.target.is_empty() {
                        manager.loaded_ids()
                    } else {
                        vec![msg.target.clone()]
                    }
                };
                for id in targets {
                    let Some(handle) = manager_dispatch.lock().map_err(lock_err)?.instance_handle(&id)?
                    else {
                        continue;
                    };
                    let Ok(mut instance) = handle.try_lock() else {
                        log::warn!("[plugins] skip message for busy instance {id}");
                        continue;
                    };
                    instance.handle_message(&msg.source, &msg.topic, &msg.payload)?;
                }
                Ok(())
            });

        let bus = Arc::new(PluginBus::new(sync.clone(), dispatcher));
        Arc::new(Self {
            manager,
            dsp_registry: Arc::new(micyou_plugin::PluginDspRegistry::new()),
            sync,
            hotkeys: HotkeyService::new(host.clone(), bus.clone()),
            bus,
            logs: PluginLogs::new(),
            sound: sound_player::SoundPlayer::new(server.audio_output.clone()),
            host,
            panel_icons: Mutex::new(HashMap::new()),
            controls: OnceLock::new(),
            server,
        })
    }

    pub(crate) fn attach_controls(&self, controls: Controls) {
        if self.controls.set(controls).is_err() {
            log::warn!("[plugins] runtime controls attached twice, keeping the first");
        }
    }

    fn controls(&self) -> PluginResult<&Controls> {
        self.controls
            .get()
            .ok_or_else(|| PluginError::Runtime("server controls are not attached".into()))
    }

    /// Scan the plugin directory and start every plugin marked enabled.
    pub fn load_saved_plugins(self: &Arc<Self>) {
        let report = match self.manager.lock().map_err(lock_err).and_then(|mut m| m.scan()) {
            Ok(report) => report,
            Err(e) => {
                log::warn!("[plugins] scan failed: {e}");
                return;
            }
        };
        for entry in report.discovered {
            if entry.state.is_enabled() {
                if let Err(e) = self.enable_plugin(&entry.manifest.id) {
                    log::warn!("[plugins] failed to start {}: {e}", entry.manifest.id);
                }
            }
        }
    }

    /// Deliver a UI action (panel button etc.) to a plugin.
    pub fn trigger(&self, plugin_id: &str, action: &str, payload: &[u8]) {
        let bytes = if payload.is_empty() {
            serde_json::json!({ "action": action }).to_string().into_bytes()
        } else {
            payload.to_vec()
        };
        let msg = PluginMessage::new("ui", plugin_id, &format!("ui:{action}"), bytes);
        self.bus.handle_incoming(&msg);
    }

    fn check_dependencies(&self, manifest: &micyou_plugin::PluginManifest) -> PluginResult<()> {
        let manager = self.manager.lock().map_err(lock_err)?;
        for dep in manifest.dependencies.iter().filter(|dep| !dep.optional) {
            let Some(entry) = manager.entry(&dep.id)? else {
                return Err(PluginError::Runtime(format!(
                    "dependency {} is not installed (required by {})",
                    dep.id, manifest.id
                )));
            };
            if !entry.state.is_enabled() {
                return Err(PluginError::Runtime(format!(
                    "dependency {} is disabled (enable it first, required by {})",
                    dep.id, manifest.id
                )));
            }
            if dep.version.is_empty() {
                continue;
            }
            let req = semver::VersionReq::parse(&dep.version)
                .map_err(|e| PluginError::Runtime(format!("invalid version req: {e}")))?;
            let installed = semver::Version::parse(&entry.manifest.version)
                .map_err(|e| PluginError::Runtime(format!("dep version parse: {e}")))?;
            if !req.matches(&installed) {
                return Err(PluginError::Runtime(format!(
                    "dependency {} version {} does not satisfy {} (required by {})",
                    dep.id, entry.manifest.version, dep.version, manifest.id
                )));
            }
        }
        Ok(())
    }

    pub fn enable_plugin(self: &Arc<Self>, id: &str) -> PluginResult<()> {
        let entry = {
            let manager = self.manager.lock().map_err(lock_err)?;
            if manager.is_loaded(id) {
                return Ok(());
            }
            manager
                .entry(id)?
                .ok_or_else(|| PluginError::UnknownPlugin(id.to_string()))?
        };
        self.check_dependencies(&entry.manifest)?;

        let host_api: Arc<dyn HostApi> =
            Arc::new(PluginHostApi::new(self.clone(), id.to_string(), entry.dir.clone()));
        let mut instance = match entry.manifest.runtime {
            RuntimeKind::Native => micyou_plugin::native::load_native_instance(
                entry.manifest.clone(),
                &entry.dir,
                host_api.clone(),
            )?,
            RuntimeKind::Wasm => micyou_plugin::wasm::load_wasm_instance(
                entry.manifest.clone(),
                &entry.dir,
                host_api.clone(),
            )?,
        };
        instance.init(&*host_api)?;

        let dsp_handle = {
            let mut manager = self.manager.lock().map_err(lock_err)?;
            manager.set_enabled(id, true)?;
            manager.register_instance(instance)?;
            manager.instance_handle(id)?
        };

        if entry.manifest.kind == PluginKind::Dsp {
            let dsp = entry.manifest.dsp.clone().unwrap_or_default();
            let handle = dsp_handle.ok_or_else(|| PluginError::NotLoaded(id.to_string()))?;
            self.dsp_registry.register(micyou_plugin::DspNode::new(
                id.to_string(),
                dsp.first,
                dsp.insert_after.clone(),
                handle,
            ))?;
        }
        log::info!("[plugins] enabled {id}");
        Ok(())
    }

    pub fn disable_plugin(&self, id: &str) -> PluginResult<()> {
        self.dsp_registry.unregister(id)?;
        let mut manager = self.manager.lock().map_err(lock_err)?;
        manager.unregister_instance(id)?;
        manager.set_enabled(id, false)?;
        log::info!("[plugins] disabled {id}");
        Ok(())
    }

    pub fn broadcast_event(&self, event: &PluginEvent) {
        let handles = match self.manager.lock() {
            Ok(manager) => manager
                .loaded_ids()
                .into_iter()
                .filter_map(|id| manager.instance_handle(&id).ok().flatten())
                .collect::<Vec<_>>(),
            Err(_) => {
                log::error!("[plugins] manager lock poisoned, event {event:?} dropped");
                return;
            }
        };
        for handle in handles {
            // A plugin busy in its own callback skips this event rather than
            // deadlocking the caller.
            if let Ok(mut instance) = handle.try_lock() {
                if let Err(e) = instance.handle_event(event) {
                    log::warn!("[plugins] event handler failed: {e}");
                }
            }
        }
    }

    pub fn uninstall_plugin(&self, id: &str) -> PluginResult<()> {
        self.dsp_registry.unregister(id)?;
        self.manager.lock().map_err(lock_err)?.uninstall(id)?;
        log::info!("[plugins] uninstalled {id}");
        Ok(())
    }

    /// Synchronize the runtime processing chain with the DSP plugin registry
    /// (see [`reconcile_plugin_chain`]). Runtime-only; persistence happens
    /// through the normal settings save paths.
    pub fn ensure_plugin_chain_node(
        &self,
        dsp_settings: &RwLock<micyou_audio::dsp::AudioDspSettings>,
    ) {
        let ids = self.dsp_registry.plugin_ids();
        let mut settings = dsp_settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        reconcile_plugin_chain(&mut settings.processing_chain, &ids);
    }

    /// Reconcile the chain inside a settings value about to be applied, so a
    /// full-settings write can neither drop active nor keep stale nodes.
    pub fn reconcile_settings_chain(&self, settings: &mut micyou_audio::dsp::AudioDspSettings) {
        let ids = self.dsp_registry.plugin_ids();
        reconcile_plugin_chain(&mut settings.processing_chain, &ids);
    }

    pub fn dsp_hook(&self) -> micyou_audio::dsp::ExternalDspHook {
        micyou_plugin::PluginDspBridge::new(self.dsp_registry.clone()).hook()
    }

    pub fn panel_icons(&self, plugin_id: &str) -> HashMap<String, String> {
        self.panel_icons
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(plugin_id)
            .cloned()
            .unwrap_or_default()
    }
}
