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

//! Plugin DSP integration.
//!
//! Plugins with `kind: dsp` register processing nodes here. The registry
//! orders nodes by their manifest position hints (`first` / `insert_after`
//! applied plugin-to-plugin). The audio thread pulls a closure through
//! `micyou_audio::DspProcessor::set_external_hook`; the closure receives the
//! processing-chain node name and runs either a single plugin (per-plugin
//! `Plugin:<id>` nodes, issue #347) or every registered node in registry
//! order (the legacy synthetic `"Plugins"` node).
//!
//! Real-time safety: `process_all` / `process_one` run on the audio thread.
//! Nodes are `Arc<Mutex<PluginInstance>>` — the lock is uncontended in steady
//! state (plugins only change on load/unload) but plugin code itself must
//! obey the real-time rules (no allocations, no blocking host calls, bounded
//! work).

use crate::error::{PluginError, PluginResult};
use crate::plugin::{AudioFrameCtx, PluginInstance, PluginRuntime, ProcessStatus};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// Consecutive failed frames after which a node is skipped until the plugin
/// is enabled again (about one second of audio).
const MAX_CONSECUTIVE_FAILURES: u32 = 50;

/// Per-node DSP callback: node name, interleaved samples, channels, queued ms.
pub type DspHook = Box<dyn FnMut(&str, &mut Vec<f32>, usize, f64) + Send>;

/// Legacy synthetic chain node name that runs **all** registered plugins.
/// Must stay in sync with `micyou_audio::dsp::PLUGIN_CHAIN_NODE` (this crate
/// intentionally does not depend on `micyou-audio`).
pub const CHAIN_NODE_ALL_PLUGINS: &str = "Plugins";

/// Prefix of per-plugin chain nodes (`Plugin:<plugin-id>`): one node per
/// registered DSP plugin so each can be reordered independently (#347).
/// Must stay in sync with `micyou_audio::dsp::PLUGIN_NODE_PREFIX`.
pub const CHAIN_NODE_PLUGIN_PREFIX: &str = "Plugin:";

/// A registered plugin DSP node.
pub struct DspNode {
    /// Plugin id (also the node id).
    pub plugin_id: String,
    /// Manifest position hint: run before all others.
    pub first: bool,
    /// Manifest position hint: run right after this plugin id (if present).
    pub insert_after: Option<String>,
    /// The loaded instance, shared with the plugin manager.
    pub instance: Arc<Mutex<PluginInstance>>,
    failures: AtomicU32,
}

impl DspNode {
    pub fn new(
        plugin_id: String,
        first: bool,
        insert_after: Option<String>,
        instance: Arc<Mutex<PluginInstance>>,
    ) -> Self {
        Self {
            plugin_id,
            first,
            insert_after,
            instance,
            failures: AtomicU32::new(0),
        }
    }
}

/// Ordered, thread-safe registry of active DSP plugin nodes.
pub struct PluginDspRegistry {
    nodes: RwLock<Vec<DspNode>>,
    /// Set when the chain should include the synthetic `"Plugins"` node.
    active: AtomicBool,
}

