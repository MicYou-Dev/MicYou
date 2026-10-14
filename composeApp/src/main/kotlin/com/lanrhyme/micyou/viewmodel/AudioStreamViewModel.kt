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
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.lanrhyme.micyou.audio.AudioEngine
import com.lanrhyme.micyou.audio.AudioFormat
import com.lanrhyme.micyou.audio.ChannelCount
import com.lanrhyme.micyou.audio.SampleRate
import com.lanrhyme.micyou.audio.availableAudioFormats
import com.lanrhyme.micyou.audio.defaultAudioFormat
import com.lanrhyme.micyou.network.calculateUdpPort
import com.lanrhyme.micyou.network.ConnectionErrorDetails
import com.lanrhyme.micyou.network.ConnectionErrorHelper
import com.lanrhyme.micyou.network.DeviceDiscoveryManager
import com.lanrhyme.micyou.network.DiscoveredDevice
import com.lanrhyme.micyou.settings.Settings
import com.lanrhyme.micyou.settings.SettingsFactory
import com.lanrhyme.micyou.R
import com.lanrhyme.micyou.util.getString
import com.lanrhyme.micyou.util.Constants
import com.lanrhyme.micyou.util.Logger
import com.lanrhyme.micyou.viewmodel.AudioStreamUiState
import java.util.concurrent.atomic.AtomicBoolean

data class AudioStreamUiState(
    val mode: ConnectionMode = ConnectionMode.Wifi,
    val transportProtocol: TransportProtocol = TransportProtocol.Both,
    val streamState: StreamState = StreamState.Idle,
    val ipAddress: String = "192.168.1.5",
    val port: String = Constants.DEFAULT_TCP_PORT.toString(),
    val errorMessage: String? = null,
    val sampleRate: SampleRate = SampleRate.Rate48000,
    val channelCount: ChannelCount = ChannelCount.Stereo,
    val audioFormat: AudioFormat = defaultAudioFormat(),
    val isMuted: Boolean = false,
    val isAutoConfig: Boolean = true,
    // Error Dialog State
    val showErrorDialog: Boolean = false,
    val errorDetails: ConnectionErrorDetails? = null,

    // UDP Warning Dialog State
    val showUdpWarningDialog: Boolean = false,

    val androidAudioSourceName: String = "Mic"
)

