<div align="center">
  
  <h1>MicYou</h1>
  
  <img src="./img/app_icon.png" width="128" height="128" />

  <br>

  <a href="https://hellogithub.com/repository/LanRhyme/MicYou" target="_blank"><img src="https://abroad.hellogithub.com/v1/widgets/recommend.svg?rid=14f1a7bc70dc4b3daa2b4668200286e3&claim_uid=9B3umTywnclH56q&theme=neutral" alt="Featured｜HelloGitHub" style="width: 250px; height: 54px;" width="250" height="54" /></a>
 <a href="https://trendshift.io/repositories/25451" target="_blank"><img src="https://trendshift.io/api/badge/repositories/25451" alt="LanRhyme%2FMicYou | Trendshift" style="width: 250px; height: 55px;" width="250" height="55"/></a>

  <a href="./README_zh-cn.md">简体中文</a> | <a href="./README_zh-tw.md">繁體中文</a> | <a href="./README.md">English</a> | <b>日本語</b>

  <a href="https://aur.archlinux.org/packages/micyou-bin"><img alt="AUR Version" src="https://img.shields.io/aur/version/micyou-bin?logo=archlinux&label=micyou-bin"></a>
  <a href="https://qm.qq.com/q/V16hPpWPKO"><img alt="QQ" src="https://img.shields.io/badge/QQ-995452107-12B7F5?style=flat&logo=qq&logoColor=white"></a>
  <a href="https://t.me/MicYouChannel"><img alt="TG" src="https://img.shields.io/badge/Telegram-@MicYouChannel-2CA5E0?style=flat&logo=telegram&logoColor=white"></a>

  <h6>開発者を支援する</h6>

  <a href="https://afdian.com/a/LanRhyme" target="_blank" rel="noopener noreferrer"><img src="https://img.shields.io/badge/afdian-@LanRhyme-946ce6?style=for-the-badge&logo=afdian&logoColor=white" alt="afdian"></a>

  MicYou は、スマートフォンを PC 用の高品質なマイクに変えます。

  ネイティブの Android / iOS クライアント · Windows / Linux / macOS 用デスクトップサーバー · Wi-Fi / USB / Web

</div>

## 機能

