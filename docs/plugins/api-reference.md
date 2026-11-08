# API 参考

插件系统的完整接口定义：Host API、Plugin API、消息协议 / 事件定义、错误码与权限清单

## Host API（宿主向插件提供的服务）

API 版本：`HOST_API_VERSION = 2`，`MIN_SUPPORTED_API_VERSION = 1`（见 manifest `apiVersion`）
- 追加式演进：**新字段只能加在 `mpl_host_api_t` 的 `ctx` 之后**，禁止插入中间，旧插件按旧偏移仍能正确读取
- manifest `minHostVersion` 声明插件所需的最低 **MicYou 宿主应用版本**（SemVer，例如 `2.1.0`）；当宿主版本低于该值时拒绝加载。它和 `apiVersion` 是两套独立协商：`apiVersion` 管 Host API / ABI 版本区间，`minHostVersion` 管具体应用版本
- 插件能力（capabilities）在 manifest 中声明，宿主在每次调用时强制检查，越权返回 `MPL_ERR_PERMISSION`

### 调用约定与并发安全约束

Native 插件与宿主进程共享同一进程空间，需遵循以下调用约定：

1. **结构体按值拷贝**：
   `micyou_plugin_init(host)` 传入的 `host` 指针仅在初始化调用期间保证有效。插件应在 `init` 阶段将 `mpl_host_api_t` 结构体按值完整拷贝至插件私有静态或全局状态中，而非仅保留指针引用。

2. **调用线程上下文约束**：
   - **禁止在实时音频线程中调用**：`micyou_plugin_process` 运行于极低延迟优先级的实时音频渲染线程，严禁在此方法内调用任何 Host API（包括 I/O、配置读取与日志），以避免音频流欠载或死锁。
   - **线程分发约束**：Host API 通常要求在宿主派发的回调线程（如 `handle_message`、`handle_event` 或定时器回调）中执行。插件自行派生的后台工作线程如需调用宿主能力，建议通过通道或任务队列交由派发线程代为调用。

### Native（C ABI，`mpl_host_api_t`）

```c
typedef struct mpl_host_api {
    void (*log)(void *ctx, mpl_log_level_t level, const char *msg);
    mpl_result_t (*get_config)(void *ctx, const char *key, char *out, uint32_t *out_size);
    mpl_result_t (*set_config)(void *ctx, const char *key, const char *json_value);
    mpl_result_t (*emit_event)(void *ctx, const char *topic, const char *json_payload);
    mpl_result_t (*send_message)(void *ctx, const char *target_json, const uint8_t *payload, uint32_t payload_len);
    mpl_result_t (*audio_state)(void *ctx, char *out, uint32_t *out_size);
    mpl_result_t (*connected_devices)(void *ctx, char *out, uint32_t *out_size);
    void *ctx;
    /* 以下字段一律追加在 ctx 之后（追加式演进，见上） */
    mpl_result_t (*play_sound)(void *ctx, const char *path);
    mpl_result_t (*plugin_dir)(void *ctx, char *out, uint32_t *out_size);
    mpl_result_t (*register_hotkey)(void *ctx, const char *shortcut, uint64_t *out_id);
    mpl_result_t (*open_window)(void *ctx, const char *panel_id);/* 仅TauriGUI可用 */
    mpl_result_t (*fs_read)(void *ctx, const char *path, char *out, uint32_t *out_size);
    mpl_result_t (*fs_write)(void *ctx, const char *path, const char *content);
    mpl_result_t (*set_timeout)(void *ctx, uint64_t ms, const char *payload, uint64_t *out_id);
    mpl_result_t (*clear_timeout)(void *ctx, uint64_t id);
    mpl_result_t (*http_request)(void *ctx, const char *method, const char *url,
                                 const char *headers_json, const char *body, uint64_t *out_id);
    mpl_result_t (*set_interval)(void *ctx, uint64_t ms, const char *payload, uint64_t *out_id);
    mpl_result_t (*clear_interval)(void *ctx, uint64_t id);
    mpl_result_t (*open_url)(void *ctx, const char *url);
    mpl_result_t (*notify)(void *ctx, const char *title, const char *body);
    mpl_result_t (*locale)(void *ctx, char *out, uint32_t *out_size);
    mpl_result_t (*host_info)(void *ctx, char *out, uint32_t *out_size);
    mpl_result_t (*clipboard_read)(void *ctx, char *out, uint32_t *out_size);
    mpl_result_t (*clipboard_write)(void *ctx, const char *text);
    mpl_result_t (*set_panel_icon)(void *ctx, const char *panel_id, const char *icon);
    /* API Version 2: 控制面信令（追加式演进） */
    mpl_result_t (*set_muted)(void *ctx, uint32_t muted);
    mpl_result_t (*get_muted)(void *ctx, uint32_t *out_muted);
    mpl_result_t (*set_monitoring)(void *ctx, uint32_t enabled);
    mpl_result_t (*get_monitoring)(void *ctx, uint32_t *out_enabled);
    mpl_result_t (*get_dsp_settings)(void *ctx, char *out, uint32_t *out_size);
    mpl_result_t (*set_dsp_settings)(void *ctx, const char *settings_json);
} mpl_host_api_t;
```

