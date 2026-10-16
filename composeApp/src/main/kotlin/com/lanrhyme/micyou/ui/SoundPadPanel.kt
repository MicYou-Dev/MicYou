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

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.RepeatOne
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material.icons.filled.VolumeUp
import androidx.compose.material.icons.rounded.RecordVoiceOver
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lanrhyme.micyou.R
import com.lanrhyme.micyou.soundpad.SoundPadSession
import com.lanrhyme.micyou.soundpad.SoundPadSound
import com.lanrhyme.micyou.soundpad.SoundPadUiState
import com.lanrhyme.micyou.ui.background.HazeSurface
import com.lanrhyme.micyou.ui.compose.haze.HazeState
import com.lanrhyme.micyou.viewmodel.MainViewModel
import java.util.Locale

// ==================== 底部栏切换按钮 ====================

/**
 * Bottom-bar entry for the desktop soundpad. Only composed while the desktop
 * peer answers discovery ([SoundPadUiState.available]) — the button IS the
 * "desktop has a soundpad plugin" indicator.
 */
@Composable
fun SoundPadToggleButton(
    isActive: Boolean,
    onToggle: () -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.9f else 1f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioHighBouncy),
        label = "soundpadBtnScale"
    )
    val bgColor by animateColorAsState(
        targetValue = if (isActive) MaterialTheme.colorScheme.primaryContainer
        else MaterialTheme.colorScheme.surfaceContainerHighest,
        animationSpec = tween(200),
        label = "soundpadBtnBg"
    )
    val contentColor by animateColorAsState(
        targetValue = if (isActive) MaterialTheme.colorScheme.onPrimaryContainer
        else MaterialTheme.colorScheme.onSurfaceVariant,
        animationSpec = tween(200),
        label = "soundpadBtnContent"
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
                Icons.Filled.MusicNote,
                contentDescription = stringResource(R.string.soundpadLabel),
                tint = contentColor,
                modifier = Modifier.size(18.dp)
            )
            Text(
                stringResource(R.string.soundpadLabel),
                style = MaterialTheme.typography.labelMedium,
                color = contentColor
            )
        }
    }
}

// ==================== 核心面板 ====================

/**
 * Remote-control panel for the desktop `opss.soundpad` plugin: sound grid +
 * per-session transport (progress / volume / loop / stop). UI structure
 * follows the classic MicYouSoundpad client panel; the audio itself lives on
 * the desktop (mixed into the mic stream there).
 */
