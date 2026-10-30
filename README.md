<div align="center">
  
  <h1>MicYou</h1>
  
  <img src="./img/app_icon.png" width="128" height="128" />

  <br>

  <a href="https://hellogithub.com/repository/LanRhyme/MicYou" target="_blank"><img src="https://abroad.hellogithub.com/v1/widgets/recommend.svg?rid=14f1a7bc70dc4b3daa2b4668200286e3&claim_uid=9B3umTywnclH56q&theme=neutral" alt="Featured｜HelloGitHub" style="width: 250px; height: 54px;" width="250" height="54" /></a>
 <a href="https://trendshift.io/repositories/25451" target="_blank"><img src="https://trendshift.io/api/badge/repositories/25451" alt="LanRhyme%2FMicYou | Trendshift" style="width: 250px; height: 55px;" width="250" height="55"/></a>

  <a href="./README_zh-cn.md">简体中文</a> | <a href="./README_zh-tw.md">繁體中文</a> | <b>English</b>

  <a href="https://aur.archlinux.org/packages/micyou-bin"><img alt="AUR Version" src="https://img.shields.io/aur/version/micyou-bin?logo=archlinux&label=micyou-bin"></a>
  <a href="https://qm.qq.com/q/V16hPpWPKO"><img alt="QQ" src="https://img.shields.io/badge/QQ-995452107-12B7F5?style=flat&logo=qq&logoColor=white"></a>
  <a href="https://t.me/MicYouChannel"><img alt="TG" src="https://img.shields.io/badge/Telegram-@MicYouChannel-2CA5E0?style=flat&logo=telegram&logoColor=white"></a>

  <h6>Support Me</h6>

  <a href="https://afdian.com/a/LanRhyme" target="_blank" rel="noopener noreferrer"><img src="https://img.shields.io/badge/afdian-@LanRhyme-946ce6?style=for-the-badge&logo=afdian&logoColor=white" alt="afdian"></a>

  MicYou turns your phone into a high-quality microphone for your PC.

  Native Android and iOS clients · Windows / Linux / macOS desktop server · Wi-Fi / USB / Web

</div>

## Features

- Stream from the Android app over Wi-Fi or USB (ADB), from the [iOS app](https://github.com/MicYou-Dev/MicYou-iOS) over Wi-Fi or USB, or use Web mode directly from a mobile browser by scanning a QR code.
- Run the desktop server on Windows, Linux, and macOS with a full GUI, a low-overhead CLI, or an interactive TUI dashboard.
- Configure a processing pipeline with AI and traditional noise suppression, acoustic echo cancellation (AEC), dereverberation, equalizer, amplification, automatic gain control (AGC), and voice activity detection (VAD).
- Inspect audio level, bitrate, latency, jitter, packet loss, and buffer status; mute or monitor incoming audio at any time.
- Route audio through VB-CABLE on Windows, PipeWire on Linux, or BlackHole on macOS for use in calls, games, streaming, and recording apps.
- Customize Material 3 interfaces with light/dark themes, dynamic colors, custom backgrounds, desktop pocket mode, system tray controls, and multiple languages.
- Share connection, DSP, language, and theme preferences between the desktop GUI, CLI, and TUI.
- Extend functionality with a dual-runtime plugin system (Native and WASM) and an in-app marketplace for custom DSP nodes, soundpads, and control widgets.

## Screenshots

### Android App

|                        Main Screen                        |                           Settings                            |
|:---------------------------------------------------------:|:-------------------------------------------------------------:|
| <img src="img/android_screenshot_main.png" width="300" /> | <img src="img/android_screenshot_settings.png" width="300" /> |

### Desktop App

|                         Main Screen                          |                              Settings                               |
|:------------------------------------------------------------:|:-------------------------------------------------------------------:|
| <img src="img/desktop_screenshot.png" width="480" /> | <img src="img/desktop_screenshot_settings.png" width="480" /> |

## Getting Started

1. Download the desktop package for your operating system and the Android APK from [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases). On iPhone or iPad, get the IPA from [MicYou for iOS](https://github.com/MicYou-Dev/MicYou-iOS/releases/latest) and sideload it by following [Install on iOS](https://micyou.top/en/docs/how-to-install-in-ios).
2. Set up a virtual microphone for your platform by following the [Quick Start guide](https://micyou.top/en/docs/quick-start).
3. Choose a connection method:
   - Wi-Fi: Keep the phone and PC on the same network, then select Wi-Fi on the desktop. The Android app connects after you select Wi-Fi; the iOS app finds the computer automatically, or you can enter its IP address.
   - USB: Connect the phone with a data cable and select USB on the desktop. On Android, enable USB debugging and select USB in the app; on iOS, choose USB Cable in the app (Windows needs iTunes installed).
   - Web: Select Web on the desktop and scan the QR code with a mobile browser—no app is required.
4. Start streaming, then select the configured virtual microphone in your call, game, streaming, or recording app.

For platform-specific installation steps and troubleshooting, visit the [MicYou documentation](https://micyou.top/en/docs/quick-start).

For browser connections across private networks, see [Web mode across private networks](docs/web-private-networks.md).

## Technology

- Android: Kotlin, Jetpack Compose, Material 3
- iOS: Swift, SwiftUI ([separate repository](https://github.com/MicYou-Dev/MicYou-iOS))
- Desktop: Tauri 2, Rust, Vue 3, Vite, Tailwind CSS
- Protocol and audio: A Rust backend and shared crates for transport, buffering, DSP, and virtual audio device integration

## Contributing

We welcome contributions of all kinds! Whether you want to report a bug, suggest a feature, help with translations, or contribute code, please check our [Contributing Guidelines](./CONTRIBUTING.md) to get started.

## Contributors

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

## Acknowledgments

Special thanks to [a2heng](https://github.com/a2heng) for open-sourcing [AEC7](https://github.com/a2heng/lightweight-aec-48k) and [PureVox](https://github.com/a2heng/lightweight-denoise-48k), which power MicYou's acoustic echo cancellation and AI noise suppression.

Special thanks to [HeRNET Open Source Software Mirror](https://mirrors.ha.edu.cn/) for providing a mirror download service for this project.

Special thanks to [CQU Open Source Software Mirror](https://mirrors.cqu.edu.cn/) for your consistent support in the past.

Special thanks to [MirrorChyan](https://mirrorchyan.com/en/get-start) for providing a high-speed mirror download service for this project.

Special thanks to all the [contributors](https://github.com/MicYou-Dev/MicYou/graphs/contributors) for helping to make the project even better.

## License

This project is licensed under the [GNU General Public License v3.0 with MicYou Plugin Exception](./LICENSE)

