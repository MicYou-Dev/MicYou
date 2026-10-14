//! Direct in-process tests of the host shim functions (no dlopen involved).
use micyou_plugin::abi::{self, mpl_host_api_t, mpl_log_level_t, mpl_result_t, NativeHostCtx};
use micyou_plugin::host::{AudioStateSnapshot, DeviceSnapshot, HostApi, MessageTarget};
use micyou_plugin::{PluginLogLevel, PluginResult};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DirectHost {
    config: Mutex<std::collections::HashMap<String, serde_json::Value>>,
    logs: Mutex<Vec<String>>,
    muted_state: Mutex<bool>,
    monitoring_state: Mutex<bool>,
    dsp_state: Mutex<String>,
}
impl HostApi for DirectHost {
    fn log(&self, _level: PluginLogLevel, message: &str) {
        self.logs.lock().unwrap().push(message.to_string());
    }
    fn get_config(&self, key: &str) -> Option<serde_json::Value> {
        self.config.lock().unwrap().get(key).cloned()
    }
    fn set_config(&self, _k: &str, _v: serde_json::Value) -> micyou_plugin::PluginResult<()> { Ok(()) }
    fn emit_event(&self, _t: &str, _p: serde_json::Value) -> micyou_plugin::PluginResult<()> { Ok(()) }
    fn send_message(&self, _t: MessageTarget, _p: Vec<u8>) -> micyou_plugin::PluginResult<()> { Ok(()) }
    fn audio_state(&self) -> AudioStateSnapshot { AudioStateSnapshot::default() }
    fn get_muted(&self) -> PluginResult<bool> {
        Ok(*self.muted_state.lock().unwrap())
    }
    fn set_muted(&self, muted: bool) -> PluginResult<()> {
        *self.muted_state.lock().unwrap() = muted;
        Ok(())
    }
    fn get_monitoring(&self) -> PluginResult<bool> {
        Ok(*self.monitoring_state.lock().unwrap())
    }
    fn set_monitoring(&self, enabled: bool) -> PluginResult<()> {
        *self.monitoring_state.lock().unwrap() = enabled;
        Ok(())
    }
    fn get_dsp_settings(&self) -> PluginResult<String> {
        let s = self.dsp_state.lock().unwrap();
        if s.is_empty() {
            Ok(r#"{"gain":0.0,"nsEnabled":false}"#.into())
        } else {
            Ok(s.clone())
        }
    }
    fn set_dsp_settings(&self, settings_json: &str) -> PluginResult<()> {
        *self.dsp_state.lock().unwrap() = settings_json.to_string();
        Ok(())
    }
    fn play_sound(&self, _path: &str) -> PluginResult<()> { Ok(()) }
    fn plugin_dir(&self) -> String { "/tmp/plugin-dir".to_string() }
    fn register_hotkey(&self, _s: &str) -> PluginResult<u64> { Ok(7) }
    fn open_window(&self, _p: &str) -> PluginResult<()> { Ok(()) }
    fn fs_read(&self, _path: &str) -> PluginResult<String> {
        Ok("mock fs content".into())
    }

    fn set_timeout(&self, _ms: u64, _payload: &str) -> PluginResult<u64> {
        Ok(7)
    }

    fn set_interval(&self, _ms: u64, _payload: &str) -> PluginResult<u64> {
        Ok(8)
    }

    fn clear_interval(&self, _id: u64) -> PluginResult<()> {
        Ok(())
    }

    fn open_url(&self, _url: &str) -> PluginResult<()> {
        Ok(())
    }

    fn notify(&self, _title: &str, _body: &str) -> PluginResult<()> {
        Ok(())
    }

    fn locale(&self) -> String {
        "zh-CN".into()
    }

    fn clipboard_read(&self) -> PluginResult<String> {
        Ok("mock clipboard".into())
    }

    fn clipboard_write(&self, _text: &str) -> PluginResult<()> {
        Ok(())
    }
    fn set_panel_icon(&self, _panel_id: &str, _icon: &str) -> PluginResult<()> { Ok(()) }

    fn host_info(&self) -> String {
        "{\"name\":\"micyou\",\"version\":\"test\",\"apiVersion\":1}".into()
    }

    fn http_request(
        &self,
        _method: &str,
        _url: &str,
        _headers_json: &str,
        _body: &str,
    ) -> PluginResult<u64> {
        Ok(9)
    }

    fn clear_timeout(&self, _id: u64) -> PluginResult<()> {
        Ok(())
    }

    fn fs_write(&self, _path: &str, _content: &str) -> PluginResult<()> {
        Ok(())
    }

    fn connected_devices(&self) -> Vec<DeviceSnapshot> { Vec::new() }
}

#[test]
fn shims_roundtrip_without_dlopen() {
    let host = Arc::new(DirectHost::default());
    host.config.lock().unwrap().insert("fixture.key".into(), serde_json::json!({"enabled":true}));
    let ctx = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![micyou_plugin::capabilities::CONFIG_READ.to_string()],
    });
    let table: mpl_host_api_t = abi::host_table_for(ctx);

    // get_config
    let mut buf = [0i8; 512];
    let mut size: u32 = 512;
    let key = std::ffi::CString::new("fixture.key").unwrap();
    let result = unsafe {
        (table.get_config)(table.ctx, key.as_ptr(), buf.as_mut_ptr(), &mut size)
    };
    assert_eq!(result, mpl_result_t::MPL_OK);
    let value = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy();
    assert!(value.contains("enabled"));

    // log
    let msg = std::ffi::CString::new("hello from shim test").unwrap();
    unsafe { (table.log)(table.ctx, mpl_log_level_t::MPL_LOG_INFO, msg.as_ptr()) };
    assert_eq!(host.logs.lock().unwrap().len(), 1);

    // capability denial
    let ctx_no_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: Vec::new(),
    });
    let table2: mpl_host_api_t = abi::host_table_for(ctx_no_cap);
    let result = unsafe { (table2.get_config)(table2.ctx, key.as_ptr(), buf.as_mut_ptr(), &mut size) };
    assert_eq!(result, mpl_result_t::MPL_ERR_PERMISSION);
    unsafe { abi::release_host_ctx(table.ctx) };
    unsafe { abi::release_host_ctx(table2.ctx) };
}

