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

package com.lanrhyme.micyou.soundpad

import android.os.SystemClock
import com.lanrhyme.micyou.audio.AudioEngine
import com.lanrhyme.micyou.network.PluginMessage
import com.lanrhyme.micyou.util.Logger
import com.lanrhyme.micyou.viewmodel.StreamState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.util.concurrent.atomic.AtomicLong

/** One live playback session on the desktop, with a local interpolation base. */
data class SoundPadSession(
    val sessionId: Long,
    val soundId: String,
    val label: String,
    val phase: String,
    val positionMs: Long,
    val durationMs: Long,
    val volume: Float,
    val loop: Boolean,
    /** elapsedRealtime when this snapshot arrived (interpolation baseline). */
    val updatedAt: Long
) {
    val isPlaying: Boolean get() = phase == SoundPadSessionDto.PHASE_PLAYING
    val isLoading: Boolean get() = phase == SoundPadSessionDto.PHASE_LOADING

    /** Wall-clock extrapolated position — keeps sliders smooth between pushes. */
    fun interpolatedPosition(now: Long = SystemClock.elapsedRealtime()): Long {
        if (phase != SoundPadSessionDto.PHASE_PLAYING) return positionMs
        val raw = positionMs + (now - updatedAt).coerceAtLeast(0L)
        return if (loop && durationMs > 0) raw % durationMs else raw.coerceAtMost(durationMs)
    }

    companion object {
        fun from(dto: SoundPadSessionDto, now: Long = SystemClock.elapsedRealtime()) = SoundPadSession(
            sessionId = dto.sessionId,
            soundId = dto.id,
            label = dto.label,
            phase = dto.phase,
            positionMs = dto.positionMs,
            durationMs = dto.durationMs,
            volume = dto.volume.toFloat(),
            loop = dto.loop,
            updatedAt = now
        )
    }
}

data class SoundPadUiState(
    /** Desktop answered a hello — the soundpad plugin is present and enabled. */
    val available: Boolean = false,
    val desktopVersion: String = "",
    val sounds: List<SoundPadSound> = emptyList(),
    val sessions: List<SoundPadSession> = emptyList(),
    val showPanel: Boolean = false,
    val selectedSessionId: Long? = null,
    /** VAD anti-collision guard (desktop ducks sounds while you speak). */
    val vadGuard: Boolean = false,
    /** Folder being browsed in the panel ("" = library root). */
    val currentDir: String = "",
    /** A `list` request is in flight (toolbar spinner). */
    val refreshing: Boolean = false,
    /** One-shot error text for a snackbar; cleared via [SoundPadController.consumeError]. */
    val errorMessage: String? = null
) {
    val selectedSession: SoundPadSession?
        get() = selectedSessionId?.let { id -> sessions.find { it.sessionId == id } }
            ?: sessions.firstOrNull { it.isPlaying }
}

/**
 * Remote control for the desktop `opss.soundpad` plugin.
 *
 * Lifecycle: owned by MainViewModel. Watches [AudioEngine.streamState] —
 * while connected, polls `hello` (2 s until discovered, then 5 s keepalive);
 * three missed keepalives flip [SoundPadUiState.available] back to false
 * (plugin disabled / desktop restarted). Commands and pushes ride the
 * cross-device plugin messages added to the control protocol (field 7).
 */