### WASM（导入模块 `micyou`）

| 导入 | 签名 | 说明 |
| --- | --- | --- |
| `log` | `(level: i32, msg_ptr: i32) -> ()` | level：0=Error 1=Warn 2=Info 3=Debug 4=Trace |
| `get_config` | `(key_ptr: i32) -> i32` | 返回宿主分配的 JSON 指针（0 = 无该键） |
| `set_config` | `(key_ptr: i32, value_json_ptr: i32) -> i32` | 返回结果码 |
| `emit_event` | `(topic_ptr: i32, payload_json_ptr: i32) -> i32` | 返回结果码 |
| `send_message` | `(target_json_ptr: i32, payload_ptr: i32, len: i32) -> i32` | 返回结果码 |
| `audio_state` | `() -> i32` | 返回宿主分配的 JSON 指针 |
| `connected_devices` | `() -> i32` | 返回宿主分配的 JSON 数组指针 |
| `play_sound` | `(path_ptr: i32) -> i32` | 排队播放 WAV（需 audio.play），返回结果码 |
| `plugin_dir` | `() -> i32` | 返回插件安装目录绝对路径字符串 |
| `register_hotkey` | `(shortcut_ptr: i32) -> i64` | 注册全局快捷键（仅 X11 会话），返回句柄 id（0 = 失败） |
| `open_window` | `(panel_ptr: i32) -> i32` | 打开插件自己的面板窗口（仅TauriGUI可用）|
| `fs_read` | `(path_ptr: i32) -> i32` | 读取插件目录内文本文件（需 fs.read），返回字符串指针 |
| `fs_write` | `(path_ptr: i32, content_ptr: i32) -> ()` | 写文件（需 fs.write） |
| `set_timeout` | `(ms: i64, payload_ptr: i32) -> i64` | 一次性定时器，返回 id |
| `clear_timeout` | `(id: i64) -> ()` | 取消定时器 |
| `http_request` | `(method_ptr, url_ptr, headers_ptr, body_ptr) -> i64` | 异步 HTTP（需 network.io），返回请求 id |
| `set_interval` | `(ms: i64, payload_ptr: i32) -> i64` | 循环定时器，返回 id |
| `clear_interval` | `(id: i64) -> ()` | 停止循环定时器 |
| `open_url` | `(url_ptr: i32) -> ()` | 默认浏览器打开（需 open.url） |
| `notify` | `(title_ptr: i32, body_ptr: i32) -> ()` | 系统通知 |
| `locale` | `() -> i32` | 宿主 UI 语言（如 zh-CN / en），字符串指针 |
| `host_info` | `() -> i32` | 宿主身份 JSON 字符串指针 |
| `clipboard_read` | `() -> i32` | 读剪贴板（需 clipboard.read） |
| `clipboard_write` | `(text_ptr: i32) -> ()` | 写剪贴板（需 clipboard.write） |
| `set_panel_icon` | `(panel_id_ptr: i32, icon_ptr: i32) -> ()` | 设置设置侧边栏面板图标 |
| `get_muted` | `() -> i32` | 读静音状态（需 control.observe，0=未静音 1=静音） |
| `set_muted` | `(muted: i32) -> i32` | 设静音状态（需 control.intercept，0=取消 1=静音） |
| `get_monitoring` | `() -> i32` | 读耳返监听状态（需 control.observe，0=关 1=开） |
| `set_monitoring` | `(enabled: i32) -> i32` | 设耳返监听状态（需 control.intercept，0=关 1=开） |
| `get_dsp_settings` | `() -> i32` | 读当前 DSP 配置 JSON 字符串指针（需 control.observe） |
| `set_dsp_settings` | `(json_ptr: i32) -> i32` | 覆写或局部 Patch DSP 配置（需 control.intercept） |

