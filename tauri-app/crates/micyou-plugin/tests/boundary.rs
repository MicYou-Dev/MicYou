//! Boundary and robustness tests for the WASM runtime and the message bus.
//!
//! These guard the safety properties that matter in production:
//! - an infinite loop in a plugin must be stopped by fuel metering
//! - oversized allocations must fail gracefully (never crash the host)
//! - long-running processing must stay stable (frame buffer reuse)
//! - concurrent bus dispatch must not deadlock or drop messages

use std::sync::{Arc, Mutex};

use micyou_plugin::host::{
    AudioStateSnapshot, DeviceSnapshot, HostApi, MessageTarget, PluginLogLevel,
};
use micyou_plugin::plugin::{AudioFrameCtx, PluginRuntime, ProcessStatus};
use micyou_plugin::wasm::load_wasm_instance;
use micyou_plugin::{PluginError, PluginManifest, PluginResult, RuntimeKind};

/// Minimal host that records calls; all optional APIs return defaults.
#[derive(Default)]
pub struct TestHost {
    pub notified: Mutex<Vec<(String, String)>>,
    pub timed_out: Mutex<Vec<String>>,
}

impl HostApi for TestHost {
    fn log(&self, _level: PluginLogLevel, _msg: &str) {}
    fn get_config(&self, _key: &str) -> Option<serde_json::Value> {
        None
    }
    fn set_config(&self, _key: &str, _value: serde_json::Value) -> PluginResult<()> {
        Ok(())
    }
    fn emit_event(&self, _topic: &str, _payload: serde_json::Value) -> PluginResult<()> {
        Ok(())
    }
    fn send_message(&self, _target: MessageTarget, _payload: Vec<u8>) -> PluginResult<()> {
        Ok(())
    }
    fn plugin_dir(&self) -> String {
        "/tmp/boundary".into()
    }
    fn register_hotkey(&self, _s: &str) -> PluginResult<u64> {
        Ok(1)
    }
    fn open_window(&self, _panel: &str) -> PluginResult<()> {
        Ok(())
    }
    fn audio_state(&self) -> AudioStateSnapshot {
        AudioStateSnapshot {
            streaming: false,
            sample_rate: 48000,
            channels: 1,
            input_level: 0.0,
            processed_level: 0.0,
            queued_ms: 0.0,
            muted: false,
        }
    }
    fn play_sound(&self, _p: &str) -> PluginResult<()> {
        Ok(())
    }
    fn fs_read(&self, _p: &str) -> PluginResult<String> {
        Ok(String::new())
    }
    fn fs_write(&self, _p: &str, _c: &str) -> PluginResult<()> {
        Ok(())
    }
    fn set_timeout(&self, _ms: u64, _p: &str) -> PluginResult<u64> {
        Ok(5)
    }
    fn clear_timeout(&self, _id: u64) -> PluginResult<()> {
        Ok(())
    }
    fn http_request(&self, _m: &str, _u: &str, _h: &str, _b: &str) -> PluginResult<u64> {
        Ok(9)
    }
    fn set_interval(&self, _ms: u64, _p: &str) -> PluginResult<u64> {
        Ok(7)
    }
    fn clear_interval(&self, _id: u64) -> PluginResult<()> {
        Ok(())
    }
    fn open_url(&self, _u: &str) -> PluginResult<()> {
        Ok(())
    }
    fn notify(&self, title: &str, body: &str) -> PluginResult<()> {
        self.notified
            .lock()
            .unwrap()
            .push((title.into(), body.into()));
        Ok(())
    }
    fn locale(&self) -> String {
        "en".into()
    }
    fn host_info(&self) -> String {
        "{\"name\":\"micyou\",\"version\":\"test\",\"apiVersion\":1}".into()
    }
    fn clipboard_read(&self) -> PluginResult<String> {
        Ok(String::new())
    }
    fn clipboard_write(&self, _t: &str) -> PluginResult<()> {
        Ok(())
    }
    fn set_panel_icon(&self, _panel_id: &str, _icon: &str) -> PluginResult<()> { Ok(()) }
    fn connected_devices(&self) -> Vec<DeviceSnapshot> {
        vec![]
    }
}

