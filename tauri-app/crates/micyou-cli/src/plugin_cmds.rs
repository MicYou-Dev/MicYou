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

//! `micyou-cli plugin` — plugin development toolkit.
//!
//! Subcommands:
//! - `validate <dir>`  validate a plugin directory's plugin.json
//! - `package <dir>`   pack a plugin directory into an importable .zip
//! - `create <id>`     scaffold a new plugin (wasm or native skeleton)

use std::io::Write as _;
use std::path::Path;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum PluginAction {
    /// 校验插件目录中的 plugin.json（结构、版本、能力、平台）
    Validate {
        /// 插件目录（含 plugin.json）
        dir: String,
        /// 以 JSON 输出校验结果（CI 集成用）
        #[arg(long)]
        json: bool,
    },
    /// 将插件目录打包为可导入的 .zip（根目录含 plugin.json）
    Package {
        /// 插件目录
        dir: String,
        /// 输出 zip 路径（默认 <plugin_id>.zip）
        #[arg(short, long)]
        out: Option<String>,
    },
    /// 生成新插件骨架（wasm 或 native 模板）
    Create {
        /// 插件 id（反向域名，如 dev.micyou.myplugin）
        id: String,
        /// 运行时：wasm（默认，沙箱安全）| native（高级 DSP/系统集成）
        #[arg(long, value_parser = ["wasm", "native"], default_value = "wasm")]
        runtime: String,
        /// 插件显示名（默认取自 id）
        #[arg(long)]
        name: Option<String>,
        /// 插件类型：utility（默认）| dsp（处理链节点）| ui
        #[arg(long, value_parser = ["utility", "dsp", "ui"], default_value = "utility")]
        kind: String,
        /// 能力列表（逗号分隔，如 config.read,config.write）
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,
        /// WASM 源码语言：rust（默认，推荐从高级语言编译）| wat
        #[arg(long, value_parser = ["rust", "wat"], default_value = "rust")]
        lang: String,
        /// 输出目录（默认 ./<id 最后一段>）
        #[arg(short, long)]
        out: Option<String>,
    },
    /// 安装插件目录到应用插件目录（构建后一键部署）
    Install {
        /// 插件目录（含 plugin.json 与构建产物）
        dir: String,
    },
    /// 开发模式：监听插件目录变更并自动重新安装（Ctrl+C 退出）
    Dev {
        /// 插件目录
        dir: String,
        /// 监听间隔秒（默认 1.5）
        #[arg(short, long, default_value = "1.5")]
        interval: f64,
    },
    /// 递增插件 manifest 版本（patch 默认，或指定完整 semver）
    Bump {
        /// 插件目录（含 plugin.json）
        dir: String,
        /// 新版本（如 1.2.0），缺省则 patch +1
        version: Option<String>,
    },
    /// 列出已安装插件（id、版本、运行时、状态）
    List,
    /// 启用指定插件
    Enable {
        /// 插件 ID（如 dev.micyou.example.soundpad）
        id: String,
    },
    /// 禁用指定插件
    Disable {
        /// 插件 ID（如 dev.micyou.example.soundpad）
        id: String,
    },
}

pub fn run(action: PluginAction) -> Result<(), String> {
    match action {
        PluginAction::Validate { dir, json: _json } => validate(&dir),
        PluginAction::Package { dir, out } => package(&dir, out.as_deref()),
        PluginAction::Create {
            id,
            runtime,
            name,
            kind,
            capabilities,
            lang,
            out,
        } => create(
            &id,
            &runtime,
            name.as_deref(),
            &kind,
            &capabilities,
            &lang,
            out.as_deref(),
        ),
        PluginAction::Install { dir } => install(&dir),
        PluginAction::Dev { dir, interval } => dev(&dir, interval),
        PluginAction::Bump { dir, version } => bump(&dir, version.as_deref()),
        PluginAction::List => list_installed(),
        PluginAction::Enable { id } => enable_plugin(&id),
        PluginAction::Disable { id } => disable_plugin(&id),
    }
}

