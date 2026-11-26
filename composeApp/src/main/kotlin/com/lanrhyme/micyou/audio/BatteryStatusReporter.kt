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

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import com.lanrhyme.micyou.network.ClientStatusMessage
import com.lanrhyme.micyou.util.Logger

/**
 * 读取本机电量并监听变化，把结果作为 `ClientStatusMessage` 上报给桌面端。
 *
 * 只依赖 `ACTION_BATTERY_CHANGED` 粘性广播：注册时系统立刻回调一次当前值，
 * 之后每次电量/充电状态变化再回调，因此调用方无需自己轮询。
 */
class BatteryStatusReporter(private val context: Context) {

    private var receiver: BroadcastReceiver? = null

    /**
     * 开始监听，[onChange] 会在注册时收到一次当前状态，之后仅在变化时回调。
     * 该回调可能运行在主线程，实现必须是快速且非阻塞的。
     */
    fun start(onChange: (ClientStatusMessage) -> Unit) {
        if (receiver != null) return
        val callback = object : BroadcastReceiver() {
            override fun onReceive(receiverContext: Context?, intent: Intent?) {
                val status = read(intent) ?: return
                onChange(status)
            }
        }
        receiver = callback
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                context.registerReceiver(callback, IntentFilter(Intent.ACTION_BATTERY_CHANGED), Context.RECEIVER_NOT_EXPORTED)
            } else {
                @Suppress("UnspecifiedRegisterReceiverFlag")
                context.registerReceiver(callback, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
            }
        } catch (e: Exception) {
            // 电量上报是锦上添花，注册失败不应拖垮音频会话。
            Logger.w("BatteryStatusReporter", "Failed to register battery receiver: ${e.message}")
            receiver = null
        }
    }

    fun stop() {
        val callback = receiver ?: return
        receiver = null
        try {
            context.unregisterReceiver(callback)
        } catch (e: Exception) {
            Logger.w("BatteryStatusReporter", "Failed to unregister battery receiver: ${e.message}")
        }
    }

    /** 粘性广播的 extras → 协议字段；读不到时返回 null。 */
    private fun read(intent: Intent?): ClientStatusMessage? {
        if (intent == null) return null
        val level = intent.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
        val scale = intent.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
        if (level < 0 || scale <= 0) return null
        val percent = (level * 100 / scale).coerceIn(0, 100)
        val status = intent.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
        val charging = status == BatteryManager.BATTERY_STATUS_CHARGING ||
            status == BatteryManager.BATTERY_STATUS_FULL
        return ClientStatusMessage(batteryLevel = percent, isCharging = charging)
    }
}
