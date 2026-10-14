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

//! WASM plugin runtime.
//!
//! WebAssembly modules run in the `wasmi` interpreter — a pure-Rust engine
//! with no native dependencies, so the same runtime can later be embedded on
//! Android. Plugins import host functions under the `micyou` module; the host
//! writes strings/buffers into the plugin's linear memory through the
//! exported `alloc`/`dealloc` pair.
//!
//! Sandboxing: every entry-point call runs under a fuel budget (`EngineConfig`
//! with `consume_fuel`) so a plugin stuck in an infinite loop is trapped
//! instead of hanging the host. WASM DSP nodes are explicitly best-effort:
//! interpreter latency cannot guarantee real-time safety.

use crate::abi::mpl_result_t;
use crate::error::{PluginError, PluginResult};
use crate::host::HostApi;
use crate::host::PluginLogLevel;
use crate::manifest::PluginManifest;
use crate::plugin::{AudioFrameCtx, PluginEvent, PluginInstance, PluginRuntime, ProcessStatus};
use std::cell::Cell;
use std::path::Path;
use std::sync::Arc;
use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, TypedFunc, WasmParams, WasmResults,
};

/// Fuel granted to a plugin call before the engine traps it.
const CALL_FUEL_BUDGET: u64 = 100_000;

/// Host functions a WASM plugin can import (module `micyou`).
pub const WASM_IMPORT_MODULE: &str = "micyou";

/// Host-side state stored inside the wasmi `Store`.
pub struct WasmHostCtx {
    pub host: Arc<dyn HostApi>,
    pub capabilities: Vec<String>,
    /// 宿主端复用缓冲区（audio_state / connected_devices 高频路径避免 bump 泄漏）
    /// (ptr, capacity)；容量不足时重新分配（罕见，泄漏一次）
    pub scratch: Cell<Option<(i32, i32)>>,
}

impl WasmHostCtx {
    fn require(&self, capability: &str) -> Result<(), PluginError> {
        if self.capabilities.iter().any(|c| c == capability) {
            Ok(())
        } else {
            Err(PluginError::PermissionDenied(format!(
                "plugin lacks capability {capability}"
            )))
        }
    }

    /// `require` for host functions: a missing capability traps the call.
    fn check(&self, capability: &str) -> Result<(), wasmi::Error> {
        self.require(capability)
            .map_err(|e| wasmi::Error::new(e.to_string()))
    }
}

/// A loaded WASM plugin.
pub struct WasmPlugin {
    manifest: PluginManifest,
    /// Kept alive so `store`/`instance`/`memory` remain valid.
    #[allow(dead_code)]
    engine: Engine,
    store: Store<WasmHostCtx>,
    instance: Instance,
    memory: Memory,
    f_init: Option<TypedFunc<(), i32>>,
    f_deinit: Option<TypedFunc<(), ()>>,
    f_process: Option<TypedFunc<(i32, i32, i32, f64), i32>>,
    f_event: Option<TypedFunc<(i32,), i32>>,
    f_message: Option<TypedFunc<(i32, i32), i32>>,
    f_alloc: TypedFunc<(i32,), i32>,
    f_dealloc: TypedFunc<(i32, i32), ()>,
    /// Reused per-frame buffer (ptr, capacity bytes) so bump-allocator
    /// plugins do not exhaust linear memory: `process_audio` allocates once
    /// and reuses the same region for every frame.
    frame_buf: Option<(i32, usize)>,
}

impl WasmPlugin {
    /// Load + instantiate a WASM module from `<plugin_dir>/<manifest.entry>`.
    pub fn load(
        manifest: PluginManifest,
        plugin_dir: &Path,
        host: Arc<dyn HostApi>,
    ) -> PluginResult<Self> {
        let entry = manifest.entry_path(plugin_dir);
        let bytes = std::fs::read(&entry)
            .map_err(|e| PluginError::NotFound(format!("{}: {e}", entry.display())))?;
        Self::from_bytes(manifest, bytes, host)
    }

