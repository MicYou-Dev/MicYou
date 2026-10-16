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

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * Wire identities and JSON shapes of the Soundpad cross-device protocol
 * (v1). The desktop side is the `opss.soundpad` native plugin
 * (github.com/OrientCOMPASS/MicYouSoundpad-plugin, docs/PROTOCOL.md).
 *
 * Transport: [com.lanrhyme.micyou.network.PluginMessage] inside the TCP
 * control channel. Phone → desktop uses topic [TOPIC_CMD]; desktop → phone
 * arrives with the topic rewritten to our plugin id by the host, so routing
 * is done on `source` + the payload's `op` field only.
 */
object SoundPadProtocol {
    /** Desktop plugin id — discovery target and inbound-message filter. */
    const val DESKTOP_PLUGIN_ID = "opss.soundpad"

    /**
     * Virtual plugin id of this app on the plugin bus. The desktop plugin
     * echoes it back as the reply target; it never needs to exist as an
     * installed plugin anywhere.
     */
    const val PHONE_PLUGIN_ID = "micyou.android.soundpad"

    /** Topic for every phone → desktop command. */
    const val TOPIC_CMD = "cmd"
}

/** Lenient JSON: the desktop may add fields in future versions. */
val soundPadJson = Json {
    ignoreUnknownKeys = true
    isLenient = true
    coerceInputValues = true
    explicitNulls = false
}

/** One playable sound in the desktop library. */
@Serializable
data class SoundPadSound(
    val id: String,
    val label: String = "",
    val durationMs: Long = 0,
    /** Folder relative to the desktop sounds root ("" = root). */
    val dir: String = ""
) {
    /** File-stem display name (id may carry a folder prefix). */
    val displayName: String get() = label.ifBlank { id.substringAfterLast('/') }
}

/** Desktop-reported playback session (dto over the wire). */
@Serializable
data class SoundPadSessionDto(
    val sessionId: Long = 0,
    val id: String = "",
    val label: String = "",
    val phase: String = PHASE_LOADING,
    val positionMs: Long = 0,
    val durationMs: Long = 0,
    val volume: Double = 1.0,
    val loop: Boolean = false
) {
    companion object {
        const val PHASE_LOADING = "loading"
        const val PHASE_PLAYING = "playing"
        const val PHASE_STOPPING = "stopping"
    }
}

/** Envelope for every desktop → phone payload. */
@Serializable
data class SoundPadEnvelope(
    val op: String = "",
    val req: Long = 0,
    val plugin: String = "",
    val version: String = "",
    val message: String = "",
    val sounds: List<SoundPadSound> = emptyList(),
    val sessions: List<SoundPadSessionDto> = emptyList(),
    val session: SoundPadSessionDto? = null
)