- Android アプリから Wi-Fi または USB (ADB) で、[iOS アプリ](https://github.com/MicYou-Dev/MicYou-iOS) から Wi-Fi または USB でストリーミングできます。また、QR コードを読み取ってモバイルブラウザから直接 Web モードを使うこともできます。
- デスクトップサーバーは Windows、Linux、macOS で動作し、フル機能の GUI、低負荷の CLI、インタラクティブな TUI ダッシュボードから選べます。
- AI および従来型のノイズ抑制、エコーキャンセレーション (AEC)、残響除去、イコライザー、増幅、自動ゲイン制御 (AGC)、音声区間検出 (VAD) を組み合わせた処理パイプラインを設定できます。
- 音声レベル、ビットレート、レイテンシ、ジッター、パケットロス、バッファの状態を確認でき、受信した音声はいつでもミュートやモニタリングができます。
- Windows では Steam Streaming Microphone、Linux では PipeWire、macOS では BlackHole を経由して音声をルーティングし、通話、ゲーム、配信、録音アプリで使用できます。
- Windows インストーラーは Valve の Steam Streaming Microphone をインストールし、マイク用に設定します (ステレオ 48 kHz 32-bit PCM、MicYou のステレオ 48 kHz F32LE DSP チェーンに合わせた形式、キャプチャゲイン等倍、排他モード無効)。MicYou の出力デバイス一覧と録音アプリの両方で **Steam Streaming Microphone** を選択してください。
- ライト/ダークテーマ、ダイナミックカラー、カスタム背景、デスクトップのポケットモード、システムトレイからの操作、多言語対応により、Material 3 のインターフェースをカスタマイズできます。
- 接続、DSP、言語、テーマの設定をデスクトップの GUI、CLI、TUI の間で共有できます。
- デュアルランタイム (Native と WASM) のプラグインシステムと、カスタム DSP ノード、サウンドパッド、コントロールウィジェットを入手できるアプリ内マーケットプレイスで機能を拡張できます。
- ハンズフリーで接続できます。Android クライアントは起動時にストリーミングを開始し、予期しない切断のあとに自動で再試行できます。試行回数、間隔、定期的な再試行は設定可能です。
- プラグインを最新の状態に保てます。デスクトップアプリは起動時にバックグラウンドでプラグインのアップデートを一度確認し、その結果を Settings → Plugins に表示します。

## スクリーンショット

### Android アプリ

|                        メイン画面                        |                           設定                            |
|:---------------------------------------------------------:|:-------------------------------------------------------------:|
| <img src="img/android_screenshot_main.png" width="300" /> | <img src="img/android_screenshot_settings.png" width="300" /> |

### デスクトップアプリ

|                         メイン画面                          |                              設定                               |
|:------------------------------------------------------------:|:-------------------------------------------------------------------:|
| <img src="img/desktop_screenshot.png" width="480" /> | <img src="img/desktop_screenshot_settings.png" width="480" /> |

## はじめに

1. お使いの OS 用のデスクトップパッケージと Android APK を [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) からダウンロードします。iPhone や iPad の場合は、[MicYou for iOS](https://github.com/MicYou-Dev/MicYou-iOS/releases/latest) から IPA を入手し、[Install on iOS](https://micyou.top/en/docs/how-to-install-in-ios) に従ってサイドロードします。
2. [Quick Start ガイド](https://micyou.top/en/docs/quick-start) に従って、お使いのプラットフォームで仮想マイクを設定します。
3. 接続方法を選択します。
   - Wi-Fi: スマートフォンと PC を同じネットワークに接続し、デスクトップで Wi-Fi を選択します。Android アプリは Wi-Fi を選択すると接続します。iOS アプリはコンピューターを自動的に検出しますが、IP アドレスを入力することもできます。
   - USB: データケーブルでスマートフォンを接続し、デスクトップで USB を選択します。Android では USB デバッグを有効にしてアプリで USB を選択し、iOS ではアプリで USB Cable を選択します (Windows では iTunes のインストールが必要です)。
   - Web: デスクトップで Web を選択し、モバイルブラウザで QR コードを読み取ります。アプリは不要です。
4. ストリーミングを開始し、通話、ゲーム、配信、録音アプリで設定済みの仮想マイクを選択します。

プラットフォームごとのインストール手順とトラブルシューティングについては、[MicYou ドキュメント](https://micyou.top/en/docs/quick-start) をご覧ください。

プライベートネットワークをまたいだブラウザ接続については、[Web mode across private networks](docs/web-private-networks.md) をご覧ください。

## 技術スタック

- Android: Kotlin、Jetpack Compose、Material 3
- iOS: Swift、SwiftUI ([別リポジトリ](https://github.com/MicYou-Dev/MicYou-iOS))
- デスクトップ: Tauri 2、Rust、Vue 3、Vite、Tailwind CSS
- プロトコルと音声: 伝送、バッファリング、DSP、仮想オーディオデバイス連携のための Rust バックエンドと共有クレート

## 貢献

あらゆる種類の貢献を歓迎します！バグの報告、機能の提案、翻訳の手伝い、コードの貢献など、まずは [Contributing Guidelines](./CONTRIBUTING.md) をご確認ください。

## コントリビューター

<a href="https://github.com/MicYou-Dev/MicYou/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=MicYou-Dev/MicYou" />
</a>

[contrib.rocks](https://contrib.rocks) で作成しました。

## スター履歴

<a href="https://www.star-history.com/?repos=micyou-dev%2Fmicyou&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=micyou-dev/micyou&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=micyou-dev/micyou&type=date&legend=top-left" />
   <img alt="スター履歴チャート" src="https://api.star-history.com/chart?repos=micyou-dev/micyou&type=date&legend=top-left" />
 </picture>
</a>

## 謝辞

MicYou のエコーキャンセレーションと AI ノイズ抑制を支える [AEC7](https://github.com/a2heng/lightweight-aec-48k) と [PureVox](https://github.com/a2heng/lightweight-denoise-48k) をオープンソースで公開してくださった [a2heng](https://github.com/a2heng) に特に感謝します。

本プロジェクトにミラーダウンロードサービスを提供してくださっている [HeRNET Open Source Software Mirror](https://mirrors.ha.edu.cn/) に特に感謝します。

これまで継続的に支援してくださった [CQU Open Source Software Mirror](https://mirrors.cqu.edu.cn/) に特に感謝します。

本プロジェクトに高速ミラーダウンロードサービスを提供してくださっている [MirrorChyan](https://mirrorchyan.com/en/get-start) に特に感謝します。

プロジェクトをより良くするために協力してくださったすべての[コントリビューター](https://github.com/MicYou-Dev/MicYou/graphs/contributors)に特に感謝します。

## ライセンス

このプロジェクトは [GNU General Public License v3.0 with MicYou Plugin Exception](./LICENSE) のもとでライセンスされています。

