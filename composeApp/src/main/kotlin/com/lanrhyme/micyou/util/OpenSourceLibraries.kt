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

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.lanrhyme.micyou.R

/**
 * 开源库信息
 */
data class OpenSourceLibrary(
    val name: String,
    val license: String
)

/**
 * 开源库列表 - 单一数据源
 */
val OpenSourceLibraries = listOf(
    OpenSourceLibrary("JetBrains Compose Multiplatform", "Apache License 2.0"),
    OpenSourceLibrary("Kotlin Coroutines", "Apache License 2.0"),
    OpenSourceLibrary("Ktor", "Apache License 2.0"),
    OpenSourceLibrary("Material 3 Components", "Apache License 2.0"),
    OpenSourceLibrary("MaterialKolor", "MIT License"),
    OpenSourceLibrary("kotlinx-datetime", "Apache License 2.0"),
    OpenSourceLibrary("kotlinx-serialization", "Apache License 2.0")
)

/**
 * 开源库列表组件 - 可在桌面端和移动端复用
 */
@Composable
fun OpenSourceLibrariesList(
    modifier: Modifier = Modifier
) {
    LazyColumn(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items(OpenSourceLibraries.size) { index ->
            val library = OpenSourceLibraries[index]
            Text(library.name, style = MaterialTheme.typography.titleSmall)
            Text(library.license, style = MaterialTheme.typography.bodySmall)
        }
    }
}
