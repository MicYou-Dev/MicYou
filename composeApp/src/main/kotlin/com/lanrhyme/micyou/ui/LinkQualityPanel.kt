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

package com.lanrhyme.micyou.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Speed
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.lanrhyme.micyou.R
import com.lanrhyme.micyou.audio.LinkQuality
import com.lanrhyme.micyou.ui.background.HazeSurface
import com.lanrhyme.micyou.ui.compose.haze.HazeState
import java.util.Locale

/**
 * 底部栏入口：只在连上桌面端后显示。桌面端每秒回传一次链路质量，
 * 因此这个按钮同时是「有实时统计」的指示器。
 */
@Composable
fun LinkQualityToggleButton(
    isActive: Boolean,
    rttMs: Int? = null,
    onToggle: () -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.9f else 1f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioHighBouncy),
        label = "linkQualityBtnScale"
    )
    val bgColor by animateColorAsState(
        targetValue = if (isActive) MaterialTheme.colorScheme.primaryContainer
        else MaterialTheme.colorScheme.surfaceContainerHighest,
        animationSpec = tween(200),
        label = "linkQualityBtnBg"
    )
    val contentColor by animateColorAsState(
        targetValue = if (isActive) MaterialTheme.colorScheme.onPrimaryContainer
        else MaterialTheme.colorScheme.onSurfaceVariant,
        animationSpec = tween(200),
        label = "linkQualityBtnContent"
    )

    Surface(
        shape = MaterialTheme.shapes.small,
        color = bgColor,
        modifier = Modifier.scale(scale).clickable(interactionSource, null) { onToggle() }
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Icon(
                Icons.Filled.Speed,
                contentDescription = stringResource(R.string.linkQualityLabel),
                tint = contentColor,
                modifier = Modifier.size(18.dp)
            )
            Text(
                if (rttMs != null) "${stringResource(R.string.linkQualityLabel)} $rttMs ms"
                else stringResource(R.string.linkQualityLabel),
                style = MaterialTheme.typography.labelMedium,
                color = contentColor
            )
        }
    }
}

/**
 * 链路质量面板：RTT / 抖动 / 丢包 / 端到端延迟，全部由桌面端测量后回传，
 * 手机端只负责展示（手机不主动发 ping）。
 */
@Composable
fun LinkQualityPanel(
    quality: LinkQuality?,
    cardOpacity: Float = 1f,
    hazeState: HazeState? = null,
    enableHazeEffect: Boolean = false,
    modifier: Modifier = Modifier
) {
    HazeSurface(
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceBright.copy(alpha = cardOpacity),
        hazeColor = MaterialTheme.colorScheme.surfaceBright.copy(alpha = cardOpacity * 0.7f),
        modifier = modifier.fillMaxWidth(),
        hazeState = hazeState,
        enabled = enableHazeEffect
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Icon(
                    Icons.Filled.Speed,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp)
                )
                Text(
                    stringResource(R.string.linkQualityLabel),
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f)
                )
                if (quality != null) {
                    GradeBadge(quality)
                }
            }

            if (quality == null) {
                Text(
                    stringResource(R.string.linkQualityWaiting),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            } else {
                MetricRow(stringResource(R.string.linkQualityRtt), "${quality.rttMs} ms")
                MetricRow(
                    stringResource(R.string.linkQualityJitter),
                    String.format(Locale.US, "%.1f ms", quality.jitterMs)
                )
                MetricRow(
                    stringResource(R.string.linkQualityLoss),
                    String.format(Locale.US, "%.2f%%", quality.packetLoss * 100f)
                )
                MetricRow(stringResource(R.string.linkQualityLatency), "${quality.latencyMs} ms")
            }

            Text(
                stringResource(R.string.linkQualityHint),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun MetricRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            value,
            style = MaterialTheme.typography.bodySmall,
            fontWeight = FontWeight.SemiBold
        )
    }
}

@Composable
private fun GradeBadge(quality: LinkQuality) {
    val (labelRes, color) = when {
        quality.rttMs <= 60 && quality.packetLoss <= 0.01f ->
            R.string.linkQualityExcellent to MaterialTheme.colorScheme.primary
        quality.rttMs <= 150 && quality.packetLoss <= 0.05f ->
            R.string.linkQualityGood to MaterialTheme.colorScheme.secondary
        quality.rttMs <= 300 && quality.packetLoss <= 0.15f ->
            R.string.linkQualityFair to MaterialTheme.colorScheme.tertiary
        else ->
            R.string.linkQualityPoor to MaterialTheme.colorScheme.error
    }
    Surface(
        shape = MaterialTheme.shapes.extraSmall,
        color = color.copy(alpha = 0.15f)
    ) {
        Text(
            stringResource(labelRes),
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.SemiBold,
            color = color,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
        )
    }
}
