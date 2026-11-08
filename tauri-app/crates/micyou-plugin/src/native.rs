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

//! Native plugin runtime: loads platform cdylibs through `libloading` and
//! translates the versioned C ABI (`abi.rs` / `include/micyou_plugin_abi.h`)
//! into the unified `PluginRuntime` contract.

use crate::abi::{
    self, mpl_host_api_t, mpl_plugin_info_t, mpl_result_t, NativeHostCtx, MPL_ABI_VERSION,
};
use crate::error::{PluginError, PluginResult};
use crate::host::HostApi;
use crate::manifest::PluginManifest;
use crate::plugin::{AudioFrameCtx, PluginEvent, PluginInstance, PluginRuntime, ProcessStatus};
use libloading::{Library, Symbol};
use std::ffi::{c_char, CStr, CString};
use std::path::Path;
use std::sync::Arc;

type InfoFn = unsafe extern "C" fn() -> *const mpl_plugin_info_t;
type InitFn = unsafe extern "C" fn(*const mpl_host_api_t) -> mpl_result_t;
type DeinitFn = unsafe extern "C" fn();
type ProcessFn = unsafe extern "C" fn(*mut f32, u32, u32, f64, *mut u32) -> mpl_result_t;
type ProcessNodeFn =
    unsafe extern "C" fn(*mut f32, u32, u32, f64, u32, *mut u32) -> mpl_result_t;
type EventFn = unsafe extern "C" fn(*const c_char, *const c_char) -> mpl_result_t;
type MessageFn = unsafe extern "C" fn(*const c_char, *const c_char, *const u8, u32) -> mpl_result_t;

/// A loaded native plugin. Keeps the `Library` (and thus the exported
/// function pointers) alive for its whole lifetime.
///
/// # Safety
/// `Symbol<'static>` pointers are obtained by transmuting the borrowed
/// symbols; this is sound because the `Library` field is dropped *after* the
/// symbols and no symbol is ever called after drop.
pub struct NativePlugin {
    manifest: PluginManifest,
    _lib: Library,
    /// Kept alive so the raw `ctx` pointer inside `host_table` stays valid for
    /// the plugin's lifetime (the field itself is only a drop-guard).
    #[allow(dead_code)]
    host_ctx: Arc<NativeHostCtx>,
    host_table: mpl_host_api_t,
    f_init: Symbol<'static, InitFn>,
    f_deinit: Symbol<'static, DeinitFn>,
    f_process: Option<Symbol<'static, ProcessFn>>,
    f_process_node: Option<Symbol<'static, ProcessNodeFn>>,
    f_event: Option<Symbol<'static, EventFn>>,
    f_message: Option<Symbol<'static, MessageFn>>,
}

// The raw pointers inside `host_table`/`host_ctx` are only dereferenced on the
// thread that holds `&mut NativePlugin` (the plugin call happens synchronously
// inside `process_audio` etc), so the instance is safe to move across threads.
unsafe impl Send for NativePlugin {}