    /// Instantiate a WASM module from raw bytes (used by tests and by embedders).
    pub fn from_bytes(
        manifest: PluginManifest,
        wasm_bytes: Vec<u8>,
        host: Arc<dyn HostApi>,
    ) -> PluginResult<Self> {
        if manifest.api_version < crate::manifest::MIN_SUPPORTED_API_VERSION
            || manifest.api_version > crate::manifest::HOST_API_VERSION
        {
            return Err(PluginError::ApiVersionMismatch {
                plugin: manifest.api_version,
                host: crate::manifest::HOST_API_VERSION,
            });
        }

        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, &wasm_bytes[..])
            .map_err(|e| PluginError::LoadFailed(format!("module parse: {e}")))?;

        let ctx = WasmHostCtx {
            host,
            capabilities: manifest.capabilities.clone(),
            scratch: Cell::new(None),
        };
        let mut store = Store::new(&engine, ctx);

        let mut linker = Linker::new(&engine);
        register_host_functions(&mut linker);

        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| PluginError::LoadFailed(format!("instantiate: {e}")))?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| PluginError::LoadFailed("module must export linear memory".into()))?;

        let f_alloc: TypedFunc<(i32,), i32> = instance
            .get_typed_func(&store, "alloc")
            .map_err(|e| PluginError::LoadFailed(format!("missing alloc export: {e}")))?;
        let f_dealloc: TypedFunc<(i32, i32), ()> = instance
            .get_typed_func(&store, "dealloc")
            .map_err(|e| PluginError::LoadFailed(format!("missing dealloc export: {e}")))?;

        let f_init = optional_func(&instance, &store, "init")?;
        let f_deinit = optional_func(&instance, &store, "deinit")?;
        let f_process = optional_func(&instance, &store, "process")?;
        let f_event = optional_func(&instance, &store, "handle_event")?;
        let f_message = optional_func(&instance, &store, "handle_message")?;

        Ok(WasmPlugin {
            manifest,
            engine,
            store,
            instance,
            memory,
            f_init,
            f_deinit,
            f_process,
            f_event,
            f_message,
            f_alloc,
            f_dealloc,
            frame_buf: None,
        })
    }

    /// Run `f` with a fresh fuel budget, mapping fuel exhaustion to an error.
    fn with_fuel<T>(&mut self, f: impl FnOnce(&mut Self) -> PluginResult<T>) -> PluginResult<T> {
        self.store
            .set_fuel(CALL_FUEL_BUDGET)
            .map_err(|e| PluginError::Runtime(format!("set fuel: {e}")))?;
        // Running out traps the call, which `f` reports as an error.
        f(self)
    }

    /// Write a NUL-terminated string into plugin memory; returns its address.
    fn write_str(&mut self, text: &str) -> PluginResult<i32> {
        let size = text.len() as i32 + 1;
        let ptr = self
            .f_alloc
            .call(&mut self.store, (size,))
            .map_err(|e| PluginError::Runtime(format!("alloc: {e}")))?;
        let bytes = text.as_bytes();
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|e| PluginError::Runtime(format!("write string: {e}")))?;
        self.memory
            .write(&mut self.store, ptr as usize + bytes.len(), &[0u8])
            .map_err(|e| PluginError::Runtime(format!("write NUL: {e}")))?;
        Ok(ptr)
    }

    /// Read a NUL-terminated string from plugin memory (test/debug helper).
    pub fn read_str(&mut self, ptr: i32) -> PluginResult<String> {
        read_c_str(self.memory.data(&self.store), ptr).map_err(PluginError::Runtime)
    }

    /// Expose the wasmi store mutably (test/debug helper).
    pub fn store_mut(&mut self) -> &mut Store<WasmHostCtx> {
        &mut self.store
    }

    /// Expose the wasmi store immutably (test/debug helper).
    pub fn store_ref(&self) -> &Store<WasmHostCtx> {
        &self.store
    }

    /// Expose the wasmi instance (test/debug helper).
    pub fn instance_ref(&self) -> &Instance {
        &self.instance
    }

    /// Fetch a typed export without tripping the borrow checker (test/debug
    /// helper). Returns `None` when the export is missing or mis-typed.
    pub fn export<Params, Results>(&mut self, name: &str) -> Option<TypedFunc<Params, Results>>
    where
        Params: WasmParams,
        Results: WasmResults,
    {
        // Fresh fuel for the caller's subsequent direct call.
        let _ = self.store.set_fuel(CALL_FUEL_BUDGET);
        self.instance.get_typed_func(&mut self.store, name).ok()
    }

    /// Write raw bytes into plugin memory; returns their address.
    fn write_bytes(&mut self, data: &[u8]) -> PluginResult<i32> {
        let ptr = self
            .f_alloc
            .call(&mut self.store, (data.len() as i32,))
            .map_err(|e| PluginError::Runtime(format!("alloc bytes: {e}")))?;
        self.memory
            .write(&mut self.store, ptr as usize, data)
            .map_err(|e| PluginError::Runtime(format!("write bytes: {e}")))?;
        Ok(ptr)
    }

    /// Serialize an event to JSON and deliver it.
    fn deliver_event(&mut self, event: &PluginEvent) -> PluginResult<()> {
        let Some(f_event) = self.f_event else {
            return Ok(());
        };
        let json = serde_json::to_string(event)
            .map_err(|e| PluginError::Runtime(format!("event serialize: {e}")))?;
        let ptr = self.write_str(&json)?;
        let code = f_event
            .call(&mut self.store, (ptr,))
            .map_err(|e| PluginError::Runtime(format!("handle_event: {e}")))?;
        self.f_dealloc
            .call(&mut self.store, (ptr, json.len() as i32 + 1))?;
        result_from_wasm_code(code, "handle_event")
    }
}