/// 列出已安装插件
fn list_installed() -> Result<(), String> {
    let plugins_dir = micyou_core::config::config_dir()
        .join("plugins");
    if !plugins_dir.exists() {
        println!("(插件目录不存在: {})", plugins_dir.display());
        return Ok(());
    }
    let mut rows: Vec<(String, String, String, bool)> = Vec::new();
    let mut total = 0usize;
    for entry in std::fs::read_dir(&plugins_dir).map_err(|e| e.to_string())? {
        let dir = entry.map_err(|e| e.to_string())?.path();
        let manifest_path = dir.join("plugin.json");
        if !manifest_path.is_file() {
            continue;
        }
        total += 1;
        let text = std::fs::read_to_string(&manifest_path).unwrap_or_default();
        let enabled = {
            // 读取全局插件状态文件（与应用共用 ~/.config/micyou/plugin-state.json）
            let state_path = micyou_core::config::config_dir().join("plugin-state.json");
            let enabled = std::fs::read_to_string(&state_path)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| {
                    // plugin-state.json 顶层键即插件 id
                    v.get(dir.file_name().and_then(|n| n.to_str()).unwrap_or(""))
                        .and_then(|p| p.get("enabled").cloned())
                        .and_then(|e| e.as_bool())
                })
                .unwrap_or(false);
            enabled
        };
        match micyou_plugin::PluginManifest::from_json(&text) {
            Ok(m) => rows.push((
                m.id,
                m.version,
                format!("{:?}", m.runtime).to_lowercase(),
                enabled,
            )),
            Err(_) => rows.push((
                dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string(),
                "?".into(),
                "?".into(),
                enabled,
            )),
        }
    }
    if rows.is_empty() {
        println!("(未安装插件)");
        return Ok(());
    }
    println!("{:<45} {:<10} {:<8} 状态", "ID", "版本", "运行时");
    for (id, ver, rt, on) in rows {
        println!(
            "{:<45} {:<10} {:<8} {}",
            id,
            ver,
            rt,
            if on { "启用" } else { "禁用" }
        );
    }
    println!("共 {} 个插件", total);
    Ok(())
}

fn enable_plugin(id: &str) -> Result<(), String> {
    let plugins_dir = micyou_core::config::config_dir().join("plugins");
    let state_path = micyou_core::config::config_dir().join("plugin-state.json");
    let mut manager = micyou_plugin::PluginManager::new(plugins_dir, state_path);
    manager.scan().map_err(|e| e.to_string())?;
    manager.set_enabled(id, true).map_err(|e| e.to_string())?;
    println!("已启用插件: {id}");
    Ok(())
}

fn disable_plugin(id: &str) -> Result<(), String> {
    let plugins_dir = micyou_core::config::config_dir().join("plugins");
    let state_path = micyou_core::config::config_dir().join("plugin-state.json");
    let mut manager = micyou_plugin::PluginManager::new(plugins_dir, state_path);
    manager.scan().map_err(|e| e.to_string())?;
    manager.set_enabled(id, false).map_err(|e| e.to_string())?;
    println!("已禁用插件: {id}");
    Ok(())
}

fn validate(dir: &str) -> Result<(), String> {
    let manifest_path = Path::new(dir).join("plugin.json");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let manifest = micyou_plugin::PluginManifest::from_json(&text)
        .map_err(|e| format!("invalid plugin.json: {e}"))?;
    
    let base_path = Path::new(dir).join(&manifest.entry);

    let entry_ok = if base_path.extension().is_none() {
        base_path.with_extension("dll").exists()
            || base_path.with_extension("so").exists()
            || base_path.with_extension("dylib").exists()
    } else {
        base_path.exists()
    };

    let entry_display_path = if base_path.extension().is_none() {
        if base_path.with_extension("dll").exists() { base_path.with_extension("dll") }
        else if base_path.with_extension("so").exists() { base_path.with_extension("so") }
        else if base_path.with_extension("dylib").exists() { base_path.with_extension("dylib") }
        else { base_path.clone() }
    } else {
        base_path.clone()
    };

    println!(
        "OK  id={} name={} version={} runtime={:?}",
        manifest.id, manifest.name, manifest.version, manifest.runtime
    );
    println!("    capabilities={:?}", manifest.capabilities);
    println!(
        "    kind={:?} platforms={:?} arches={:?}",
        manifest.kind, manifest.platforms, manifest.arches
    );
    if entry_ok {
        println!("    entry={} (exists)", entry_display_path.display());
    } else {
        let hint = if base_path.extension().is_none() {
            format!(" (expected {}.dll, {}.so, or {}.dylib)", base_path.display(), base_path.display(), base_path.display())
        } else {
            "".to_string()
        };
        return Err(format!("entry artifact missing: {}{}", entry_display_path.display(), hint));
    }
    Ok(())
}

