//! Native plugin loader integration tests.
//!
//! These load a *real* cdylib (`micyou-plugin-fixture-native`, built from
//! `fixtures/native/`) through `libloading`, exercising the versioned C ABI:
//! version handshake, id/version cross-check, host table callbacks, DSP
//! processing, event/message delivery and teardown.

use libloading::Library;
use micyou_plugin::host::{AudioStateSnapshot, DeviceSnapshot, HostApi, MessageTarget};
use micyou_plugin::manifest::{PluginKind, PluginManifest, RuntimeKind};
use micyou_plugin::plugin::{AudioFrameCtx, PluginEvent, PluginRuntime};
use micyou_plugin::{PluginLogLevel, PluginResult};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ── Mock host ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct MockHost {
    config: Mutex<std::collections::HashMap<String, serde_json::Value>>,
    log_lines: Mutex<Vec<String>>,
    emitted: Mutex<Vec<(String, serde_json::Value)>>,
    sent: Mutex<Vec<(MessageTarget, Vec<u8>)>>,
}

impl MockHost {
    fn new() -> Arc<Self> {
        let host = Arc::new(Self::default());
        host.config.lock().unwrap().insert(
            "fixture.key".into(),
            serde_json::json!({ "enabled": true, "gain": 2.0 }),
        );
        host
    }
}

impl HostApi for MockHost {
    fn log(&self, level: PluginLogLevel, message: &str) {
        self.log_lines
            .lock()
            .unwrap()
            .push(format!("{:?}: {message}", level));
    }
    fn get_config(&self, key: &str) -> Option<serde_json::Value> {
        self.config.lock().unwrap().get(key).cloned()
    }
    fn set_config(&self, key: &str, value: serde_json::Value) -> micyou_plugin::PluginResult<()> {
        self.config.lock().unwrap().insert(key.into(), value);
        Ok(())
    }
    fn emit_event(
        &self,
        topic: &str,
        payload: serde_json::Value,
    ) -> micyou_plugin::PluginResult<()> {
        self.emitted.lock().unwrap().push((topic.into(), payload));
        Ok(())
    }
    fn send_message(
        &self,
        target: MessageTarget,
        payload: Vec<u8>,
    ) -> micyou_plugin::PluginResult<()> {
        self.sent.lock().unwrap().push((target, payload));
        Ok(())
    }
    fn audio_state(&self) -> AudioStateSnapshot {
        AudioStateSnapshot::default()
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

    fn connected_devices(&self) -> Vec<DeviceSnapshot> {
        Vec::new()
    }
}

// ── Fixture discovery ──────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/micyou-plugin → workspace root (tauri-app)
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("fixture path layout changed")
        .to_path_buf()
}

/// Locate the compiled fixture cdylib. Cargo builds dev-dependencies before
/// tests, so the artifact always exists under target/<profile>/{,deps/}.
fn fixture_dylib() -> PathBuf {
    let target_dir = match std::env::var("CARGO_TARGET_DIR") {
        Ok(dir) => PathBuf::from(dir).join("debug"),
        Err(_) => workspace_root().join("target").join("debug"),
    };

    // Cargo replaces hyphens with underscores in the output file name.
    let base_name = "micyou_plugin_fixture_native";

    // Determine the exact file name based on the target OS.
    // Windows: my_crate.dll (No 'lib' prefix)
    // macOS:   libmy_crate.dylib
    // Linux:   libmy_crate.so
    let exact_file_name = if cfg!(target_os = "windows") {
        format!("{}.dll", base_name)
    } else if cfg!(target_os = "macos") {
        format!("lib{}.dylib", base_name)
    } else {
        format!("lib{}.so", base_name)
    };

    // 1. Try exact match in target/debug (Fastest and most reliable)
    let direct_path = target_dir.join(&exact_file_name);
    if direct_path.exists() {
        return direct_path;
    }

    // 2. Try exact match in target/debug/deps
    let deps_path = target_dir.join("deps").join(&exact_file_name);
    if deps_path.exists() {
        return deps_path;
    }

    // 3. Fallback to broad search in case Cargo appended hashes or used slightly different naming
    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in [target_dir.clone(), target_dir.join("deps")] {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Fix: Dynamically determine the valid prefix based on the OS
                let valid_prefix = if cfg!(target_os = "windows") {
                    name.starts_with(base_name)
                } else {
                    name.starts_with(&format!("lib{}", base_name))
                };

                if valid_prefix
                    && (name.ends_with(".so") || name.ends_with(".dylib") || name.ends_with(".dll"))
                {
                    candidates.push(entry.path());
                }
            }
        }
    }
    
    candidates.sort();
    candidates.into_iter().next().unwrap_or_else(|| {
        panic!(
            "fixture dylib not found under {} — build it with `cargo build -p micyou-plugin-fixture-native`",
            target_dir.display()
        )
    })
}

