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

package com.lanrhyme.micyou.util

import java.io.BufferedReader
import java.io.InputStreamReader
import java.util.Locale as JavaLocale
import com.lanrhyme.micyou.util.AppLanguage
import com.lanrhyme.micyou.util.ContextHelper
import com.lanrhyme.micyou.util.Logger
import com.lanrhyme.micyou.util.PermissionState
import com.lanrhyme.micyou.util.PermissionType

enum class AppLanguage(val label: String, val code: String) {
    System("跟随系统", "system"),
    Chinese("简体中文", "zh"),
    ChineseTraditional("繁體中文", "zh-TW"),
    Cantonese("粤语", "zh-HK"),
    English("English", "en"),
    ChineseCat("中文（猫猫语）🐱", "ca"),
    ChineseHard("中文（坚硬）", "zh-rHD"),
}

data class PermissionState(
    val type: PermissionType,
    val manifestPermission: String,
    val isGranted: Boolean,
    val minSdkVersion: Int = 0
)

enum class PermissionType(val labelKey: String, val descKey: String, val isRequired: Boolean) {
    RECORD_AUDIO(
        labelKey = "permissionRecordAudioLabel",
        descKey = "permissionRecordAudioDesc",
        isRequired = true
    ),
    POST_NOTIFICATIONS(
        labelKey = "permissionPostNotificationsLabel",
        descKey = "permissionPostNotificationsDesc",
        isRequired = false
    )
}

/** Captured at app startup, never modified by setAppLocale. */
private val originalSystemLocale: JavaLocale = JavaLocale.getDefault()

/** Locale for an [AppLanguage.code]; "system" is the locale the app started with. */
fun appLocale(languageCode: String): JavaLocale {
    if (languageCode == "system") return originalSystemLocale
    return try {
        // e.g. "zh-rHD" -> values-zh-rHD
        val parts = if (languageCode.contains("-r")) {
            languageCode.split("-r", limit = 2)
        } else if (languageCode.contains("-")) {
            languageCode.split("-", limit = 2)
        } else {
            null
        }
        if (parts != null && parts.size == 2) {
            JavaLocale.Builder().setLanguage(parts[0]).setRegion(parts[1]).build()
        } else {
            JavaLocale.Builder().setLanguage(languageCode).build()
        }
    } catch (e: Exception) {
        Logger.e("Localization", "Failed to parse locale: $languageCode", e)
        originalSystemLocale
    }
}

fun setAppLocale(languageCode: String) {
    val locale = appLocale(languageCode)
    JavaLocale.setDefault(locale)
    ContextHelper.setLocale(locale)
}

fun readResourceFile(path: String): String? {
    return try {
        val context = ContextHelper.getContext() ?: return null
        val assetManager = context.assets
        BufferedReader(InputStreamReader(assetManager.open(path), "UTF-8")).use { reader ->
            reader.readText()
        }
    } catch (e: Exception) {
        Logger.e("Localization", "Failed to read resource file: $path - ${e.message}")
        null
    }
}