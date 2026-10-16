/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version, with the MicYou Plugin Exception.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 */

package com.lanrhyme.micyou.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lanrhyme.micyou.theme.PaletteStyle
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.lanrhyme.micyou.audio.AudioEngine
import com.lanrhyme.micyou.audio.AudioFormat
import com.lanrhyme.micyou.audio.ChannelCount
import com.lanrhyme.micyou.audio.SampleRate
import com.lanrhyme.micyou.network.ConnectionErrorDetails
import com.lanrhyme.micyou.network.DiscoveredDevice
import com.lanrhyme.micyou.settings.Settings
import com.lanrhyme.micyou.settings.SettingsFactory
import com.lanrhyme.micyou.settings.SettingsViewModel
import com.lanrhyme.micyou.soundpad.SoundPadController
import com.lanrhyme.micyou.soundpad.SoundPadUiState
import com.lanrhyme.micyou.theme.ThemeMode
import com.lanrhyme.micyou.ui.background.BackgroundSettings
import com.lanrhyme.micyou.update.UpdateCheckResult
import com.lanrhyme.micyou.update.UpdateInfo
import com.lanrhyme.micyou.update.UpdateViewModel
import com.lanrhyme.micyou.util.AppLanguage
import com.lanrhyme.micyou.util.Constants
import com.lanrhyme.micyou.util.getString

import com.lanrhyme.micyou.R
import com.lanrhyme.micyou.viewmodel.ConnectionMode
import com.lanrhyme.micyou.viewmodel.UpdateDownloadState
enum class ConnectionMode(val label: String) {
    Wifi("Wi-Fi"),
    Usb("USB (ADB)")
}

enum class TransportProtocol(val label: String) {
    Tcp("TCP"),
    Both("TCP+UDP")
}

enum class StreamState {
    Idle, Connecting, Streaming, Error
}

enum class VisualizerStyle(val label: String) {
    VolumeRing("VolumeRing"),
    Ripple("Ripple"),
    Bars("Bars"),
    Wave("Wave"),
    Glow("Glow"),
    Particles("Particles")
}

enum class UpdateDownloadState {
    Idle, Downloading, Downloaded, Installing, Failed
}

data class AppUiState(
    // Audio Stream State
    val mode: ConnectionMode = ConnectionMode.Wifi,
    val transportProtocol: TransportProtocol = TransportProtocol.Both,
    val streamState: StreamState = StreamState.Idle,
    val ipAddress: String = "192.168.1.5",
    val port: String = Constants.DEFAULT_TCP_PORT.toString(),
    val errorMessage: String? = null,
    val sampleRate: SampleRate = SampleRate.Rate48000,
    val channelCount: ChannelCount = ChannelCount.Stereo,
    val audioFormat: AudioFormat = AudioFormat.PCM_FLOAT,
    val isMuted: Boolean = false,
    val isAutoConfig: Boolean = true,

    // Error Dialog State
    val showErrorDialog: Boolean = false,
    val errorDetails: ConnectionErrorDetails? = null,

    // UDP Warning Dialog State
    val showUdpWarningDialog: Boolean = false,

    val androidAudioSourceName: String = "Mic",

    // Settings State
    val themeMode: ThemeMode = ThemeMode.System,
    val seedColor: Long = 0xFF1565C0,
    val useDynamicColor: Boolean = false,
    val oledPureBlack: Boolean = false,
    val paletteStyle: PaletteStyle = PaletteStyle.TonalSpot,
    val useExpressiveShapes: Boolean = true,
    val language: AppLanguage = AppLanguage.System,
    val autoStart: Boolean = false,
    val keepScreenOn: Boolean = false,
    val autoCheckUpdate: Boolean = true,
    val useMirrorDownload: Boolean = false,
    val mirrorCdk: String = "",
    val showMirrorCdkDialog: Boolean = false,
    val visualizerStyle: VisualizerStyle = VisualizerStyle.VolumeRing,
    val backgroundSettings: BackgroundSettings = BackgroundSettings(),
    val showFirstLaunchDialog: Boolean = false,


    // Update State
    val updateInfo: UpdateInfo? = null,
    val updateDownloadState: UpdateDownloadState = UpdateDownloadState.Idle,
    val updateDownloadProgress: Float = 0f,
    val updateDownloadedBytes: Long = 0,
    val updateTotalBytes: Long = 0,
    val updateErrorMessage: String? = null,

    // Discovery State
    val discoveredDevices: List<DiscoveredDevice> = emptyList(),
    val isDiscovering: Boolean = false,

    // UI State
    val installMessage: String? = null,
    val snackbarMessage: String? = null
)