/// 递增 plugin.json 的 version（缺省 patch +1，或指定完整 semver）
fn bump(dir: &str, version: Option<&str>) -> Result<(), String> {
    let manifest_path = Path::new(dir).join("plugin.json");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let mut manifest: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid plugin.json: {e}"))?;
    let cur = manifest["version"]
        .as_str()
        .unwrap_or("0.0.0")
        .to_string();
    let next = match version {
        Some(v) => v.to_string(),
        None => {
            let parts: Vec<u64> = cur
                .split('.')
                .map(|p| p.parse().unwrap_or(0))
                .collect();
            let (ma, mi, pa) = match parts.as_slice() {
                [a, b, c, ..] => (*a, *b, *c + 1),
                [a, b] => (*a, *b, 1),
                [a] => (*a, 1, 0),
                _ => (0, 1, 0),
            };
            format!("{ma}.{mi}.{pa}")
        }
    };
    // 校验 semver 合法
    semver::Version::parse(&next).map_err(|e| format!("invalid version {next}: {e}"))?;
    manifest["version"] = serde_json::Value::String(next.clone());
    let out = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, out + "
").map_err(|e| format!("write: {e}"))?;
    println!("bumped {cur} -> {next} in {}", manifest_path.display());
    println!("  tip: `micyou-cli plugin package {dir}` to repackage");
    Ok(())
}

fn package(dir: &str, out: Option<&str>) -> Result<(), String> {
    let manifest_text = std::fs::read_to_string(Path::new(dir).join("plugin.json"))
        .map_err(|e| format!("read plugin.json: {e}"))?;
    let manifest = micyou_plugin::PluginManifest::from_json(&manifest_text)
        .map_err(|e| format!("invalid plugin.json: {e}"))?;
    let out_path = out
        .map(|o| o.to_string())
        .unwrap_or_else(|| format!("{}.zip", manifest.id));
    let file = std::fs::File::create(&out_path).map_err(|e| format!("create {}: {e}", out_path))?;
    let mut zipw = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Walk the plugin dir (skip target/ and hidden files).
    let mut entries = Vec::new();
    collect_entries(Path::new(dir), Path::new(dir), &mut entries)?;
    for (abs, rel) in &entries {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        zipw.start_file(rel_str.clone(), options)
            .map_err(|e| format!("zip add {}: {e}", rel_str))?;
        let bytes = std::fs::read(abs).map_err(|e| format!("read {}: {e}", abs.display()))?;
        zipw.write_all(&bytes)
            .map_err(|e| format!("zip write {}: {e}", rel_str))?;
    }
    zipw.finish().map_err(|e| format!("zip finish: {e}"))?;
    println!("packed {} entries -> {}", entries.len(), out_path);
    Ok(())
}

fn collect_entries(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>,
) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name.to_string_lossy().starts_with('.') {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_path_buf();
        if path.is_dir() {
            collect_entries(root, &path, out)?;
        } else {
            out.push((path, rel));
        }
    }
    Ok(())
}

const WASM_PLUGIN_JSON: &str = r#"{
  "id": "dev.micyou.example.myplugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "author": "you@example.com",
  "description": "A WASM plugin scaffold",
  "license": "MIT",
  "homepage": "https://example.com",
  "keywords": ["wasm"],
  "runtime": "wasm",
  "entry": "main.wasm",
  "platforms": ["linux", "windows", "macos"],
  "arches": [],
  "apiVersion": 1,
  "minHostVersion": "1.0.0",
  "capabilities": ["config.read", "config.write"],
  "kind": "utility",
  "config": {}
}
"#;

const WASM_TEMPLATE_RUST: &str = r#"#![no_main]
//! MicYou WASM 插件模板（Rust）
//!
//! 构建（需 wasm32-unknown-unknown 目标，见 README）：
//!   cargo build --release
//! 产物：target/wasm32-unknown-unknown/release/<crate>.wasm -> 复制为 main.wasm
//!
//! 无 WASI：所有 IO 走宿主 API（module "micyou"，能力在 plugin.json 声明）

use core::alloc::{GlobalAlloc, Layout};

// ─────────── 分配器：bump（宿主要求导出 alloc/dealloc）───────────
static mut HEAP: usize = 0x8000;

struct BumpAlloc;

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(4) as usize;
        let size = layout.size().max(1);
        let base = (HEAP + align - 1) & !(align - 1);
        HEAP = base + size;
        base as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc;

// ─────────── 宿主导入（签名必须与宿主一致，见 docs/plugins/api-reference.md）───────────
#[link(wasm_import_module = "micyou")]
extern "C" {
    fn log(level: i32, msg_ptr: *const u8);
    fn get_config(key_ptr: *const u8) -> i32;
    fn set_config(key_ptr: *const u8, value_ptr: *const u8) -> i32;
    fn set_panel_icon(panel_id_ptr: *const u8, icon_ptr: *const u8);
    // 按需启用更多宿主 API（audio_state/notify/fs_read/http_request/...）
}

// ─────────── 内存工具 ───────────
/// 把 &str 写入插件内存并返回 NUL 终止指针（宿主按 NUL 读取）
fn push_cstr(s: &str) -> *const u8 {
    let bytes = s.as_bytes();
    let buf: &'static mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(alloc(bytes.len() as u32 + 1) as *mut u8, bytes.len() + 1)
    };
    buf[..bytes.len()].copy_from_slice(bytes);
    buf[bytes.len()] = 0;
    buf.as_ptr()
}

