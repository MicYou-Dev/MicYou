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

//! PluginManager: discovery, state registry and lifecycle coordination.
//!
//! Layout on disk (host-provided roots, reused by CLI/TUI and later Android):
//! ```text
//! plugins_dir/
//!   <plugin.id>/plugin.json        # manifest (required)
//!   <plugin.id>/<entry artifact>   # cdylib or wasm module
//!   <plugin.id>/assets/            # optional private assets
//! state_path                       # JSON: { "<id>": { enabled, config } }
//! ```
//!
//! The manager owns *registry* state (what is installed, enabled, configured)
//! and the *load* state (native/wasm instances). Loading is pluggable: native
//! and WASM loaders are filled in by `native.rs` / `wasm.rs`.

use crate::error::{PluginError, PluginResult};
use crate::manifest::PluginManifest;
use crate::plugin::{PluginInstance, PluginRuntime, PluginState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Per-plugin persisted state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPersistedState {
    #[serde(default)]
    pub enabled: bool,
    /// Plugin-scoped configuration (defaults merged on first write).
    #[serde(default)]
    pub config: serde_json::Map<String, serde_json::Value>,
}

impl Default for PluginPersistedState {
    fn default() -> Self {
        Self {
            enabled: false,
            config: serde_json::Map::new(),
        }
    }
}

/// What the manager knows about a discovered plugin (no runtime instance yet).
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub manifest: PluginManifest,
    pub state: PluginState,
    pub dir: PathBuf,
}

/// Result of a directory scan.
#[derive(Debug, Default)]
pub struct ScanReport {
    pub discovered: Vec<PluginEntry>,
    /// (dir, reason) for directories that failed to load a manifest.
    pub skipped: Vec<(PathBuf, String)>,
}

/// Manages installed plugins: scanning, state persistence, and (de)registration
/// of loaded runtime instances.
///
/// Not `Sync`-heavy by design: mutation goes through an internal lock; each
/// plugin instance lives in its own slot keyed by id.
pub struct PluginManager {
    plugins_dir: PathBuf,
    state_path: PathBuf,
    entries: RwLock<HashMap<String, PluginEntry>>,
    /// Loaded runtime instances (native/wasm), filled by the loaders.
    /// Shared as `Arc<Mutex<..>>` so the DSP registry and the message-bus
    /// dispatcher can hold their own handle without duplicating the instance.
    /// `Mutex` (not `RwLock`) because `PluginInstance` is only `Send`.
    instances: Mutex<HashMap<String, Arc<Mutex<PluginInstance>>>>,
}

impl PluginManager {
    pub fn new(plugins_dir: PathBuf, state_path: PathBuf) -> Self {
        Self {
            plugins_dir,
            state_path,
            entries: RwLock::new(HashMap::new()),
            instances: Mutex::new(HashMap::new()),
        }
    }

    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    // ── Discovery ─────────────────────────────────────────────────────────

    /// Scan `plugins_dir` for plugin directories and load their manifests.
    /// Does not load any runtime artifact.
    pub fn scan(&mut self) -> PluginResult<ScanReport> {
        let mut report = ScanReport::default();
        if !self.plugins_dir.exists() {
            return Ok(report);
        }
        let mut entries = self.entries.write().map_err(|_| poisoned())?;
        entries.clear();

        let persisted = self.load_persisted_state();
        let read_dir = std::fs::read_dir(&self.plugins_dir)
            .map_err(|e| PluginError::Io(format!("read plugins dir: {e}")))?;

        for item in read_dir.flatten() {
            let path = item.path();
            if !path.is_dir() {
                continue;
            }
            match PluginManifest::load_from_dir(&path) {
                Ok(manifest) => {
                    let id = manifest.id.clone();
                    let enabled = persisted.get(&id).map(|s| s.enabled).unwrap_or(false);
                    entries.insert(
                        id.clone(),
                        PluginEntry {
                            manifest,
                            state: if enabled {
                                PluginState::Enabled
                            } else {
                                PluginState::Disabled
                            },
                            dir: path,
                        },
                    );
                    report.discovered.push(PluginEntry {
                        manifest: entries[&id].manifest.clone(),
                        state: entries[&id].state,
                        dir: entries[&id].dir.clone(),
                    });
                }
                Err(e) => report.skipped.push((path, e.to_string())),
            }
        }
        Ok(report)
    }

