# 架构与扩展说明

## 为安卓端预留的扩展点

### 协议统一 + 实现分离

插件系统按「协议统一 + 实现分离」设计，安卓端不照搬桌面实现：

- **统一**：Manifest 模型（`plugin.json`）、Host API 能力描述、跨端消息协议（protobuf `PluginMessage`）、总线语义（发布订阅 / RPC）两端共用
- **分离**：插件加载实现（Native 加载器 / WASM 运行时）、宿主接线（HostApi 实现、传输适配器）按平台各自实现

### 桌面端的可复用部分

| 模块 | 是否可复用于安卓 | 说明 |
| --- | --- | --- |
| `manifest.rs` | 是 | 纯 Rust，无平台依赖 |
| `plugin.rs`（统一抽象） | 是 | 运行时无关的契约 |
| `bus.rs`（PluginBus） | 是 | 纯 Rust 逻辑，仅替换 transport |
| `sync.rs`（线协议编解码） | 是 | 依赖 micyou-protocol |
| `wasm.rs`（wasmi 运行时） | 是 | wasmi 纯 Rust，无原生依赖，可直接嵌入 Android JNI |
| `native.rs`（libloading） | 否 | Android 用其他加载方式（见下） |
| `abi.rs` + `micyou_plugin_abi.h` | 参考 | 安卓可另定 JNI 绑定，但保持能力语义一致 |

## 安卓端规划

### 为什么不用重型插件化框架

RePlugin / Shadow / VirtualAPK 等动态插件框架面向「应用级插件化」（Activity / Service / 资源 / 热修复），对 MicYou 属于过度设计：

- **体积**：框架与打包链开销大，MicYou 是单模块轻量应用
- **能力不匹配**：插件需求是 DSP 节点、事件、跨端消息，不是页面跳转与组件热插拔
- **安全模型冲突**：框架追求「动态加载任意 APK」，与插件沙箱/能力授权模型不一致
- **维护成本**：与 AGP/Kotlin 版本强绑定，升级成本高

推荐路径：**协议对齐 → 轻量运行时 → 跨端同步**，加载方式可选（轻量 DEX、受信 .so、WASM），不强制上重型框架

### 分阶段路线图

#### 阶段 1：协议对齐（核心）

- 在安卓端实现 `plugin.json` 解析与校验（可与桌面共享同一份 manifest 描述，Kotlin 侧按 proto/JSON schema 对齐）
- 引入 protobuf `PluginMessage` 到安卓的 TCP 控制通道（与桌面 `tcp_server` 对称）
- 实现轻量 `PluginBus`（发布订阅 + RPC 关联）——语义与桌面 `bus.rs` 一致
- 验收：安卓客户端能解析/发送 `PluginMessage`，与桌面端插件完成一次双向消息往返

#### 阶段 2：轻量运行时

- **WASM**：优先（wasmi 可编译到 Android，纯 Rust 无原生依赖，天然沙箱）
- **受信 .so**：面向实时 DSP 场景，走 JNI + 手写 ABI（或复用桌面 C ABI 头文件），仅加载受信来源（应用内置 / 官方商店）
- **轻量 DEX**：工具类插件可选，用 `PathClassLoader` 隔离加载，不做 Activity/资源插件化
- 验收：安卓端可启用一个 WASM 工具插件与一个受信 .so DSP 插件

#### 阶段 3：跨端同步

- 接入与桌面一致的 `PluginMessage` 路由（本地分发 + 远端转发）
- 两端插件互发现（通过总线广播插件清单）
- 典型场景落地：手机传感器 → 电脑处理 → 回传
- 验收：双向 RPC 与订阅推送端到端可用

### 能力矩阵（两端对齐）