/**
 * Main ViewModel - Coordinates between specialized ViewModels
 * This ViewModel acts as a facade for the UI layer
 */
class MainViewModel : ViewModel() {
    // Specialized ViewModels
    private val audioStreamViewModel = AudioStreamViewModel()
    private val settingsViewModel = SettingsViewModel()

    private val updateViewModel = UpdateViewModel()

    // Soundpad remote control (desktop opss.soundpad plugin); rides the
    // AudioEngine control channel, so it must be created after it.
    private val soundPadController = SoundPadController(audioStreamViewModel.audioEngine, viewModelScope)
    val soundPadState: StateFlow<SoundPadUiState> = soundPadController.state

    private val _uiState = MutableStateFlow(AppUiState())
    val uiState: StateFlow<AppUiState> = _uiState.asStateFlow()

    // Expose audio levels from AudioStreamViewModel
    val audioLevels = audioStreamViewModel.audioLevels

    private val settings = SettingsFactory.getSettings()

    init {
        // Initialize from settings
        val initialLanguage = try {
            AppLanguage.valueOf(settings.getString("language", AppLanguage.System.name))
        } catch(e: Exception) {
            AppLanguage.System
        }

        // Observe and merge states from all ViewModels
        setupStateObservers()

        // Observe discovered devices
        viewModelScope.launch {
            audioStreamViewModel.discoveredDevices.collect { devices ->
                _uiState.update { it.copy(discoveredDevices = devices) }
            }
        }
        viewModelScope.launch {
            audioStreamViewModel.isDiscovering.collect { discovering ->
                _uiState.update { it.copy(isDiscovering = discovering) }
            }
        }

        // Auto-check for updates
        if (settings.getBoolean("auto_check_update", true)) {
            updateViewModel.checkUpdateAuto()
        }

        // Observe update check results for user feedback
        viewModelScope.launch {
            updateViewModel.checkResultFlow.collect { result ->
                result?.let {                    val message = when (it) {
                        is UpdateCheckResult.UpdateAvailable -> String.format(getString(R.string.updateAvailableMsg), it.info.versionName)
                        is UpdateCheckResult.NoUpdate -> getString(R.string.isLatestVersion)
                        is UpdateCheckResult.Error -> String.format(getString(R.string.updateCheckFailed), it.message)
                    }
                    _uiState.update { state -> state.copy(snackbarMessage = message) }
                }
            }
        }
    }