impl Default for PluginDspRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginDspRegistry {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(Vec::new()),
            active: AtomicBool::new(false),
        }
    }

    /// Whether any DSP plugin is registered.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Register a node. Keeps plugin-to-plugin ordering stable: `first` nodes
    /// go ahead, otherwise sorted by id for determinism.
    pub fn register(&self, node: DspNode) -> PluginResult<()> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| PluginError::Runtime("dsp registry poisoned".into()))?;
        if nodes.iter().any(|n| n.plugin_id == node.plugin_id) {
            return Err(PluginError::AlreadyExists(format!(
                "dsp node {} already registered",
                node.plugin_id
            )));
        }
        nodes.push(node);
        nodes.sort_by(|a, b| {
            b.first
                .cmp(&a.first)
                .then_with(|| a.plugin_id.cmp(&b.plugin_id))
        });
        self.active.store(true, Ordering::Release);
        Ok(())
    }

    pub fn unregister(&self, plugin_id: &str) -> PluginResult<()> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| PluginError::Runtime("dsp registry poisoned".into()))?;
        nodes.retain(|n| n.plugin_id != plugin_id);
        self.active.store(!nodes.is_empty(), Ordering::Release);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.nodes.read().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn plugin_ids(&self) -> Vec<String> {
        self.nodes
            .read()
            .map(|g| g.iter().map(|n| n.plugin_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Run a single node (audio-thread context). One broken plugin must not
    /// take the chain down: a failing node is logged once and, after
    /// `MAX_CONSECUTIVE_FAILURES` frames in a row, skipped.
    fn run_node(
        node: &DspNode,
        data: &mut Vec<f32>,
        channels: usize,
        sample_rate: u32,
        queued_ms: f64,
    ) {
        if node.failures.load(Ordering::Relaxed) >= MAX_CONSECUTIVE_FAILURES {
            return;
        }
        let Ok(mut instance) = node.instance.lock() else {
            return;
        };
        let mut ctx = AudioFrameCtx {
            data,
            channels,
            sample_rate,
            queued_ms,
        };
        match instance.process_audio(&mut ctx) {
            Ok(ProcessStatus::Ok) => {
                node.failures.store(0, Ordering::Relaxed);
                // A NaN from the plugin would stick in every recursive filter
                // after it.
                for sample in ctx.data.iter_mut().filter(|s| !s.is_finite()) {
                    *sample = 0.0;
                }
            }
            Ok(ProcessStatus::Bypass) => node.failures.store(0, Ordering::Relaxed),
            Err(e) => match node.failures.fetch_add(1, Ordering::Relaxed) + 1 {
                1 => log::warn!("[plugins] dsp node {} failed: {e}", node.plugin_id),
                MAX_CONSECUTIVE_FAILURES => log::error!(
                    "[plugins] dsp node {} failed {MAX_CONSECUTIVE_FAILURES} frames in a row, \
                     skipping it until the plugin is enabled again: {e}",
                    node.plugin_id
                ),
                _ => {}
            },
        }
    }

    /// Run every registered node on one audio frame (audio-thread context,
    /// legacy `"Plugins"` chain node).
    pub fn process_all(
        &self,
        data: &mut Vec<f32>,
        channels: usize,
        sample_rate: u32,
        queued_ms: f64,
    ) {
        let nodes = match self.nodes.read() {
            Ok(g) => g,
            Err(_) => return,
        };
        for node in nodes.iter() {
            Self::run_node(node, data, channels, sample_rate, queued_ms);
        }
    }

    /// Run a single registered node by plugin id on one audio frame
    /// (per-plugin `Plugin:<id>` chain node, issue #347). Unknown ids are a
    /// no-op so stale chain entries never break audio.
    pub fn process_one(
        &self,
        plugin_id: &str,
        data: &mut Vec<f32>,
        channels: usize,
        sample_rate: u32,
        queued_ms: f64,
    ) {
        let nodes = match self.nodes.read() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(node) = nodes.iter().find(|n| n.plugin_id == plugin_id) {
            Self::run_node(node, data, channels, sample_rate, queued_ms);
        }
    }
}

/// Bridge that turns a `PluginDspRegistry` into the closure the audio thread
/// attaches to `DspProcessor::set_external_hook`.
pub struct PluginDspBridge {
    registry: Arc<PluginDspRegistry>,
}

impl PluginDspBridge {
    pub fn new(registry: Arc<PluginDspRegistry>) -> Self {
        Self { registry }
    }

    /// The closure consumed by the audio engine. The first argument is the
    /// processing-chain node name: the legacy `"Plugins"` node runs every
    /// registered plugin; a `Plugin:<id>` node runs just that plugin.
    /// Cheap when no nodes exist.
    pub fn hook(&self) -> DspHook {
        let registry = self.registry.clone();
        Box::new(
            move |node: &str, data: &mut Vec<f32>, channels: usize, queued_ms: f64| {
                if !registry.is_active() {
                    return;
                }
                if node == CHAIN_NODE_ALL_PLUGINS {
                    registry.process_all(data, channels, 48_000, queued_ms);
                } else if let Some(plugin_id) = node.strip_prefix(CHAIN_NODE_PLUGIN_PREFIX) {
                    registry.process_one(plugin_id, data, channels, 48_000, queued_ms);
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PluginKind, PluginManifest, RuntimeKind};
    use crate::plugin::{PluginRuntime, PluginState};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingNode {
        manifest: PluginManifest,
        calls: Arc<AtomicUsize>,
        fail: bool,
    }

    impl PluginRuntime for CountingNode {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }
        fn init(&mut self, _host: &dyn crate::host::HostApi) -> PluginResult<()> {
            Ok(())
        }
        fn deinit(&mut self) {}
        fn process_audio(&mut self, ctx: &mut AudioFrameCtx<'_>) -> PluginResult<ProcessStatus> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(PluginError::Runtime("broken".into()));
            }
            for sample in ctx.data.iter_mut() {
                *sample += 1.0;
            }
            Ok(ProcessStatus::Ok)
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

    fn make_node(id: &str, first: bool, calls: Arc<AtomicUsize>) -> DspNode {
        make_node_with(id, first, calls, false)
    }

    fn make_node_with(id: &str, first: bool, calls: Arc<AtomicUsize>, fail: bool) -> DspNode {
        DspNode::new(
            id.to_string(),
            first,
            None,
            Arc::new(Mutex::new(PluginInstance::Wasm(Box::new(CountingNode {
                manifest: PluginManifest {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: "1.0.0".to_string(),
                    author: None,
                    description: None,
                    runtime: RuntimeKind::Wasm,
                    entry: "x.wasm".to_string(),
                    platforms: Vec::new(),
                    api_version: crate::manifest::HOST_API_VERSION,
                    capabilities: Vec::new(),
                    kind: PluginKind::Dsp,
                    ui: None,
                    dsp: None,
                    config: None,
                    ..Default::default()
                },
                calls,
                fail,
            })))),
        )
    }

    #[test]
    fn failing_node_is_skipped_after_consecutive_failures() {
        let registry = PluginDspRegistry::new();
        let calls = Arc::new(AtomicUsize::new(0));
        registry
            .register(make_node_with("a.broken", false, calls.clone(), true))
            .unwrap();
        let mut data = vec![0.0f32; 4];
        for _ in 0..MAX_CONSECUTIVE_FAILURES * 2 {
            registry.process_all(&mut data, 1, 48_000, 0.0);
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAX_CONSECUTIVE_FAILURES as usize
        );
    }

    #[test]
    fn registry_runs_all_nodes_in_first_first_order() {
        let registry = PluginDspRegistry::new();
        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));
        registry
            .register(make_node("z.last", false, a.clone()))
            .unwrap();
        registry
            .register(make_node("a.first", true, b.clone()))
            .unwrap();
        assert_eq!(registry.plugin_ids(), vec!["a.first", "z.last"]);
        assert!(registry.is_active());

        let mut data = vec![0.0f32; 4];
        registry.process_all(&mut data, 1, 48_000, 0.0);
        // both nodes ran (+1.0 each) and "a.first" ran first (order verified by
        // values would need shared buffer; count is enough for ordering here)
        assert_eq!(data, vec![2.0f32; 4]);
        assert_eq!(a.load(Ordering::SeqCst), 1);
        assert_eq!(b.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn registry_unregister_and_duplicate_rejected() {
        let registry = PluginDspRegistry::new();
        let calls = Arc::new(AtomicUsize::new(0));
        registry
            .register(make_node("dup", false, calls.clone()))
            .unwrap();
        let result = registry.register(make_node("dup", false, calls.clone()));
        assert!(matches!(result, Err(PluginError::AlreadyExists(_))));

        registry.unregister("dup").unwrap();
        assert!(registry.is_empty());
        assert!(!registry.is_active());
        let mut data = vec![0.0f32; 2];
        registry.process_all(&mut data, 1, 48_000, 0.0);
        assert_eq!(data, vec![0.0f32; 2]);
    }

    #[test]
    fn bridge_hook_is_noop_when_inactive() {
        let registry = Arc::new(PluginDspRegistry::new());
        let bridge = PluginDspBridge::new(registry.clone());
        let mut hook = bridge.hook();
        let mut data = vec![0.5f32; 8];
        hook(CHAIN_NODE_ALL_PLUGINS, &mut data, 1, 0.0);
        hook("Plugin:anything", &mut data, 1, 0.0);
        assert_eq!(data, vec![0.5f32; 8], "no nodes → data untouched");
    }

    #[test]
    fn bridge_hook_routes_per_plugin_nodes() {
        let registry = Arc::new(PluginDspRegistry::new());
        let a_calls = Arc::new(AtomicUsize::new(0));
        let b_calls = Arc::new(AtomicUsize::new(0));
        registry
            .register(make_node("a", false, a_calls.clone()))
            .unwrap();
        registry
            .register(make_node("b", false, b_calls.clone()))
            .unwrap();
        let bridge = PluginDspBridge::new(registry.clone());
        let mut hook = bridge.hook();

        let mut data = vec![0.0f32; 4];
        hook("Plugin:b", &mut data, 1, 0.0);
        assert_eq!(a_calls.load(Ordering::SeqCst), 0);
        assert_eq!(b_calls.load(Ordering::SeqCst), 1);
        assert_eq!(data, vec![1.0f32; 4], "only plugin b ran");

        hook("Plugin:missing", &mut data, 1, 0.0);
        assert_eq!(data, vec![1.0f32; 4], "unknown plugin node is a no-op");

        hook(CHAIN_NODE_ALL_PLUGINS, &mut data, 1, 0.0);
        assert_eq!(a_calls.load(Ordering::SeqCst), 1);
        assert_eq!(b_calls.load(Ordering::SeqCst), 2);
        assert_eq!(data, vec![3.0f32; 4], "legacy node runs every plugin");
    }

    #[test]
    fn plugin_state_enum_is_usable() {
        // keeps PluginState re-exported in this module's scope for docs
        let _ = PluginState::Enabled;
    }
}