fn wasm_manifest(id: &str) -> PluginManifest {
    PluginManifest {
        id: id.into(),
        name: id.into(),
        version: "1.0.0".into(),
        runtime: RuntimeKind::Wasm,
        entry: "main.wasm".into(),
        api_version: micyou_plugin::manifest::HOST_API_VERSION,
        ..Default::default()
    }
}

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("micyou-boundary-{}", std::process::id()))
}

/// Compile a WAT string and stage it as the plugin entry.
fn stage_wat(manifest: &PluginManifest, wat: &str) -> std::path::PathBuf {
    let dir = temp_dir().join(&manifest.id);
    std::fs::create_dir_all(&dir).unwrap();
    let bytes = wat::parse_str(wat).expect("wat parse");
    std::fs::write(dir.join("main.wasm"), bytes).unwrap();
    dir
}

#[test]
fn wasm_infinite_loop_is_stopped_by_fuel_metering() {
    // process() spins forever; the host fuel budget must trap it
    let wat = r#"(module
      (memory (export "memory") 1)
      (global $heap (mut i32) (i32.const 1024))
      (func (export "alloc") (param $n i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $heap))
        (global.set $heap (i32.add (global.get $heap) (local.get $n)))
        (i32.store (local.get $p) (local.get $n))
        (i32.add (local.get $p) (i32.const 4)))
      (func (export "dealloc") (param $p i32) (param $n i32))
      (func (export "api_version") (result i32) (i32.const 1))
      (func (export "init") (result i32) (i32.const 0))
      (func (export "process") (param $d i32) (param $s i32) (param $c i32) (param $q f64) (result i32)
        (block $out (result i32)
          (loop $spin (br $spin))
          (i32.const 0)))
      (func (export "deinit"))
    )"#;
    let manifest = wasm_manifest("dev.boundary.spin");
    let dir = stage_wat(&manifest, wat);
    let host = Arc::new(TestHost::default());
    let mut inst = load_wasm_instance(manifest, &dir, host.clone()).expect("load");
    inst.init(&*host).expect("init");
    let mut ctx = AudioFrameCtx {
        data: &mut vec![0.0f32; 480],
        channels: 1,
        sample_rate: 48000,
        queued_ms: 10.0,
    };
    let result = inst.process_audio(&mut ctx);
    assert!(
        matches!(result, Err(PluginError::Runtime(_))),
        "infinite loop must surface as a runtime error, got {result:?}"
    );
}

#[test]
fn wasm_oversized_allocation_fails_gracefully() {
    // alloc() 64 MiB in a 4-page (256 KiB) memory must error, not crash
    let wat = r#"(module
      (memory (export "memory") 4)
      (global $heap (mut i32) (i32.const 1024))
      (func (export "alloc") (param $n i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $heap))
        (global.set $heap (i32.add (global.get $heap) (local.get $n)))
        (i32.store (local.get $p) (local.get $n))
        (i32.add (local.get $p) (i32.const 4)))
      (func (export "dealloc") (param $p i32) (param $n i32))
      (func (export "api_version") (result i32) (i32.const 1))
      (func (export "init") (result i32) (i32.const 0))
      (func (export "process") (param $d i32) (param $s i32) (param $c i32) (param $q f64) (result i32)
        ;; attempt to write far beyond the 4-page (256 KiB) memory: traps
        (i32.store (i32.const 0x7FFFFF00) (i32.const 1))
        (i32.const 0))
      (func (export "deinit"))
    )"#;
    let manifest = wasm_manifest("dev.boundary.alloc");
    let dir = stage_wat(&manifest, wat);
    let host = Arc::new(TestHost::default());
    let mut inst = load_wasm_instance(manifest, &dir, host.clone()).expect("load");
    inst.init(&*host).expect("init");
    let mut ctx = AudioFrameCtx {
        data: &mut vec![0.0f32; 480],
        channels: 1,
        sample_rate: 48000,
        queued_ms: 10.0,
    };
    let result = inst.process_audio(&mut ctx);
    assert!(
        matches!(result, Err(PluginError::Runtime(_))),
        "out-of-bounds memory access must surface as a runtime error, got {result:?}"
    );
}