fn optional_func<Params, Results>(
    instance: &Instance,
    store: &Store<WasmHostCtx>,
    name: &str,
) -> PluginResult<Option<TypedFunc<Params, Results>>>
where
    Params: WasmParams,
    Results: WasmResults,
{
    Ok(instance.get_typed_func(store, name).ok())
}

/// The `len` bytes at `ptr`, if the plugin's allocation lies inside memory.
fn frame_slice(memory: &mut [u8], ptr: i32, len: usize) -> PluginResult<&mut [u8]> {
    usize::try_from(ptr)
        .ok()
        .and_then(|start| memory.get_mut(start..start.checked_add(len)?))
        .ok_or_else(|| PluginError::Runtime("frame buffer outside plugin memory".into()))
}

fn result_from_wasm_code(code: i32, context: &str) -> PluginResult<()> {
    match code {
        0 => Ok(()),
        _ => Err(PluginError::Runtime(format!(
            "{context}: plugin returned {code}"
        ))),
    }
}

impl PluginRuntime for WasmPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn init(&mut self, _host: &dyn HostApi) -> PluginResult<()> {
        let Some(f_init) = self.f_init else {
            return Ok(());
        };
        self.with_fuel(|this| {
            let code = f_init
                .call(&mut this.store, ())
                .map_err(|e| PluginError::Runtime(format!("init: {e}")))?;
            result_from_wasm_code(code, "init")
        })
    }

    fn deinit(&mut self) {
        let Some(f_deinit) = self.f_deinit else {
            return;
        };
        // Fresh budget: whatever the last call left may be too little.
        let _ = self.with_fuel(|this| {
            f_deinit
                .call(&mut this.store, ())
                .map_err(|e| PluginError::Runtime(format!("deinit: {e}")))
        });
    }

    fn process_audio(&mut self, ctx: &mut AudioFrameCtx<'_>) -> PluginResult<ProcessStatus> {
        let Some(f_process) = self.f_process else {
            return Ok(ProcessStatus::Bypass);
        };
        if ctx.data.is_empty() {
            return Ok(ProcessStatus::Bypass);
        }
        self.with_fuel(|this| {
            // Reuse a cached frame buffer (alloc once; bump allocators never
            // free, so a fresh alloc per frame exhausts linear memory)
            let need = std::mem::size_of_val(ctx.data);
            let (ptr, _cap) = match this.frame_buf {
                Some((p, c)) if c >= need => (p, c),
                _ => {
                    if let Some((old, c)) = this.frame_buf.take() {
                        let _ = this.f_dealloc.call(&mut this.store, (old, c as i32));
                    }
                    let p = this
                        .f_alloc
                        .call(&mut this.store, (need as i32,))
                        .map_err(|e| PluginError::Runtime(format!("alloc bytes: {e}")))?;
                    this.frame_buf = Some((p, need));
                    (p, need)
                }
            };
            // f32 → little-endian bytes, straight into plugin memory
            let frame = frame_slice(this.memory.data_mut(&mut this.store), ptr, need)?;
            for (chunk, sample) in frame.as_chunks_mut::<4>().0.iter_mut().zip(ctx.data.iter()) {
                *chunk = sample.to_le_bytes();
            }
            let code = f_process
                .call(
                    &mut this.store,
                    (
                        ptr,
                        ctx.data.len() as i32,
                        ctx.channels as i32,
                        ctx.queued_ms,
                    ),
                )
                .map_err(|e| PluginError::Runtime(format!("process: {e}")))?;
            // Bypass: the plugin did not write the buffer, so reading it back
            // would return the previous frame's stale data (frame_buf reuse)
            // and produce garbage / 电流麦. Return without touching ctx.data.
            if code == 1 {
                return Ok(ProcessStatus::Bypass);
            }
            result_from_wasm_code(code, "process")?;
            let frame = frame_slice(this.memory.data_mut(&mut this.store), ptr, need)?;
            for (sample, chunk) in ctx.data.iter_mut().zip(frame.as_chunks::<4>().0) {
                *sample = f32::from_le_bytes(*chunk);
            }
            Ok(ProcessStatus::Ok)
        })
    }

    fn handle_event(&mut self, event: &PluginEvent) -> PluginResult<()> {
        self.with_fuel(|this| this.deliver_event(event))
    }

    fn handle_message(&mut self, _source: &str, _topic: &str, payload: &[u8]) -> PluginResult<()> {
        let Some(f_message) = self.f_message else {
            return Ok(());
        };
        self.with_fuel(|this| {
            // Payload lives in memory; source/topic are delivered via host events.
            let ptr = this.write_bytes(payload)?;
            let code = f_message
                .call(&mut this.store, (ptr, payload.len() as i32))
                .map_err(|e| PluginError::Runtime(format!("handle_message: {e}")))?;
            this.f_dealloc
                .call(&mut this.store, (ptr, payload.len() as i32))?;
            result_from_wasm_code(code, "handle_message")
        })
    }
}

