# Contributing to MicYou

Thank you for your interest in contributing to MicYou! This guide covers the project layout, how to build the app from source, how to add translations, and how to submit changes.

We welcome bug reports, feature requests, code contributions, and translations.

## Project Layout

- `composeApp/` — Android app (Kotlin, Jetpack Compose, Material 3)
- `tauri-app/` — Desktop app
  - `src/` — Vue 3 + Vite + Tailwind CSS frontend
  - `src-tauri/` — Tauri 2 + Rust backend (GUI server)
  - `crates/` — shared Rust workspace crates:
    - `micyou-core` — headless server core, lifecycle management, and system integrations
    - `micyou-protocol` — network protocol definitions (Protobuf)
    - `micyou-audio` — audio transport, buffering, and DSP processing chain
    - `micyou-plugin` — plugin framework (Native and WASM runtimes)
    - `micyou-cli` — command-line server (binary `micyou-cli`)
    - `micyou-tui` — interactive terminal dashboard (binary `micyou-tui`)
- `plugins/` — example plugins and development resources
- `docs/` — project documentation (including plugin specifications and development guides)
- `img/` — README and project screenshots

## Environment Requirements

- Android SDK: compileSdk 36, minSdk 24, targetSdk 36 (JDK 17 or JDK 21)
- Desktop frontend: Bun 1.4+
- Desktop backend: Rust stable + Cargo. On Linux you also need the Tauri 2 system dependencies: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `libxdo-dev`, `libssl-dev`, `libasound2-dev`.

## Building from Source

### Android app (APK)

Standard build (Android 7.0+):

```bash
./gradlew :composeApp:assembleDebug
```

For compatibility builds on legacy devices (Android 5.0+ / API 21+), see the [Android Compatibility Build Guide](docs/android-compat.md).

Optional, maintainers only: release signing is configured via the `ANDROID_KEYSTORE_PATH`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD` environment variables; the `AIFADIAN_API_TOKEN` and `AIFADIAN_USER_ID` values in `local.properties` power the in-app Sponsors list (爱发电/Aifadian API). Regular contributors can ignore both — without them the Sponsors dialog just shows "API not configured".

### Desktop frontend (type check + build)

```bash
cd tauri-app
bun install          # only when dependencies need to be restored or updated
bun run build        # vue-tsc type check + vite build
```

### Web client UI (no Rust build)

The phone's web UI lives in `tauri-app/crates/micyou-core/assets/web_client.html`.
Preview that exact page and its bundled Alpine library without compiling Rust:

```bash
cd tauri-app
node scripts/web-preview.mjs
```

Open `http://127.0.0.1:8787`. The preview binds only to loopback and serves only
the HTML and Alpine assets. It supports UI and language checks, but provides no
audio server or WebSocket endpoint. Browser storage is scoped to each origin,
so preferences saved here do not affect the installed app or a different port.

For automated checks (Node 24 and the pinned Bun version):

```bash
bun install --frozen-lockfile
bun run test:web
bunx playwright install chromium webkit
bun run test:web:browser
```

The browser tests exercise Alpine initialization, language selection across
reloads, and the help sheet in Chromium and mobile WebKit. They do not request
microphone permission or test audio transport. CI runs these checks in the
path-filtered **Web Client** workflow without building Android or the desktop.

The production server embeds the HTML at Rust compile time. For an end-to-end
phone/audio test after editing it, close the installed app and run
`cargo run -p micyou-cli -- serve --mode web --port 8443` from `tauri-app`, or
rebuild the desktop GUI. Opening the static preview does not modify an installed
MicYou executable.

### Desktop GUI (Tauri)

```bash
cd tauri-app
bun run tauri dev    # development
bun run tauri build  # release bundles (NSIS installer on Windows, .deb/.rpm/.AppImage on Linux, .dmg on macOS)
```

### CLI and TUI servers

The desktop server can also run without the GUI:

```bash
cd tauri-app
cargo run -p micyou-cli -- serve    # CLI (headless; run `cargo run -p micyou-cli -- --help` for all commands)
cargo run -p micyou-tui             # interactive TUI dashboard
```

The GUI, CLI, and TUI all share the same server configuration and DSP settings.

### Versioning

The single source of truth for the version is `gradle.properties` (`project.version`, `project.version.code`). After changing it, sync the desktop-side version files:

```bash
cd tauri-app
bun run sync-version
```

## Internationalization (i18n)

User-facing strings live in Android string resources and desktop locale JSON files. The two platforms have independent locale sets; keep the key sets within each platform consistent.

### Android

- Location: `composeApp/src/main/res/values*/strings.xml`
- Base languages (must be kept in sync): English (`values/`) and Simplified Chinese (`values-zh/`)
- Language registration: the `AppLanguage` enum in `composeApp/src/main/kotlin/com/lanrhyme/micyou/util/Localization.kt`

To add a new language:

1. Create `composeApp/src/main/res/values-xx/strings.xml` (replace `xx` with the locale code, e.g. `fr` for French, per ISO 639-1 / IETF BCP 47).
2. Copy the keys from `values/strings.xml` and translate all values, keeping the keys unchanged:

```xml
<resources>
    <string name="appName">MicYou</string>
    <string name="ipLabel">IP : </string>
    <!-- ... -->
</resources>
```
1. Register the language in `AppLanguage`:

```kotlin
enum class AppLanguage(val label: String, val code: String) {
    // ... existing languages ...
    French("Français", "fr"),  // add this line
}
```

Special variants (easter eggs):

- `values-zh/` — Simplified Chinese
- `values-zh-rTW/` — Traditional Chinese (Taiwan)
- `values-zh-rHK/` — Cantonese (Hong Kong)
- `values-zh-rHD/` — Chinese hard mode (easter egg)
- `values-ca/` — Cat language (easter egg)

### Desktop (Tauri)

- Location: `tauri-app/src/shared/locales/*.json` (`en`, `zh`, `zh-hk`, `zh-tw`, `zh-ss`, `cat`, `lzh`)
- Base languages: English (`en.json`) and Simplified Chinese (`zh.json`)
- Registration: import the JSON and add it to the i18n `messages` map in `tauri-app/src/main.ts`

To add a new language:

1. Create `tauri-app/src/shared/locales/xx.json` with the same key structure as `zh.json`.
2. Import it and register it in the `messages` map in `tauri-app/src/main.ts`.

### Testing translations

- Android: build the APK (`./gradlew :composeApp:assembleDebug`) and check **Settings → Language**.
- Desktop: run `bun run tauri dev` and check **Settings → Language**.
- Verify that all strings display correctly and that layouts don't clip or overflow in your language.

## Submitting Changes

Use [Conventional Commits](https://www.conventionalcommits.org/) format for commit messages and PR titles:

- `feat(i18n): add fr (French) localization`
- `fix: resolve audio crash on Android 14`
- `docs: update build instructions`

Before opening a pull request:

- Keep localization key sets consistent across all locale files for the platform you touched.
- Make sure the Android debug build (`./gradlew :composeApp:assembleDebug`) and the desktop build (`cd tauri-app && bun run build`) pass.
- CI (`.github/workflows/development.yml`) builds the Android debug APK and the Tauri packages for Windows, macOS, and Linux on every push and pull request.

By contributing, you agree that your contributions will be licensed under the project's [GNU General Public License v3.0 with MicYou Plugin Exception](./LICENSE) and to follow the project's [Code of Conduct](./CODE_OF_CONDUCT.md)