@Composable
fun SoundPadPanel(
    soundPad: SoundPadUiState,
    viewModel: MainViewModel,
    cardOpacity: Float = 1f,
    hazeState: HazeState? = null,
    enableHazeEffect: Boolean = false,
    modifier: Modifier = Modifier
) {
    val selected = soundPad.selectedSession

    HazeSurface(
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.surfaceBright.copy(alpha = cardOpacity),
        hazeColor = MaterialTheme.colorScheme.surfaceBright.copy(alpha = cardOpacity * 0.7f),
        modifier = modifier.fillMaxWidth(),
        hazeState = hazeState,
        enabled = enableHazeEffect
    ) {
        Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp).fillMaxSize()) {

            // === 工具栏：返回 / 标题 / 防撞开关 / 全部停止 ===
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // 面板仅通过再次点击底部栏 SoundPad 按钮收起（无收起钮）
                // 文件夹导航：进入子目录后出现返回键（沿袭旧版工具栏）
                if (soundPad.currentDir.isNotEmpty()) {
                    IconButton(onClick = { viewModel.soundPadNavigateUp() }, modifier = Modifier.size(32.dp)) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.soundpadBack),
                            modifier = Modifier.size(18.dp)
                        )
                    }
                } else {
                    Spacer(Modifier.width(8.dp))
                }
                Icon(
                    if (soundPad.currentDir.isEmpty()) Icons.Filled.MusicNote else Icons.Filled.Folder,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp)
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = if (soundPad.currentDir.isEmpty()) stringResource(R.string.soundpadLabel)
                           else soundPad.currentDir.substringAfterLast('/'),
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                if (soundPad.desktopVersion.isNotBlank()) {
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = "v${soundPad.desktopVersion}",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Spacer(Modifier.weight(1f))
                if (soundPad.sessions.isNotEmpty()) {
                    Text(
                        text = "${soundPad.sessions.size}",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.width(2.dp))
                }
                // 防撞开关（VAD Ducking）：说话时桌面端把音效压低到 20%；
                // 图标右侧带文字状态（防撞启用/防撞禁用）
                run {
                    val vadColor = if (soundPad.vadGuard) MaterialTheme.colorScheme.primary
                    else MaterialTheme.colorScheme.onSurfaceVariant
                    Row(
                        modifier = Modifier
                            .clip(MaterialTheme.shapes.small)
                            .clickable { viewModel.soundPadSetVadGuard(!soundPad.vadGuard) }
                            .padding(horizontal = 6.dp, vertical = 5.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(3.dp)
                    ) {
                        Icon(
                            Icons.Rounded.RecordVoiceOver,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp),
                            tint = vadColor
                        )
                        Text(
                            text = stringResource(
                                if (soundPad.vadGuard) R.string.soundpadVadOn else R.string.soundpadVadOff
                            ),
                            style = MaterialTheme.typography.labelSmall,
                            color = vadColor,
                            maxLines = 1
                        )
                    }
                }
                IconButton(
                    onClick = { viewModel.soundPadStop(null) },
                    modifier = Modifier.size(32.dp),
                    enabled = soundPad.sessions.isNotEmpty()
                ) {
                    Icon(
                        Icons.Filled.Stop,
                        contentDescription = stringResource(R.string.soundpadStopAll),
                        modifier = Modifier.size(17.dp),
                        tint = if (soundPad.sessions.isNotEmpty()) MaterialTheme.colorScheme.error
                        else MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            // === 控制区 / 状态行（带动画切换，沿袭旧版面板）===
            AnimatedContent(
                targetState = selected != null,
                transitionSpec = {
                    if (targetState) {
                        slideInVertically(initialOffsetY = { it }) + fadeIn() togetherWith
                                slideOutVertically(targetOffsetY = { -it }) + fadeOut()
                    } else {
                        slideInVertically(initialOffsetY = { -it }) + fadeIn() togetherWith
                                slideOutVertically(targetOffsetY = { it }) + fadeOut()
                    }
                },
                label = "soundpadControlArea",
                modifier = Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 10.dp)
            ) { hasSelection ->
                if (hasSelection && selected != null) {
                    SoundPadTransportRow(
                        session = selected,
                        viewModel = viewModel
                    )
                } else {
                    SoundPadStatusRow(soundPad)
                }
            }

            // === 音效网格 ===
            if (soundPad.sounds.isEmpty()) {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(
                            Icons.Filled.Folder,
                            null,
                            Modifier.size(44.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Spacer(Modifier.height(10.dp))
                        Text(
                            stringResource(R.string.soundpadEmptyLibrary),
                            style = MaterialTheme.typography.titleSmall
                        )
                        Spacer(Modifier.height(4.dp))
                        Text(
                            stringResource(R.string.soundpadEmptyLibraryHint),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center
                        )
                        Spacer(Modifier.height(14.dp))
                        Button(onClick = { viewModel.soundPadRefresh() }) {
                            Text(stringResource(R.string.soundpadRefresh))
                        }
                    }
                }
            } else {
                // 当前目录的直接子文件夹 + 本级音效（文件夹在前，均按名称排序）
                val prefix = if (soundPad.currentDir.isEmpty()) "" else soundPad.currentDir + "/"
                val subfolders = remember(soundPad.sounds, soundPad.currentDir) {
                    soundPad.sounds
                        .filter { it.dir.startsWith(prefix) && it.dir.length > prefix.length }
                        .map { prefix + it.dir.removePrefix(prefix).substringBefore('/') }
                        .distinct()
                        .sortedBy { it.substringAfterLast('/').lowercase() }
                }
                val filesHere = remember(soundPad.sounds, soundPad.currentDir) {
                    soundPad.sounds
                        .filter { it.dir == soundPad.currentDir }
                        .sortedBy { it.displayName.lowercase() }
                }
                LazyVerticalGrid(
                    // 145dp：360dp 手机（内容宽 ~304dp）稳定 2 列；平板（~744dp）保持
                    // 4 列不变（与 160dp 时相同）；412dp 手机 → 2 列宽卡
                    columns = GridCells.Adaptive(minSize = 145.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                    contentPadding = PaddingValues(bottom = 8.dp)
                ) {
                    items(subfolders, key = { "dir:$it" }) { folder ->
                        val count = soundPad.sounds.count { it.dir == folder || it.dir.startsWith("$folder/") }
                        SoundPadFolderCard(
                            name = folder.substringAfterLast('/'),
                            count = count,
                            onClick = { viewModel.soundPadEnterFolder(folder) }
                        )
                    }
                    items(filesHere, key = { it.id }) { sound ->
                        val sessionsOfSound = soundPad.sessions.filter { it.soundId == sound.id }
                        val isPlaying = sessionsOfSound.isNotEmpty()
                        val isSelected = selected != null && sessionsOfSound.any { it.sessionId == selected.sessionId }
                        SoundPadItemCard(
                            sound = sound,
                            isPlaying = isPlaying,
                            isSelected = isSelected,
                            isLoading = sessionsOfSound.any { it.isLoading },
                            onClick = {
                                if (isPlaying) {
                                    // Bring this sound's voice into the transport controls.
                                    viewModel.soundPadSelect(sessionsOfSound.first().sessionId)
                                } else {
                                    viewModel.soundPadPlay(sound.id)
                                }
                            },
                            onStop = {
                                sessionsOfSound.forEach { viewModel.soundPadStop(it.sessionId) }
                            }
                        )
                    }
                }
            }
        }
    }
}