/// Convenience: load a WASM plugin and wrap it as a `PluginInstance`.
pub fn load_wasm_instance(
    manifest: PluginManifest,
    plugin_dir: &Path,
    host: Arc<dyn HostApi>,
) -> PluginResult<PluginInstance> {
    Ok(PluginInstance::Wasm(Box::new(WasmPlugin::load(
        manifest, plugin_dir, host,
    )?)))
}

// ── Host function registration ─────────────────────────────────────────────

fn register_host_functions(linker: &mut Linker<WasmHostCtx>) {
    // log(level: i32, msg_ptr: i32)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "log",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>, level: i32, ptr: i32| {
                let memory = caller.get_export("memory").and_then(|e| e.into_memory());
                let Some(memory) = memory else {
                    return Err(wasmi::Error::new("memory export missing"));
                };
                let text = read_str_from_memory(&mut caller, &memory, ptr)?;
                let level = match level {
                    0 => PluginLogLevel::Error,
                    1 => PluginLogLevel::Warn,
                    2 => PluginLogLevel::Info,
                    3 => PluginLogLevel::Debug,
                    _ => PluginLogLevel::Trace,
                };
                caller.data().host.log(level, &text);
                Ok(())
            },
        )
        .unwrap();

    // get_config(key_ptr: i32) -> ptr (host-allocated JSON, or 0)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "get_config",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             key_ptr: i32|
             -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONFIG_READ)?;
                let memory = export_memory(&caller)?;
                let key = read_str_from_memory(&mut caller, &memory, key_ptr)?;
                match caller.data().host.get_config(&key) {
                    Some(value) => {
                        let json = value.to_string();
                        let ptr = write_str_to_memory(&mut caller, &memory, &json)?;
                        Ok(ptr)
                    }
                    None => Ok(0),
                }
            },
        )
        .unwrap();

    // set_config(key_ptr, value_ptr) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_config",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             key_ptr: i32,
             value_ptr: i32|
             -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONFIG_WRITE)?;
                let memory = export_memory(&caller)?;
                let key = read_str_from_memory(&mut caller, &memory, key_ptr)?;
                let value_json = read_str_from_memory(&mut caller, &memory, value_ptr)?;
                let value: serde_json::Value = serde_json::from_str(&value_json)
                    .map_err(|e| wasmi::Error::new(format!("invalid json config: {e}")))?;
                caller
                    .data()
                    .host
                    .set_config(&key, value)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // emit_event(topic_ptr, payload_ptr) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "emit_event",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             topic_ptr: i32,
             payload_ptr: i32|
             -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::EVENT_EMIT)?;
                let memory = export_memory(&caller)?;
                let topic = read_str_from_memory(&mut caller, &memory, topic_ptr)?;
                let payload_json = read_str_from_memory(&mut caller, &memory, payload_ptr)?;
                let payload: serde_json::Value = serde_json::from_str(&payload_json)
                    .map_err(|e| wasmi::Error::new(format!("invalid json payload: {e}")))?;
                caller
                    .data()
                    .host
                    .emit_event(&topic, payload)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // send_message(target_ptr, payload_ptr, payload_len) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "send_message",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             target_ptr: i32,
             payload_ptr: i32,
             payload_len: i32|
             -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::MESSAGE_SEND)?;
                let memory = export_memory(&caller)?;
                let target_json = read_str_from_memory(&mut caller, &memory, target_ptr)?;
                let target: crate::host::MessageTarget = serde_json::from_str(&target_json)
                    .map_err(|e| wasmi::Error::new(format!("invalid target: {e}")))?;
                let payload =
                    read_bytes_from_memory(&mut caller, &memory, payload_ptr, payload_len)?;
                caller
                    .data()
                    .host
                    .send_message(target, payload)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // audio_state() -> ptr (host-allocated JSON, or 0)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "audio_state",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::AUDIO_STATE)?;
                let memory = export_memory(&caller)?;
                let json = serde_json::to_string(&caller.data().host.audio_state())
                    .map_err(|e| wasmi::Error::new(format!("serialize: {e}")))?;
                let ptr = write_scratch(&mut caller, &memory, &json)?;
                Ok(ptr)
            },
        )
        .unwrap();

    // play_sound(path_ptr) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "play_sound",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             path_ptr: i32|
             -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::AUDIO_PLAY)?;
                let memory = export_memory(&caller)?;
                let path = read_str_from_memory(&mut caller, &memory, path_ptr)?;
                caller
                    .data()
                    .host
                    .play_sound(&path)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // register_hotkey(shortcut_ptr) -> i64 (handle id, 0 on failure)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "register_hotkey",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             shortcut_ptr: i32|
             -> Result<i64, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let shortcut = read_str_from_memory(&mut caller, &memory, shortcut_ptr)?;
                let id = match caller.data().host.register_hotkey(&shortcut) {
                    Ok(id) => id as i64,
                    Err(e) => {
                        caller.data().host.log(
                            PluginLogLevel::Warn,
                            &format!("register_hotkey failed ({shortcut}): {e}"),
                        );
                        0
                    }
                };
                Ok(id)
            },
        )
        .unwrap();

    // open_window(panel_id_ptr) -> i32 (result code)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "open_window",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             panel_ptr: i32|
             -> Result<i32, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let panel = read_str_from_memory(&mut caller, &memory, panel_ptr)?;
                caller
                    .data()
                    .host
                    .open_window(&panel)
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(0)
            },
        )
        .unwrap();

    // plugin_dir() -> ptr (host-allocated absolute path string, or 0)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "plugin_dir",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let dir = caller.data().host.plugin_dir();
                let ptr = write_str_to_memory(&mut caller, &memory, &dir)?;
                Ok(ptr)
            },
        )
        .unwrap();

    // connected_devices() -> ptr (host-allocated JSON array, or 0)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "connected_devices",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::DEVICE_LIST)?;
                let memory = export_memory(&caller)?;
                let json = serde_json::to_string(&caller.data().host.connected_devices())
                    .map_err(|e| wasmi::Error::new(format!("serialize: {e}")))?;
                let ptr = write_scratch(&mut caller, &memory, &json)?;
                Ok(ptr)
            },
        )
        .unwrap();

    // fs_read(path_ptr) -> ptr (host-allocated text, or 0)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "fs_read",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             path_ptr: i32|
             -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::FS_READ)?;
                let memory = export_memory(&caller)?;
                let path = read_str_from_memory(&mut caller, &memory, path_ptr)?;
                let text = caller
                    .data()
                    .host
                    .fs_read(&path)
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                write_str_to_memory(&mut caller, &memory, &text)
            },
        )
        .unwrap();

    // fs_write(path_ptr, content_ptr)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "fs_write",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             path_ptr: i32,
             content_ptr: i32|
             -> Result<(), wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::FS_WRITE)?;
                let memory = export_memory(&caller)?;
                let path = read_str_from_memory(&mut caller, &memory, path_ptr)?;
                let content = read_str_from_memory(&mut caller, &memory, content_ptr)?;
                caller
                    .data()
                    .host
                    .fs_write(&path, &content)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // set_timeout(ms: i64, payload_ptr) -> i64 (timer id)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_timeout",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             ms: i64,
             payload_ptr: i32|
             -> Result<i64, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let payload = read_str_from_memory(&mut caller, &memory, payload_ptr)?;
                let id = caller
                    .data()
                    .host
                    .set_timeout(ms.max(0) as u64, &payload)
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(id as i64)
            },
        )
        .unwrap();

    // clear_timeout(id: i64)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "clear_timeout",
            |caller: wasmi::Caller<'_, WasmHostCtx>, id: i64| -> Result<(), wasmi::Error> {
                caller
                    .data()
                    .host
                    .clear_timeout(id.max(0) as u64)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // http_request(method_ptr, url_ptr, headers_ptr, body_ptr) -> i64 (request id)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "http_request",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             method_ptr: i32,
             url_ptr: i32,
             headers_ptr: i32,
             body_ptr: i32|
             -> Result<i64, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::NETWORK_IO)?;
                let memory = export_memory(&caller)?;
                let method = read_str_from_memory(&mut caller, &memory, method_ptr)?;
                let url = read_str_from_memory(&mut caller, &memory, url_ptr)?;
                let headers = read_str_from_memory(&mut caller, &memory, headers_ptr)?;
                let body = read_str_from_memory(&mut caller, &memory, body_ptr)?;
                let id = caller
                    .data()
                    .host
                    .http_request(&method, &url, &headers, &body)
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(id as i64)
            },
        )
        .unwrap();

    // set_interval(ms: i64, payload_ptr) -> i64
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_interval",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             ms: i64,
             payload_ptr: i32|
             -> Result<i64, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let payload = read_str_from_memory(&mut caller, &memory, payload_ptr)?;
                let id = caller
                    .data()
                    .host
                    .set_interval(ms.max(0) as u64, &payload)
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(id as i64)
            },
        )
        .unwrap();

    // clear_interval(id: i64)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "clear_interval",
            |caller: wasmi::Caller<'_, WasmHostCtx>, id: i64| -> Result<(), wasmi::Error> {
                caller
                    .data()
                    .host
                    .clear_interval(id.max(0) as u64)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // open_url(url_ptr)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "open_url",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             url_ptr: i32|
             -> Result<(), wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::OPEN_URL)?;
                let memory = export_memory(&caller)?;
                let url = read_str_from_memory(&mut caller, &memory, url_ptr)?;
                caller
                    .data()
                    .host
                    .open_url(&url)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // notify(title_ptr, body_ptr)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "notify",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             title_ptr: i32,
             body_ptr: i32|
             -> Result<(), wasmi::Error> {
                let memory = export_memory(&caller)?;
                let title = read_str_from_memory(&mut caller, &memory, title_ptr)?;
                let body = read_str_from_memory(&mut caller, &memory, body_ptr)?;
                caller
                    .data()
                    .host
                    .notify(&title, &body)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // locale() -> ptr
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "locale",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let text = caller.data().host.locale();
                write_str_to_memory(&mut caller, &memory, &text)
            },
        )
        .unwrap();

    // host_info() -> ptr
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "host_info",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                let memory = export_memory(&caller)?;
                let text = caller.data().host.host_info();
                write_str_to_memory(&mut caller, &memory, &text)
            },
        )
        .unwrap();

    // clipboard_read() -> ptr
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "clipboard_read",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CLIPBOARD_READ)?;
                let memory = export_memory(&caller)?;
                let text = caller
                    .data()
                    .host
                    .clipboard_read()
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                write_str_to_memory(&mut caller, &memory, &text)
            },
        )
        .unwrap();

    // set_panel_icon(panel_id_ptr, icon_ptr)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_panel_icon",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             panel_id_ptr: i32,
             icon_ptr: i32|
             -> Result<(), wasmi::Error> {
                let memory = export_memory(&caller)?;
                let panel_id = read_str_from_memory(&mut caller, &memory, panel_id_ptr)?;
                let icon = read_str_from_memory(&mut caller, &memory, icon_ptr)?;
                caller
                    .data()
                    .host
                    .set_panel_icon(&panel_id, &icon)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // clipboard_write(text_ptr)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "clipboard_write",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>,
             text_ptr: i32|
             -> Result<(), wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CLIPBOARD_WRITE)?;
                let memory = export_memory(&caller)?;
                let text = read_str_from_memory(&mut caller, &memory, text_ptr)?;
                caller
                    .data()
                    .host
                    .clipboard_write(&text)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // set_muted(muted: i32) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_muted",
            |caller: wasmi::Caller<'_, WasmHostCtx>, muted: i32| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONTROL_INTERCEPT)?;
                caller
                    .data()
                    .host
                    .set_muted(muted != 0)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // get_muted() -> i32 (0 or 1)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "get_muted",
            |caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONTROL_OBSERVE)?;
                let muted = caller
                    .data()
                    .host
                    .get_muted()
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(if muted { 1 } else { 0 })
            },
        )
        .unwrap();

    // set_monitoring(enabled: i32) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_monitoring",
            |caller: wasmi::Caller<'_, WasmHostCtx>, enabled: i32| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONTROL_INTERCEPT)?;
                caller
                    .data()
                    .host
                    .set_monitoring(enabled != 0)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();

    // get_monitoring() -> i32 (0 or 1)
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "get_monitoring",
            |caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONTROL_OBSERVE)?;
                let enabled = caller
                    .data()
                    .host
                    .get_monitoring()
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(if enabled { 1 } else { 0 })
            },
        )
        .unwrap();

    // get_dsp_settings() -> ptr
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "get_dsp_settings",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONTROL_OBSERVE)?;
                let text = caller
                    .data()
                    .host
                    .get_dsp_settings()
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                let memory = export_memory(&caller)?;
                write_str_to_memory(&mut caller, &memory, &text)
            },
        )
        .unwrap();

    // set_dsp_settings(ptr: i32) -> result code
    linker
        .func_wrap(
            WASM_IMPORT_MODULE,
            "set_dsp_settings",
            |mut caller: wasmi::Caller<'_, WasmHostCtx>, ptr: i32| -> Result<i32, wasmi::Error> {
                caller
                    .data()
                    .check(crate::manifest::capabilities::CONTROL_INTERCEPT)?;
                let memory = export_memory(&caller)?;
                let settings_json = read_str_from_memory(&mut caller, &memory, ptr)?;
                caller
                    .data()
                    .host
                    .set_dsp_settings(&settings_json)
                    .map(|_| mpl_result_t::MPL_OK as i32)
                    .map_err(|e| wasmi::Error::new(e.to_string()))
            },
        )
        .unwrap();
}