| 能力 | 桌面 | 安卓 | 说明 |
| --- | --- | --- | --- |
| Manifest / 能力声明 | ✅ | ✅ | 同一 schema |
| Host API 逻辑接口 | ✅ | ✅ | 同一语义，不同绑定 |
| WASM 运行时 | ✅ | ✅（规划） | 同一模块产物 |
| Native 运行时 | cdylib + libloading | 受信 .so + JNI（规划） | 加载方式不同 |
| 跨端消息协议 | ✅ | ✅（阶段 1） | 同一 protobuf |
| 插件总线（本地） | ✅ | ✅（阶段 1） | 同一语义 |
| 插件总线（跨端） | ✅ | ✅（阶段 3） | 同一传输协议 |
| DSP 链节点 | ✅（逐插件 `Plugin:<id>` 节点，可选 `allowMultipleNodes`） | 规划 | 安卓侧处理链在 AudioRecord 管线内 |
| UI 按钮面板（`ui.route=buttons`） | ✅ | 规划 | 音效板等声明式面板 |
| 专属设置页（`ui.panels`） | ✅ | 规划 | 沙箱 iframe + postMessage 桥 |
| 全局快捷键（register_hotkey） | ✅ | 规划 | 系统级快捷键消息 |
| `audio.play`（播放音效） | ✅ | 规划 | 安卓可映射到 MediaPlayer |
| 前端管理界面 | ✅（Vue） | 规划 | Compose 面板 |
| `network.io` / `fs.read` | 预留 | 预留 | — |

## 版本兼容与解耦策略

- **Host API 版本**（`apiVersion`）：宿主支持版本区间协商（当前 `MIN_SUPPORTED_API_VERSION = 1`，`HOST_API_VERSION = 2`），声明为 v1 或 v2 的插件均可平滑加载，超出支持范围才会拒绝加载（错误码 7）
- **ABI 版本**（Native）：`MPL_ABI_VERSION`（当前为 1）保护结构体基础布局；破坏性变更才会升级 ABI 版本
- **Host 函数表追加式演进**：`mpl_host_api_t` 新字段（如 API v2 的控制面系列接口）严格追加在 `ctx` 之后，禁止插入中间——旧版 Native 插件读取已有字段的偏移保持不变，无需重新编译
- **生命周期解耦**：插件运行时与 GUI 界面彻底解耦，无论在 GUI、CLI 还是 TUI 启动服务端，核心启动流程均会自动加载并执行已启用的插件，支持无头环境运行
- **minHostVersion**：插件声明所需最低 MicYou 宿主应用版本（SemVer，例如 `2.1.0`）。宿主把自身版本（`HOST_VERSION`，由工作区版本自动同步）与它做完整的三段比较，低于要求即拒绝加载；`major/minor/patch` 任一段不满足都会被拒绝，而不是只比较 major
- **WASM 导入表**：新增导入不影响不引用它的旧插件；导入签名不匹配的调用在宿主侧报错而非崩溃
- **线协议**：`MessageWrapper` 采用 proto3 未知字段兼容——旧客户端不识别 `pluginMessage` 字段时自动跳过，新字段只在双方都支持时生效
- **错误码**：wire 错误码（0-12）冻结，新增错误只追加
- **配置格式**：`plugin-state.json` 按 id 组织，缺失字段走默认值
- **API 变更流程**：新增能力 = 追加字段 + 新能力名（旧插件不受影响）；破坏性变更 = 升级 `HOST_API_VERSION`，新旧插件并存（按 apiVersion 分发）

## 安全模型

### Native 插件

- 全权进程内执行，等同本地应用代码——宿主只做「能力授权」与「版本校验」，不做代码沙箱
- 安装来源信任：用户手动放入插件目录，或未来接入签名校验（预留）
- 实时安全：`realtimeSafe` 声明是宿主信任依据，违反者造成音频质量问题由插件负责

### WASM 插件

- **内存沙箱**：wasmi 解释器，插件无法越界访问宿主内存
- **燃料计量**：每次入口调用注入固定燃料预算（默认 100 000），死循环被 trap，插件无法挂起宿主
- **能力授权**：所有 host 函数按 manifest capabilities 逐调用校验，未授权返回 `MPL_ERR_PERMISSION`
- **无系统访问**：不提供 WASI / 文件 / 网络导入，需系统能力请用 Native 插件

### 消息安全

- 跨端消息按插件 id 路由，未注册的 target 返回 unknown plugin
- RPC 带超时，不会无限阻塞
- 总线事件可广播到远端设备——插件需自行评估发布内容的敏感性

### 审计与日志

- 插件日志独立缓冲（每插件 500 行环形），GUI 可查看
- 宿主日志记录插件启停、加载失败与错误

## 性能预算

- DSP 插件节点：单节点单帧处理建议 < 1 ms（48 kHz / 480 样本帧）
- WASM DSP：best-effort，禁止声称 realtimeSafe
- 插件调度：音频线程通过 `Arc<Mutex<PluginInstance>>` 持有节点句柄，稳态无锁竞争（插件仅启停时变更）
- 总线消息：控制通道容量 100，`try_send` 非阻塞发送，满则丢弃并报错