    private fun setupStateObservers() {
        viewModelScope.launch {
            combine(
                audioStreamViewModel.uiState,
                settingsViewModel.uiState,
                updateViewModel.uiState
            ) { audioState, settingsState, updateState ->
                _uiState.update { current ->
                    current.copy(
                        mode = audioState.mode,
                        transportProtocol = audioState.transportProtocol,
                        streamState = audioState.streamState,
                        ipAddress = audioState.ipAddress,
                        port = audioState.port,
                        errorMessage = audioState.errorMessage,
                        sampleRate = audioState.sampleRate,
                        channelCount = audioState.channelCount,
                        audioFormat = audioState.audioFormat,
                        isMuted = audioState.isMuted,
                        isAutoConfig = audioState.isAutoConfig,
                        showErrorDialog = audioState.showErrorDialog,
                        errorDetails = audioState.errorDetails,
                        showUdpWarningDialog = audioState.showUdpWarningDialog,
                        androidAudioSourceName = audioState.androidAudioSourceName,
                        themeMode = settingsState.themeMode,
                        seedColor = settingsState.seedColor,
                        useDynamicColor = settingsState.useDynamicColor,
                        oledPureBlack = settingsState.oledPureBlack,
                        paletteStyle = settingsState.paletteStyle,
                        useExpressiveShapes = settingsState.useExpressiveShapes,
                        language = settingsState.language,
                        autoStart = settingsState.autoStart,
                        keepScreenOn = settingsState.keepScreenOn,
                        autoCheckUpdate = settingsState.autoCheckUpdate,
                        useMirrorDownload = settingsState.useMirrorDownload,
                        mirrorCdk = settingsState.mirrorCdk,
                        showMirrorCdkDialog = settingsState.showMirrorCdkDialog,
                        visualizerStyle = settingsState.visualizerStyle,
                        backgroundSettings = settingsState.backgroundSettings,
                        showFirstLaunchDialog = settingsState.showFirstLaunchDialog,

                        updateInfo = updateState.updateInfo,
                        updateDownloadState = updateState.updateDownloadState,
                        updateDownloadProgress = updateState.updateDownloadProgress,
                        updateDownloadedBytes = updateState.updateDownloadedBytes,
                        updateTotalBytes = updateState.updateTotalBytes,
                        updateErrorMessage = updateState.updateErrorMessage,
                        snackbarMessage = settingsState.snackbarMessage
                    )
                }
            }.collect {
                // No-op: state merged from specialized ViewModels
            }
        }
    }

    // Delegate methods to specialized ViewModels
    // Audio Stream methods
    fun toggleStream() = audioStreamViewModel.toggleStream()
    fun toggleMute() = audioStreamViewModel.toggleMute()
    fun startStream() = audioStreamViewModel.startStream()
    fun stopStream() = audioStreamViewModel.stopStream()
    fun setMode(mode: ConnectionMode) = audioStreamViewModel.setMode(mode)
    fun setTransportProtocol(protocol: TransportProtocol) = audioStreamViewModel.setTransportProtocol(protocol)
    fun startDiscovery() = audioStreamViewModel.startDiscovery()
    fun stopDiscovery() = audioStreamViewModel.stopDiscovery()
    fun restartDiscovery() = audioStreamViewModel.restartDiscovery()
    fun selectDiscoveredDevice(device: DiscoveredDevice) {
        audioStreamViewModel.setIp(device.hostAddress)
        audioStreamViewModel.setPort(device.port.toString())
    }
    fun setIp(ip: String, restartStream: Boolean = false) = audioStreamViewModel.setIp(ip, restartStream)
    fun setPort(port: String) = audioStreamViewModel.setPort(port)
    fun setSampleRate(rate: SampleRate) = audioStreamViewModel.setSampleRate(rate)
    fun setChannelCount(count: ChannelCount) = audioStreamViewModel.setChannelCount(count)
    fun setAudioFormat(format: AudioFormat) = audioStreamViewModel.setAudioFormat(format)
    fun setAndroidAudioSource(sourceName: String) = audioStreamViewModel.setAndroidAudioSource(sourceName)
    fun setAutoConfig(enabled: Boolean) = audioStreamViewModel.setAutoConfig(enabled)
    fun dismissErrorDialog() = audioStreamViewModel.dismissErrorDialog()
    fun dismissUdpWarningDialog() = audioStreamViewModel.dismissUdpWarningDialog()
    fun retryAfterError() = audioStreamViewModel.retryAfterError()

