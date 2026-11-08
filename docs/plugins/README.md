# MicYou 插件系统

MicYou 插件系统允许第三方为桌面端与（未来）安卓端扩展能力：实时 DSP 节点、工具逻辑、UI 面板与跨端同步桥

| 文档 | 说明 |
| --- | --- |
| [总览](overview.md) | 架构、双运行时、DSP 集成与跨端同步模型 |
| [开发指南](development-guide.md) | 编写 Native / WASM 插件、Manifest、Host API、实时安全规范 |
| [用户指南](user-guide.md) | 安装卸载、GUI 管理、配置与跨端同步使用 |
| [API 参考](api-reference.md) | Host API、Plugin API、消息协议、错误码与权限清单 |
| [架构与扩展](architecture-extensibility.md) | 安卓端扩展计划、版本兼容、安全模型 |
| [市场与许可规范](marketplace-policy.md) | 插件许可证例外条款、市场准入要求与安全审核规范 |

## 快速导航

- 想写插件：读 [开发指南](development-guide.md)
- 想装插件：读 [用户指南](user-guide.md)
- 想了解协议与权限：读 [API 参考](api-reference.md)
- 想了解市场政策与许可条款：读 [市场与许可规范](marketplace-policy.md)
- 想知道安卓端怎么规划：读 [架构与扩展](architecture-extensibility.md)

## 示例插件

| 示例 | 运行时 | 类型 | 位置 |
| --- | --- | --- | --- |
| native-soundpad | Native (cdylib) | 音效板：按钮面板 + 专属设置页 + 快捷键 + 音频播放 + 插件自主开窗 | `plugins/examples/native-soundpad/` |
| wasm-voicechanger | WASM | 变声器（实时 DSP in wasmi）：专属设置页 + 配置热更新 | `plugins/examples/wasm-voicechanger/` |
| wasm-audioinspector | WASM | 音频状态监视器（interval 采样 + 面板实时显示） | `plugins/examples/wasm-audioinspector/` |

## Host API 能力总表

| 类别 | API | 能力 |
| --- | --- | --- |
| 控制面 | `get_muted` / `set_muted` | control.observe / control.intercept |
| 控制面 | `get_monitoring` / `set_monitoring` | control.observe / control.intercept |
| 控制面 | `get_dsp_settings` / `set_dsp_settings` | control.observe / control.intercept |
| 日志 | `log` | 无 |
| 配置 | `get_config` / `set_config` | config.read / config.write |
| 事件 | `emit_event` | event.emit |
| 消息 | `send_message` | message.send |
| 音频 | `audio_state` / `play_sound` | audio.state / audio.play |
| 设备 | `connected_devices` | device.list |
| 文件 | `fs_read` / `fs_write`（插件目录沙箱） | fs.read / fs.write |
| 定时器 | `set_timeout` / `clear_timeout` / `set_interval` / `clear_interval` | 无 |
| 网络 | `http_request`（异步回调） | network.io |
| 浏览器 | `open_url` | open.url |
| 通知 | `notify` | 无 |
| 剪贴板 | `clipboard_read` / `clipboard_write` | clipboard.read / clipboard.write |
| 环境 | `locale` / `host_info` / `plugin_dir` | 无 |
| UI | `open_window` / 专属设置页（iframe 桥） | 无 |
| 快捷键 | `register_hotkey` | 无（仅 X11） |
| 宿主事件 | 设备连接/断开、静音、监听、DSP变更 → `handle_event` | 无（静音与监听等控制面事件需 control.observe） |
| 依赖联动 | `dependencies`（前置插件声明） | 无 |

## 安装与更新

- 导入 zip 前展示**权限预览**（能力清单/作者/许可），确认后才安装
- manifest 声明 `updateUrl` 后可**检查更新**（semver 对比）并一键更新
- 桌面端**启动时在后台自动检查一次**插件更新（非阻塞、失败静默），结果缓存到「设置 → 插件」的更新横幅；工具栏仍可手动刷新
- 导入/扫描会校验 `apiVersion`（宿主支持 1..2）和 `minHostVersion`（宿主应用版本），不兼容的插件被拒绝并记录原因（导入时直接返回错误）
- manifest `configSchema` 声明字段后宿主**自动生成配置表单**（滑杆/开关/下拉）
- 插件生命周期与 GUI 界面解耦，CLI 与 TUI 服务端启动时均能自动加载并运行已启用插件

## 开发工具

```bash
micyou-cli plugin create dev.micyou.myplugin --kind utility --capabilities config.read   # wasm 骨架默认 Rust（自动编译）
micyou-cli plugin create dev.micyou.myplugin --lang wat                                 # 高级场景才手写 WAT
micyou-cli plugin create dev.micyou.mynative --runtime native --kind dsp
micyou-cli plugin validate ./myplugin
micyou-cli plugin install ./myplugin            # 一键部署到应用插件目录
micyou-cli plugin dev ./myplugin                # 监听变更自动重装（开发循环）
micyou-cli plugin package ./myplugin -o out.zip
micyou-cli plugin bump ./myplugin               # 版本 patch +1
```bash
# 常用 CLI 插件命令（在 tauri-app/ 下通过 cargo 运行或直接使用 micyou-cli）
micyou-cli plugin list                          # 列出已安装插件及状态
micyou-cli plugin enable <id>                   # 启用指定插件
micyou-cli plugin disable <id>                  # 禁用指定插件
micyou-cli plugin install ./myplugin            # 安装插件目录到配置目录
micyou-cli plugin validate ./myplugin           # 校验清单与入口文件
micyou-cli plugin package ./myplugin -o out.zip # 打包为分发 zip
micyou-cli plugin dev ./myplugin                # 监听变更自动重装
micyou-cli plugin create dev.micyou.demo        # 生成 WASM 插件骨架
```

### 插件市场

GUI 客户端提供内置插件市场（**设置 → 插件 → 插件市场**）：
- 在线浏览官方索引的插件（封面、能力要求、支持平台等）
- 安装前支持权限与风险确认
- 支持检查并一键更新版本

## 代码结构

```text
tauri-app/crates/micyou-plugin/            # 插件框架核心（清单校验、双运行时、消息总线、DSP 集成）
├── src/manifest.rs                        # 清单模型与结构校验
├── src/plugin.rs                          # 插件统一接口抽象
├── src/native.rs                          # Native 动态库加载器（libloading + C ABI）
├── src/wasm.rs                            # WASM 沙箱运行时（wasmi）
├── src/abi.rs                             # C ABI 宿主接口桥接
├── src/dsp.rs                             # DSP 节点注册与音频链调用
├── src/bus.rs                             # 插件消息总线（Pub/Sub 与 RPC）
├── src/sync.rs                            # 跨端线协议编解码
├── include/micyou_plugin_abi.h            # Native C ABI 接口头文件
├── fixtures/                              # 测试用夹具产物
└── tests/                                 # 单元与集成测试
tauri-app/crates/micyou-core/src/plugins/  # 服务端宿主实现（PluginHost / API 桥接 / 持久化）
tauri-app/src-tauri/src/commands/plugins.rs# GUI Tauri 命令转接
tauri-app/src/features/plugins/            # 桌面端管理界面（Vue 3）
plugins/examples/                          # 官方示例插件源码
docs/plugins/                              # 插件系统文档
```