### 缓冲区契约（out / out_size）

- `out` / `out_size` 描述插件提供的缓冲区（UTF-8）
- 成功：写入 NUL 结尾字符串，`*out_size` = 字节数（不含 NUL），返回 `MPL_OK`
- 缓冲区太小：`*out_size` = 所需大小，返回 `MPL_ERR_BUFFER_TOO_SMALL`
- `audio_state` 返回 JSON 快照：

```json
{ "streaming": true, "sampleRate": 48000, "channels": 1, "inputLevel": 0.42, "processedLevel": 0.38, "queuedMs": 12.5, "muted": false }
```

- `connected_devices` 返回 JSON 数组：

```json
[ { "mode": "wifi", "label": "MicYou Mobile", "audioActive": true } ]
```

- `host_info` 返回：

```json
{ "name": "micyou", "version": "2.1.1", "apiVersion": 2 }
```

### play_sound（音频播放）

- 参数为 WAV 文件路径，相对路径解析到插件自己的安装目录（插件可自带或动态生成音效文件）
- 音效**混入虚拟麦克风输出流**（SoundMixer），对方与用户都能听到，等同于真实麦克风输入
- 排队即返回，混音在输出设备线程完成，**非实时安全**，禁止在 process 中调用
- 常见用法：ui 按钮面板（`ui.route=buttons`）点击 → `handle_message` 收到 `ui:play` → 查配置 → `play_sound`

### register_hotkey（全局快捷键）

- 参数为快捷键字符串（如 `ctrl+shift+p`），返回句柄 id
- 按下时宿主经总线投递消息给插件：topic `hotkey:<id>`，payload `{"shortcut":"..."}`
- 插件在 `handle_message` 中处理；进程退出自动注销
- 快捷键格式由 global-hotkey 解析：修饰键 ctrl/alt/shift/super/cmd + 键名（字母、数字、f1-f12 等）
- **平台限制**：仅 X11 会话可用；Wayland 会话注册返回明确错误（合成器不转发 X11 全局抓取），请改用面板按钮

### fs_read / fs_write（文件访问）

- 读写插件**自身安装目录内**的 UTF-8 文本文件（需 `fs.read` / `fs.write`）
- 路径沙箱：绝对路径与 `..` 穿越一律拒绝（`sandbox_path`），插件无法触及目录外文件
- `fs_write` 自动创建父目录
- 常见用途：缓存、状态文件、生成资源（音效、脚本）、插件数据持久化

### set_timeout / clear_timeout（一次性定时器）

- `set_timeout(ms, payload)` 返回定时器 id，到期后宿主经总线投递：
  topic `timer:expired`，payload `{"timer":<id>,"payload":"<payload>"}`
- 异步事件模型：定时器线程投递消息，不阻塞插件调用线程
- `clear_timeout(id)` 取消尚未到期的定时器

### set_interval / clear_interval（循环定时器）

- `set_interval(ms, payload)` 返回循环定时器 id，每隔 `ms` 毫秒投递：
  topic `interval:tick`，payload `{"interval":<id>,"payload":"<payload>"}`
- `clear_interval(id)` 停止；插件卸载时宿主自动停止其全部定时器

### http_request（异步 HTTP）

