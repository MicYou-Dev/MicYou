This document has been moved to https://micyou.top/en/docs/quick-start. If you'd like to contribute to this document, please visit https://github.com/MicYou-Dev/Website-MicYou

<!-- # MicYou Docs

<p align="center">
   <a href="./FAQ_ZH.md">简体中文</a> | <a href="./FAQ_TW.md">繁體中文</a> | <b>English</b>
</p>

## Quick Start

### 1. Download ADB

Download from [Android Developers](https://developer.android.com/tools/releases/platform-tools?hl=zh_cn), or install via package manager:

- `winget install -e --id Google.PlatformTools` (Windows)
- `sudo apt install android-tools-adb` (Ubuntu/Debian)
- `sudo pacman -S android-tools` (Arch)
- Other platforms, please refer to [official documentation](https://developer.android.com/tools/releases/platform-tools)

### 2. Enable USB Debugging

Using OneUI 8 as an example:

1. Go to Settings > About phone
2. Tap Software information, find Build number and tap it 7 times to enable Developer Options
3. Go back to Settings > Developer options, and enable USB debugging

### 3. USB connection

Use a **stable** data cable, and set the connection mode to `USB` on **both** the desktop app and the Android app.

### 4. Wi-Fi connection

Ensure your Android device and PC are on the **same network**, and set the connection mode to `Wi-Fi` on **both** the desktop app and the Android app.

### Android

1. Download and install the APK on your Android device.
2. Ensure your device is on the same network as your PC (for Wi-Fi) or connected via USB.

### Windows

1. Run the desktop application.
2. Configure the connection mode to match the Android app.

### macOS

For the best experience, install the following dependencies via Homebrew:

```bash
brew install blackhole-2ch --cask
brew install switchaudio-osx --formulae
```

> **BlackHole** is required (virtual audio driver). If you do not have Homebrew, go to https://existential.audio/blackhole/download/ to download the installer.

Please restart your Mac after installation.

After downloading the app from [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases) and installing it in your Applications folder, Gatekeeper may block it during first use:

- If prompted with "Untrusted Developer," navigate to **System Settings/System Preferences > Privacy & Security** to allow the app to run.
- If prompted with "The application is damaged," execute the following command:

```bash
sudo xattr -r -d com.apple.quarantine /Applications/MicYou.app
```

You will need to enter your account password during the process. The password will be hidden while you are entering it. Press Enter after you have finished entering it.

### Linux

#### Using pre-built packages (recommended)

Pre-built packages are available in [GitHub Releases](https://github.com/MicYou-Dev/MicYou/releases). Choose the appropriate package for your Linux distribution.

**DEB package (Debian/Ubuntu/Mint etc.):**

```bash
sudo dpkg -i MicYou-*.deb
# If dependencies are missing:
sudo apt install -f
```

**RPM package (Fedora/RHEL/openSUSE etc.):**

```bash
sudo rpm -i MicYou-*.rpm
# Or use dnf/yum:
sudo dnf install MicYou-*.rpm
```

**AUR (Arch Linux and derivatives):**

```bash
git clone https://aur.archlinux.org/micyou-bin.git
cd micyou-bin
makepkg -si
```

Or use an AUR helper like paru:

```bash
paru -S micyou-bin
```

**Run the application:**

From the application menu, or run from terminal:

```bash
MicYou
```

## FAQ

### Cannot connect to device

#### Wi-Fi Mode

1. **Check Firewall Settings**

   Windows Firewall may block inbound connections. Please follow these steps to manually allow the ports:

   1. Press `Win+R`, type `powershell`, then hold `Ctrl+Shift` and click "OK" to run PowerShell as administrator.
   2. Enter the following commands:

      ```powershell
      New-NetFirewallRule -DisplayName "MicYou-6000-TCP" -Direction Inbound -LocalPort 6000 -Protocol TCP -Action Allow
      New-NetFirewallRule -DisplayName "MicYou-6001-UDP" -Direction Inbound -LocalPort 6001 -Protocol UDP -Action Allow
      ```

      > MicYou uses TCP port `6000` (control) and UDP port `6001` (audio data) by default. Change the port numbers if you have configured a different port.

      If no error appears, the operation was successful. Try connecting again.

2. **Check if devices are on the same subnet**

   - Ensure the Android phone and PC are connected to the **same** Wi-Fi router.
   - Ensure that **AP Isolation** or **Network Device Isolation** features are disabled in router settings (refer to your router's manual).

> [!TIP]
> Advanced users can try using tools like Nmap or ping to check connectivity.

#### USB (ADB) Mode

1. **Enable Developer Options**

   > The steps below may vary depending on your device. **Please use a search engine** to find instructions for your specific device.

   - Find "About phone" in phone settings, tap "Build number" 7 times to enable Developer Options.
   - Enter Developer Options and enable **USB debugging**.

2. **Confirm ADB connection**

   > ADB tools must be installed on the computer (see Step 1: Download ADB above).

   Run the following command to verify that one and only one device is connected:

   ```bash
   adb devices
   ```

   If multiple devices are listed, specify the target device for port forwarding:

   ```bash
   adb -s <device_serial_number> reverse tcp:6000 tcp:6000
   ```

   > The device serial number can be found in the output of `adb devices`.

### No audio output after connecting

Please ensure that the VB-Audio driver is correctly installed and that the following devices are **not disabled**:

- **Output Device**: CABLE Input (VB-Audio Virtual Cable)
- **Input Device**: CABLE Output (VB-Audio Virtual Cable)

To check: Open Settings > Sound, and verify both devices are **Enabled**:

![Input device](https://github.com/user-attachments/assets/1cf5f97f-1647-4fb0-a152-85be2697df39)
![Output device](https://github.com/user-attachments/assets/9e9ef42d-186f-42a6-ba4d-7b1a3815f860) -->