// ==================== 传输控制行 ====================

@Composable
private fun SoundPadTransportRow(
    session: SoundPadSession,
    viewModel: MainViewModel
) {
    // 本地拖拽状态：解决 Slider 跟手与远端进度回推的冲突（旧版同款方案）
    var localDragProgress by remember(session.sessionId) { mutableStateOf<Float?>(null) }
    var localDragVolume by remember(session.sessionId) { mutableStateOf<Float?>(null) }

    val duration = session.durationMs.coerceAtLeast(1L)
    val displayProgress = (localDragProgress ?: (session.interpolatedPosition().toFloat() / duration))
        .coerceIn(0f, 1f)
    val displayVolume = localDragVolume ?: session.volume

    Column(modifier = Modifier.fillMaxWidth().defaultMinSize(minHeight = 48.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = session.label.ifBlank { session.soundId },
                style = MaterialTheme.typography.labelLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )
            if (session.isLoading) {
                CircularProgressIndicator(Modifier.size(13.dp), strokeWidth = 2.dp)
                Spacer(Modifier.width(6.dp))
                Text(
                    stringResource(R.string.soundpadLoading),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            IconButton(
                onClick = { viewModel.soundPadStop(session.sessionId) },
                modifier = Modifier.size(30.dp)
            ) {
                Icon(
                    Icons.Filled.Stop,
                    contentDescription = stringResource(R.string.soundpadStop),
                    modifier = Modifier.size(18.dp),
                    tint = MaterialTheme.colorScheme.error
                )
            }
        }
        // 旧版单行布局：宽度充足时 进度条(weight 1f) 与 音量条(weight 0.6f)
        // 同排、循环钮收尾；窄屏拆两行——进度条独占一行，音量条+循环钮一行。
        BoxWithConstraints(modifier = Modifier.fillMaxWidth()) {
            val progressSlider: @Composable RowScope.() -> Unit = {
                Slider(
                    value = displayProgress,
                    onValueChange = { localDragProgress = it },
                    onValueChangeFinished = {
                        localDragProgress?.let {
                            viewModel.soundPadSeek(session.sessionId, (it * duration).toLong())
                        }
                        localDragProgress = null
                    },
                    modifier = Modifier.weight(1f).height(30.dp).padding(horizontal = 4.dp),
                    colors = SliderDefaults.colors(
                        thumbColor = MaterialTheme.colorScheme.primary,
                        activeTrackColor = MaterialTheme.colorScheme.primary,
                        inactiveTrackColor = MaterialTheme.colorScheme.surfaceContainerHigh
                    )
                )
            }
            val loopButton: @Composable () -> Unit = {
                IconButton(
                    onClick = { viewModel.soundPadSetLoop(session.sessionId, !session.loop) },
                    modifier = Modifier.size(32.dp)
                ) {
                    Icon(
                        if (session.loop) Icons.Filled.RepeatOne else Icons.Filled.Repeat,
                        contentDescription = stringResource(R.string.soundpadLoop),
                        tint = if (session.loop) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
            val volumeSlider: @Composable RowScope.(Float) -> Unit = { w ->
                Slider(
                    value = displayVolume.coerceIn(0f, 1f),
                    onValueChange = { localDragVolume = it },
                    onValueChangeFinished = {
                        localDragVolume?.let { viewModel.soundPadSetVolume(session.sessionId, it) }
                        localDragVolume = null
                    },
                    modifier = Modifier.weight(w).height(30.dp).padding(horizontal = 4.dp)
                )
            }

            // 400dp：手机竖屏（面板内容宽 ≈304-356dp）走两行拆分；
            // 平板/横屏/折叠屏内屏走旧版单行。
            if (maxWidth >= 400.dp) {
                // ── 宽：单行（旧版参数：44dp 时间 + 1f 进度 + 44dp 时长
                //    + 36dp 音量图标 + 0.6f 音量 + 32dp 循环）──
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = formatSoundPadTime((displayProgress * duration).toLong()),
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.width(44.dp)
                    )
                    progressSlider()
                    Text(
                        text = formatSoundPadTime(session.durationMs),
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.width(44.dp)
                    )
                    Icon(
                        Icons.Filled.VolumeUp,
                        contentDescription = stringResource(R.string.soundpadVolume),
                        modifier = Modifier.size(36.dp).padding(start = 8.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    volumeSlider(0.6f)
                    loopButton()
                }
            } else {
                // ── 窄：两行，进度条独占一行；循环钮在音量行 ──
                Column {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = formatSoundPadTime((displayProgress * duration).toLong()),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.width(44.dp)
                        )
                        progressSlider()
                        Text(
                            text = formatSoundPadTime(session.durationMs),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.width(44.dp)
                        )
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            Icons.Filled.VolumeUp,
                            contentDescription = stringResource(R.string.soundpadVolume),
                            modifier = Modifier.size(36.dp).padding(start = 8.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        volumeSlider(1f)
                        loopButton()
                    }
                }
            }
        }
    }
}

// ==================== 状态行（无选中会话）====================

@Composable
private fun SoundPadStatusRow(soundPad: SoundPadUiState) {
    val playingLabels = soundPad.sessions
        .filter { it.isPlaying }
        .map { it.label.ifBlank { it.soundId } }
    Row(
        modifier = Modifier.fillMaxWidth().defaultMinSize(minHeight = 48.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            Icons.Filled.MusicNote,
            contentDescription = null,
            modifier = Modifier.size(30.dp).padding(end = 10.dp),
            tint = MaterialTheme.colorScheme.primary
        )
        Text(
            text = when {
                playingLabels.isNotEmpty() ->
                    stringResource(R.string.soundpadNowPlaying, playingLabels.joinToString(" | "))
                soundPad.sessions.isNotEmpty() ->
                    stringResource(R.string.soundpadLoading)
                else -> stringResource(R.string.soundpadIdleHint)
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f)
        )
    }
}

// ==================== 文件夹卡片 ====================

@Composable
private fun SoundPadFolderCard(
    name: String,
    count: Int,
    onClick: () -> Unit
) {
    Card(
        onClick = onClick,
        modifier = Modifier
            .fillMaxWidth()
            .defaultMinSize(minWidth = 140.dp, minHeight = 100.dp)
            .height(100.dp),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.secondaryContainer,
            contentColor = MaterialTheme.colorScheme.onSecondaryContainer
        ),
        shape = MaterialTheme.shapes.medium
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp).fillMaxSize(),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Icon(
                imageVector = Icons.Filled.Folder,
                contentDescription = null,
                modifier = Modifier.size(36.dp)
            )
            Spacer(Modifier.height(2.dp))
            Text(
                text = name,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                textAlign = TextAlign.Center
            )
            Text(
                text = stringResource(R.string.soundpadFolderCount, count),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.7f)
            )
        }
    }
}

