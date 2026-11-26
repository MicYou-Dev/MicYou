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

package com.lanrhyme.micyou.audio

/**
 * 桌面端每秒回传一次的链路质量，数据来自桌面接收侧测量
 * （micyou-protocol network.proto 的 `StatsMessage`）。
 *
 * @property rttMs 平滑后的 ping/pong 往返时延，首个 pong 之前为 0
 * @property jitterMs 音频包到达抖动（毫秒）
 * @property packetLoss 丢包率，0.0 ~ 1.0
 * @property latencyMs 端到端估算（接收缓冲 + RTT）
 */
data class LinkQuality(
    val rttMs: Int,
    val jitterMs: Float,
    val packetLoss: Float,
    val latencyMs: Int
)
