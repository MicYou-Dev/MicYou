# 插件开发指南

面向插件作者的完整指南：Native 与 WASM 插件如何编写、Manifest 怎么写、Host API 怎么用、实时安全要求与跨端通信方法

## 目录

1. [目录结构](#目录结构)
2. [Manifest（plugin.json）](#manifestpluginjson)
3. [编写 Native 插件](#编写-native-插件)
4. [编写 WASM 插件](#编写-wasm-插件)
5. [Host API 使用](#host-api-使用)
6. [实时 DSP 插件规范](#实时-dsp-插件规范)
7. [跨端通信 API](#跨端通信-api)
8. [调试与测试](#调试与测试)
9. [端到端开发流程](#端到端开发流程)

## 目录结构

```text
<插件目录>/
├── plugin.json          # 清单（必需）
├── <entry>              # 入口产物：xxx.dll / xxx.so / xxx.dylib / xxx.wasm（与清单 entry 一致，Native 跨平台建议省略后缀与 lib 前缀）
└── assets/              # 可选私有资源
```

插件目录放在宿主的插件目录下，每个插件一个子目录，目录名建议与插件 id 一致：

- Linux: `~/.config/micyou/plugins/`
- Windows: `%APPDATA%\micyou\plugins\`
- macOS: `~/.config/micyou/plugins/`

## Manifest（plugin.json）

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `id` | string | 是 | 唯一反向域名标识，如 `dev.micyou.example.gain`（仅支持小写字母、数字、点号和连字符） |
| `name` | string | 是 | 插件显示名称 |
| `version` | string | 是 | SemVer 版本号（如 `1.0.0`） |
| `runtime` | string | 是 | 运行时类型：`wasm`（推荐）或 `native` |
| `entry` | string | 是 | 入口文件。Native 插件建议省略 `lib` 前缀与平台后缀（如填 `my_plugin`），宿主将根据 OS 自动补全 |
| `apiVersion` | number | 否 | 插件构建时使用的 Host API 版本，支持 `1` 或 `2`；省略时默认 `2`（当前宿主为 `2`） |
| `author` | string | 否 | 作者昵称或联系邮箱 |
| `description` | string | 否 | 插件功能简述 |
| `license` | string | 否 | SPDX 许可证标识（如 `MIT`, `Apache-2.0`） |
| `homepage` | string | 否 | 项目主页或官方仓库 URL |
| `repository` | string | 否 | 源码仓库地址 |
| `keywords` | string[] | 否 | 检索关键词列表 |
| `platforms` | string[] | 否 | 支持的操作系统（`linux`, `windows`, `macos`, `android`），空表示全部 |
| `arches` | string[] | 否 | Native 插件支持的 CPU 架构（`x86_64`, `aarch64` 等），空表示全部 |
| `minHostVersion` | string | 否 | 所需最低 MicYou 宿主应用版本（SemVer，如 `2.1.0`）；宿主版本更旧时拒绝加载 |
| `capabilities` | string[] | 否 | 申请的宿主能力清单，参见 [API 参考](api-reference.md#权限清单) |
| `kind` | string | 否 | 插件类型：`dsp` / `utility` / `ui` / `bridge`，默认为 `utility` |
| `ui` | object | 否 | UI 面板配置：`{ route, label, panels }` |
| `dsp` | object | 否 | DSP 节点声明：`{ insertAfter, first, frameSize, realtimeSafe, allowMultipleNodes }`；`allowMultipleNodes: true` 允许同一 `Plugin:<id>` 在链中重复出现（配合 `micyou_plugin_process_node` 区分节点序号） |
| `config` | object | 否 | 首次启用时初始化的默认配置对象 |
| `configSchema` | object | 否 | 声明式配置项描述，宿主据此自动生成原生风格设置表单 |
| `dependencies` | object[] | 否 | 前置插件依赖声明：`[{ id, version, optional }]` |
| `nameI18n` | object | 否 | 多语言名称字典（键为 BCP-47 标签，如 `zh`, `en`） |
| `descriptionI18n` | object | 否 | 多语言描述字典 |
| `updateUrl` | string | 否 | 远端 Manifest JSON 地址，用于应用内检查更新 |

Manifest 示例（`plugin.json`）：

```json
{
  "id": "dev.micyou.example.gain",
  "name": "Audio Gain",
  "nameI18n": { "zh": "音频增益", "en": "Audio Gain" },
  "version": "1.0.0",
  "author": "MicYou Team",
  "description": "实时音频增益 DSP 插件",
  "license": "MIT",
  "runtime": "native",
  "entry": "micyou_example_gain",
  "platforms": ["linux", "windows", "macos"],
  "arches": ["x86_64", "aarch64"],
  "apiVersion": 1,
  "capabilities": ["dsp.node", "config.read"],
  "kind": "dsp",
  "dsp": { "insertAfter": "AEC", "realtimeSafe": true },
  "config": { "gain": 1.5 },
  "configSchema": {
    "fields": [
      { "key": "gain", "fieldType": "number", "label": "增益倍数", "min": 0.0, "max": 5.0, "step": 0.1, "default": 1.5 }
    ]
  }
}
```

校验规则（不满足即拒绝加载并给出原因）：

- id 必须合法反向域名格式
- version 必须合法 SemVer
- `apiVersion` 必须落在宿主支持区间（当前 `1..=2`，省略时按 `2` 处理）
- `minHostVersion` 必须是合法 SemVer；宿主版本（`gradle.properties` 的 `project.version`）低于它时拒绝加载
- capabilities 必须是已知能力（未知能力拒绝）
- WASM DSP 插件不得声明 `realtimeSafe: true`
- `ui` 类型插件必须声明 `ui` 描述

### 配置表单自动生成（configSchema）

```json
"configSchema": {
  "fields": [
    { "key": "workMin", "fieldType": "number", "label": "工作时长", "min": 1, "max": 120, "step": 1, "default": 25 },
    { "key": "enabled", "fieldType": "boolean", "label": "启用", "default": true },
    { "key": "mode", "fieldType": "select", "options": [{ "value": "a", "label": "A" }] }
  ]
}
```

- 插件声明 schema 后无需手写设置页，宿主在插件卡片渲染原生风格表单
- 支持 number（滑杆）/ boolean（开关）/ string（输入）/ select（下拉）
- 保存走 set_plugin_config，配置热更新链路自动生效

### 插件依赖（dependencies）

```json
"dependencies": [
  { "id": "dev.micyou.effect", "version": "^1.0.0", "optional": false }
]
```

- 启用前宿主校验：依赖须已安装、已启用、版本满足 semver 约束
- optional=true 时缺失仅警告不阻塞；插件间调用复用 send_message 路由

### 更新机制（updateUrl）

- 声明 `updateUrl` 指向远端 manifest JSON，应用内「检查更新」做 semver 对比
- 有新版时一键更新：下载 zip → 替换安装目录 → 按原状态重新启用

### 本地化适配（nameI18n / descriptionI18n）

宿主 UI（插件管理页卡片、详情对话框、**插件市场页**）会按当前界面语言优先展示本地化文本，缺失时回退基础 `name` / `description`——**旧插件不带这两个字段也能正常展示**，但新插件建议适配：

```json
{
  "name": "FocusCapture",
  "description": "Hotkey-toggled capture of the focused application's audio output…",
  "nameI18n": { "zh": "焦点声音捕获", "zh-CN": "焦点声音捕获" },
  "descriptionI18n": {
    "zh": "快捷键一键捕获当前焦点应用的声音（Windows 进程环回），混入 MicYou 麦克风流。",
    "zh-CN": "快捷键一键捕获当前焦点应用的声音（Windows 进程环回），混入 MicYou 麦克风流。"
  }
}
```

规则与约定：

- 键为 BCP-47 标签。**宿主界面 locale 的实际取值**为 `zh` / `zh-hk` / `zh-tw` / `zh-ss` / `en` / `cat` / `lzh`（见 `main.ts`），与常见的 `zh-CN` 写法不同；匹配规则对大小写与区域后缀**双向宽容**（精确 → 小写精确 → 语言前缀双向：宿主 `zh` 可命中插件键 `zh-CN`，插件键 `zh` 也可命中宿主 `zh-CN`）→ 回退基础字段。**建议同时提供 `zh` 与 `zh-CN`（及 `en`）**以兼容旧版宿主的精确匹配；
- 仅影响**展示文本**：`id`、日志、能力名等不参与本地化；面板（panel.html）内部文案请自行用桥接 `locale` API 适配；
- 市场仓库的 `generate_catalog.ts` 会把这两个字段透传进 `index.json`，市场页据此展示；市场条目（`plugin/<id>/plugin.json`）同样建议携带；
- `configSchema` 的 label/description 暂不参与本地化（保持单一语言文案）。

### 运行时选择：WASM 优先

- **WASM（默认推荐）**：沙箱隔离、内存与燃料受限、跨平台（同一 .wasm 在
  Windows/macOS/Linux/未来 Android 通用）、能力由宿主授权
  - 适用：逻辑、UI 面板、自动化、定时任务、HTTP、文件（插件沙箱内）、配置
  - 性能：wasmi 解释器 100-500 Mops/s，48kHz 音频帧处理实测 ~70µs/帧（预算 10ms）
- **Native（cdylib）**：宿主完整权限，用于实时 DSP 与深度系统集成
  - 适用：自研降噪/变声算法、ONNX 推理、硬件交互
  - 要求：按平台分别编译（.so / .dylib / .dll），须声明 `arches`
  - 注意：process() 内禁止调用宿主 API（实时安全）

### 开发工具（micyou-cli plugin）

```bash
micyou-cli plugin create dev.micyou.myplugin          # 生成 wasm 骨架（默认）
micyou-cli plugin create dev.micyou.mynative --runtime native
micyou-cli plugin validate ./myplugin                 # 校验 plugin.json 与入口产物
micyou-cli plugin package ./myplugin -o out.zip       # 打包为可导入 zip
```

- `create` 生成 plugin.json + 入口模板 + panel.html + README
- wasm 骨架（默认 Rust 高级语言）：cargo build --release 产出 main.wasm（create 已内置编译）
- native 骨架：cargo build --release 后复制产物并按 entry 命名
- `package` 自动跳过 target/ 与隐藏文件，产物根目录含 plugin.json，应用内可直接导入

## 编写 Native 插件

Native 插件是平台 cdylib，通过版本化 C ABI 与宿主交互，ABI 定义在
[`micyou_plugin_abi.h`](../../tauri-app/crates/micyou-plugin/include/micyou_plugin_abi.h)

### 必需符号

```c
// 静态插件身份（abiVersion 必须等于 1，apiVersion 为 1 或 2，id 必须与 manifest 一致）
const mpl_plugin_info_t *micyou_plugin_info(void);

// 初始化：保存 host 回调表（生命周期内有效）
mpl_result_t micyou_plugin_init(const mpl_host_api_t *host);

// 反初始化（库卸载前调用一次）
void micyou_plugin_deinit(void);
```

### 可选符号（缺省视为旁路 / 无操作）

```c
// 实时 DSP：原地处理 samples 个交错 f32，bypass=1 表示本帧旁路
mpl_result_t micyou_plugin_process(float *data, uint32_t samples, uint32_t channels, double queued_ms, uint32_t *bypass);

// 多节点 DSP（可选）：manifest 声明 dsp.allowMultipleNodes 后，宿主优先调用它并传入
// node_index（同名 Plugin:<id> 节点在链中的 0-based 序号）
mpl_result_t micyou_plugin_process_node(float *data, uint32_t samples, uint32_t channels, double queued_ms, uint32_t node_index, uint32_t *bypass);

// 本地事件通知（type 为事件类型，json 为负载）
mpl_result_t micyou_plugin_handle_event(const char *type, const char *json);

// 跨端消息（source 来源插件 id，topic 主题，payload 二进制负载）
mpl_result_t micyou_plugin_handle_message(const char *source, const char *topic, const uint8_t *payload, uint32_t payload_len);
```

### 完整最小示例（Rust）

`plugins/examples/native-gain/` 是完整可构建示例（`cargo build --release`），核心骨架：

```rust
#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_void, CStr};

const MPL_ABI_VERSION: u32 = 1;
const MPL_API_VERSION: u32 = 1;
const PLUGIN_ID: &[u8] = b"dev.micyou.example.gain\0";

#[repr(C)]
#[derive(PartialEq, Eq)]
pub enum mpl_result_t { MPL_OK = 0, /* ... */ }

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mpl_host_api_t {
    pub log: unsafe extern "C" fn(*mut c_void, mpl_log_level_t, *const c_char),
    pub get_config: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, *mut u32) -> mpl_result_t,
    pub set_config: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> mpl_result_t,
    pub emit_event: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> mpl_result_t,
    pub send_message: unsafe extern "C" fn(*mut c_void, *const c_char, *const u8, u32) -> mpl_result_t,
    pub audio_state: unsafe extern "C" fn(*mut c_void, *mut c_char, *mut u32) -> mpl_result_t,
    pub connected_devices: unsafe extern "C" fn(*mut c_void, *mut c_char, *mut u32) -> mpl_result_t,
    pub ctx: *mut c_void,
}

static mut HOST: Option<mpl_host_api_t> = None;
static mut GAIN: f64 = 2.0;

// 防止 panic 跨 FFI 边界（UB），统一转运行时错误码
fn guard<F: FnOnce() -> mpl_result_t + std::panic::UnwindSafe>(f: F) -> mpl_result_t {
    std::panic::catch_unwind(f).unwrap_or(mpl_result_t::MPL_ERR_RUNTIME)
}

#[unsafe(no_mangle)]
pub extern "C" fn micyou_plugin_info() -> *const mpl_plugin_info_t {
    static INFO: mpl_plugin_info_t = /* abiVersion=1, apiVersion=1, id=PLUGIN_ID, version=... */;
    &INFO
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn micyou_plugin_init(host: *const mpl_host_api_t) -> mpl_result_t {
    guard(|| {
        if host.is_null() { return mpl_result_t::MPL_ERR_INVALID_ARG; }
        unsafe { HOST = Some(*host); }
        mpl_result_t::MPL_OK
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn micyou_plugin_process(
    data: *mut f32, samples: u32, _channels: u32, _queued_ms: f64, bypass: *mut u32,
) -> mpl_result_t {
    guard(|| {
        let gain = unsafe { GAIN };
        if gain <= 0.0 { unsafe { *bypass = 1 }; return mpl_result_t::MPL_OK; }
        unsafe { for i in 0..samples as usize { *data.add(i) *= gain as f32; } *bypass = 0; }
        mpl_result_t::MPL_OK
    })
}
```

要点：

- 所有跨 FFI 的函数必须 `#[unsafe(no_mangle)] extern "C"`，返回值用 `mpl_result_t`
- panic 必须被捕获（`catch_unwind`），绝不跨 ABI 边界传播
- 字符串通过 NUL 结尾指针传递；host 回调的 `out/out_size` 采用缓冲区契约（详见 [API 参考](api-reference.md#缓冲区契约)）
- 配置读取：`init` 时通过 `host.get_config("gain")` 获取 JSON 字符串

### 用 C 编写

C 插件直接 `#include "micyou_plugin_abi.h"` 实现符号即可，导出宏已处理各平台（`MPL_EXPORT`）

### 跨平台产物命名规范

为了实现单 ZIP 包跨平台分发（Windows/Linux/macOS），宿主支持在 `entry` 字段省略后缀，并在加载时根据当前操作系统自动补全（`.dll` / `.so` / `.dylib`）。

**规范要求**：
1. `plugin.json` 中的 `entry` 填写**不带后缀的基础名**（如 `"entry": "my_plugin"`）。
2. 构建产物必须**统一去除 Linux/macOS 传统的 `lib` 前缀**。

| 平台 | 期望的构建产物文件名 |
| --- | --- |
| Windows | `my_plugin.dll` |
| Linux | `my_plugin.so` (而非 `libmy_plugin.so`) |
| macOS | `my_plugin.dylib` (而非 `libmy_plugin.dylib`) |

> 打包 ZIP 时，将这三个产物与 `plugin.json` 一同放入根目录，即可实现一次发布，全平台自动适配。

## 编写 WASM 插件

WASM 插件是 core wasm 模块（无需 WASI），在 `wasmi` 纯 Rust 解释器中沙箱执行

> **用高级语言写，别手写 WAT**
> 推荐用 Rust 编译到 `wasm32-unknown-unknown`（`micyou-cli plugin create --runtime wasm`
> 默认就生成 Rust 骨架并自动编译），类型安全、可维护、标准库可用
> WAT 手写仅保留给体积极致或零工具链的高级场景（`--lang wat`）

### 导出（宿主期望）

| 导出 | 签名 | 必填 | 说明 |
| --- | --- | --- | --- |
| `memory` | memory | 是 | 线性内存，宿主通过它交换数据 |
| `alloc` | `(i32) -> i32` | 是 | 分配 size 字节，返回地址 |
| `dealloc` | `(i32, i32) -> ()` | 是 | 释放 |
| `api_version` | `() -> i32` | 否 | 返回 1 |
| `init` | `() -> i32` | 否 | 初始化，0=成功 |
| `process` | `(i32,i32,i32,f64) -> i32` | 否 | DSP 处理，0=ok 1=bypass |
| `handle_event` | `(i32) -> i32` | 否 | 事件（JSON 字符串指针） |
| `handle_message` | `(i32,i32) -> i32` | 否 | 跨端消息（指针, 长度） |
| `deinit` | `() -> ()` | 否 | 反初始化 |

### 导入（宿主提供，模块名 `micyou`）

| 导入 | 签名 | 说明 |
| --- | --- | --- |
| `log` | `(i32, i32) -> ()` | level(0-4), NUL 字符串指针 |
| `get_config` | `(i32) -> i32` | key 指针 -> 宿主分配 JSON 指针（0 = 无） |
| `set_config` | `(i32, i32) -> i32` | key, value JSON 指针 -> 结果码 |
| `emit_event` | `(i32, i32) -> i32` | topic, payload JSON 指针 -> 结果码 |
| `send_message` | `(i32, i32, i32) -> i32` | target JSON, 数据指针, 长度 -> 结果码 |
| `audio_state` | `() -> i32` | -> 宿主分配 JSON 指针 |
| `connected_devices` | `() -> i32` | -> 宿主分配 JSON 数组指针 |
| `play_sound` | `(i32) -> i32` | WAV 路径指针 -> 结果码（需 audio.play） |
| `plugin_dir` | `() -> i32` | -> 插件安装目录绝对路径字符串 |
| `register_hotkey` | `(i32) -> i64` | 快捷键字符串指针 -> 句柄 id（0 = 失败） |

### 完整最小示例（Rust）

`micyou-cli plugin create dev.micyou.hello --runtime wasm` 生成完整 Rust 骨架（推荐路径）

#### 构建

```bash
rustup target add wasm32-unknown-unknown   # 一次性
cargo build --release                      # 骨架内执行（.cargo/config.toml 已配置目标）
cp target/wasm32-unknown-unknown/release/myplugin.wasm main.wasm
```

`micyou-cli plugin create` 已内置编译：检测到 wasm32 目标时自动产出 `main.wasm`

#### 核心骨架（src/lib.rs）

```rust
#![no_main]
use core::alloc::{GlobalAlloc, Layout};

// 宿主要求导出 alloc/dealloc：bump 分配器
#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc;
struct BumpAlloc;
unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        static mut HEAP: usize = 0x8000;
        let align = layout.align().max(4) as usize;
        let base = (HEAP + align - 1) & !(align - 1);
        HEAP = base + layout.size().max(1);
        base as *mut u8
    }
    unsafe fn dealloc(&self, _p: *mut u8, _l: Layout) {}
}

// 宿主导入：模块名 micyou，签名见上方导入表
#[link(wasm_import_module = "micyou")]
extern "C" {
    fn log(level: i32, msg_ptr: *const u8);
    fn set_config(key_ptr: *const u8, value_ptr: *const u8) -> i32;
    fn set_panel_icon(panel_id_ptr: *const u8, icon_ptr: *const u8);
}

// 契约导出：alloc/dealloc/api_version/init/process/handle_message/deinit
#[no_mangle] pub extern "C" fn api_version() -> i32 { 1 }
#[no_mangle] pub extern "C" fn alloc(n: u32) -> *mut u8 { /* bump */ }
#[no_mangle] pub extern "C" fn dealloc(_p: *mut u8, _n: u32) {}

#[no_mangle]
pub extern "C" fn init() -> i32 {
    unsafe { set_panel_icon(b"control\0".as_ptr(), "🧩".as_bytes().as_ptr()) }
    0
}

#[no_mangle]
pub extern "C" fn handle_message(ptr: *const u8, len: i32) -> i32 {
    // payload 自描述（含动作文本），按需解析
    0
}
```

> 完整模板含全部工具函数（字符串写入/读取、payload 读取）与 `process` DSP 示例，
> 见 `micyou-cli plugin create --runtime wasm` 生成的骨架

### 高级场景：手写 WAT（仅限特殊需求）

仅在需要极致体积或无法引入完整编译器时，可手写 WebAssembly 文本格式（WAT）：

```bash
micyou-cli plugin create <id> --runtime wasm --lang wat
```

要点说明：
- 静态字符串存放在数据段（data segment），指针对应模块线性内存起始偏移。
- 宿主调用导出函数前会注入燃料预算（默认 100,000 instructions），防止死循环挂死宿主。
- WASM 解释执行无法保证硬实时，因此 WASM DSP 插件不得声明 `realtimeSafe`。
- 每次函数调用重置燃料计数，宿主回调函数同样受燃料计量约束。

## Host API 使用

插件通过 host 回调访问宿主能力，全部能力需要 manifest 中声明对应 capability，未声明会被拒绝（错误码 `MPL_ERR_PERMISSION` / 8）

| 能力 | 对应 API | 说明 |
| --- | --- | --- |
| `config.read` / `config.write` | get_config / set_config | 插件私有配置（持久化在 `plugin-state.json`） |
| `event.emit` | emit_event | 向总线发布事件（本地订阅者 + 已连接的远端） |
| `message.send` | send_message | 向本地/远端插件发消息 |
| `audio.state` | audio_state | 实时音频流快照 |
| `audio.play` | play_sound | 播放 WAV 音效（异步，非实时） |
| `device.list` | connected_devices | 已连接设备 |
| `dsp.node` | （manifest 声明） | 注册为 DSP 链节点 |
| `plugin_dir` | 无需能力 | 查询插件安装目录（只读） |
| `network.io` | — | 预留：出站网络 |
| `fs.read` | — | 预留：插件沙箱内文件读取 |

## 实时 DSP 插件规范

实时安全是硬性要求（违反可能导致爆音或卡顿）

- 不得在 `process` 中分配堆内存（`Vec`、`String`、格式化等）
- 不得调用阻塞 host API（`get_config` 每次调用涉及锁与 I/O，仅限 `init` 中使用）
- 单帧处理时间必须远小于帧时长（48 kHz 下 480 样本 ≈ 10 ms），建议 < 1 ms
- 状态（滤波器系数、历史缓冲）在插件内预先分配
- 宿主在加载时按 `dsp.realtimeSafe` 信任 Native 插件；WASM DSP 永远视为 best-effort
- 出错返回错误码并保持输出可预测（静音或旁路），绝不 panic 或返回未初始化数据

## 跨端通信 API

手机与电脑连接后（Wi-Fi / USB / Web），两端插件可通过总线通信

### 发消息（插件视角）

```c
// Native：目标为 JSON 对象
// {"type":"local","pluginId":"dev.micyou.other"} 或
// {"type":"remote","pluginId":"dev.micyou.phone.sensor"} 或 {"type":"broadcast"}
host->send_message(host->ctx,
    "{\"type\":\"remote\",\"pluginId\":\"dev.micyou.phone.sensor\"}",
    payload, payload_len);
```

### 收消息（插件视角）

实现 `micyou_plugin_handle_message(source, topic, payload, len)`，宿主会把远端发来的消息路由进来

### RPC（请求-响应）

- 宿主总线用 `correlationId` 配对请求与响应
- 插件间 RPC 需要自行约定主题格式（推荐 `rpc:<method>`），响应通过 `handle_message` 回传
- 宿主代码可用 `PluginBus::request` 发起带超时的同步 RPC（禁止在实时音频线程调用）

### 事件订阅

- 插件可用 `emit_event` 发布事件；本地与远端订阅者都会收到
- 宿主总线内置 `handle_incoming` 路由：响应完成 pending RPC，请求/事件投递给本地分发器与主题订阅者

## 官方示例参考

`plugins/examples/` 目录下提供了完整可运行的参考插件：

### 音频状态监视器（wasm-audioinspector）
- **类型**：纯 WASM 插件（WAT 编写）
- **核心逻辑**：使用 `set_interval` 每 2 秒定时采样 `audio_state` 与 `connected_devices`，通过 `set_config` 持久化状态，前端面板轮询展示
- **特性展示**：WASM 内存 scratch 缓冲区复用、双语面板、设置侧边栏图标 `set_panel_icon`
- **源码路径**：`plugins/examples/wasm-audioinspector/`

### 音效板（native-soundpad）
- **类型**：Native (cdylib)
- **核心逻辑**：动态注册全局快捷键（如 `Ctrl+Shift+S`），接收热键或前端按钮点击后通过 `play_sound` 触发音频混音播放
- **特性展示**：通用按钮面板（`ui.route=buttons`）、专属设置页面（`ui.panels`）、自包含 HTML 面板通信
- **源码路径**：`plugins/examples/native-soundpad/`

```json
{
  "ui": {
    "route": "buttons",
    "label": "Soundpad",
    "panels": [ { "id": "console", "label": "控制台", "entry": "panel.html" } ]
  },
  "capabilities": ["config.read", "config.write", "audio.play"],
  "config": { "sounds": [ { "id": "beep", "label": "Beep", "file": "sounds/beep.wav" } ] }
}
```

### 降噪引擎（native-noisegate）：实时 DSP 处理

- 帧 RMS 噪声门：低于阈值按 depth 衰减，attack/release 包络平滑避免咔哒声
- 全程无分配、无 host 调用（配置经原子变量无锁读取），满足实时安全
- 进 DSP 链的位置由 `dsp.insertAfter` 决定；缺失节点默认补在最后一个插件节点之后，没有插件节点时补在 `AEC` 之后，否则链尾。`AEC` 只是普通内置节点，用户可以在链编辑器里把它拖到任意位置
- 每个启用中的 DSP 插件在链中拥有独立的 `Plugin:<id>` 节点；声明 `dsp.allowMultipleNodes: true` 后同一个 `Plugin:<id>` 可以重复出现，每次出现都会通过 `AudioFrameCtx::node_index` / `micyou_plugin_process_node` 得到自己的 0-based 序号

## 编写插件专属设置页（ui.panels）

插件可在设置对话框侧边栏拥有专属页面（渲染在「插件」之后）

1. manifest 声明 `ui.panels`，`entry` 是插件目录内的自包含 HTML 文件
2. 宿主命令 `get_plugin_panel` 返回 HTML，前端用沙箱 iframe（`allow-scripts`，无 same-origin）渲染
3. 可用 `set_panel_icon(panel_id, icon)` 设置侧边栏图标（emoji/文本）

#### 面板开发工作流（单文件自包含）

面板通过沙箱 iframe 的 `srcdoc` 属性渲染，无法直接引用相对路径的外部 JS/CSS 文件，需采用自包含单文件：

- **单文件编写**：直接在一个 HTML 文件内编写内联 `<style>` 与 `<script>`。宿主会自动注入 Material 3 CSS 变量（如 `hsl(var(--primary))`），面板样式可自适应应用主题。
- **打包工具内联**：如使用工程化开发，可在构建阶段打包内联为单文件：
  - Vite: 结合 `vite-plugin-singlefile`
  - esbuild: `esbuild src/main.ts --bundle --outfile=panel.html`
- **本地调试**：修改 HTML 文件后无需重启服务端，在界面中重新打开面板即可读取最新内容。

#### postMessage 通信桥

面板脚本通过标准 `window.postMessage` 契约与宿主双向通信：

```js
function call(api, args) {
  return new Promise((resolve, reject) => {
    const id = Math.random().toString(36).slice(2);
    const onMsg = (e) => {
      if (e.data && e.data.__micyou === 1 && e.data.id === id) {
        window.removeEventListener('message', onMsg);
        e.data.ok ? resolve(e.data.value) : reject(new Error(e.data.error));
      }
    };
    window.addEventListener('message', onMsg);
    window.parent.postMessage({ __micyou: 1, id, api, args: args || {} }, '*');
  });
}

// 示例：读取配置与触发动作
const cfg = await call('get_config', {});
await call('play', { id: 'beep' });
```

可用桥 API 清单：

| API | 参数 | 说明 |
| --- | --- | --- |
| `get_config` | `{}` | 读取插件持久化配置 |
| `set_config` | `{key, value}` | 保存插件配置项 |
| `play` | `{id}` | 触发音效播放（投递 `ui:play` 消息） |
| `trigger` | `{action, payload}` | 触发插件自定义 UI 动作 |
| `log` | `{level, message}` | 输出到插件独立日志流 |
| `get_logs` | `{}` | 获取最近日志记录 |
| `get_sync_status` | `{}` | 查询与移动端的同步状态 |

## 使用全局快捷键

- 插件在 `init` 阶段调用 `register_hotkey("ctrl+shift+s")` 注册系统级快捷键
- 快捷键触发时，宿主将向插件派发 `hotkey:<id>` 消息，插件在 `handle_message` 中执行相应逻辑
- 插件卸载或退出时，快捷键由宿主自动注销
- **平台支持**：目前仅在 Linux X11 会话及支持全局快捷键的桌面环境下可用

## 调试与测试

- **独立日志**：在 GUI 插件面板的「日志」标签页中查看插件的标准输出与错误追踪
- **配置管理**：直接在配置面板中编辑并验证 JSON 配置持久化
- **加载排查**：若插件未能成功加载，`list_plugins` 命令将返回详细的 `error` 错误码与诊断信息
- **单元测试**：可参考 `crates/micyou-plugin/tests/` 中的加载器与总线集成测试用例


## 端到端开发流程

从零到市场发布的完整流程（配合 `micyou-cli plugin` 工具链）

### 第 0 步：规划

| 决策 | 依据 |
|---|---|
| 插件 id | 反向域名，如 `dev.yourname.eq`，必须含点，小写字母数字与连字符 |
| 运行时 | 逻辑/面板/定时/网络/文件用 **wasm**（沙箱安全）；实时 DSP、系统级集成用 **native**（需声明 arches） |
| kind | utility（工具）/ dsp（处理链节点）/ ui（纯面板） |
| capabilities | 只声明需要的：control.observe/intercept、config.read/write、audio.state、device.list、network.io、open.url、clipboard.read/write、fs.read/write、audio.play |
| 依赖 | 如依赖其他插件，声明 dependencies（id + semver 范围） |

### 第 1 步：生成骨架

```bash
# WASM 插件（默认 Rust 骨架，自动编译 main.wasm）
micyou-cli plugin create dev.yourname.myplugin \
  --kind utility --capabilities config.read,config.write

# Native 插件
micyou-cli plugin create dev.yourname.mydsp \
  --runtime native --kind dsp
```

骨架包含：plugin.json / 入口源文件 / panel.html（面板）/ README，manifest 已按参数预填

### 第 2 步：开发循环（热重装）

```bash
micyou-cli plugin dev <插件目录>
```

- 首次自动 validate + install（部署到 `~/.config/micyou/plugins/<id>/`）
- 之后监听目录变更，保存即重新安装
- 重启应用 → 设置-插件页启用 → 验证效果
- 改面板 HTML 无需重启应用，重新打开面板即生效（HTML 运行时读取）

### 第 3 步：验证

```bash
micyou-cli plugin validate <插件目录>
# 校验 manifest 结构 + 入口产物存在性
```

应用内验证：设置-插件 → 启用 → 看日志（面板有日志查看）；DSP 插件在处理链中确认节点位置

### 第 4 步：打包

```bash
micyou-cli plugin package <插件目录> -o myplugin.zip
# zip 根目录含 plugin.json，应用内可导入
```

### 第 5 步：发布至官方市场

MicYou 插件市场索引仅维护元数据，插件二进制由开发者仓库的 CI 自动构建并通过 GitHub Releases 分发：

1. **配置构建流水线**：在插件仓库中通过 GitHub Actions 构建 WASM 或 Native 产物，打包为 ZIP 格式并发布为 Release 资产。
2. **声明更新源**：在 `plugin.json` 中配置 `updateUrl`，指向市场目录下的元数据地址（例如 `https://micyou-dev.github.io/MicYou-Plugins/plugin/<id>/plugin.json`）。
3. **提交 PR 登记元数据**：向 `MicYou-Dev/MicYou-Plugins` 仓库提交 Pull Request，包含：
   - `plugin.json`：声明插件元信息及 `downloadUrl`（对应第一步生成的 Release ZIP 下载直链）
   - `preview.png`（可选）：插件卡片封面图（建议比例 16:9）
   - 源码与使用说明 README
4. **自动集成与上架**：市场仓库合入后，CI 将自动生成全量 `index.json` 并部署至 GitHub Pages，客户端市场即刻同步生效。

### 第 6 步：版本迭代与更新

```bash
# 升级小版本 (patch +1) 或指定版本
micyou-cli plugin bump <插件目录>
micyou-cli plugin bump <插件目录> 1.1.0

# 重新打包
micyou-cli plugin package <插件目录> -o plugin.zip
```

发布新版本只需在代码仓库创建新 Release 并向市场仓库同步更新 `plugin.json` 中的 `version` 与 `downloadUrl`，客户端检测到新版本后即可一键无感更新。