    /// Register a plugin that was placed on disk after startup.
    pub fn discover_plugin(&mut self, dir: PathBuf) -> PluginResult<String> {
        let manifest = PluginManifest::load_from_dir(&dir)?;
        if self.entry(&manifest.id)?.is_some() {
            return Err(PluginError::AlreadyExists(manifest.id));
        }
        let persisted = self.load_persisted_state();
        let enabled = persisted
            .get(&manifest.id)
            .map(|s| s.enabled)
            .unwrap_or(false);
        let id = manifest.id.clone();
        self.entries.write().map_err(|_| poisoned())?.insert(
            id.clone(),
            PluginEntry {
                manifest,
                state: if enabled {
                    PluginState::Enabled
                } else {
                    PluginState::Disabled
                },
                dir,
            },
        );
        Ok(id)
    }

    /// Remove a plugin from the registry (and drop any loaded instance).
    pub fn remove_plugin(&mut self, id: &str) -> PluginResult<()> {
        let mut instances = self.instances.lock().map_err(|_| poisoned())?;
        if let Some(handle) = instances.remove(id) {
            if let Ok(mut instance) = handle.lock() {
                instance.deinit();
            }
        }
        self.entries.write().map_err(|_| poisoned())?.remove(id);
        self.persist_state()
    }

    /// Remove a plugin directory from disk entirely (uninstall).
    pub fn uninstall(&mut self, id: &str) -> PluginResult<()> {
        let dir = self
            .entry(id)?
            .map(|e| e.dir.clone())
            .ok_or_else(|| PluginError::UnknownPlugin(id.to_string()))?;
        self.remove_plugin(id)?;
        std::fs::remove_dir_all(&dir)
            .map_err(|e| PluginError::Io(format!("remove {}: {e}", dir.display())))
    }

    // ── Registry queries ───────────────────────────────────────────────────