/// 从宿主返回的指针读 NUL 终止字符串（宿主用插件的 alloc 分配，无需释放）
fn read_cstr(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }
    unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len)) }
}

/// 从 handle_message 的 (ptr, len) 读 payload 字节
fn read_payload(ptr: *const u8, len: i32) -> &'static [u8] {
    if ptr.is_null() || len <= 0 {
        return &[];
    }
    unsafe { core::slice::from_raw_parts(ptr, len as usize) }
}

// ─────────── 宿主契约导出 ───────────
#[no_mangle]
pub extern "C" fn alloc(n: u32) -> *mut u8 {
    unsafe { ALLOC.alloc(Layout::from_size_align(n as usize, 4).unwrap()) }
}

#[no_mangle]
pub extern "C" fn dealloc(_ptr: *mut u8, _n: u32) {}

#[no_mangle]
pub extern "C" fn api_version() -> i32 {
    1
}

static PANEL_ID: &[u8] = b"control\0";
static ICON: &[u8] = "\u{1F9E9}".as_bytes(); // 🧩 侧边栏面板图标（可改文本/emoji/图片文件名）

#[no_mangle]
pub extern "C" fn init() -> i32 {
    unsafe {
        set_panel_icon(PANEL_ID.as_ptr(), ICON.as_ptr());
        log(2, push_cstr("plugin initialized (rust)"));
    }
    0
}

/// 返回 0 = 已处理；返回 1 = 直通（bypass）
#[no_mangle]
pub extern "C" fn process(_data: *mut f32, _samples: i32, _channels: i32, _queued_ms: f64) -> i32 {
    // TODO: 实时安全 DSP 在这里写（禁止调用任何宿主 API，禁止分配）
    // 例：把 data 的前 samples*channels 个采样放大两倍
    // let n = (samples * channels) as usize;
    // for i in 0..n { unsafe { *data.add(i) *= 2.0; } }
    0
}

/// 接收面板 trigger / 定时器 / 配置变更等消息（payload 自描述，含动作文本）
#[no_mangle]
pub extern "C" fn handle_message(ptr: *const u8, len: i32) -> i32 {
    let payload = read_payload(ptr, len);
    let text = core::str::from_utf8(payload).unwrap_or("");
    if text.contains("ping") {
        // 演示：回复一条宿主日志
        unsafe { log(2, push_cstr("pong")) };
    } else if text.contains("echo") {
        // 演示：把收到的内容写进配置（面板可轮询 get_config 看到它）
        let key = push_cstr("lastEcho");
        let value = push_cstr(text);
        unsafe { set_config(key, value) };
    }
    0
}

#[no_mangle]
pub extern "C" fn deinit() {}
"#;

const WASM_RUST_CARGO: &str = r#"[package]
name = "myplugin"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[profile.release]
panic = "abort"
lto = true
opt-level = "s"
"#;

const WASM_RUST_CONFIG: &str = r#"[build]
target = "wasm32-unknown-unknown"
rustflags = ["-C", "link-arg=--export-memory"]
"#;

const WASM_TEMPLATE_WAT: &str = r#";; MicYou WASM plugin template
;; Build: micyou-cli plugin package <dir> (or compile with wat2wasm)
;; Host API 一览见 docs/plugins/api-reference.md（WASM import 表）
(module
  (import "micyou" "log" (func $log (param i32 i32)))
  (import "micyou" "get_config" (func $get_config (param i32 i32) (result i32)))
  (import "micyou" "set_config" (func $set_config (param i32 i32 i32) (result i32)))
  ;; set_panel_icon：给设置-插件 侧边栏面板设置图标（文本/emoji）
  (import "micyou" "set_panel_icon" (func $set_panel_icon (param i32 i32)))
  (memory (export "memory") 4)
  ;; bump allocator (heap starts after statics)
  (global $heap (mut i32) (i32.const 0x2000))
  (func (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $n)))
    (i32.store (local.get $p) (local.get $n))
    (i32.add (local.get $p) (i32.const 4)))
  (func (export "dealloc") (param $p i32) (param $n i32))
  (func (export "api_version") (result i32) (i32.const 1))
  (func (export "init") (result i32)
    (i32.store (i32.const 0) (i32.const 0))
    (call $set_panel_icon (i32.const 0x100) (i32.const 0x108))
    (i32.const 0))
  (func (export "process") (param $data i32) (param $samples i32) (param $channels i32) (param $queued f64) (result i32)
    ;; 返回 0 = 已处理，返回 1 = 直通（bypass）
    (i32.const 0))
  (func (export "handle_message") (param $ptr i32) (param $len i32) (result i32)
    (i32.const 0))
  (func (export "deinit"))
  ;; static data: panel id "control\0" + icon "🧩\0"（UTF-8）
  (data (i32.const 0x100) "control\00")
  (data (i32.const 0x108) "\f0\9f\a7\a9\00")
)
"#;

