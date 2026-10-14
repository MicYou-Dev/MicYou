/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * Haze bridge — compat mode: simple semi-transparent background fallbacks.
 * Real haze/blur effects require newer Compose versions not available on minSdk 21.
 */

package com.lanrhyme.micyou.ui.compose.haze

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.foundation.background
import androidx.compose.ui.graphics.Color

class HazeState

data class HazeStyle(
    val backgroundColor: Color = Color.White.copy(alpha = 0.7f),
    val tints: List<HazeTint> = emptyList()
)

data class HazeTint(
    val color: Color = Color.White.copy(alpha = 0.5f)
)

fun Modifier.hazeEffect(state: HazeState, style: HazeStyle): Modifier =
    this.background(style.backgroundColor)

fun Modifier.hazeSource(state: HazeState): Modifier = this

@Composable
fun rememberHazeState(): HazeState = remember { HazeState() }
