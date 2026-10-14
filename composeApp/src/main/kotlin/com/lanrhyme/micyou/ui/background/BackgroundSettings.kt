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

package com.lanrhyme.micyou.ui.background

import android.net.Uri
import android.webkit.MimeTypeMap
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import com.lanrhyme.micyou.util.ContextHelper
import com.lanrhyme.micyou.util.Logger

data class BackgroundSettings(
    val imagePath: String = "",
    val brightness: Float = 0.5f,
    val blurRadius: Float = 0f,
    val cardOpacity: Float = 1f,
    val enableHazeEffect: Boolean = false
) {
    val hasCustomBackground: Boolean
        get() = imagePath.isNotEmpty()
}

object BackgroundImagePicker {
    private var launcher: ActivityResultLauncher<PickVisualMediaRequest>? = null
    private var pending: CompletableDeferred<Uri?>? = null

    // 必须在 Activity 进入 STARTED 之前调用（onCreate 中）；重建时会替换旧的 launcher
    fun register(activity: ComponentActivity) {
        launcher = activity.registerForActivityResult(PickVisualMedia()) { uri ->
            pending?.complete(uri)
            pending = null
        }
    }

    fun pickImage(scope: CoroutineScope, onResult: (String?) -> Unit) {
        scope.launch {
            try {
                val uri = pick()
                val savedPath = uri?.let { copyToInternalStorage(it) }
                onResult(savedPath)
            } catch (e: Exception) {
                Logger.e("BackgroundImagePicker", "Failed to pick image", e)
                onResult(null)
            }
        }
    }

    private suspend fun pick(): Uri? {
        val launcher = launcher ?: return null
        pending?.complete(null)
        val result = CompletableDeferred<Uri?>()
        pending = result
        launcher.launch(PickVisualMediaRequest(PickVisualMedia.ImageOnly))
        return result.await()
    }

    private suspend fun copyToInternalStorage(uri: Uri): String? = withContext(Dispatchers.IO) {
        try {
            val context = ContextHelper.getContext() ?: return@withContext null
            val resolver = context.contentResolver
            val extension = resolver.getType(uri)
                ?.let { MimeTypeMap.getSingleton().getExtensionFromMimeType(it) }
                ?: "jpg"
            val backgroundDir = File(context.filesDir, "backgrounds")
            if (!backgroundDir.exists()) {
                backgroundDir.mkdirs()
            }
            val outputFile = File(backgroundDir, "custom_background.$extension")
            val input = resolver.openInputStream(uri) ?: return@withContext null
            input.use { src -> outputFile.outputStream().use { src.copyTo(it) } }

            outputFile.absolutePath
        } catch (e: Exception) {
            Logger.e("BackgroundImagePicker", "Failed to copy image to internal storage", e)
            null
        }
    }
}
