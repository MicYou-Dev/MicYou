<div align="center">
  
  <h1>MicYou</h1>
  
  <img src="./img/app_icon.png" width="128" height="128" />

  <br>

  <a href="https://hellogithub.com/repository/LanRhyme/MicYou" target="_blank"><img src="https://abroad.hellogithub.com/v1/widgets/recommend.svg?rid=14f1a7bc70dc4b3daa2b4668200286e3&claim_uid=9B3umTywnclH56q&theme=neutral" alt="Featured｜HelloGitHub" style="width: 250px; height: 54px;" width="250" height="54" /></a>
  <a href="https://trendshift.io/repositories/25451" target="_blank"><img src="https://trendshift.io/api/badge/repositories/25451" alt="LanRhyme%2FMicYou | Trendshift" style="width: 250px; height: 55px;" width="250" height="55"/></a>

  <a href="./README_zh-cn.md">简体中文</a> | <b>繁體中文</b> | <a href="./README.md">English</a>

  <a href="https://aur.archlinux.org/packages/micyou-bin"><img alt="AUR Version" src="https://img.shields.io/aur/version/micyou-bin?logo=archlinux&label=micyou-bin"></a>
  <a href="https://qm.qq.com/q/V16hPpWPKO"><img alt="QQ" src="https://img.shields.io/badge/QQ-995452107-12B7F5?style=flat&logo=qq&logoColor=white"></a>
  <a href="https://t.me/MicYouChannel"><img alt="TG" src="https://img.shields.io/badge/Telegram-@MicYouChannel-2CA5E0?style=flat&logo=telegram&logoColor=white"></a>

  <h6>贊助我</h6>

  <a href="https://afdian.com/a/LanRhyme" target="_blank" rel="noopener noreferrer"><img src="https://img.shields.io/badge/愛發電-@LanRhyme-946ce6?style=for-the-badge&logo=afdian&logoColor=white" alt="愛發電"></a>

  MicYou 可將手機變為高品質的 PC 麥克風。

  原生 Android 與 iOS 客戶端 · Windows / Linux / macOS 桌面端 · Wi-Fi / USB / Web

</div>

## 主要功能