impl NativePlugin {
    /// Load a native plugin from `<plugin_dir>/<manifest.entry>`.
    pub fn load(
        manifest: PluginManifest,
        plugin_dir: &Path,
        host: Arc<dyn HostApi>,
    ) -> PluginResult<Self> {
        let entry = manifest.entry_path(plugin_dir);
        if !entry.exists() {
            return Err(PluginError::NotFound(format!(
                "Entry artifact not found: {}. For cross-platform native plugins, ensure the base name matches '{}' and the file has the correct platform suffix (.so/.dylib/.dll) without 'lib' prefix.", 
                entry.display(), manifest.entry
            )));
        }

        unsafe {
            if manifest.api_version < crate::manifest::MIN_SUPPORTED_API_VERSION
                || manifest.api_version > crate::manifest::HOST_API_VERSION
            {
                return Err(PluginError::ApiVersionMismatch {
                    plugin: manifest.api_version,
                    host: crate::manifest::HOST_API_VERSION,
                });
            }

            let lib = Library::new(&entry)
                .map_err(|e| PluginError::LoadFailed(format!("{}: {e}", entry.display())))?;

            // ── Version handshake ──
            let info_fn: Symbol<'_, InfoFn> = lib
                .get(b"micyou_plugin_info\0")
                .map_err(|e| PluginError::LoadFailed(format!("missing micyou_plugin_info: {e}")))?;
            let info_ptr = info_fn();
            if info_ptr.is_null() {
                return Err(PluginError::LoadFailed(
                    "micyou_plugin_info returned NULL".into(),
                ));
            }
            let info = &*info_ptr;
            if info.abi_version != MPL_ABI_VERSION {
                return Err(PluginError::LoadFailed(format!(
                    "ABI version mismatch: plugin {}, host {MPL_ABI_VERSION}",
                    info.abi_version
                )));
            }
            if info.api_version < crate::manifest::MIN_SUPPORTED_API_VERSION
                || info.api_version > crate::manifest::HOST_API_VERSION
            {
                return Err(PluginError::ApiVersionMismatch {
                    plugin: info.api_version,
                    host: crate::manifest::HOST_API_VERSION,
                });
            }
            if !info.id.is_null() {
                let plugin_id = CStr::from_ptr(info.id).to_string_lossy();
                if plugin_id != manifest.id {
                    return Err(PluginError::Validation(format!(
                        "native id {plugin_id:?} does not match manifest id {:?}",
                        manifest.id
                    )));
                }
            }

            // ── Optional entry points ──
            let f_process = get_optional::<ProcessFn>(&lib, b"micyou_plugin_process\0")
                .map(|s| transmute_symbol(s));
            let f_process_node =
                get_optional::<ProcessNodeFn>(&lib, b"micyou_plugin_process_node\0")
                    .map(|s| transmute_symbol(s));
            let f_event = get_optional::<EventFn>(&lib, b"micyou_plugin_handle_event\0")
                .map(|s| transmute_symbol(s));
            let f_message = get_optional::<MessageFn>(&lib, b"micyou_plugin_handle_message\0")
                .map(|s| transmute_symbol(s));

            let f_init: Symbol<'_, InitFn> = lib
                .get(b"micyou_plugin_init\0")
                .map_err(|e| PluginError::LoadFailed(format!("missing micyou_plugin_init: {e}")))?;
            let f_deinit: Symbol<'_, DeinitFn> =
                lib.get(b"micyou_plugin_deinit\0").map_err(|e| {
                    PluginError::LoadFailed(format!("missing micyou_plugin_deinit: {e}"))
                })?;
            let f_init: Symbol<'static, InitFn> = transmute_symbol(f_init);
            let f_deinit: Symbol<'static, DeinitFn> = transmute_symbol(f_deinit);

            // ── Host table ──
            let host_ctx = Arc::new(NativeHostCtx {
                host,
                capabilities: manifest.capabilities.clone(),
            });
            let host_table = abi::host_table_for(host_ctx.clone());

            let plugin = NativePlugin {
                manifest,
                _lib: lib,
                host_ctx,
                host_table,
                f_init,
                f_deinit,
                f_process,
                f_process_node,
                f_event,
                f_message,
            };

            // ── init ──
            let code = (plugin.f_init)(&plugin.host_table);
            if code != mpl_result_t::MPL_OK {
                return Err(abi::result_from_code(code, "micyou_plugin_init").unwrap_err());
            }
            Ok(plugin)
        }
    }
}

unsafe fn get_optional<'a, T>(lib: &'a Library, name: &'a [u8]) -> Option<Symbol<'a, T>> {
    unsafe { lib.get(name).ok() }
}

