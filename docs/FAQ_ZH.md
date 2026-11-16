本文档已移至 https://micyou.top/docs/quick-start 。如果您希望为本文档做出贡献，请访问 https://github.com/MicYou-Dev/Website-MicYou

<!-- # MicYou 文档

<p align="center">
   <b>简体中文</b> | <a href="./FAQ_TW.md">繁體中文</a> | <a href="./FAQ.md">English</a>
</p>

## 快速开始

### 1. 下载 ADB

从 [Android Developers](https://developer.android.com/tools/releases/platform-tools?hl=zh_cn) 下载，或使用系统包管理器：

- `winget install -e --id Google.PlatformTools`（Windows）
- `sudo apt install android-tools-adb`（Ubuntu/Debian）
- `sudo pacman -S android-tools`（Arch）
- 其他平台请参考 [官方文档](https://developer.android.com/tools/releases/platform-tools)

### 2. 启用 USB 调试

以 OneUI 8 为例：

1. 进入「设置」> 「关于手机」
2. 点击「软件信息」，找到「编译编号」，点击 7 次以启用开发者选项
3. 返回「设置」> 「开发者选项」，启用「USB 调试」

### 3. 使用 USB 连接

请使用稳定的数据线，并确保桌面端和 Android 客户端都将连接模式切换为 `USB`。

### 4. 使用 Wi-Fi 连接

请确保 Android 设备与 PC 处于同一网络环境，并且桌面端和 Android 客户端都将连接模式切换为 `Wi-Fi`。

### Android

1. 下载并安装 APK 到您的 Android 设备
2. 确保您的设备与 PC 处于同一网络（Wi-Fi 模式），或通过 USB 连接

### Windows

1. 运行桌面端应用程序
2. 配置连接模式以匹配 Android 应用

### macOS

为了获得更好的使用体验，建议通过 Homebrew 安装以下依赖：

```bash
brew install blackhole-2ch --cask
brew install switchaudio-osx --formulae
```

> **BlackHole** 必须安装（虚拟音频驱动）。若未安装 Homebrew，请前往 https://existential.audio/blackhole/download/ 下载安装程序并安装。

完成后请重启您的 Mac。

在 [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) 下载应用并安装到“应用程序”目录后，首次启动可能会被 Gatekeeper 拦截。

- 若提示「不受信任的开发者」，请前往 **「系统设置」/「系统偏好设置」 > 「隐私与安全」** 允许应用运行。
- 若提示「应用已损坏」，请在终端执行：

```bash
sudo xattr -r -d com.apple.quarantine /Applications/MicYou.app
```
执行时需要输入您的用户账号密码，输入时密码不可见，完成后回车即可。

### Linux

#### 使用预编译包（推荐）
预编译包可在 [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) 下载。

**DEB 包（适用于 Debian/Ubuntu/Mint 等发行版）：**
```bash
# 从 GitHub Releases 下载 .deb 包
sudo dpkg -i MicYou-*.deb
# 如果缺少依赖：
sudo apt install -f
```

**RPM 包（适用于 Fedora/RHEL/openSUSE 等发行版）：**
```bash
# 从 GitHub Releases 下载 .rpm 包
sudo rpm -i MicYou-*.rpm
# 或者使用 dnf/yum：
sudo dnf install MicYou-*.rpm
```

**AUR 仓库（适用于 Arch Linux 及其衍生发行版）：**
```bash
# 克隆 AUR 仓库并自动安装软件包及其依赖
git clone https://aur.archlinux.org/micyou-bin.git
cd micyou-bin
makepkg -si

# 或者使用 paru 等 AUR helpers
paru -S micyou-bin
```

**运行应用：**
```bash
# 安装后可以从应用菜单运行 MicYou
# 或者从终端运行：
MicYou
```

## 常见问题

### 无法连接设备

#### Wi-Fi 模式

1. **确认防火墙设置**

   Windows 防火墙可能会拦截入站连接。请按照以下方法手动放行端口：

   1. 按下 `Win+R`，输入 `powershell`，同时按住 `Ctrl+Shift`，点击「确定」以管理员身份运行 PowerShell。
   2. 输入以下命令：

      ```powershell
      New-NetFirewallRule -DisplayName "MicYou-6000-TCP" -Direction Inbound -LocalPort 6000 -Protocol TCP -Action Allow
      New-NetFirewallRule -DisplayName "MicYou-6001-UDP" -Direction Inbound -LocalPort 6001 -Protocol UDP -Action Allow
      ```

      > MicYou 默认使用 TCP 端口 `6000`（控制通道）和 UDP 端口 `6001`（音频数据）。如已修改端口号，请将命令中的端口号替换为实际值。

      若未出现任何错误提示，表示操作成功，可以重新尝试连接。

2. **检查设备是否在同一子网**

   - 确保 Android 手机和 PC 连接的是**同一个**路由器的 Wi-Fi
   - 确保路由器已关闭 **AP 隔离** 或 **网络设备隔离** 功能（详情请参阅路由器说明书）

> [!TIP]
> 高级用户可尝试使用 Nmap 或 ping 等工具检查连接性

#### USB (ADB) 模式

1. **开启开发者选项**

   > 此处列出的步骤可能不适用于所有设备，**建议使用搜索引擎**查找适用于您设备的 ADB 教程。

   - 在手机设置中找到“关于本机”，连续点击 7 次“系统版本号”开启开发者选项
   - 进入开发者选项，开启 **USB 调试**

2. **确认 ADB 连接**

   > 电脑端需要安装 ADB 工具（参见第 1 步：下载 ADB）。

   运行以下命令，确认有且仅有一个设备已连接：

   ```bash
   adb devices
   ```

   如果列出了多个设备，则需要指定目标设备进行端口转发：

   ```bash
   adb -s <设备序列号> reverse tcp:6000 tcp:6000
   ```

   > 设备序列号可在 `adb devices` 的输出中找到。

### 连接设备后无声音输出

请确保 VB-Audio 驱动已正确安装，且以下设备均**未被禁用**：

- **输出设备**：CABLE Input (VB-Audio Virtual Cable)
- **输入设备**：CABLE Output (VB-Audio Virtual Cable)

检查方式：打开「设置」> 「声音」，验证两个设备均为**已启用**状态：

![输入设备](https://github.com/user-attachments/assets/1cf5f97f-1647-4fb0-a152-85be2697df39)
![输出设备](https://github.com/user-attachments/assets/9e9ef42d-186f-42a6-ba4d-7b1a3815f860) -->