    // Settings methods
    fun setThemeMode(mode: ThemeMode) = settingsViewModel.setThemeMode(mode)
    fun setSeedColor(color: Long) = settingsViewModel.setSeedColor(color)
    fun setUseDynamicColor(enable: Boolean) = settingsViewModel.setUseDynamicColor(enable)
    fun setOledPureBlack(enabled: Boolean) = settingsViewModel.setOledPureBlack(enabled)
    fun setPaletteStyle(style: PaletteStyle) = settingsViewModel.setPaletteStyle(style)
    fun setUseExpressiveShapes(enabled: Boolean) = settingsViewModel.setUseExpressiveShapes(enabled)
    fun setLanguage(language: AppLanguage) = settingsViewModel.setLanguage(language)
    fun setAutoStart(enabled: Boolean) = settingsViewModel.setAutoStart(enabled)
    fun setKeepScreenOn(enabled: Boolean) = settingsViewModel.setKeepScreenOn(enabled)
    fun setVisualizerStyle(style: VisualizerStyle) = settingsViewModel.setVisualizerStyle(style)
    fun setAutoCheckUpdate(enabled: Boolean) = settingsViewModel.setAutoCheckUpdate(enabled)
    fun setUseMirrorDownload(enabled: Boolean) = settingsViewModel.setUseMirrorDownload(enabled)
    fun setMirrorCdk(cdk: String) = settingsViewModel.setMirrorCdk(cdk)
    fun confirmMirrorCdk(cdk: String) = settingsViewModel.confirmMirrorCdk(cdk)
    fun dismissMirrorCdkDialog() = settingsViewModel.dismissMirrorCdkDialog()
    fun setBackgroundImage(path: String?) = settingsViewModel.setBackgroundImage(path)
    fun setBackgroundBrightness(brightness: Float) = settingsViewModel.setBackgroundBrightness(brightness)
    fun setBackgroundBlur(blurRadius: Float) = settingsViewModel.setBackgroundBlur(blurRadius)
    fun setCardOpacity(opacity: Float) = settingsViewModel.setCardOpacity(opacity)
    fun setEnableHazeEffect(enabled: Boolean) = settingsViewModel.setEnableHazeEffect(enabled)
    fun clearBackgroundImage() = settingsViewModel.clearBackgroundImage()
    fun pickBackgroundImage() = settingsViewModel.pickBackgroundImage()
    fun showSnackbar(message: String) = settingsViewModel.showSnackbar(message)
    fun clearSnackbar() = settingsViewModel.clearSnackbar()
    fun dismissFirstLaunchDialog() = settingsViewModel.dismissFirstLaunchDialog()
    fun exportLog(onResult: (String?) -> Unit) = settingsViewModel.exportLog(onResult)



    // Update methods
    fun checkUpdateManual() {
        viewModelScope.launch {            _uiState.update { it.copy(snackbarMessage = getString(R.string.checkingUpdate)) }
            updateViewModel.checkUpdateManual()
        }
    }
    fun downloadAndInstallUpdate(useMirror: Boolean = _uiState.value.useMirrorDownload) = updateViewModel.downloadAndInstallUpdate(useMirror)
    fun dismissUpdateDialog() = updateViewModel.dismissUpdateDialog()
    fun openGitHubRelease() = updateViewModel.openGitHubRelease()

    fun clearInstallMessage() {
        _uiState.update { it.copy(installMessage = null) }
    }

    // Soundpad methods (delegate to SoundPadController)
    fun toggleSoundPadPanel() = soundPadController.togglePanel()
    fun soundPadPlay(soundId: String) = soundPadController.play(soundId)
    fun soundPadStop(sessionId: Long? = null) = soundPadController.stop(sessionId)
    fun soundPadSeek(sessionId: Long, positionMs: Long) = soundPadController.seek(sessionId, positionMs)
    fun soundPadSetVolume(sessionId: Long, volume: Float) = soundPadController.setVolume(sessionId, volume)
    fun soundPadSetLoop(sessionId: Long, loop: Boolean) = soundPadController.setLoop(sessionId, loop)
    fun soundPadRefresh() = soundPadController.refreshLibrary()
    fun soundPadEnterFolder(dir: String) = soundPadController.enterFolder(dir)
    fun soundPadNavigateUp() = soundPadController.navigateUp()
    fun soundPadSelect(sessionId: Long?) = soundPadController.selectSession(sessionId)
    fun soundPadConsumeError() = soundPadController.consumeError()

    override fun onCleared() {
        audioStreamViewModel.close()
        settingsViewModel.close()
        updateViewModel.close()
        super.onCleared()
    }
}