    pub fn entries(&self) -> Vec<PluginEntry> {
        self.entries
            .read()
            .map_err(|_| poisoned())
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Look up a registered entry by id.
    pub fn entry(&self, id: &str) -> PluginResult<Option<PluginEntry>> {
        Ok(self
            .entries
            .read()
            .map_err(|_| poisoned())?
            .get(id)
            .cloned())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> PluginResult<()> {
        let mut entries = self.entries.write().map_err(|_| poisoned())?;
        let entry = entries
            .get_mut(id)
            .ok_or_else(|| PluginError::UnknownPlugin(id.to_string()))?;
        entry.state = if enabled {
            PluginState::Enabled
        } else {
            PluginState::Disabled
        };
        drop(entries);
        self.persist_state()
    }

    pub fn is_enabled(&self, id: &str) -> PluginResult<bool> {
        self.entry(id)?
            .map(|e| e.state.is_enabled())
            .ok_or_else(|| PluginError::UnknownPlugin(id.to_string()))
    }

    // ── Loaded instances ───────────────────────────────────────────────────

    /// Register a loaded runtime instance (called by the loaders).
    pub fn register_instance(&self, instance: PluginInstance) -> PluginResult<()> {
        let id = instance.id().to_string();
        if !self.is_enabled(&id)? {
            return Err(PluginError::NotLoaded(format!(
                "{id} is disabled; enable it before loading"
            )));
        }
        let mut instances = self.instances.lock().map_err(|_| poisoned())?;
        if instances.contains_key(&id) {
            return Err(PluginError::AlreadyExists(format!(
                "instance for {id} already loaded"
            )));
        }
        instances.insert(id, Arc::new(Mutex::new(instance)));
        Ok(())
    }

    pub fn unregister_instance(&self, id: &str) -> PluginResult<()> {
        let mut instances = self.instances.lock().map_err(|_| poisoned())?;
        if let Some(handle) = instances.remove(id) {
            if let Ok(mut instance) = handle.lock() {
                instance.deinit();
            }
        }
        Ok(())
    }

    /// Get a shared handle to a loaded instance (for the DSP registry and the
    /// message dispatcher). The caller locks it for each call.
    pub fn instance_handle(&self, id: &str) -> PluginResult<Option<Arc<Mutex<PluginInstance>>>> {
        Ok(self
            .instances
            .lock()
            .map_err(|_| poisoned())?
            .get(id)
            .cloned())
    }

    /// Run a closure against a loaded instance's runtime (shared handle).
    pub fn with_instance<T>(
        &self,
        id: &str,
        f: impl FnOnce(&mut PluginInstance) -> PluginResult<T>,
    ) -> PluginResult<T> {
        let handle = self
            .instance_handle(id)?
            .ok_or_else(|| PluginError::NotLoaded(id.to_string()))?;
        let mut instance = handle
            .lock()
            .map_err(|_| PluginError::Runtime(format!("instance {id} poisoned")))?;
        f(&mut instance)
    }

    pub fn loaded_ids(&self) -> Vec<String> {
        self.instances
            .lock()
            .map_err(|_| poisoned())
            .map(|g| g.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn is_loaded(&self, id: &str) -> bool {
        self.instances
            .lock()
            .map(|g| g.contains_key(id))
            .unwrap_or(false)
    }

    // ── Persistence ────────────────────────────────────────────────────────

    fn load_persisted_state(&self) -> HashMap<String, PluginPersistedState> {
        std::fs::read_to_string(&self.state_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn persist_state(&self) -> PluginResult<()> {
        // Preserve per-plugin config; only the enabled flag comes from the
        // in-memory registry.
        let mut state = self.load_persisted_state();
        for e in self.entries() {
            state.entry(e.manifest.id).or_default().enabled = e.state.is_enabled();
        }
        let ids: std::collections::HashSet<String> =
            self.entries().into_iter().map(|e| e.manifest.id).collect();
        state.retain(|id, _| ids.contains(id));
        self.save_state(state)
    }

    fn save_state(&self, state: HashMap<String, PluginPersistedState>) -> PluginResult<()> {
        if let Some(parent) = self.state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json =
            serde_json::to_string_pretty(&state).map_err(|e| PluginError::Io(e.to_string()))?;
        std::fs::write(&self.state_path, json).map_err(|e| PluginError::Io(e.to_string()))
    }

    // ── Plugin-scoped config ───────────────────────────────────────────────

    /// Read the persisted config map for a plugin.
    pub fn plugin_config(
        &self,
        id: &str,
    ) -> PluginResult<serde_json::Map<String, serde_json::Value>> {
        if self.entry(id)?.is_none() {
            return Err(PluginError::UnknownPlugin(id.to_string()));
        }
        Ok(self
            .load_persisted_state()
            .get(id)
            .map(|s| s.config.clone())
            .unwrap_or_default())
    }

    /// Write one config value for a plugin (persisted).
    pub fn set_plugin_config(
        &self,
        id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> PluginResult<()> {
        if self.entry(id)?.is_none() {
            return Err(PluginError::UnknownPlugin(id.to_string()));
        }
        let mut state = self.load_persisted_state();
        let entry = state.entry(id.to_string()).or_default();
        entry.config.insert(key.to_string(), value);
        self.save_state(state)
    }
}

fn poisoned() -> PluginError {
    PluginError::Runtime("plugin manager lock poisoned".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::HostApi;
    use crate::manifest::{RuntimeKind, MANIFEST_FILE_NAME};
    use std::fs;

    fn write_manifest(dir: &Path, id: &str, runtime: RuntimeKind, entry: &str) {
        fs::create_dir_all(dir).unwrap();
        let json = format!(
            r#"{{
                "id": "{id}",
                "name": "Test {id}",
                "version": "1.0.0",
                "runtime": "{runtime}",
                "entry": "{entry}"
            }}"#
        );
        fs::write(dir.join(MANIFEST_FILE_NAME), json).unwrap();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "micyou-plugin-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn scan_discovers_valid_plugins_and_skips_broken_dirs() {
        let root = temp_dir("scan");
        let plugins_dir = root.join("plugins");
        write_manifest(
            &plugins_dir.join("dev.micyou.alpha"),
            "dev.micyou.alpha",
            RuntimeKind::Native,
            "liba.so",
        );
        write_manifest(
            &plugins_dir.join("dev.micyou.beta"),
            "dev.micyou.beta",
            RuntimeKind::Wasm,
            "b.wasm",
        );
        fs::create_dir_all(plugins_dir.join("broken-dir")).unwrap(); // no manifest

        let mut manager = PluginManager::new(plugins_dir.clone(), root.join("state.json"));
        let report = manager.scan().unwrap();

        assert_eq!(report.discovered.len(), 2);
        assert_eq!(report.skipped.len(), 1);
        assert!(manager.entry("dev.micyou.alpha").unwrap().is_some());
        assert!(manager.entry("dev.micyou.beta").unwrap().is_some());
        assert!(manager.entry("nope").unwrap().is_none());
    }

    #[test]
    fn state_persists_enable_disable_across_restart() {
        let root = temp_dir("state");
        let plugins_dir = root.join("plugins");
        let state_path = root.join("state.json");
        write_manifest(
            &plugins_dir.join("dev.micyou.alpha"),
            "dev.micyou.alpha",
            RuntimeKind::Native,
            "liba.so",
        );

        {
            let mut manager = PluginManager::new(plugins_dir.clone(), state_path.clone());
            manager.scan().unwrap();
            assert!(!manager.is_enabled("dev.micyou.alpha").unwrap());
            manager.set_enabled("dev.micyou.alpha", true).unwrap();
            assert!(manager.is_enabled("dev.micyou.alpha").unwrap());
        }
        {
            // Simulated restart: state must be reloaded from disk
            let mut manager = PluginManager::new(plugins_dir.clone(), state_path.clone());
            manager.scan().unwrap();
            assert!(manager.is_enabled("dev.micyou.alpha").unwrap());
        }
    }

    #[test]
    fn discover_plugin_and_remove() {
        let root = temp_dir("discover");
        let plugins_dir = root.join("plugins");
        let mut manager = PluginManager::new(plugins_dir.clone(), root.join("state.json"));
        write_manifest(
            &plugins_dir.join("dev.micyou.gamma"),
            "dev.micyou.gamma",
            RuntimeKind::Wasm,
            "g.wasm",
        );
        let id = manager
            .discover_plugin(plugins_dir.join("dev.micyou.gamma"))
            .unwrap();
        assert_eq!(id, "dev.micyou.gamma");
        assert!(!manager.is_enabled(&id).unwrap());

        manager.set_enabled(&id, true).unwrap();
        manager.remove_plugin(&id).unwrap();
        assert!(manager.entry(&id).unwrap().is_none());
    }

    #[test]
    fn register_instance_requires_enabled_plugin() {
        let root = temp_dir("instance");
        let plugins_dir = root.join("plugins");
        let mut manager = PluginManager::new(plugins_dir.clone(), root.join("state.json"));
        write_manifest(
            &plugins_dir.join("dev.micyou.alpha"),
            "dev.micyou.alpha",
            RuntimeKind::Native,
            "liba.so",
        );
        manager.scan().unwrap();

        // Disabled → load rejected
        let result = manager.register_instance(dummy_instance("dev.micyou.alpha"));
        assert!(matches!(result, Err(PluginError::NotLoaded(_))));

        manager.set_enabled("dev.micyou.alpha", true).unwrap();
        manager
            .register_instance(dummy_instance("dev.micyou.alpha"))
            .unwrap();
        assert!(manager.is_loaded("dev.micyou.alpha"));
        assert_eq!(manager.loaded_ids(), vec!["dev.micyou.alpha".to_string()]);

        // Shared handle round-trip
        manager
            .with_instance("dev.micyou.alpha", |_instance| Ok(()))
            .unwrap();
        assert!(manager.is_loaded("dev.micyou.alpha"));
    }

    fn dummy_instance(id: &str) -> PluginInstance {
        use crate::plugin::PluginRuntime;
        struct Dummy {
            manifest: PluginManifest,
        }
        impl PluginRuntime for Dummy {
            fn manifest(&self) -> &PluginManifest {
                &self.manifest
            }
            fn init(&mut self, _host: &dyn HostApi) -> PluginResult<()> {
                Ok(())
            }
            fn deinit(&mut self) {}
            fn process_audio(
                &mut self,
                _ctx: &mut crate::plugin::AudioFrameCtx,
            ) -> PluginResult<crate::plugin::ProcessStatus> {
                Ok(crate::plugin::ProcessStatus::Bypass)
            }
            fn handle_event(&mut self, _event: &crate::plugin::PluginEvent) -> PluginResult<()> {
                Ok(())
            }
            fn handle_message(
                &mut self,
                _source: &str,
                _topic: &str,
                _payload: &[u8],
            ) -> PluginResult<()> {
                Ok(())
            }
        }
        let manifest = PluginManifest {
            id: id.to_string(),
            name: format!("Test {id}"),
            version: "1.0.0".to_string(),
            author: None,
            description: None,
            runtime: RuntimeKind::Wasm,
            entry: "x.wasm".to_string(),
            platforms: Vec::new(),
            api_version: crate::manifest::HOST_API_VERSION,
            capabilities: Vec::new(),
            kind: crate::manifest::PluginKind::Utility,
            ui: None,
            dsp: None,
            config: None,
        ..Default::default()
        };
        PluginInstance::Wasm(Box::new(Dummy { manifest }))
    }
}