const WASM_PANEL_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh">
<head><meta charset="utf-8"><title>插件面板</title>
<style>
  body { font-family: system-ui, sans-serif; background: hsl(var(--surface)); color: hsl(var(--on-surface)); padding: 20px; }
  .card { background: hsl(var(--surface-bright)); border: 1px solid hsl(var(--border)); border-radius: 1rem; padding: 16px; }
  h2 { color: hsl(var(--primary)); margin: 0 0 12px; }
</style>
</head>
<body>
<div class="card"><h2>插件面板</h2><p>在此编写你的插件 UI，通过 postMessage 桥调用宿主 API</p></div>
<script>
function call(api, args) {
  return new Promise((resolve, reject) => {
    const id = Math.random().toString(36).slice(2);
    const on = (e) => { if (e.data && e.data.__micyou === 1 && e.data.id === id) {
      window.removeEventListener('message', on);
      e.data.ok ? resolve(e.data.value) : reject(new Error(e.data.error));
    } };
    window.addEventListener('message', on);
    window.parent.postMessage({ __micyou: 1, id, api, args: args || {} }, '*');
  });
}
call('get_config', {}).then((cfg) => console.log('config', cfg)).catch(console.error);
</script>
</body>
</html>
"#;

const NATIVE_PLUGIN_JSON: &str = r#"{
  "id": "dev.micyou.example.mynative",
  "name": "My Native Plugin",
  "version": "1.0.0",
  "author": "you@example.com",
  "description": "A native cdylib plugin scaffold",
  "license": "MIT",
  "runtime": "native",
  "entry": "mynative",
  "platforms": ["linux", "windows", "macos"],
  "arches": ["x86_64", "aarch64"],
  "apiVersion": 1,
  "minHostVersion": "1.0.0",
  "capabilities": ["config.read", "config.write"],
  "kind": "utility",
  "config": {}
}
"#;

const NATIVE_TEMPLATE_LIB: &str = r#"//! MicYou native plugin template (cdylib).
//! Copy include/micyou_plugin_abi.h into your crate or match these structs.

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum mpl_result_t {
    MPL_OK = 0,
    MPL_ERR_NOT_IMPLEMENTED = 1,
    MPL_ERR_INVALID_ARG = 2,
    MPL_ERR_RUNTIME = 3,
    MPL_ERR_BUFFER_TOO_SMALL = 4,
    MPL_ERR_PERMISSION = 5,
}

#[repr(C)]
pub struct mpl_plugin_info_t {
    pub abi_version: u32,
    pub api_version: u32,
    pub id: *const std::ffi::c_char,
    pub version: *const std::ffi::c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpl_host_api_t {
    pub log: unsafe extern "C" fn(*mut std::ffi::c_void, i32, *const std::ffi::c_char) -> mpl_result_t,
    pub get_config: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub set_config: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const std::ffi::c_char) -> mpl_result_t,
    pub emit_event: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const std::ffi::c_char) -> mpl_result_t,
    pub send_message: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const u8, u32) -> mpl_result_t,
    pub audio_state: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub connected_devices: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    // 仅追加在 ctx 之后（append-only ABI，勿插入字段）
    pub ctx: *mut std::ffi::c_void,
    pub play_sound: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char) -> mpl_result_t,
    pub plugin_dir: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub register_hotkey: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *mut u64) -> mpl_result_t,
    pub open_window: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char) -> mpl_result_t,
    pub fs_read: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub fs_write: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const std::ffi::c_char) -> mpl_result_t,
    pub set_timeout: unsafe extern "C" fn(*mut std::ffi::c_void, u64, *const std::ffi::c_char, *mut u64) -> mpl_result_t,
    pub clear_timeout: unsafe extern "C" fn(*mut std::ffi::c_void, u64) -> mpl_result_t,
    pub http_request: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const std::ffi::c_char, *const std::ffi::c_char, *const std::ffi::c_char, *mut u64) -> mpl_result_t,
    pub set_interval: unsafe extern "C" fn(*mut std::ffi::c_void, u64, *const std::ffi::c_char, *mut u64) -> mpl_result_t,
    pub clear_interval: unsafe extern "C" fn(*mut std::ffi::c_void, u64) -> mpl_result_t,
    pub open_url: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char) -> mpl_result_t,
    pub notify: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const std::ffi::c_char) -> mpl_result_t,
    pub locale: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub host_info: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub clipboard_read: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_char, *mut u32) -> mpl_result_t,
    pub clipboard_write: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char) -> mpl_result_t,
    pub set_panel_icon: unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_char, *const std::ffi::c_char) -> mpl_result_t,
}