#[test]
fn release_host_ctx_keeps_arc_alive() {
    let host = Arc::new(DirectHost::default());
    let ctx = Arc::new(NativeHostCtx { host: host.clone(), capabilities: Vec::new() });
    let raw = Arc::into_raw(ctx);
    unsafe {
        abi::release_host_ctx(raw as *mut c_void);
    }
    assert!(Arc::strong_count(&host) >= 1);
}

#[test]
fn set_muted_respects_control_intercept_capability() {
    let host = Arc::new(DirectHost::default());
    let ctx_with_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![micyou_plugin::capabilities::CONTROL_INTERCEPT.to_string()],
    });
    let table = abi::host_table_for(ctx_with_cap);
    let res = unsafe { (table.set_muted)(table.ctx, 1) };
    assert_eq!(res, mpl_result_t::MPL_OK);
    assert!(*host.muted_state.lock().unwrap());

    let ctx_no_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![],
    });
    let table2 = abi::host_table_for(ctx_no_cap);
    let res2 = unsafe { (table2.set_muted)(table2.ctx, 0) };
    assert_eq!(res2, mpl_result_t::MPL_ERR_PERMISSION);
    assert!(*host.muted_state.lock().unwrap());

    unsafe { abi::release_host_ctx(table.ctx) };
    unsafe { abi::release_host_ctx(table2.ctx) };
}