- `http_request(method, url, headersJson, body)` 返回请求 id（需 `network.io`）
- 请求在宿主线程执行（10 秒超时），响应经总线异步投递：
  topic `http:response`，payload `{"request":<id>,"ok":true,"status":200,"body":"...","error":null}`
- 失败时 `ok=false` 且 `error` 含原因
- 插件不得在实时音频线程（process）中发起请求

### open_url / notify（外部动作）

- `open_url(url)`：系统默认浏览器打开（需 `open.url`）
- `notify(title, body)`：系统通知（无能力要求）

### locale / host_info（环境查询）

- `locale()`：宿主当前 UI 语言（如 `zh-CN`、`en`），插件面板据此切换自身文案
- `host_info()`：宿主身份与 API 版本，用于运行时兼容判断

### clipboard_read / clipboard_write（剪贴板）

- 读/写系统剪贴板文本（需 `clipboard.read` / `clipboard.write`）
- 常见用途：自动化复制粘贴、把插件生成内容送剪贴板


### set_panel_icon（侧边栏面板图标）

插件在 `init` 或运行期调用，设置自己在设置对话框侧边栏中的面板图标

- 参数 `panel_id`：插件声明的 ui.panels 中的面板 id
- 参数 `icon`：简短文本或 emoji（如 🎛 / 🍅），或插件目录内图片文件名（前端暂以文本渲染）
- 无需能力声明，宿主按插件隔离存储（plugin id -> panel id -> icon）

WASM 导入签名：`(panel_id_ptr: i32, icon_ptr: i32) -> ()`

示例（WAT）：
```wat
(call $set_panel_icon (i32.const 0x210) (i32.const 0x218))
```
### 事件投递（宿主 → 插件）

- 设备连接/断开时宿主向所有已加载插件派发 `PluginEvent::DeviceConnected`（含 mode/label）与 `DeviceDisconnected`
- 插件在 `handle_event` 中处理（native 为 `micyou_plugin_handle_event`，wasm 为 `handle_event` 导出）

## Plugin API（插件向宿主实现的接口）

> **跨平台入口加载（Entry Auto-Extension）**
> 对于 `runtime: "native"` 的插件，若 manifest 中的 `entry` 字段无扩展名，宿主在加载时会自动根据当前操作系统补全后缀（Windows: `.dll`, Linux: `.so`, macOS: `.dylib`）。此机制要求开发者在编译时去除 Linux/macOS 产物的 `lib` 前缀，以保证单 ZIP 包跨平台分发时文件名一致。WASM 插件不受此影响。

### Native（C ABI 符号）

| 符号 | 必需 | 说明 |
| --- | --- | --- |
| `micyou_plugin_info` | 是 | 返回静态 `mpl_plugin_info_t`，含 ABI / API 版本与 id |
| `micyou_plugin_init(host)` | 是 | 保存 host 表，读取配置，返回结果码 |
| `micyou_plugin_deinit` | 是 | 清理（宿主卸载库前调用一次） |
| `micyou_plugin_process(data, samples, channels, queued_ms, bypass)` | 否 | 实时 DSP |
| `micyou_plugin_process_node(data, samples, channels, queued_ms, node_index, bypass)` | 否 | 多节点实时 DSP；manifest 声明 `dsp.allowMultipleNodes` 时优先调用，`node_index` 为同名 `Plugin:<id>` 节点在链中的 0-based 序号 |
| `micyou_plugin_handle_event(type, json)` | 否 | 事件通知 |
| `micyou_plugin_handle_message(source, topic, payload, len)` | 否 | 跨端消息 |

### WASM（导出）

| 导出 | 必需 | 说明 |
| --- | --- | --- |
| `memory` | 是 | 线性内存 |
| `alloc` / `dealloc` | 是 | 内存分配（宿主写字符串 / 音频数据用） |
| `api_version` | 否 | 返回 Host API 版本（1 或 2，缺省为 2） |
| `init` | 否 | 初始化，0 = 成功 |
| `process` | 否 | DSP，0 = ok 1 = bypass |
| `handle_event` | 否 | 事件（JSON 指针） |
| `handle_message` | 否 | 跨端消息（指针, 长度） |
| `deinit` | 否 | 反初始化 |