const ID: &str = "dev.micyou.example.mynative";

/// raw 指针静态：需 Sync 才能放进 static
unsafe impl Sync for mpl_plugin_info_t {}

#[no_mangle]
pub extern "C" fn micyou_plugin_info() -> *const mpl_plugin_info_t {
    static INFO: mpl_plugin_info_t = mpl_plugin_info_t {
        abi_version: 1,
        api_version: 1,
        id: b"dev.micyou.example.mynative\0".as_ptr() as *const std::ffi::c_char,
        version: b"1.0.0\0".as_ptr() as *const std::ffi::c_char,
    };
    &INFO
}

#[no_mangle]
pub extern "C" fn micyou_plugin_init(host: *const mpl_host_api_t) -> mpl_result_t {
    unsafe {
        let host = &*host;
        let msg = std::ffi::CString::new(format!("{ID} initialized")).unwrap();
        ((*host).log)(host.ctx, 2, msg.as_ptr());
    }
    mpl_result_t::MPL_OK
}

#[no_mangle]
pub extern "C" fn micyou_plugin_deinit() {}

#[no_mangle]
pub extern "C" fn micyou_plugin_process(
    data: *mut f32,
    samples: u32,
    channels: u32,
    queued_ms: f64,
) -> mpl_result_t {
    unsafe {
        // TODO: real-time-safe DSP here. Never call host APIs from process().
        let _ = (data, samples, channels, queued_ms);
    }
    mpl_result_t::MPL_OK
}

#[no_mangle]
pub extern "C" fn micyou_plugin_handle_message(
    source: *const std::ffi::c_char,
    topic: *const std::ffi::c_char,
    payload: *const u8,
    payload_len: u32,
) -> mpl_result_t {
    let _ = (source, topic, payload, payload_len);
    mpl_result_t::MPL_OK
}
"#;

const NATIVE_CARGO: &str = r#"[package]
name = "micyou-example-mynative"
version = "1.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
"#;