fn export_memory(caller: &wasmi::Caller<'_, WasmHostCtx>) -> Result<Memory, wasmi::Error> {
    caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmi::Error::new("memory export missing"))
}

/// A NUL-terminated string at `ptr` in plugin memory; a negative pointer
/// means "no string".
fn read_c_str(memory: &[u8], ptr: i32) -> Result<String, String> {
    let Ok(start) = usize::try_from(ptr) else {
        return Ok(String::new());
    };
    let tail = memory
        .get(start..)
        .ok_or_else(|| format!("read string: pointer {ptr} outside memory"))?;
    let len = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or("read string: missing NUL terminator")?;
    Ok(String::from_utf8_lossy(&tail[..len]).into_owned())
}

fn read_str_from_memory(
    caller: &mut wasmi::Caller<'_, WasmHostCtx>,
    memory: &Memory,
    ptr: i32,
) -> Result<String, wasmi::Error> {
    read_c_str(memory.data(&*caller), ptr).map_err(wasmi::Error::new)
}

fn read_bytes_from_memory(
    caller: &mut wasmi::Caller<'_, WasmHostCtx>,
    memory: &Memory,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>, wasmi::Error> {
    if ptr < 0 || len <= 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0u8; len as usize];
    memory
        .read(&mut *caller, ptr as usize, &mut buf)
        .map_err(|e| wasmi::Error::new(format!("read bytes: {e}")))?;
    Ok(buf)
}