- Android 客戶端支援 Wi-Fi 與 USB (ADB)，[iOS 客戶端](https://github.com/MicYou-Dev/MicYou-iOS)支援 Wi-Fi 與 USB；Web 模式可透過 QR Code 直接連接手機瀏覽器，無需安裝客戶端
- 桌面端支援 Windows、Linux 與 macOS，並提供完整 GUI、低資源占用 CLI 及互動式 TUI 儀表板
- 可設定包含 AI 與傳統降噪、聲學回聲消除 (AEC)、去混響、等化器、放大、自動增益控制 (AGC) 及語音活動偵測 (VAD) 的音訊處理鏈
- 可即時查看音量、位元率、延遲、抖動、封包遺失率及緩衝區狀態，並隨時靜音或監聽輸入音訊
- 透過 Windows 的 VB-CABLE、Linux 的 PipeWire 或 macOS 的 BlackHole，將手機音訊用於通話、遊戲、直播與錄音軟體
- 支援 Material 3、深淺色主題、動態取色、自訂背景、桌面袖珍模式、系統匣控制及多語言介面
- 桌面端 GUI、CLI 與 TUI 共用連線、DSP、語言及主題設定
- 支援外掛程式擴充生態（Native 與 WASM 雙執行時期），內建應用程式內外掛市場，支援自訂 DSP 節點、音效板與控制擴充

## 軟體截圖

### Android 客戶端
|                            主畫面                             |                           設定                               |
|:-----------------------------------------------------------:|:-------------------------------------------------------------:|
| <img src="img/android_screenshot_main_zh-tw.png" width="300" /> | <img src="img/android_screenshot_settings_zh-tw.png" width="300" /> |

### 桌面端
|                              主畫面                               |                                  設定                                   |
|:-----------------------------------------------------------------:|:-----------------------------------------------------------------------:|
| <img src="img/desktop_screenshot_zh-tw.png" width="480" /> | <img src="img/desktop_screenshot_settings_zh-tw.png" width="480" /> |

## 使用說明

1. 從 [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) 下載對應作業系統的桌面端安裝套件及 Android APK；iPhone 或 iPad 請從 [MicYou for iOS](https://github.com/MicYou-Dev/MicYou-iOS/releases/latest) 下載 IPA，並參照[在 iOS 上安裝](https://micyou.top/zh-TW/docs/how-to-install-in-ios)側載
2. 按照[快速開始指南](https://micyou.top/zh-TW/docs/quick-start)為電腦設定虛擬麥克風
3. 選擇連線方式：
   - Wi-Fi：確保手機與電腦位於同一網路，並在桌面端選擇 Wi-Fi 模式；Android 客戶端同樣選擇 Wi-Fi 模式，iOS 客戶端會自動搜尋到電腦，也可以手動輸入 IP 位址
   - USB：透過資料線連接手機，並在桌面端選擇 USB 模式；Android 需啟用 USB 偵錯並在客戶端選擇 USB 模式，iOS 在客戶端選擇「USB 傳輸線」（Windows 需安裝 iTunes）
   - Web：在桌面端選擇 Web 模式，再用手機瀏覽器掃描 QR Code，無需安裝客戶端
4. 啟動音訊傳輸，然後在通話、遊戲、直播或錄音軟體中選擇已設定的虛擬麥克風

更多平台安裝步驟與疑難排解，請參閱 [MicYou 官方文件](https://micyou.top/zh-TW/docs/quick-start)。

## 技術堆疊

- Android：Kotlin、Jetpack Compose、Material 3
- iOS：Swift、SwiftUI（[獨立儲存庫](https://github.com/MicYou-Dev/MicYou-iOS)）
- 桌面端：Tauri 2、Rust、Vue 3、Vite、Tailwind CSS
- 協定與音訊：由 Rust 後端與共用 Crate 實作傳輸、緩衝、DSP 及虛擬音訊裝置整合

## 貢獻指南

我們歡迎各種形式的貢獻！無論是回報 Bug、提出功能建議、協助翻譯還是貢獻程式碼，都請參閱我們的 [貢獻指南](./CONTRIBUTING_zh-tw.md) 以開始參與。

## 貢獻者
<a href="https://github.com/MicYou-Dev/MicYou/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=MicYou-Dev/MicYou" />
</a>

Made with [contrib.rocks](https://contrib.rocks).

## Star History

<a href="https://www.star-history.com/?repos=micyou-dev%2Fmicyou&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=micyou-dev/micyou&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=micyou-dev/micyou&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=micyou-dev/micyou&type=date&legend=top-left" />
 </picture>
</a>

## 致謝

特別感謝 [a2heng](https://github.com/a2heng) 開源 [AEC7](https://github.com/a2heng/lightweight-aec-48k) 與 [PureVox](https://github.com/a2heng/lightweight-denoise-48k)，為 MicYou 提供聲學回聲消除與 AI 降噪能力。

特別感謝 [河南省教育科研網開源軟件鏡像站](https://mirrors.ha.edu.cn/) 為本專案提供鏡像下載服務。

特別感謝 [重慶大學開源軟體鏡像站](https://mirrors.cqu.edu.cn/) 過往的穩定支持。

特別感謝 [Mirror 醬](https://mirrorchyan.com/zh/get-start) 為本專案提供高速鏡像下載服務。

特別感謝所有的 [貢獻者](https://github.com/MicYou-Dev/MicYou/graphs/contributors)，你們讓專案變得更好。

## 許可證

本專案基於 [GNU General Public License v3.0 附帶 MicYou Plugin Exception](./LICENSE) 開源

