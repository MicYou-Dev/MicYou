此文檔已移至 https://micyou.top/zh-TW/docs/quick-start 。若您想為此文檔貢獻心力，請造訪 https://github.com/MicYou-Dev/Website-MicYou

<!-- # MicYou 文檔

<p align="center">
  <a href="./FAQ_ZH.md">简体中文</a> | <b>繁體中文</b> | <a href="./FAQ.md">English</a>
</p>

## 快速開始

### 1. 下載 ADB

從 [Android Developers](https://developer.android.com/tools/releases/platform-tools?hl=zh_cn) 下載，或使用套件管理工具：

- `winget install -e --id Google.PlatformTools`（Windows）
- `sudo apt install android-tools-adb`（Ubuntu/Debian）
- `sudo pacman -S android-tools`（Arch）
- 其他平台請參考 [官方文檔](https://developer.android.com/tools/releases/platform-tools)

大多數情況下 ADB 會自動加入系統環境變數，如未加入請自行設定。

### 2. 啟用 USB 偵錯

以 OneUI 8 為例：

1. 進入「設定」> 「關於手機」
2. 點選「軟體資訊」，找到「編譯編號」，連點 7 次以啟用開發者選項
3. 返回「設定」> 「開發者選項」，啟用「USB 偵錯」

### 3. 使用 USB 連線

請使用品質穩定的傳輸線，並在桌面端與 Android 應用同時將連線模式切換為 `USB`。

### 4. 使用 Wi‑Fi 連線

請確保 Android 裝置與電腦位於同一網路，並在桌面端與 Android 應用同時切換成 `Wi‑Fi` 模式。

### Android

1. 下載並安裝 APK 到您的 Android 裝置
2. 確保您的裝置與 PC 位於同一網路（Wi-Fi 模式），或透過 USB 連線

### Windows

1. 執行桌面端應用程式
2. 設定連線模式以匹配 Android 應用


### macOS

為了獲得更佳的使用體驗，建議透過 Homebrew 安裝以下套件：

```bash
brew install blackhole-2ch --cask
brew install switchaudio-osx --formulae
```

> **BlackHole** 為必要軟體（虛擬音訊驅動程式）。若未安裝 Homebrew，可至 https://existential.audio/blackhole/download/ 下載安裝程式。

安裝完成後請重新啟動電腦。

從 [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) 下載並安裝到「應用程式」資料夾。首次開啟可能會被 Gatekeeper 攔截：

- 若顯示「不受信任的開發者」，請至「系統設定」> 「隱私權與安全性」允許應用程式
- 若顯示「應用程式已損毀」，在終端執行：

```bash
sudo xattr -r -d com.apple.quarantine /Applications/MicYou.app
```

執行時需輸入您的使用者帳號密碼，輸入時密碼不可見，完成後回車即可。

### Linux

#### 使用預編譯套件（推薦）

預編譯套件可在 [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) 下載。根據您的 Linux 發行版選擇相應的套件。

**DEB 套件（Debian/Ubuntu/Mint 等）：**

```bash
sudo dpkg -i MicYou-*.deb
# 如果缺少依賴：
sudo apt install -f
```

**RPM 套件（Fedora/RHEL/openSUSE 等）：**

```bash
sudo rpm -i MicYou-*.rpm
# 或使用 dnf/yum：
sudo dnf install MicYou-*.rpm
```

**AUR（Arch Linux 及衍生發行版）：**

```bash
git clone https://aur.archlinux.org/micyou-bin.git
cd micyou-bin
makepkg -si
```

或使用 AUR 輔助程式（如 paru）：

```bash
paru -S micyou-bin
```

**執行應用：**

從應用程式選單執行，或在終端執行：

```bash
MicYou
```

## 常見問題

### 無法連線裝置

#### Wi-Fi 模式

1. **確認防火牆設定**

   Windows 防火牆可能會攔截入站連線。請按照以下步驟手動放行連接埠：

   1. 按下 `Win+R`，輸入 `powershell`，同時按住 `Ctrl+Shift`，點選「確定」以系統管理員身分執行 PowerShell。
   2. 輸入以下命令：

      ```powershell
      New-NetFirewallRule -DisplayName "MicYou-6000-TCP" -Direction Inbound -LocalPort 6000 -Protocol TCP -Action Allow
      New-NetFirewallRule -DisplayName "MicYou-6001-UDP" -Direction Inbound -LocalPort 6001 -Protocol UDP -Action Allow
      ```

      > MicYou 預設使用 TCP 連接埠 `6000`（控制通道）和 UDP 連接埠 `6001`（音訊資料）。如已修改連接埠號，請將命令中的連接埠號替換為實際值。

      若未出現任何錯誤訊息，表示操作成功，可嘗試重新連線。

2. **檢查裝置是否在同一子網路**

   - 確保 Android 手機與 PC 連接的是**同一個**路由器的 Wi-Fi
   - 確保路由器已關閉 **AP 隔離** 或 **網路裝置隔離** 功能（詳情請參考路由器說明書）

> [!TIP]
> 進階使用者可使用 Nmap 或 ping 等工具檢查連線性。

#### USB (ADB) 模式

1. **開啟開發者選項**

   > 不同裝置的步驟可能不同，**建議使用搜尋引擎**搜尋為您的裝置開啟 ADB 的教學。

   - 在手機設定中找到「關於本機」，連續點擊 7 次「系統版本號」以開啟開發者選項
   - 進入開發者選項，開啟 **USB 偵錯**

2. **確認 ADB 連線**

   > 電腦端需安裝 ADB 工具（參見第 1 步：下載 ADB）。

   執行以下命令確認有且僅有一個裝置已連線：

   ```bash
   adb devices
   ```

   若列出多個裝置，需指定目標裝置進行連接埠轉發：

   ```bash
   adb -s <裝置序列號> reverse tcp:6000 tcp:6000
   ```

   > 裝置序列號可在 `adb devices` 的輸出中找到。

### 連線裝置後無聲音輸出

請確保 VB-Audio 驅動已正確安裝，且以下裝置未被停用：

- **輸出裝置**：CABLE Input (VB-Audio Virtual Cable)
- **輸入裝置**：CABLE Output (VB-Audio Virtual Cable)

檢查方式：開啟「設定」> 「聲音」，確保以下兩項均為**已啟用**狀態：

![輸入裝置](https://github.com/user-attachments/assets/1cf5f97f-1647-4fb0-a152-85be2697df39)
![輸出裝置](https://github.com/user-attachments/assets/9e9ef42d-186f-42a6-ba4d-7b1a3815f860) -->