/// Call the plugin's exported `alloc`.
fn plugin_alloc(
    caller: &mut wasmi::Caller<'_, WasmHostCtx>,
    size: i32,
) -> Result<i32, wasmi::Error> {
    let alloc: TypedFunc<(i32,), i32> = caller
        .get_export("alloc")
        .and_then(|e| e.into_func())
        .ok_or_else(|| wasmi::Error::new("alloc export missing"))?
        .typed(&mut *caller)
        .map_err(|e| wasmi::Error::new(format!("alloc typed: {e}")))?;
    alloc
        .call(&mut *caller, (size,))
        .map_err(|e| wasmi::Error::new(format!("alloc call: {e}")))
}

/// Write `text` plus a NUL at `ptr`.
fn write_c_str(
    caller: &mut wasmi::Caller<'_, WasmHostCtx>,
    memory: &Memory,
    ptr: i32,
    text: &str,
) -> Result<(), wasmi::Error> {
    let bytes = text.as_bytes();
    memory
        .write(&mut *caller, ptr as usize, bytes)
        .and_then(|()| memory.write(&mut *caller, ptr as usize + bytes.len(), &[0u8]))
        .map_err(|e| wasmi::Error::new(format!("write string: {e}")))
}

/// Allocate + write a NUL-terminated string via the plugin's exported alloc.
fn write_str_to_memory(
    caller: &mut wasmi::Caller<'_, WasmHostCtx>,
    memory: &Memory,
    text: &str,
) -> Result<i32, wasmi::Error> {
    let ptr = plugin_alloc(caller, text.len() as i32 + 1)?;
    write_c_str(caller, memory, ptr, text)?;
    Ok(ptr)
}

/// 复用宿主 scratch 缓冲区写字符串（一次性分配，之后原地覆盖）
fn write_scratch(
    caller: &mut wasmi::Caller<'_, WasmHostCtx>,
    memory: &Memory,
    text: &str,
) -> Result<i32, wasmi::Error> {
    let need = text.len() as i32 + 1;
    let ptr = match caller.data().scratch.get() {
        Some((ptr, capacity)) if capacity >= need => ptr,
        _ => {
            let capacity = need.max(4096);
            let ptr = plugin_alloc(caller, capacity)?;
            caller.data().scratch.set(Some((ptr, capacity)));
            ptr
        }
    };
    write_c_str(caller, memory, ptr, text)?;
    Ok(ptr)
}