fn fixture_manifest() -> PluginManifest {
    PluginManifest {
        id: "test.native.minimal".to_string(),
        name: "Native Minimal".to_string(),
        version: "1.0.0".to_string(),
        author: None,
        description: None,
        runtime: RuntimeKind::Native,
        entry: "fixture_native".to_string(),
        platforms: Vec::new(),
        api_version: micyou_plugin::HOST_API_VERSION,
        capabilities: vec![
            micyou_plugin::capabilities::CONFIG_READ.to_string(),
            micyou_plugin::capabilities::CONFIG_WRITE.to_string(),
            micyou_plugin::capabilities::EVENT_EMIT.to_string(),
            micyou_plugin::capabilities::MESSAGE_SEND.to_string(),
        ],
        kind: PluginKind::Dsp,
        ui: None,
        dsp: None,
        config: None,
        ..Default::default()
    }
}
static COPY_MUTEX: Mutex<()> = Mutex::new(());

/// Build a plugin directory with the manifest + the artifact.
/// Returns (temp dir, artifact path, manifest with the correct entry name).
fn stage_fixture() -> (tempfile_dir::TempDir, PathBuf, PluginManifest) {
    let _lock = COPY_MUTEX.lock().unwrap();

    let dir = tempfile_dir::TempDir::new("micyou-native-fixture");
    let dylib = fixture_dylib();
    
    let ext = dylib
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let entry_name = format!("fixture_native.{ext}");
    let staged_path = dir.path().join(&entry_name);
    let mut attempts = 0;
    loop {
        match std::fs::copy(&dylib, &staged_path) {
            Ok(_) => break,
            Err(e) if e.raw_os_error() == Some(32) && attempts < 5 => {
                // Error 32: The process cannot access the file because it is being used by another process.
                attempts += 1;
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("Failed to copy fixture dylib after retries: {}", e),
        }
    }

    let mut manifest = fixture_manifest();
    manifest.entry = entry_name;
    std::fs::write(
        dir.path().join("plugin.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    (dir, staged_path, manifest)
}

// tiny tempdir helper (avoids an extra dev-dependency)
mod tempfile_dir {
    use std::path::{Path, PathBuf};

    pub struct TempDir {
        path: PathBuf,
    }
    impl TempDir {
        pub fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }
        pub fn path(&self) -> &Path {
            &self.path
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

fn test_helpers(dylib: &PathBuf) -> Library {
    unsafe { Library::new(dylib).expect("re-open fixture for test helpers") }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn native_plugin_loads_and_processes_audio() {
    let (dir, dylib, manifest) = stage_fixture();
    let host = MockHost::new();
    let mut instance =
        micyou_plugin::native::load_native_instance(manifest.clone(), dir.path(), host.clone())
            .expect("fixture must load");
    assert_eq!(instance.id(), "test.native.minimal");
    assert_eq!(instance.runtime_kind(), RuntimeKind::Native);

    // Gain 2.0 via the test helper, then process a frame.
    let helpers = test_helpers(&dylib);
    unsafe {
        helpers
            .get::<unsafe extern "C" fn(f64)>(b"test_native_set_gain\0")
            .unwrap()(2.0);
    }

    let mut data = vec![0.1f32, -0.2, 0.3];
    let mut ctx = AudioFrameCtx {
        data: &mut data,
        channels: 1,
        sample_rate: 48000,
        queued_ms: 10.0,
        node_index: None,
    };
    let status = instance.process_audio(&mut ctx).expect("process ok");
    assert_eq!(status, micyou_plugin::ProcessStatus::Ok);
    assert_eq!(data, vec![0.2, -0.4, 0.6]);

    // Bypass path: gain <= 0 means the plugin sets bypass=1.
    unsafe {
        helpers
            .get::<unsafe extern "C" fn(f64)>(b"test_native_set_gain\0")
            .unwrap()(-1.0);
    }
    let mut data2 = vec![1.0f32, 2.0];
    let mut ctx2 = AudioFrameCtx {
        data: &mut data2,
        channels: 1,
        sample_rate: 48000,
        queued_ms: 10.0,
        node_index: None,
    };
    let status = instance.process_audio(&mut ctx2).unwrap();
    assert_eq!(status, micyou_plugin::ProcessStatus::Bypass);
    assert_eq!(data2, vec![1.0, 2.0]); // untouched
}

#[test]
fn native_plugin_delivers_events_and_messages() {
    let (dir, dylib, manifest) = stage_fixture();
    let host = MockHost::new();
    let mut instance =
        micyou_plugin::native::load_native_instance(manifest, dir.path(), host.clone()).unwrap();

    instance
        .handle_event(&PluginEvent::MuteChanged { muted: true })
        .unwrap();
    instance
        .handle_event(&PluginEvent::DeviceConnected {
            mode: "wifi".into(),
            label: "phone".into(),
        })
        .unwrap();
    instance
        .handle_message("test.native.peer", "plugin:test.native.minimal", b"hi")
        .unwrap();

    let helpers = test_helpers(&dylib);
    unsafe {
        let events = helpers
            .get::<unsafe extern "C" fn() -> i32>(b"test_native_events\0")
            .unwrap()();
        let messages = helpers
            .get::<unsafe extern "C" fn() -> i32>(b"test_native_messages\0")
            .unwrap()();
        assert_eq!(events, 2);
        assert_eq!(messages, 1);
    }
}

#[test]
fn native_plugin_host_callbacks_work() {
    let (dir, dylib, manifest) = stage_fixture();
    let host = MockHost::new();
    let instance =
        micyou_plugin::native::load_native_instance(manifest, dir.path(), host.clone()).unwrap();

    // The fixture's test_native_host_call reads "fixture.key" through the host
    // table of the shared module (HOST is set by init while the instance lives).
    let helpers = test_helpers(&dylib);
    unsafe {
        let code = helpers
            .get::<unsafe extern "C" fn() -> i32>(b"test_native_host_call\0")
            .unwrap()();
        assert_eq!(code, 0, "host call must return MPL_OK");
    }
    // Scope the guard: `drop(instance)` runs plugin deinit, which calls back
    // into HostApi::log — a held std MutexGuard on the same thread would
    // self-deadlock (std Mutex is not reentrant).
    {
        let logs = host.log_lines.lock().unwrap();
        assert!(
            logs.iter()
                .any(|l| l.contains("fixture config") && l.contains("enabled")),
            "expected fixture config log, got: {logs:?}"
        );
    }
    drop(instance); // exercises deinit path
}

#[test]
fn native_plugin_rejects_id_mismatch_and_wrong_abi() {
    // manifest id differs from the fixture's internal id
    let (dir, _dylib, mut manifest) = stage_fixture();
    manifest.id = "test.native.other".to_string();
    let host = MockHost::new();
    let result = micyou_plugin::native::load_native_instance(manifest, dir.path(), host.clone());
    assert!(matches!(
        result,
        Err(micyou_plugin::PluginError::Validation(_))
    ));

    // wrong host API version
    let (dir2, _dylib2, mut manifest) = stage_fixture();
    manifest.api_version = 99;
    let result = micyou_plugin::native::load_native_instance(manifest, dir2.path(), host.clone());
    assert!(matches!(
        result,
        Err(micyou_plugin::PluginError::ApiVersionMismatch { plugin: 99, .. })
    ));
}

#[test]
fn native_plugin_missing_entry_reports_not_found() {
    let host = MockHost::new();
    let result = micyou_plugin::native::load_native_instance(
        fixture_manifest(),
        PathBuf::from("/nonexistent/plugin/dir").as_path(),
        host.clone(),
    );
    assert!(matches!(
        result,
        Err(micyou_plugin::PluginError::NotFound(_))
    ));
}

#[test]
fn native_plugin_deinit_is_called_once() {
    let (dir, _dylib, manifest) = stage_fixture();
    let host = MockHost::new();
    let instance =
        micyou_plugin::native::load_native_instance(manifest, dir.path(), host.clone()).unwrap();
    drop(instance); // triggers deinit through Drop
    let logs = host.log_lines.lock().unwrap();
    let deinit_logs = logs.iter().filter(|l| l.contains("deinitialized")).count();
    assert_eq!(deinit_logs, 1, "deinit must run exactly once: {logs:?}");
}
