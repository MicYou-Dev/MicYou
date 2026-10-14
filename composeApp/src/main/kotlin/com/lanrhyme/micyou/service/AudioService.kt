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

package com.lanrhyme.micyou.service
import com.lanrhyme.micyou.R

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.res.Configuration
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.net.wifi.WifiManager
import android.app.PendingIntent
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.lanrhyme.micyou.audio.AudioEngine
import com.lanrhyme.micyou.MainActivity
import com.lanrhyme.micyou.util.AppLanguage
import com.lanrhyme.micyou.util.appLocale
import com.lanrhyme.micyou.util.getString
class AudioService : Service() {

    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    /** Whether this instance has called startForeground. */
    private var inForeground = false

    companion object {
        private const val CHANNEL_ID = "AudioServiceChannel"
        private const val NOTIFICATION_ID = 1
        const val ACTION_START = "ACTION_START"
        const val ACTION_START_IDLE = "ACTION_START_IDLE"
        const val ACTION_STOP = "ACTION_STOP"
        const val ACTION_DISCONNECT = "ACTION_DISCONNECT"
        const val EXTRA_USE_WIFI_LOCK = "EXTRA_USE_WIFI_LOCK"
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startForegroundService(
                intent.getBooleanExtra(EXTRA_USE_WIFI_LOCK, false),
                streaming = true
            )
            ACTION_START_IDLE -> startForegroundService(useWifiLock = false, streaming = false)
            // Streaming ended. A fresh instance means the service was already stopped
            // (the app was swiped away): there is no notification to downgrade, and
            // startForeground from the background would throw on Android 12+.
            ACTION_STOP -> if (inForeground) enterIdle() else stopSelf()
            ACTION_DISCONNECT -> {
                AudioEngine.requestDisconnectFromNotification()
                enterIdle()
            }
        }
        // The stream lives in AudioEngine, which dies with the process: a service the
        // system recreates would only show a stale notification, and Android 14+ refuses
        // to start a microphone foreground service from the background anyway.
        return START_NOT_STICKY
    }

    private fun startForegroundService(useWifiLock: Boolean, streaming: Boolean) {
        if (streaming) {
            acquireSessionLocks(useWifiLock)
        } else {
            releaseSessionLocks()
        }
        val notification = createNotification(streaming)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val type = if (streaming) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
            } else {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
            }
            startForeground(NOTIFICATION_ID, notification, type)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        inForeground = true
    }

    private fun enterIdle() {
        releaseSessionLocks()
        val notification = createNotification(streaming = false)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun acquireSessionLocks(useWifiLock: Boolean) {
        if (wakeLock?.isHeld != true) {
            wakeLock = (getSystemService(POWER_SERVICE) as PowerManager).newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK,
                "$packageName:audio-stream"
            ).apply {
                setReferenceCounted(false)
                acquire()
            }
        }
        if (useWifiLock && wifiLock?.isHeld != true) {
            @Suppress("DEPRECATION")
            wifiLock = (applicationContext.getSystemService(WIFI_SERVICE) as WifiManager).createWifiLock(
                WifiManager.WIFI_MODE_FULL_HIGH_PERF,
                "$packageName:audio-stream"
            ).apply {
                setReferenceCounted(false)
                acquire()
            }
        } else if (!useWifiLock) {
            wifiLock?.let { if (it.isHeld) it.release() }
            wifiLock = null
        }
    }

    private fun releaseSessionLocks() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
        wifiLock?.let { if (it.isHeld) it.release() }
        wifiLock = null
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        // Swiping the app away exits it, streaming or not. The engine belongs to the
        // activity's view model and stops with it anyway; stop it here explicitly.
        AudioEngine.requestDisconnectFromNotification()
        releaseSessionLocks()
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        inForeground = false
        stopSelf()
    }

    // [API 21+ 兼容] PendingIntent.FLAG_IMMUTABLE 是 API 23+ 才有的常量，
    // 低版本只用 FLAG_UPDATE_CURRENT（默认行为即不可变）。
    private fun pendingIntentFlags(): Int = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    } else {
        PendingIntent.FLAG_UPDATE_CURRENT
    }

    override fun onDestroy() {
        releaseSessionLocks()
        super.onDestroy()
    }

    private fun createNotification(streaming: Boolean): Notification {
        val (title, text) = if (streaming) {
            resolveNotificationText()
        } else {
            resolveIdleNotificationText()
        }
        val contentIntent = if (streaming) {
            val disconnectIntent = Intent(this, AudioService::class.java).apply { action = ACTION_DISCONNECT }
            PendingIntent.getService(
                this,
                0,
                disconnectIntent,
                pendingIntentFlags()
            )
        } else {
            val openApp = Intent(this, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            }
            PendingIntent.getActivity(
                this,
                1,
                openApp,
                pendingIntentFlags()
            )
        }

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(text)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setContentIntent(contentIntent)
            .build()
    }

    private fun resolveNotificationText(): Pair<String, String> {
        val context = localizedContext()
        return context.getString(R.string.streaming_notification_title) to
            context.getString(R.string.streaming_notification_text)
    }

    private fun resolveIdleNotificationText(): Pair<String, String> {
        val context = localizedContext()
        return context.getString(R.string.notification_idle_title) to
            context.getString(R.string.notification_idle_text)
    }

    /**
     * Resources in the language picked in the app. The service can run without the
     * activity (Quick Settings tile), so it reads the preference itself instead of
     * relying on the locale the activity installs.
     */
    private fun localizedContext(): Context {
        val saved = getSharedPreferences("android_mic_prefs", Context.MODE_PRIVATE)
            .getString("language", AppLanguage.System.name)
        val language = AppLanguage.entries.firstOrNull { it.name == saved } ?: AppLanguage.System
        if (language == AppLanguage.System) return this
        val config = Configuration(resources.configuration).apply { setLocale(appLocale(language.code)) }
        return createConfigurationContext(config)
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channelName = getString(R.string.audioStreamingService)
            val serviceChannel = NotificationChannel(
                CHANNEL_ID,
                channelName,
                NotificationManager.IMPORTANCE_LOW
            )
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.createNotificationChannel(serviceChannel)
        }
    }

    override fun onBind(intent: Intent?): IBinder? {
        return null
    }
}
