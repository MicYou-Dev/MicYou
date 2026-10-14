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

package com.lanrhyme.micyou

import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.tooling.preview.Preview
import androidx.lifecycle.viewmodel.compose.viewModel
import com.lanrhyme.micyou.ui.background.BackgroundImagePicker
import com.lanrhyme.micyou.audio.AudioEngine
import com.lanrhyme.micyou.service.AudioService
import com.lanrhyme.micyou.theme.isDarkThemeActive
import com.lanrhyme.micyou.ui.dialog.getRequiredPermissions
import com.lanrhyme.micyou.ui.dialog.hasAllRequiredPermissions
import com.lanrhyme.micyou.util.AppLanguage
import com.lanrhyme.micyou.util.ContextHelper
import com.lanrhyme.micyou.util.Logger
import com.lanrhyme.micyou.util.PermissionState
import com.lanrhyme.micyou.util.setAppLocale
import com.lanrhyme.micyou.viewmodel.MainViewModel
import com.lanrhyme.micyou.viewmodel.StreamState
import java.util.Locale

class MainActivity : ComponentActivity() {

    private var permissionDialogState = mutableStateOf(false)
    private var currentPermissionsState = mutableStateOf<List<PermissionState>>(emptyList())
    private var permissionDialogDismissed = mutableStateOf(false)

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { _ ->
        // Update permission states after request
        currentPermissionsState.value = getRequiredPermissions(this)

        // Check if all required permissions are granted
        val allRequiredGranted = hasAllRequiredPermissions(currentPermissionsState.value)
        if (!allRequiredGranted) {
            // Keep showing dialog if required permissions not granted
            permissionDialogState.value = true
        } else {
            permissionDialogState.value = false
            permissionDialogDismissed.value = true
        }
    }

    fun requestPermissions(permissions: List<String>) {
        permissionLauncher.launch(permissions.toTypedArray())
    }

    fun showPermissionDialog() {
        currentPermissionsState.value = getRequiredPermissions(this)
        permissionDialogState.value = true
    }

    fun hidePermissionDialog() {
        permissionDialogState.value = false
        permissionDialogDismissed.value = true
    }

    fun shouldShowPermissionDialog(): Boolean {
        return !hasAllRequiredPermissions(getRequiredPermissions(this))
    }

    override fun attachBaseContext(newBase: Context) {
        val savedLanguage = try {
            val prefs = newBase.getSharedPreferences("android_mic_prefs", Context.MODE_PRIVATE)
            val raw = prefs.getString("language", null)
            if (raw != null) {
                try {
                    AppLanguage.valueOf(raw).code
                } catch (_: Exception) {
                    raw
                }
            } else {
                "system"
            }
        } catch (_: Exception) {
            "system"
        }
        val wrappedContext = if (savedLanguage != "system") {
            wrapContextWithLocale(newBase, savedLanguage)
        } else {
            newBase
        }
        super.attachBaseContext(wrappedContext)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        ContextHelper.init(this)

        // Initialize locale from saved settings
        val savedLanguage = try {
            val prefs = getSharedPreferences("android_mic_prefs", MODE_PRIVATE)
            val raw = prefs.getString("language", null)
            if (raw != null) {
                try {
                    AppLanguage.valueOf(raw).code
                } catch (_: Exception) {
                    raw
                }
            } else {
                "system"
            }
        } catch (_: Exception) {
            "system"
        }
        setAppLocale(savedLanguage)

        Logger.init(this)
        Logger.i("MainActivity", "App started")

        BackgroundImagePicker.register(this)
        val shouldQuickStart = intent?.action == ACTION_QUICK_START

        // Initialize permission state
        currentPermissionsState.value = getRequiredPermissions(this)

        // Show permission dialog first if needed (before first launch dialog)
        val needsPermissions = shouldShowPermissionDialog()
        if (needsPermissions) {
            permissionDialogState.value = true
        } else {
            // No permissions needed, mark as dismissed so first launch can show
            permissionDialogDismissed.value = true
        }

        // 打开 App 即常驻显示通知（未串流时为空闲前台服务）。
        // 串流时 AudioEngine 会用 ACTION_START 升级为 mic 前台服务。
        if (!AudioEngine.isStreaming()) {
            try {
                val idleIntent = Intent(this, AudioService::class.java).apply {
                    action = AudioService.ACTION_START_IDLE
                }
                // [API 21+ 兼容] startForegroundService() 是 API 26+ 才有的，
                // 低版本用 startService() 启动前台服务（service 内部会调用 startForeground()）。
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    startForegroundService(idleIntent)
                } else {
                    startService(idleIntent)
                }
            } catch (_: Exception) {
                // 空闲保活失败不阻塞启动
            }
        }