#[test]
fn get_muted_respects_control_observe_capability() {
    let host = Arc::new(DirectHost::default());
    *host.muted_state.lock().unwrap() = true;

    let ctx_with_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![micyou_plugin::capabilities::CONTROL_OBSERVE.to_string()],
    });
    let table = abi::host_table_for(ctx_with_cap);
    let mut out_muted = 0u32;
    let res = unsafe { (table.get_muted)(table.ctx, &mut out_muted) };
    assert_eq!(res, mpl_result_t::MPL_OK);
    assert_eq!(out_muted, 1);

    let ctx_no_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![],
    });
    let table2 = abi::host_table_for(ctx_no_cap);
    let mut out_muted2 = 0u32;
    let res2 = unsafe { (table2.get_muted)(table2.ctx, &mut out_muted2) };
    assert_eq!(res2, mpl_result_t::MPL_ERR_PERMISSION);

    unsafe { abi::release_host_ctx(table.ctx) };
    unsafe { abi::release_host_ctx(table2.ctx) };
}

#[test]
fn monitoring_respects_capabilities() {
    let host = Arc::new(DirectHost::default());

    let ctx_full = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![
            micyou_plugin::capabilities::CONTROL_OBSERVE.to_string(),
            micyou_plugin::capabilities::CONTROL_INTERCEPT.to_string(),
        ],
    });
    let table = abi::host_table_for(ctx_full);

    // Set monitoring
    let res = unsafe { (table.set_monitoring)(table.ctx, 1) };
    assert_eq!(res, mpl_result_t::MPL_OK);
    assert!(*host.monitoring_state.lock().unwrap());

    // Get monitoring
    let mut out_enabled = 0u32;
    let res = unsafe { (table.get_monitoring)(table.ctx, &mut out_enabled) };
    assert_eq!(res, mpl_result_t::MPL_OK);
    assert_eq!(out_enabled, 1);

    let ctx_no_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![],
    });
    let table2 = abi::host_table_for(ctx_no_cap);
    let res_set = unsafe { (table2.set_monitoring)(table2.ctx, 0) };
    assert_eq!(res_set, mpl_result_t::MPL_ERR_PERMISSION);

    let res_get = unsafe { (table2.get_monitoring)(table2.ctx, &mut out_enabled) };
    assert_eq!(res_get, mpl_result_t::MPL_ERR_PERMISSION);

    unsafe { abi::release_host_ctx(table.ctx) };
    unsafe { abi::release_host_ctx(table2.ctx) };
}

#[test]
fn dsp_settings_respects_capabilities() {
    let host = Arc::new(DirectHost::default());

    let ctx_full = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![
            micyou_plugin::capabilities::CONTROL_OBSERVE.to_string(),
            micyou_plugin::capabilities::CONTROL_INTERCEPT.to_string(),
        ],
    });
    let table = abi::host_table_for(ctx_full);

    // Set DSP settings
    let json_input = std::ffi::CString::new(r#"{"gain":6.0,"nsEnabled":true}"#).unwrap();
    let res = unsafe { (table.set_dsp_settings)(table.ctx, json_input.as_ptr()) };
    assert_eq!(res, mpl_result_t::MPL_OK);
    assert_eq!(*host.dsp_state.lock().unwrap(), r#"{"gain":6.0,"nsEnabled":true}"#);

    // Get DSP settings
    let mut buf = [0i8; 128];
    let mut size = buf.len() as u32;
    let res = unsafe { (table.get_dsp_settings)(table.ctx, buf.as_mut_ptr(), &mut size) };
    assert_eq!(res, mpl_result_t::MPL_OK);
    let s = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
    assert!(s.contains("gain"));

    let ctx_no_cap = Arc::new(NativeHostCtx {
        host: host.clone(),
        capabilities: vec![],
    });
    let table2 = abi::host_table_for(ctx_no_cap);
    let res_set = unsafe { (table2.set_dsp_settings)(table2.ctx, json_input.as_ptr()) };
    assert_eq!(res_set, mpl_result_t::MPL_ERR_PERMISSION);

    let res_get = unsafe { (table2.get_dsp_settings)(table2.ctx, buf.as_mut_ptr(), &mut size) };
    assert_eq!(res_get, mpl_result_t::MPL_ERR_PERMISSION);

    unsafe { abi::release_host_ctx(table.ctx) };
    unsafe { abi::release_host_ctx(table2.ctx) };
}