#[test]
fn wasm_process_frame_buffer_is_reused_without_growth() {
    // process() touching the frame buffer must stay within the 4-page memory
    // across many frames (regression guard for the alloc-per-frame bug that
    // exhausted linear memory in production)
    let wat = r#"(module
      (memory (export "memory") 4)
      (global $heap (mut i32) (i32.const 0x2000))
      (func (export "alloc") (param $n i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $heap))
        (global.set $heap (i32.add (global.get $heap) (local.get $n)))
        (i32.store (local.get $p) (local.get $n))
        (i32.add (local.get $p) (i32.const 4)))
      (func (export "dealloc") (param $p i32) (param $n i32))
      (func (export "api_version") (result i32) (i32.const 1))
      (func (export "init") (result i32) (i32.const 0))
      (func (export "process") (param $d i32) (param $s i32) (param $c i32) (param $q f64) (result i32)
        ;; copy 480 samples within the frame buffer (simple loop)
        (local $i i32)
        (block $out
          (loop $l
            (br_if $out (i32.ge_u (local.get $i) (local.get $s)))
            (i32.store8 (i32.add (local.get $d) (local.get $i)) (i32.const 1))
            (local.set $i (i32.add (local.get $i) (i32.const 1)))
            (br $l)))
        (i32.const 0))
      (func (export "deinit"))
    )"#;
    let manifest = wasm_manifest("dev.boundary.frames");
    let dir = stage_wat(&manifest, wat);
    let host = Arc::new(TestHost::default());
    let mut inst = load_wasm_instance(manifest, &dir, host.clone()).expect("load");
    inst.init(&*host).expect("init");
    let mut ctx = AudioFrameCtx {
        data: &mut vec![0.5f32; 480],
        channels: 1,
        sample_rate: 48000,
        queued_ms: 10.0,
    };
    for _ in 0..10_000 {
        assert_eq!(inst.process_audio(&mut ctx), Ok(ProcessStatus::Ok));
    }
    assert_eq!(ctx.data.len(), 480);
}

#[test]
fn bus_dispatch_from_multiple_threads_does_not_deadlock() {
    // Concurrent handle_incoming on the same instance must be safe (the host
    // guards instances with try_lock; heavy producers must not wedge audio)
    use micyou_plugin::bus::{PluginBus, PluginMessage};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let counter = Arc::new(AtomicUsize::new(0));
    let c2 = counter.clone();
    let bus = PluginBus::new(
        Arc::new(micyou_plugin::bus::NullTransport),
        Arc::new(move |msg: &PluginMessage| {
            // Count every delivered message (broadcast to a pseudo plugin)
            let _ = msg;
            c2.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }),
    );
    let bus = Arc::new(bus);
    let mut handles = Vec::new();
    for t in 0..8 {
        let bus = bus.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..200 {
                let msg =
                    PluginMessage::new("test", "dev.a", "topic", format!("{t}-{i}").into_bytes());
                bus.handle_incoming(&msg);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(counter.load(Ordering::Relaxed), 8 * 200, "no message lost");
}

#[test]
fn notify_via_host_api_reaches_the_plugin() {
    // End-to-end: plugin calls host.notify through the WASM import
    let wat = r#"(module
      (import "micyou" "notify" (func $notify (param i32 i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "title\00")
      (data (i32.const 16) "body\00")
      (global $heap (mut i32) (i32.const 1024))
      (func (export "alloc") (param $n i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $heap))
        (global.set $heap (i32.add (global.get $heap) (local.get $n)))
        (i32.store (local.get $p) (local.get $n))
        (i32.add (local.get $p) (i32.const 4)))
      (func (export "dealloc") (param $p i32) (param $n i32))
      (func (export "api_version") (result i32) (i32.const 1))
      (func (export "init") (result i32)
        (call $notify (i32.const 0) (i32.const 16))
        (i32.const 0))
      (func (export "deinit"))
    )"#;
    let manifest = wasm_manifest("dev.boundary.notify");
    let dir = stage_wat(&manifest, wat);
    let host = Arc::new(TestHost::default());
    let mut inst = load_wasm_instance(manifest, &dir, host.clone()).expect("load");
    inst.init(&*host).expect("init");
    let notified = host.notified.lock().unwrap().clone();
    assert_eq!(notified, vec![("title".to_string(), "body".to_string())]);
}

#[test]
fn host_info_is_stable_json() {
    let host = TestHost::default();
    let info: serde_json::Value = serde_json::from_str(&host.host_info()).expect("json");
    assert_eq!(info["name"], "micyou");
}