### 事件类型（`PluginEvent`）

| 事件 | 负载 |
| --- | --- |
| `device_connected` | `{ mode, label }` |
| `device_disconnected` | — |
| `mute_changed` | `{ muted }`（需 control.observe） |
| `monitoring_changed` | `{ enabled }`（需 control.observe） |
| `dsp_settings_changed` | —（需 control.observe） |
| `state_changed` | `{ enabled }` |

## 消息协议

线协议为 protobuf（`crates/micyou-protocol/proto/network.proto`），挂载在控制通道 `MessageWrapper` 字段 7：

```proto
message PluginMessage {
    string source = 1;       // 发送方插件 id
    string target = 2;       // 接收方插件 id，"" = 广播
    string topic = 3;        // 主题
    bytes payload = 4;       // 插件自定义负载
    uint64 correlationId = 5;
    bool isResponse = 6;
    int32 errorCode = 7;     // 0 = ok
    string errorMessage = 8;
}
```

语义：

- **发布订阅**：`target` 为空，按 `topic` 分发（本地订阅者 + 远端广播）
- **请求响应**：`correlationId` 非 0 配对请求与响应；`isResponse` 标记响应
- **错误响应**：`errorCode` 非 0 + `errorMessage`

## 错误码

`PluginError` 与 wire 错误码的稳定映射（改变即破坏兼容）：

| 码 | 含义 |
| --- | --- |
| 0 | ok |
| 1 | not found（入口产物缺失。跨平台 Native 插件请确认已去除 `lib` 前缀，且 `entry` 字段未写死特定平台后缀） |
| 2 | invalid manifest |
| 3 | validation failed（清单语义校验） |
| 4 | unknown plugin |
| 5 | not loaded |
| 6 | load failed |
| 7 | api / host version mismatch（`apiVersion` 超出宿主支持区间，或宿主版本低于 `minHostVersion`） |
| 8 | permission denied（能力未授予） |
| 9 | already exists |
| 10 | runtime error（含 WASM trap / 燃料耗尽） |
| 11 | message delivery failed（无设备 / 超时） |
| 12 | io error |

Native 的 `mpl_result_t` 数值与此保持一致（0-5 子集）

## 权限清单

| 能力 | 授予的 API | 风险 |
| --- | --- | --- |
| `dsp.node` | 处理链节点注册 | 实时音频数据访问 |
| `config.read` | get_config | 插件自身配置 |
| `config.write` | set_config | 插件自身配置 |
| `event.emit` | emit_event | 总线事件（含远端广播） |
| `message.send` | send_message | 跨端消息 |
| `audio.state` | audio_state | 音频流状态快照 |
| `audio.play` | play_sound | 播放 WAV 音效（异步，非实时） |
| `device.list` | connected_devices | 已连接设备信息 |
| `network.io` | http_request | 出站 HTTP（异步，10s 超时） |
| `open.url` | open_url | 默认浏览器打开链接 |
| `clipboard.read` | clipboard_read | 读取剪贴板文本 |
| `clipboard.write` | clipboard_write | 写入剪贴板文本 |
| `fs.read` | fs_read | 插件目录内文件读取（沙箱） |
| `fs.write` | fs_write | 插件目录内文件写入（沙箱，自动建父目录） |
| `control.observe` | get_muted / get_monitoring / get_dsp_settings，并接收控制面状态事件 | 观测宿主静音、耳返及 DSP 处理链状态 |
| `control.intercept` | set_muted / set_monitoring / set_dsp_settings | 主动接管并修改宿主静音、耳返与 DSP 滤波参数 |
| `register_hotkey` | 无需能力 | 全局快捷键（仅 X11，触发仅通知注册的插件） |
| 无 | set_timeout/clear_timeout/set_interval/clear_interval | 定时器（不访问资源） |
| 无 | notify/locale/host_info/plugin_dir/open_window | 通知与环境查询 |

- 未知能力声明会被清单校验拒绝
- 未声明能力的 API 调用被 host 回调层拒绝（`MPL_ERR_PERMISSION` / 错误码 8）