class SoundPadController(
    private val engine: AudioEngine,
    private val scope: CoroutineScope
) {
    private val _state = MutableStateFlow(SoundPadUiState())
    val state: StateFlow<SoundPadUiState> = _state.asStateFlow()

    private val nextReq = AtomicLong(1)
    @Volatile
    private var missedHellos = 0

    private var discoveryJob: Job? = null
    private var tickerJob: Job? = null
    private var refreshTimeoutJob: Job? = null

    init {
        scope.launch {
            engine.streamState.collect { st ->
                if (st == StreamState.Streaming) startDiscovery() else onDisconnected()
            }
        }
        scope.launch {
            engine.pluginMessages.collect { onPluginMessage(it) }
        }
    }

    // ─────────────────────────── discovery ───────────────────────────

    private fun startDiscovery() {
        ensureTicker()
        if (discoveryJob?.isActive == true) return
        missedHellos = 0
        discoveryJob = scope.launch {
            while (isActive) {
                sendHello()
                val interval = if (_state.value.available) KEEPALIVE_MS else DISCOVERY_MS
                delay(interval)
            }
        }
    }

    private fun onDisconnected() {
        discoveryJob?.cancel()
        discoveryJob = null
        missedHellos = 0
        _state.update {
            it.copy(
                available = false, sessions = emptyList(), selectedSessionId = null,
                currentDir = "", refreshing = false
            )
        }
    }

    /** Local progress interpolation while any voice plays. */
    private fun ensureTicker() {
        if (tickerJob?.isActive == true) return
        tickerJob = scope.launch {
            while (isActive) {
                delay(TICK_MS)
                val s = _state.value
                if (s.sessions.any { it.isPlaying }) {
                    // Recompute interpolated positions; copying the list keeps
                    // Compose observers informed without a server round-trip.
                    val now = SystemClock.elapsedRealtime()
                    _state.update {
                        it.copy(sessions = it.sessions.map { sess ->
                            sess.copy(
                                positionMs = sess.interpolatedPosition(now),
                                updatedAt = now
                            )
                        })
                    }
                }
            }
        }
    }

    // ─────────────────────────── inbound ───────────────────────────

    private fun onPluginMessage(pm: PluginMessage) {
        if (pm.source != SoundPadProtocol.DESKTOP_PLUGIN_ID) return
        val text = pm.payload.toString(Charsets.UTF_8)
        val env = try {
            soundPadJson.decodeFromString(SoundPadEnvelope.serializer(), text)
        } catch (e: Exception) {
            Logger.w(TAG, "Unparsable soundpad payload: ${e.message}")
            return
        }
        when (env.op) {
            "hello" -> {
                missedHellos = 0
                val now = SystemClock.elapsedRealtime()
                _state.update {
                    it.copy(
                        available = true,
                        desktopVersion = env.version,
                        sounds = env.sounds,
                        sessions = env.sessions.map { dto -> SoundPadSession.from(dto, now) },
                        selectedSessionId = keepSelection(it.selectedSessionId, env.sessions.map { s -> s.sessionId }),
                        vadGuard = env.vadGuard,
                        refreshing = false
                    )
                }
            }
            "list" -> _state.update {
                it.copy(available = true, sounds = env.sounds, refreshing = false)
            }
            "state" -> applySessions(env.sessions, vadGuard = env.vadGuard)
            "play" -> {
                env.session?.let { dto ->
                    applySessions(
                        mergeSession(currentDtos(), dto),
                        select = dto.sessionId
                    )
                }
            }
            "error" -> {
                Logger.w(TAG, "Desktop soundpad error (req=${env.req}): ${env.message}")
                _state.update {
                    it.copy(errorMessage = env.message.ifBlank { "Soundpad error" })
                }
            }
            else -> Logger.d(TAG, "Ignored soundpad op: ${env.op}")
        }
    }

    private fun applySessions(dtos: List<SoundPadSessionDto>, select: Long? = null, vadGuard: Boolean? = null) {
        val now = SystemClock.elapsedRealtime()
        _state.update {
            it.copy(
                sessions = dtos.map { dto -> SoundPadSession.from(dto, now) },
                selectedSessionId = select ?: keepSelection(it.selectedSessionId, dtos.map { d -> d.sessionId }),
                vadGuard = vadGuard ?: it.vadGuard
            )
        }
    }

    private fun currentDtos(): List<SoundPadSessionDto> = _state.value.sessions.map {
        SoundPadSessionDto(
            sessionId = it.sessionId, id = it.soundId, label = it.label, phase = it.phase,
            positionMs = it.positionMs, durationMs = it.durationMs,
            volume = it.volume.toDouble(), loop = it.loop
        )
    }

    private fun mergeSession(
        dtos: List<SoundPadSessionDto>,
        dto: SoundPadSessionDto
    ): List<SoundPadSessionDto> {
        val out = dtos.toMutableList()
        val idx = out.indexOfFirst { it.sessionId == dto.sessionId }
        if (idx >= 0) out[idx] = dto else out.add(dto)
        return out
    }

    private fun keepSelection(current: Long?, ids: List<Long>): Long? {
        if (current != null && ids.contains(current)) return current
        return null // panel falls back to the first playing session
    }

    // ─────────────────────────── outbound ───────────────────────────

    private fun send(obj: JsonObject): Boolean {
        val payload = obj.toString().toByteArray(Charsets.UTF_8)
        val ok = engine.sendPluginMessage(
            PluginMessage(
                source = SoundPadProtocol.PHONE_PLUGIN_ID,
                target = SoundPadProtocol.DESKTOP_PLUGIN_ID,
                topic = SoundPadProtocol.TOPIC_CMD,
                payload = payload
            )
        )
        if (!ok) Logger.d(TAG, "Soundpad command dropped (no active control channel)")
        return ok
    }

    private fun sendHello() {
        val sent = send(buildJsonObject {
            put("op", "hello")
            put("req", nextReq.getAndIncrement())
            put("client", "android")
        })
        if (sent && _state.value.available) {
            val missed = ++missedHellos
            if (missed >= MAX_MISSED_HELLOS) {
                Logger.i(TAG, "Desktop soundpad stopped answering ($missed hellos) — hiding panel button")
                _state.update { it.copy(available = false, sessions = emptyList(), selectedSessionId = null) }
                missedHellos = 0
            }
        }
    }

    // ─────────────────────────── public actions ───────────────────────────

    /**
     * Play a sound. Volume is intentionally omitted so the desktop resolves
     * its per-sound remembered level (the play reply/state carries the
     * effective volume back for the slider).
     */
    fun play(soundId: String, loop: Boolean = false) {
        send(buildJsonObject {
            put("op", "play")
            put("req", nextReq.getAndIncrement())
            put("id", soundId)
            put("loop", loop)
        })
    }

    /** Stop one session; null = stop all. */
    fun stop(sessionId: Long? = null) {
        send(buildJsonObject {
            put("op", "stop")
            put("req", nextReq.getAndIncrement())
            if (sessionId != null) put("sessionId", sessionId)
        })
        if (sessionId == null) {
            _state.update { it.copy(sessions = emptyList(), selectedSessionId = null) }
        }
    }

    fun seek(sessionId: Long, positionMs: Long) {
        // Optimistic local update keeps the slider where the user dropped it.
        _state.update { st ->
            st.copy(sessions = st.sessions.map {
                if (it.sessionId == sessionId) it.copy(
                    positionMs = positionMs,
                    updatedAt = SystemClock.elapsedRealtime()
                ) else it
            })
        }
        send(buildJsonObject {
            put("op", "seek")
            put("req", nextReq.getAndIncrement())
            put("sessionId", sessionId)
            put("positionMs", positionMs)
        })
    }

    fun setVolume(sessionId: Long, volume: Float) {
        updateLocal(sessionId) { it.copy(volume = volume) }
        send(buildJsonObject {
            put("op", "volume")
            put("req", nextReq.getAndIncrement())
            put("sessionId", sessionId)
            put("volume", volume.toDouble())
        })
    }

    /** Toggle the desktop VAD anti-collision guard (ducking while speaking). */
    fun setVadGuard(enable: Boolean) {
        // Optimistic: the desktop echoes the authoritative value in `state`.
        _state.update { it.copy(vadGuard = enable) }
        send(buildJsonObject {
            put("op", "vad")
            put("req", nextReq.getAndIncrement())
            put("enable", enable)
        })
    }

    fun setLoop(sessionId: Long, loop: Boolean) {
        updateLocal(sessionId) { it.copy(loop = loop) }
        send(buildJsonObject {
            put("op", "loop")
            put("req", nextReq.getAndIncrement())
            put("sessionId", sessionId)
            put("loop", loop)
        })
    }

    /** Re-request the sound list (desktop rescans its sounds directory). */
    fun refreshLibrary() {
        _state.update { it.copy(refreshing = true) }
        send(buildJsonObject {
            put("op", "list")
            put("req", nextReq.getAndIncrement())
        })
        // Fallback: never leave the spinner stuck when the reply is lost.
        refreshTimeoutJob?.cancel()
        refreshTimeoutJob = scope.launch {
            delay(REFRESH_TIMEOUT_MS)
            _state.update { it.copy(refreshing = false) }
        }
    }

    // ─────────────────────────── folder navigation ───────────────────────────

    /** Browse into a subfolder (path relative to the library root). */
    fun enterFolder(dir: String) {
        _state.update { it.copy(currentDir = dir) }
    }

    /** Up one folder level; no-op at the root. */
    fun navigateUp() {
        _state.update {
            val cur = it.currentDir
            it.copy(currentDir = if ('/' in cur) cur.substringBeforeLast('/') else "")
        }
    }

    /** Pull a fresh session snapshot (e.g. when the panel opens). */
    fun requestState() {
        send(buildJsonObject {
            put("op", "state")
            put("req", nextReq.getAndIncrement())
        })
    }

    fun togglePanel() {
        _state.update { it.copy(showPanel = !it.showPanel) }
        if (_state.value.showPanel) {
            refreshLibrary()
            requestState()
        }
    }

    fun selectSession(sessionId: Long?) {
        _state.update { it.copy(selectedSessionId = sessionId) }
    }

    fun consumeError() {
        _state.update { it.copy(errorMessage = null) }
    }

    private fun updateLocal(sessionId: Long, f: (SoundPadSession) -> SoundPadSession) {
        _state.update { st ->
            st.copy(sessions = st.sessions.map { if (it.sessionId == sessionId) f(it) else it })
        }
    }

    companion object {
        private const val TAG = "SoundPadController"
        private const val DISCOVERY_MS = 2_000L
        private const val KEEPALIVE_MS = 5_000L
        private const val TICK_MS = 150L
        private const val MAX_MISSED_HELLOS = 3
        private const val REFRESH_TIMEOUT_MS = 3_000L
    }
}
