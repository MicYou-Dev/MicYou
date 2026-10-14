/*
 * MicYou — Turns your Android device into a high-quality PC microphone.
 * Copyright (C) 2026 LanRhyme <https://github.com/MicYou-Dev/MicYou>
 *
 * Haze bridge — normal mode: delegates to dev.chrisbanes.haze library.
 */

package com.lanrhyme.micyou.ui.compose.haze

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import dev.chrisbanes.haze.HazeInput
import dev.chrisbanes.haze.HazePerformanceMode
import dev.chrisbanes.haze.blur.HazeBlurStyle
import dev.chrisbanes.haze.blur.HazeColorEffect
import dev.chrisbanes.haze.blur.hazeBlur
import dev.chrisbanes.haze.HazeState as RealHazeState
import dev.chrisbanes.haze.hazeSource as realHazeSource
import dev.chrisbanes.haze.rememberHazeState as realRememberHazeState

typealias HazeState = RealHazeState

// Haze 2 replaced HazeStyle and HazeTint with replayable HazeBlurStyle programs;
// these keep the shape the compat bridge shares.
data class HazeStyle(
    val backgroundColor: Color = Color.Unspecified,
    val tints: List<HazeTint> = emptyList()
)

data class HazeTint(
    val color: Color
)

// Quality keeps the full-resolution input Haze 1 used by default.
fun Modifier.hazeEffect(state: HazeState, style: HazeStyle): Modifier =
    this.then(
        Modifier.hazeBlur(
            input = HazeInput.Sources(state),
            style = HazeBlurStyle {
                if (style.backgroundColor.isSpecified) backgroundColor(style.backgroundColor)
                colorEffects(style.tints.map { HazeColorEffect.tint(it.color) })
            },
            performanceMode = HazePerformanceMode.Quality
        )
    )

fun Modifier.hazeSource(state: HazeState): Modifier =
    this.then(Modifier.realHazeSource(state = state))

@Composable
fun rememberHazeState(): HazeState = realRememberHazeState()