unsafe fn transmute_symbol<T>(symbol: Symbol<'_, T>) -> Symbol<'static, T> {
    unsafe { std::mem::transmute::<Symbol<'_, T>, Symbol<'static, T>>(symbol) }
}

impl Drop for NativePlugin {
    fn drop(&mut self) {
        // Symbols (which live inside the Library) are dropped with the struct
        // before the Library field is dropped (declaration order).
        unsafe {
            (self.f_deinit)();
            abi::release_host_ctx(self.host_table.ctx);
        }
    }
}

impl PluginRuntime for NativePlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn init(&mut self, _host: &dyn HostApi) -> PluginResult<()> {
        // init already happened inside `load` (the host table must exist before
        // the plugin can use host callbacks).
        Ok(())
    }

    fn deinit(&mut self) {
        // Handled by Drop.
    }

    fn process_audio(&mut self, ctx: &mut AudioFrameCtx<'_>) -> PluginResult<ProcessStatus> {
        let mut bypass: u32 = 0;
        // A multi-node host passes the chain occurrence index; prefer the
        // extended export so the plugin can address each node separately.
        if let (Some(f_process_node), Some(node_index)) = (&self.f_process_node, ctx.node_index) {
            let code = unsafe {
                f_process_node(
                    ctx.data.as_mut_ptr(),
                    ctx.data.len() as u32,
                    ctx.channels as u32,
                    ctx.queued_ms,
                    node_index as u32,
                    &mut bypass,
                )
            };
            abi::result_from_code(code, "micyou_plugin_process_node")?;
            return Ok(if bypass != 0 {
                ProcessStatus::Bypass
            } else {
                ProcessStatus::Ok
            });
        }
        let Some(f_process) = &self.f_process else {
            return Ok(ProcessStatus::Bypass);
        };
        let code = unsafe {
            f_process(
                ctx.data.as_mut_ptr(),
                ctx.data.len() as u32,
                ctx.channels as u32,
                ctx.queued_ms,
                &mut bypass,
            )
        };
        abi::result_from_code(code, "micyou_plugin_process")?;
        Ok(if bypass != 0 {
            ProcessStatus::Bypass
        } else {
            ProcessStatus::Ok
        })
    }

    fn handle_event(&mut self, event: &PluginEvent) -> PluginResult<()> {
        let Some(f_event) = &self.f_event else {
            return Ok(());
        };
        let event_type =
            serde_json::to_string(event).map_err(|e| PluginError::Runtime(e.to_string()))?;
        let type_name = CString::new(match event {
            PluginEvent::DeviceConnected { .. } => "device_connected",
            PluginEvent::DeviceDisconnected => "device_disconnected",
            PluginEvent::MuteChanged { .. } => "mute_changed",
            PluginEvent::MonitoringChanged { .. } => "monitoring_changed",
            PluginEvent::DspSettingsChanged => "dsp_settings_changed",
            PluginEvent::StateChanged { .. } => "state_changed",
        })
        .unwrap_or_default();
        let payload = CString::new(event_type).unwrap_or_default();
        let code = unsafe { f_event(type_name.as_ptr(), payload.as_ptr()) };
        abi::result_from_code(code, "micyou_plugin_handle_event")
    }

    fn handle_message(&mut self, source: &str, topic: &str, payload: &[u8]) -> PluginResult<()> {
        let Some(f_message) = &self.f_message else {
            return Ok(());
        };
        let source_c = CString::new(source).map_err(|e| PluginError::Runtime(e.to_string()))?;
        let topic_c = CString::new(topic).map_err(|e| PluginError::Runtime(e.to_string()))?;
        let code = unsafe {
            f_message(
                source_c.as_ptr(),
                topic_c.as_ptr(),
                payload.as_ptr(),
                payload.len() as u32,
            )
        };
        abi::result_from_code(code, "micyou_plugin_handle_message")
    }
}

/// Convenience entry: load a native plugin and wrap it as a `PluginInstance`.
pub fn load_native_instance(
    manifest: PluginManifest,
    plugin_dir: &Path,
    host: Arc<dyn HostApi>,
) -> PluginResult<PluginInstance> {
    Ok(PluginInstance::Native(Box::new(NativePlugin::load(
        manifest, plugin_dir, host,
    )?)))
}

/// Utility used by tests: read a C string.
#[allow(dead_code)]
unsafe fn cstr_ptr<'a>(ptr: *const c_char) -> &'a str {
    unsafe { CStr::from_ptr(ptr).to_str().unwrap_or_default() }
}