class AudioStreamViewModel : ViewModel() {
    private val _audioEngine = AudioEngine()
    val audioEngine: AudioEngine get() = _audioEngine
    private val viewModelCoroutineExceptionHandler = CoroutineExceptionHandler { _, throwable ->
        Logger.e("AudioStreamViewModel", "Unhandled coroutine error (${throwable.javaClass.simpleName}): ${throwable.message}", throwable)
        if (throwable !is kotlinx.coroutines.CancellationException) {
            _uiState.update {
                it.copy(
                    streamState = StreamState.Error,
                    errorMessage = "${throwable.javaClass.simpleName}: ${throwable.message}",
                    showErrorDialog = true,
                    errorDetails = null
                )
            }
        }
    }
    private val auxiliaryScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate + viewModelCoroutineExceptionHandler)
    private val closed = AtomicBoolean(false)
    private val closeLock = Any()
    private var closeJob: Job? = null
    private val _uiState = MutableStateFlow(AudioStreamUiState())
    val uiState: StateFlow<AudioStreamUiState> = _uiState.asStateFlow()

    // 音频电平相关
    val audioLevels = _audioEngine.audioLevels

    // 设备发现
    private val discoveryManager = DeviceDiscoveryManager()
    val discoveredDevices: StateFlow<List<DiscoveredDevice>> = discoveryManager.discoveredDevices
    val isDiscovering: StateFlow<Boolean> = discoveryManager.isDiscovering

    private val settings = SettingsFactory.getSettings()
    private var isStartStreamRequestPending = false
    private var isStopStreamRequestPending = false

    init {
        loadSettings()
        setupAudioEngineObservers()
        if (_uiState.value.mode == ConnectionMode.Wifi) {
            discoveryManager.startDiscovery()
        }
    }

    private fun loadSettings() {
        val savedModeName = settings.getString("connection_mode", ConnectionMode.Wifi.name)
    val savedMode = when (savedModeName) {
            "WifiUdp" -> ConnectionMode.Wifi
            else -> try { ConnectionMode.valueOf(savedModeName) } catch(e: Exception) { ConnectionMode.Wifi }
        }
        val effectiveMode = savedMode
        val savedProtocolName = settings.getString("transport_protocol", TransportProtocol.Both.name)
        val savedProtocol = try { TransportProtocol.valueOf(savedProtocolName) } catch(e: Exception) { TransportProtocol.Both }
    val savedIp = settings.getString("ip_address", "192.168.1.5")
    val savedPort = settings.getString("port", Constants.DEFAULT_TCP_PORT.toString())
    val savedSampleRateName = settings.getString("sample_rate", SampleRate.Rate48000.name)
    val savedSampleRate = try { SampleRate.valueOf(savedSampleRateName) } catch(e: Exception) { SampleRate.Rate48000 }
    val savedChannelCountName = settings.getString("channel_count", ChannelCount.Stereo.name)
    val savedChannelCount = try { ChannelCount.valueOf(savedChannelCountName) } catch(e: Exception) { ChannelCount.Stereo }
    val savedAudioFormatName = settings.getString("audio_format", defaultAudioFormat().name)
    val savedAudioFormat = try { AudioFormat.valueOf(savedAudioFormatName) } catch(e: Exception) { defaultAudioFormat() }
    // 校验已保存的格式在当前设备是否可用，不可用则回退到安全默认值
    val effectiveAudioFormat = if (availableAudioFormats().contains(savedAudioFormat)) savedAudioFormat else defaultAudioFormat()

    val savedAndroidAudioSourceName = settings.getString("android_audio_source", "Mic")
    val savedIsAutoConfig = settings.getBoolean("is_auto_config", true)

        _uiState.update {
            it.copy(
                mode = effectiveMode,
                transportProtocol = savedProtocol,
                ipAddress = savedIp,
                port = savedPort,
                sampleRate = savedSampleRate,
                channelCount = savedChannelCount,
                audioFormat = effectiveAudioFormat,
                androidAudioSourceName = savedAndroidAudioSourceName,
                isAutoConfig = savedIsAutoConfig
            )
        }

        // Apply auto config on startup if enabled
        if (savedIsAutoConfig) {
            applyAutoConfig()
        }
    }

    private fun setupAudioEngineObservers() {
        auxiliaryScope.launch {
            _audioEngine.streamState.collect { state ->
                _uiState.update { it.copy(streamState = state) }
            }
        }

        auxiliaryScope.launch {
            _audioEngine.lastError.collect { error ->
                if (error == "UDP_AUDIO_WARNING") {
                    _uiState.update { it.copy(showUdpWarningDialog = true) }
                } else {
                    _uiState.update { it.copy(errorMessage = error) }
                }
            }
        }

        auxiliaryScope.launch {
            _audioEngine.isMuted.collect { muted ->
                _uiState.update { it.copy(isMuted = muted) }
            }
        }

        // Auto-start handled via MainViewModel
    }

    private fun applyAutoConfig() {
        setSampleRate(SampleRate.Rate48000)
        setChannelCount(ChannelCount.Stereo)
        setAudioFormat(AudioFormat.PCM_16BIT)
    }

    fun toggleStream() {
        if (_uiState.value.streamState == StreamState.Streaming || _uiState.value.streamState == StreamState.Connecting) {
            stopStream()
        } else {
            startStream()
        }
    }

    fun toggleMute() {
        val newMuteState = !_uiState.value.isMuted
        auxiliaryScope.launch {
            _audioEngine.setMute(newMuteState)
        }
    }

    fun startStream() {
        if (isStartStreamRequestPending ||
            isStopStreamRequestPending ||
            _uiState.value.streamState == StreamState.Streaming ||
            _uiState.value.streamState == StreamState.Connecting
        ) {
            Logger.d("AudioStreamViewModel", "Start stream request ignored: already starting or running")
            return
        }

        isStartStreamRequestPending = true
        auxiliaryScope.launch {
            try {
                startStreamInternal()
            } finally {
                isStartStreamRequestPending = false
            }
        }
    }

    private suspend fun startStreamInternal() {
        Logger.i("AudioStreamViewModel", "Starting stream")
        val mode = _uiState.value.mode
        val ip = _uiState.value.ipAddress.trim()

        // 端口验证：确保端口在有效范围内 (1-65535)
        val rawPort = _uiState.value.port.toIntOrNull()
        val port = when {
            rawPort == null -> {
                Logger.w("AudioStreamViewModel", "Invalid port format: ${_uiState.value.port}, using default ${Constants.DEFAULT_TCP_PORT}")
                Constants.DEFAULT_TCP_PORT
            }
            rawPort <= 0 || rawPort > 65535 -> {
                Logger.w("AudioStreamViewModel", "Port out of range: $rawPort, using default ${Constants.DEFAULT_TCP_PORT}")
                Constants.DEFAULT_TCP_PORT
            }
            else -> rawPort
        }

        // Hostnames and IPv6 are valid targets, so only an empty field is rejected here.
        if (ip.isBlank()) {
            Logger.e("AudioStreamViewModel", "IP address is empty")
            _uiState.update {
                it.copy(
                    streamState = StreamState.Error,
                    errorMessage = getString(R.string.errorIpAddressEmpty),
                    showErrorDialog = true
                )
            }
            return
        }

        val sampleRate = _uiState.value.sampleRate
        val channelCount = _uiState.value.channelCount
        val audioFormat = _uiState.value.audioFormat

        _uiState.update { it.copy(streamState = StreamState.Connecting, errorMessage = null, showErrorDialog = false, errorDetails = null) }

        try {
            Logger.d("AudioStreamViewModel", "Calling _audioEngine.start()")
            _audioEngine.start(ip, port, mode, true, sampleRate, channelCount, audioFormat, _uiState.value.transportProtocol)
            Logger.i("AudioStreamViewModel", "Stream started successfully")
        } catch (e: kotlinx.coroutines.CancellationException) {
            Logger.i("AudioStreamViewModel", "Stream start cancelled by user")
            return
        } catch (t: Throwable) {
            // ⚠️ 捕获 Throwable（包含 NoSuchMethodError/NoClassDefFoundError 等 Error）
            Logger.e("AudioStreamViewModel", "Failed to start stream (catch Throwable): ${t.javaClass.name}", t)

            val cause = if (t is Exception) t else Exception("${t.javaClass.simpleName}: ${t.message}", t)
            val errorType = ConnectionErrorHelper.analyzeError(cause, mode)
            val rawMessage = when (t) {
                is NoSuchMethodError -> "Incompatible system API (NoSuchMethodError): ${t.message}"
                is NoClassDefFoundError -> "Missing runtime class (NoClassDefFoundError): ${t.message}"
                else -> t.message ?: "Unknown error"
            }
            val errorDetails = ConnectionErrorHelper.generateErrorDetails(
                type = errorType,
                originalMessage = rawMessage,
                mode = mode,
                port = port,
                ip = ip
            )

            _uiState.update {
                it.copy(
                    streamState = StreamState.Error,
                    errorMessage = errorDetails.localizedMessage,
                    showErrorDialog = true,
                    errorDetails = errorDetails
                )
            }
            return
        }
    }

    fun stopStream() {
        Logger.i("AudioStreamViewModel", "Stopping stream")
        if (isStopStreamRequestPending) {
            Logger.d("AudioStreamViewModel", "Stop stream request ignored: stop already pending")
            return
        }
        isStopStreamRequestPending = true
        auxiliaryScope.launch {
            try {
                _audioEngine.stopAndWait()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                Logger.e("AudioStreamViewModel", "Failed to stop stream", e)
            } finally {
                isStopStreamRequestPending = false
            }
        }
    }

    fun setMode(mode: ConnectionMode) {
        Logger.i("AudioStreamViewModel", "Setting connection mode to $mode")

        val current = _uiState.value

        val updatedPort = when (mode) {
            ConnectionMode.Usb -> {
                val parsed = current.port.toIntOrNull()
                if (parsed == null || parsed <= 0) Constants.DEFAULT_TCP_PORT.toString() else current.port
            }
            else -> current.port
        }

        // Auto-configure if enabled
        if (current.isAutoConfig) {
            applyAutoConfig()
        }

        _uiState.update { it.copy(mode = mode, port = updatedPort) }
        settings.putString("connection_mode", mode.name)

        // Manage discovery lifecycle based on mode
        if (mode == ConnectionMode.Wifi) {
            discoveryManager.startDiscovery()
        } else {
            discoveryManager.stopDiscovery()
        }
        if (updatedPort != current.port) {
            settings.putString("port", updatedPort)
        }
    }

    fun setTransportProtocol(protocol: TransportProtocol) {
        Logger.i("AudioStreamViewModel", "Setting transport protocol to $protocol")
        _uiState.update { it.copy(transportProtocol = protocol) }
        settings.putString("transport_protocol", protocol.name)
    }

    fun setIp(ip: String, restartStream: Boolean = false) {
        Logger.d("AudioStreamViewModel", "Setting IP to $ip, restartStream=$restartStream")
        val wasRunning = _uiState.value.streamState == StreamState.Streaming || _uiState.value.streamState == StreamState.Connecting

        _uiState.update {
            it.copy(
                ipAddress = ip
            )
        }
        if (ip.isNotBlank()) {
            settings.putString("ip_address", ip.trim())
        }

        // 如果要求重启流（IP 切换时），先停止再启动
        // Holds the start flag so a concurrent startStream() cannot race the restart;
        // stays on the main dispatcher like every other uiState writer.
        if (restartStream && wasRunning && ip.isNotBlank() && !isStartStreamRequestPending) {
            isStartStreamRequestPending = true
            auxiliaryScope.launch {
                try {
                    _audioEngine.stopAndWait()
                    startStreamInternal()
                } catch (e: kotlinx.coroutines.CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Logger.e("AudioStreamViewModel", "Failed to restart stream after IP change", e)
                } finally {
                    isStartStreamRequestPending = false
                }
            }
        }
    }

    fun setPort(port: String) {
        // 允许空字符串，以便用户重新输入
        if (port.isBlank()) {
            _uiState.update { it.copy(port = "") }
            return
        }

        // 验证端口输入是否为数字且在有效范围内
        val portInt = port.toIntOrNull()
        if (portInt != null && portInt in 1..65535) {
            Logger.d("AudioStreamViewModel", "Setting port to $port")
            _uiState.update { it.copy(port = port) }
            settings.putString("port", port)
        } else {
            Logger.d("AudioStreamViewModel", "Invalid port input ignored: $port")
            // 如果是非数字字符，我们不更新状态，保持原样
        }
    }

    fun setSampleRate(rate: SampleRate) {
        _uiState.update { it.copy(sampleRate = rate) }
        settings.putString("sample_rate", rate.name)
    }

    fun setChannelCount(count: ChannelCount) {
        _uiState.update { it.copy(channelCount = count) }
        settings.putString("channel_count", count.name)
    }

    fun setAudioFormat(format: AudioFormat) {
        _uiState.update { it.copy(audioFormat = format) }
        settings.putString("audio_format", format.name)
    }

    fun setAndroidAudioSource(sourceName: String) {
        _uiState.update { it.copy(androidAudioSourceName = sourceName) }
        settings.putString("android_audio_source", sourceName)
        _audioEngine.setAudioSource(sourceName)
    }

    fun setAutoConfig(enabled: Boolean) {
        _uiState.update { it.copy(isAutoConfig = enabled) }
        settings.putBoolean("is_auto_config", enabled)
        if (enabled) {
            applyAutoConfig()
        }
    }

    fun dismissErrorDialog() {
        _uiState.update { it.copy(showErrorDialog = false) }
    }

    fun dismissUdpWarningDialog() {
        _uiState.update { it.copy(showUdpWarningDialog = false) }
    }

    fun retryAfterError() {
        dismissErrorDialog()
        startStream()
    }

    fun close(): Job = synchronized(closeLock) {
        closeJob?.let { return@synchronized it }
        closed.set(true)
        discoveryManager.stopDiscovery()
        val engineCloseJob = _audioEngine.close()
        auxiliaryScope.cancel()
        closeJob = engineCloseJob
        engineCloseJob
    }

    override fun onCleared() {
        close()
        super.onCleared()
    }

    fun startDiscovery() {
        discoveryManager.startDiscovery()
    }
    fun stopDiscovery() {
        discoveryManager.stopDiscovery()
    }
    fun restartDiscovery() {
        discoveryManager.stopDiscovery()
        discoveryManager.startDiscovery()
    }

}