// ==================== 音效卡片 ====================

@Composable
private fun SoundPadItemCard(
    sound: SoundPadSound,
    isSelected: Boolean,
    isPlaying: Boolean,
    isLoading: Boolean,
    onClick: () -> Unit,
    onStop: () -> Unit
) {
    // 播放态 → Material 3 主题色映射（沿袭旧版四态卡片）
    val containerColor by animateColorAsState(
        targetValue = when {
            isPlaying && isSelected -> MaterialTheme.colorScheme.tertiaryContainer
            isPlaying -> MaterialTheme.colorScheme.primaryContainer
            else -> MaterialTheme.colorScheme.surfaceContainerHigh
        },
        animationSpec = tween(300),
        label = "soundpadCardBg"
    )
    val contentColor by animateColorAsState(
        targetValue = when {
            isPlaying && isSelected -> MaterialTheme.colorScheme.onTertiaryContainer
            isPlaying -> MaterialTheme.colorScheme.onPrimaryContainer
            else -> MaterialTheme.colorScheme.onSurfaceVariant
        },
        animationSpec = tween(300),
        label = "soundpadCardContent"
    )

    Card(
        onClick = onClick,
        modifier = Modifier
            .fillMaxWidth()
            .defaultMinSize(minWidth = 140.dp, minHeight = 100.dp)
            .height(100.dp),
        colors = CardDefaults.cardColors(
            containerColor = containerColor,
            contentColor = contentColor
        ),
        shape = MaterialTheme.shapes.medium
    ) {
        Box(modifier = Modifier.fillMaxSize()) {
            Column(
                modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp).fillMaxSize(),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Icon(
                    imageVector = Icons.Filled.MusicNote,
                    contentDescription = null,
                    modifier = Modifier.size(36.dp)
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = sound.displayName,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    textAlign = TextAlign.Center
                )
                Text(
                    text = formatSoundPadTime(sound.durationMs),
                    style = MaterialTheme.typography.labelSmall,
                    color = contentColor.copy(alpha = 0.7f)
                )
                // 播放状态指示器（呼吸灯）
                if (isPlaying) {
                    Spacer(Modifier.height(2.dp))
                    val infiniteTransition = rememberInfiniteTransition(label = "soundpadPulse")
                    val alpha by infiniteTransition.animateFloat(
                        initialValue = 0.3f,
                        targetValue = 1f,
                        animationSpec = infiniteRepeatable(tween(600), RepeatMode.Reverse),
                        label = "soundpadPulseAlpha"
                    )
                    Box(
                        modifier = Modifier
                            .size(4.dp)
                            .clip(CircleShape)
                            .background(contentColor.copy(alpha = alpha))
                    )
                }
            }
            // 播放中的卡片右上角：停止本音效的全部声部
            if (isPlaying) {
                IconButton(
                    onClick = onStop,
                    modifier = Modifier.align(Alignment.TopEnd).size(26.dp)
                ) {
                    Icon(
                        if (isLoading) Icons.Filled.Refresh else Icons.Filled.Stop,
                        contentDescription = stringResource(R.string.soundpadStop),
                        modifier = Modifier.size(14.dp),
                        tint = contentColor
                    )
                }
            }
        }
    }
}

// ==================== 工具函数 ====================

/** Milliseconds → m:ss (same formatting as the classic soundpad client). */
private fun formatSoundPadTime(ms: Long): String {
    val safe = ms.coerceAtLeast(0L)
    val totalSeconds = safe / 1000
    val minutes = totalSeconds / 60
    val seconds = totalSeconds % 60
    return String.format(Locale.US, "%d:%02d", minutes, seconds)
}