        setContent {
            val appViewModel: MainViewModel = viewModel()
            val uiState by appViewModel.uiState.collectAsState()

            val keepScreenOn by remember { derivedStateOf { uiState.keepScreenOn } }
            val streamState by remember { derivedStateOf { uiState.streamState } }
            val themeMode by remember { derivedStateOf { uiState.themeMode } }

            LaunchedEffect(shouldQuickStart) {
                if (shouldQuickStart && uiState.streamState == StreamState.Idle) {
                    // Don't auto-start if permissions are missing
                    if (!needsPermissions) {
                        appViewModel.startStream()
                        moveTaskToBack(true)
                    }
                }
            }

            LaunchedEffect(shouldQuickStart, streamState) {
                if (shouldQuickStart && !needsPermissions) {
                    when (streamState) {
                        StreamState.Streaming -> {
                            Toast.makeText(this@MainActivity, R.string.qs_toast_connected, Toast.LENGTH_SHORT).show()
                        }
                        StreamState.Error -> {
                            Toast.makeText(this@MainActivity, R.string.qs_toast_failed, Toast.LENGTH_SHORT).show()
                        }
                        else -> {}
                    }
                }
            }

            val isDark = isDarkThemeActive(themeMode)

            DisposableEffect(isDark) {
                this@MainActivity.enableEdgeToEdge(
                    statusBarStyle = if (isDark) {
                        SystemBarStyle.dark(android.graphics.Color.TRANSPARENT)
                    } else {
                        SystemBarStyle.light(
                            android.graphics.Color.TRANSPARENT,
                            android.graphics.Color.TRANSPARENT
                        )
                    },
                    navigationBarStyle = if (isDark) {
                        SystemBarStyle.dark(android.graphics.Color.TRANSPARENT)
                    } else {
                        SystemBarStyle.light(
                            android.graphics.Color.TRANSPARENT,
                            android.graphics.Color.TRANSPARENT
                        )
                    }
                )
                onDispose {}
            }

            DisposableEffect(keepScreenOn) {
                if (keepScreenOn) {
                    window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                } else {
                    window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                }

                onDispose {
                    window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                }
            }

            // Permission dialog state from activity
            val showPermissionDialog by permissionDialogState
            val currentPermissions by currentPermissionsState
            val isPermissionDialogDismissed by permissionDialogDismissed

            App(
                viewModel = appViewModel,
                activity = this@MainActivity,
                showPermissionDialog = showPermissionDialog,
                currentPermissions = currentPermissions,
                onRequestPermissions = { perms ->
                    requestPermissions(perms)
                },
                onPermissionDialogDismiss = {
                    hidePermissionDialog()
                },
                isPermissionDialogDismissed = isPermissionDialogDismissed
            )
        }
    }

    companion object {
        const val ACTION_QUICK_START = "com.lanrhyme.micyou.ACTION_QUICK_START"

        fun wrapContextWithLocale(context: Context, languageCode: String): Context {
            val locale = try {
                val parts = when {
                    languageCode.contains("-r") -> languageCode.split("-r", limit = 2)
                    languageCode.contains("-") -> languageCode.split("-", limit = 2)
                    else -> listOf(languageCode)
                }
                Locale.Builder().setLanguage(parts[0]).apply {
                    parts.getOrNull(1)?.let(::setRegion)
                }.build()
            } catch (_: Exception) {
                Locale.getDefault()
            }
            val config = Configuration(context.resources.configuration)
            config.setLocale(locale)
            return context.createConfigurationContext(config)
        }
    }
}

@Preview
@Composable
fun AppAndroidPreview() {
    App()
}