fn create(
    id: &str,
    runtime: &str,
    name: Option<&str>,
    kind: &str,
    capabilities: &[String],
    lang: &str,
    out: Option<&str>,
) -> Result<(), String> {
    let last = id.rsplit('.').next().unwrap_or(id);
    let out_dir = out
        .map(|o| o.to_string())
        .unwrap_or_else(|| last.to_string());
    let dir = Path::new(&out_dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {out_dir}: {e}"))?;
    let display_name = name.unwrap_or(last).to_string();
    // kind + capabilities 注入 JSON（按运行时模板）
    let kind_json = match kind {
        "dsp" => "\"dsp\"",
        "ui" => "\"ui\"",
        _ => "\"utility\"",
    };
    let caps_json = if capabilities.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = capabilities
            .iter()
            .map(|c| format!("\"{c}\"", c = c.trim()))
            .collect();
        format!("[{}]", items.join(", "))
    };
    if runtime == "native" {
        let mut plugin_json = NATIVE_PLUGIN_JSON
            .replace("dev.micyou.example.mynative", id)
            .replace("My Native Plugin", &display_name)
            .replace("\"entry\": \"mynative\"", &format!("\"entry\": \"{last}\""))
            .replace("\"utility\"", kind_json);
        if !capabilities.is_empty() {
            // 模板默认 capabilities 数组：整体替换（粗粒度但够用）
            plugin_json = plugin_json.replacen("[]", &caps_json, 1);
        }
        write_file(dir, "plugin.json", &plugin_json)?;
        write_file(dir, "README.md", NATIVE_README)?;
        write_file(dir, "Cargo.toml", NATIVE_CARGO)?;
        let lib = NATIVE_TEMPLATE_LIB.replace("dev.micyou.example.mynative", id);
        std::fs::create_dir_all(dir.join("src")).map_err(|e| format!("mkdir src: {e}"))?;
        write_file(&dir.join("src"), "lib.rs", &lib)?;
    } else if lang == "wat" {
        // 保留 WAT 路径：高级场景（体积最小/无工具链）才手写 WAT
        let mut plugin_json = WASM_PLUGIN_JSON
            .replace("dev.micyou.example.myplugin", id)
            .replace("My Plugin", &display_name)
            .replace("\"utility\"", kind_json);
        if !capabilities.is_empty() {
            plugin_json = plugin_json.replacen("[]", &caps_json, 1);
        }
        write_file(dir, "plugin.json", &plugin_json)?;
        write_file(dir, "README.md", WASM_README)?;
        write_file(dir, "main.wat", WASM_TEMPLATE_WAT)?;
        write_file(dir, "panel.html", WASM_PANEL_HTML)?;
        let wat_path = dir.join("main.wat");
        let wasm_bytes = wat::parse_file(&wat_path)
            .map_err(|e| format!("compile main.wat: {e}"))?;
        std::fs::write(dir.join("main.wasm"), &wasm_bytes)
            .map_err(|e| format!("write main.wasm: {e}"))?;
        println!("  compiled main.wat -> main.wasm ({} bytes)", wasm_bytes.len());
    } else {
        // 推荐路径：Rust 高级语言编译（wasm32-unknown-unknown）
        let mut plugin_json = WASM_PLUGIN_JSON
            .replace("dev.micyou.example.myplugin", id)
            .replace("My Plugin", &display_name)
            .replace("\"utility\"", kind_json);
        if !capabilities.is_empty() {
            plugin_json = plugin_json.replacen("[]", &caps_json, 1);
        }
        write_file(dir, "plugin.json", &plugin_json)?;
        write_file(dir, "README.md", WASM_RUST_README)?;
        write_file(dir, "Cargo.toml", WASM_RUST_CARGO)?;
        write_file(dir, "panel.html", WASM_PANEL_HTML)?;
        std::fs::create_dir_all(dir.join("src")).map_err(|e| format!("mkdir src: {e}"))?;
        write_file(&dir.join("src"), "lib.rs", WASM_TEMPLATE_RUST)?;
        std::fs::create_dir_all(dir.join(".cargo")).map_err(|e| format!("mkdir .cargo: {e}"))?;
        write_file(&dir.join(".cargo"), "config.toml", WASM_RUST_CONFIG)?;
        // 尝试本地编译（有 wasm32 目标时）；失败仅提示，不阻断
        let build = std::process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .current_dir(dir)
            .output();
        match build {
            Ok(out) if out.status.success() => {
                let wasm = dir
                    .join("target/wasm32-unknown-unknown/release/myplugin.wasm");
                if wasm.exists() {
                    let bytes = std::fs::read(&wasm).unwrap_or_default();
                    println!("  compiled src/lib.rs -> main.wasm ({} bytes)", bytes.len());
                    let _ = std::fs::copy(&wasm, dir.join("main.wasm"));
                }
            }
            _ => {
                println!("  (未检测到 wasm32 目标，跳过编译：cargo build --release 后复制 main.wasm)");
            }
        }
    }
    println!(
        "created {runtime} plugin skeleton in {}/  \n  next: `micyou-cli plugin dev {out_dir}` (watch) or `micyou-cli plugin install {out_dir}`",
        dir.display()
    );
    if runtime == "native" {
        println!(
            "\n⚠️  [Cross-Platform Notice]\n\
             For native plugins, the host auto-appends platform extensions (.dll/.so/.dylib).\n\
             Ensure your build/packaging script removes the 'lib' prefix from Linux/macOS outputs:\n\
               - Expected: {last}.so / {last}.dylib / {last}.dll\n\
               - NOT: lib{last}.so"
        );
    }
    Ok(())
}

const WASM_README: &str = r#"# 插件骨架

## 构建入口
`main.wat` 编译为 `main.wasm`（wat2wasm 或 wat crate），产物放回本目录

## 安装
- 开发：把本目录放入 ~/.config/micyou/plugins/<id>/
- 分发：`micyou-cli plugin package .` 打包 zip 后在应用内导入

## 面板
panel.html 通过 postMessage 桥调用宿主 API（get_config/set_config/trigger 等）

## 能力
在 plugin.json 的 capabilities 中声明所需能力（config.read/config.write/dsp.node/...）
"#;

const WASM_RUST_README: &str = r#"# WASM 插件骨架（Rust）

## 构建（推荐：从高级语言编译，不要手写 WAT）
1. 安装 wasm32 目标：`rustup target add wasm32-unknown-unknown`
2. 构建：`cargo build --release`
3. 复制产物为入口：`cp target/wasm32-unknown-unknown/release/myplugin.wasm main.wasm`

## 开发循环
`micyou-cli plugin dev .` 监听变更自动重装（可先执行一次构建）

## 安装
- 开发：`micyou-cli plugin install .`
- 分发：`micyou-cli plugin package . -o out.zip` 后在应用内导入

## 模板说明
- `src/lib.rs`：宿主导入（module "micyou"）与宿主契约导出（alloc/dealloc/
  api_version/init/process/handle_message/deinit），字符串经插件内存传递
- 无 WASI：所有 IO（日志/配置/通知/网络/定时器...）走宿主 API，
  能力在 plugin.json 的 capabilities 中声明
- `process` 内禁止调用宿主 API（实时音频线程）
- 面板：panel.html 通过 postMessage 桥调用宿主 API

## 手动 WAT 路径（不推荐）
`micyou-cli plugin create <id> --runtime wasm --lang wat` 生成 WAT 骨架，
仅适合体积极致/无工具链的高级场景
"#;

const NATIVE_README: &str = r#"# Native 插件骨架

## 构建
`cargo build --release`，产物 target/release/lib*.so 复制到插件目录并改名与
plugin.json 的 entry 一致

## 说明
native 插件拥有宿主完整权限，用于实时 DSP、硬件与深度系统集成；
普通逻辑/UI 优先使用 wasm 插件（沙箱安全）

## 能力
按需声明 capabilities；process() 内禁止调用宿主 API（实时安全）

### 构建与跨平台打包

由于宿主支持单 ZIP 包跨平台分发，它会自动根据操作系统为 `entry` 补全后缀（.dll / .so / .dylib）。
因此，在打包发布前，**必须去除 Linux 和 macOS 产物的 `lib` 前缀**：

```bash
cargo build --release
# Linux: 将 lib<name>.so 重命名为 <name>.so
mv target/release/lib<name>.so target/release/<name>.so
# macOS: 将 lib<name>.dylib 重命名为 <name>.dylib
mv target/release/lib<name>.dylib target/release/<name>.dylib
# Windows 产物默认为 <name>.dll，无需处理
"#;

fn write_file(dir: &Path, name: &str, content: &str) -> Result<(), String> {
    let p = dir.join(name);
    std::fs::write(&p, content).map_err(|e| format!("write {}: {e}", p.display()))
}

/// 安装插件目录到应用插件目录（~/.config/micyou/plugins/<id>/）
fn install(dir: &str) -> Result<(), String> {
    let manifest_text = std::fs::read_to_string(Path::new(dir).join("plugin.json"))
        .map_err(|e| format!("read plugin.json: {e}"))?;
    let manifest = micyou_plugin::PluginManifest::from_json(&manifest_text)
        .map_err(|e| format!("invalid plugin.json: {e}"))?;
    let plugins_dir = micyou_core::config::config_dir().join("plugins");
    let target = plugins_dir.join(&manifest.id);
    std::fs::create_dir_all(&target).map_err(|e| format!("mkdir {}: {e}", target.display()))?;
    let mut copied = 0u32;
    for entry in walk(dir, &[]) {
        let rel = entry.strip_prefix(dir).map_err(|e| e.to_string())?;
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&entry, &dst).map_err(|e| format!("copy {}: {e}", entry.display()))?;
        copied += 1;
    }
    // 入口产物必须存在
    let base_path = target.join(&manifest.entry);
    let entry_exists = if base_path.extension().is_none() {
        base_path.with_extension("dll").exists()
            || base_path.with_extension("so").exists()
            || base_path.with_extension("dylib").exists()
    } else {
        base_path.exists()
    };

    if !entry_exists {
        return Err(format!(
            "entry artifact missing in {} (build it first)",
            target.display()
        ));
    }
    println!(
        "installed {} v{} -> {} ({copied} files)",
        manifest.id,
        manifest.version,
        target.display()
    );
    Ok(())
}

/// 开发模式：轮询监听目录变更，自动重新安装
fn dev(dir: &str, interval: f64) -> Result<(), String> {
    // 先校验 + 安装一次
    validate(dir)?;
    install(dir)?;
    let ms = (interval.max(0.3) * 1000.0) as u64;
    println!(
        "watching {dir} every {ms}ms ... (edit files, changes auto-install; Ctrl+C to stop)"
    );
    let mut prev = snapshot(dir);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let now = snapshot(dir);
        if now != prev {
            prev = now;
            println!("-- change detected, reinstalling --");
            if let Err(e) = install(dir) {
                println!("install failed: {e} (fix and save again)");
            }
        }
    }
}

/// 目录文件快照：相对路径 + 修改时间（排除构建产物与隐藏文件）
fn snapshot(dir: &str) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = Vec::new();
    let ignore = ["target/", ".git/", "node_modules/", "plugin.zip"];
    for entry in walk(dir, &ignore) {
        let rel = entry.strip_prefix(dir).unwrap_or(&entry).to_string_lossy().into_owned();
        let mtime = std::fs::metadata(&entry)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
            .unwrap_or(0);
        out.push((rel, mtime));
    }
    out.sort();
    out
}

/// 递归收集目录下所有文件（跳过忽略前缀与隐藏文件）
fn walk(dir: &str, ignore: &[&str]) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for ent in rd.flatten() {
            let p = ent.path();
            let rel = p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().into_owned();
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || ignore.iter().any(|i| rel.starts_with(i)) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}
