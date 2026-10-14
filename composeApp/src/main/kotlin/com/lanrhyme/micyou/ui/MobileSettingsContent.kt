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
import com.lanrhyme.micyou.R

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.TextSnippet
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.rounded.Description
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.People
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import com.lanrhyme.micyou.ui.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lanrhyme.micyou.theme.ExpressiveListItem
import com.lanrhyme.micyou.theme.ExpressiveSettingsBoxItem
import com.lanrhyme.micyou.theme.ExpressiveSettingsDropdownItem
import com.lanrhyme.micyou.theme.ExpressiveSettingsSwitchItem
import com.lanrhyme.micyou.theme.PaletteStyle
import com.lanrhyme.micyou.ui.compose.haze.HazeState
import com.lanrhyme.micyou.ui.background.rememberHazeState
import com.lanrhyme.micyou.ui.compose.haze.HazeStyle
import com.lanrhyme.micyou.ui.compose.haze.HazeTint
import com.lanrhyme.micyou.ui.compose.haze.hazeEffect
import com.lanrhyme.micyou.ui.compose.haze.hazeSource
import androidx.compose.ui.res.stringResource
import com.lanrhyme.micyou.audio.AudioFormat
import com.lanrhyme.micyou.audio.ChannelCount
import com.lanrhyme.micyou.audio.availableAudioFormats
import com.lanrhyme.micyou.audio.getAudioSourceOptions
import com.lanrhyme.micyou.audio.SampleRate
import com.lanrhyme.micyou.theme.isDynamicColorSupported
import com.lanrhyme.micyou.theme.PresetColors
import com.lanrhyme.micyou.theme.ThemeMode
import com.lanrhyme.micyou.ui.dialog.ContributorsDialog
import com.lanrhyme.micyou.ui.dialog.SponsorsDialog
import com.lanrhyme.micyou.util.AppLanguage
import com.lanrhyme.micyou.util.getAppVersion
import com.lanrhyme.micyou.util.OpenSourceLibrariesList
import com.lanrhyme.micyou.viewmodel.AppUiState
import com.lanrhyme.micyou.viewmodel.MainViewModel
import com.lanrhyme.micyou.viewmodel.TransportProtocol
import com.lanrhyme.micyou.viewmodel.VisualizerStyle

/**
 * M3 Expressive 风格的手机端设置页面
 * 使用单层列表卡片容器，第一项顶部大圆角，最后一项底部大圆角，中间项之间有空隙
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MobileSettingsPage(
    viewModel: MainViewModel,
    onClose: () -> Unit,
    hazeState: HazeState?
) {
    val state by viewModel.uiState.collectAsState()
        val backgroundColor = MaterialTheme.colorScheme.surfaceContainer
    val topBarBackgroundColor = backgroundColor.copy(alpha = 0.8f)
    val topBarHazeState = rememberHazeState()
    val cardOpacity = state.backgroundSettings.cardOpacity
    val enableHaze = state.backgroundSettings.enableHazeEffect && state.backgroundSettings.hasCustomBackground
    val baseContainerColor = MaterialTheme.colorScheme.surfaceBright
    val containerColor = baseContainerColor.copy(alpha = cardOpacity)

    // About section dialog states
    var showLicenseDialog by remember { mutableStateOf(false) }
    var showContributorsDialog by remember { mutableStateOf(false) }
    var showSponsorsDialog by remember { mutableStateOf(false) }

    // About section dialogs
    if (showContributorsDialog) {
        ContributorsDialog(onDismiss = { showContributorsDialog = false })
    }
    if (showSponsorsDialog) {
        SponsorsDialog(onDismiss = { showSponsorsDialog = false })
    }
    if (showLicenseDialog) {
        AlertDialog(
            onDismissRequest = { showLicenseDialog = false },
            title = { Text(stringResource(R.string.licensesTitle)) },
            text = { OpenSourceLibrariesList() },
            confirmButton = {
                TextButton(onClick = { showLicenseDialog = false }) {
                    Text(stringResource(R.string.close))
                }
            }
        )
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    if (state.backgroundSettings.hasCustomBackground) Color.Transparent
                    else backgroundColor
                )
                .hazeSource(state = topBarHazeState)
        ) {
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 16.dp),
            contentPadding = PaddingValues(top = 64.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            item { Spacer(modifier = Modifier.height(32.dp)) }

            // General Section
            item { SectionHeader(stringResource(R.string.generalSection)) }
            generalSettingsItems(state, viewModel, containerColor, enableHaze, hazeState)

            // Appearance Section
            item { SectionHeader(stringResource(R.string.appearanceSection)) }
            appearanceSettingsItems(state, viewModel, containerColor, enableHaze, hazeState)

            // Audio Section
            item { SectionHeader(stringResource(R.string.audioSection)) }
            audioSettingsItems(state, viewModel, containerColor, enableHaze, hazeState)

            // About Section
            item { SectionHeader(stringResource(R.string.aboutSection)) }
            aboutSettingsItems(
                state, viewModel, containerColor, enableHaze, hazeState,
                onShowLicenseDialog = { showLicenseDialog = true },
                onShowContributorsDialog = { showContributorsDialog = true },
                onShowSponsorsDialog = { showSponsorsDialog = true }
            )

            item { Spacer(Modifier.height(16.dp)) }
        }
        }

        // 顶部导航栏 - 0.8 不透明度 + haze 毛玻璃效果（始终启用，与内容区域是兄弟关系）
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .hazeEffect(
                    state = topBarHazeState,
                    style = HazeStyle(
                        backgroundColor = topBarBackgroundColor,
                        tints = listOf(HazeTint(color = topBarBackgroundColor))
                    )
                )
        ) {
            TopAppBar(
                title = {
                    Text(
                        stringResource(R.string.settingsTitle),
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onClose) {
                        Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = stringResource(R.string.close))
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                )
            )
        }

        // Mirror CDK Dialog
        if (state.showMirrorCdkDialog) {
            MirrorCdkDialog(
                cdk = state.mirrorCdk,
                onDismiss = { viewModel.dismissMirrorCdkDialog() },
                onConfirm = { cdk -> viewModel.confirmMirrorCdk(cdk) })
        }
    }
}

@Composable
private fun SectionHeader(title: String) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(start = 4.dp, bottom = 12.dp)
    ) {
        Box(
            modifier = Modifier
                .size(height = 18.dp, width = 5.dp)
                .clip(androidx.compose.foundation.shape.RoundedCornerShape(3.dp))
                .background(MaterialTheme.colorScheme.primary)
        )
        Spacer(Modifier.width(8.dp))
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.primary,
            fontWeight = FontWeight.Bold
        )
    }
}

private fun LazyListScope.generalSettingsItems(
    state: AppUiState,
    viewModel: MainViewModel,
    containerColor: Color,
    enableHaze: Boolean,
    hazeState: HazeState? ) {
    item {
        val items = mutableListOf<@Composable (isFirst: Boolean, isLast: Boolean) -> Unit>()

        items.add { isFirst, isLast ->
            ExpressiveSettingsDropdownItem(
                headline = stringResource(R.string.languageLabel),
                selected = state.language,
                options = AppLanguage.entries.toList(),
                labelProvider = { it.label },
                onSelect = { viewModel.setLanguage(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        items.add { isFirst, isLast ->
            ExpressiveSettingsDropdownItem(
                headline = stringResource(R.string.transportProtocolLabel),
                selected = state.transportProtocol,
                options = TransportProtocol.entries.toList(),
                labelProvider = { it.label },
                onSelect = { viewModel.setTransportProtocol(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        items.add { isFirst, isLast ->
            ExpressiveSettingsSwitchItem(
                headline = stringResource(R.string.keepScreenOnLabel),
                supporting = stringResource(R.string.keepScreenOnDesc),
                checked = state.keepScreenOn,
                onCheckedChange = { viewModel.setKeepScreenOn(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        items.add { isFirst, isLast ->
            ExpressiveSettingsSwitchItem(
                headline = stringResource(R.string.autoCheckUpdateLabel),
                supporting = stringResource(R.string.autoCheckUpdateDesc),
                checked = state.autoCheckUpdate,
                onCheckedChange = { viewModel.setAutoCheckUpdate(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        items.add { isFirst, isLast ->
            ExpressiveSettingsSwitchItem(
                headline = stringResource(R.string.mirrorDownloadLabel),
                supporting = stringResource(R.string.mirrorDownloadDesc),
                checked = state.useMirrorDownload,
                onCheckedChange = { viewModel.setUseMirrorDownload(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            items.forEachIndexed { index, item ->
                item(index == 0, index == items.size - 1)
            }
        }
    }
}

private fun LazyListScope.appearanceSettingsItems(
    state: AppUiState,
    viewModel: MainViewModel,
    containerColor: Color,
    enableHaze: Boolean,
    hazeState: HazeState? ) {
    item {
        val seedColors = listOf(
            0xFF1565C0L, // Ocean Blue
            0xFF6750A4L, // M3 Purple
            0xFFE91E63L, // Rose Pink
            0xFF2E7D32L, // Forest Green
            0xFFFF5722L, // Sunset Orange
            0xFF00695CL, // Deep Teal
            0xFF283593L, // Midnight Indigo
            0xFF7B1FA2L  // Lavender Violet
        )

        val items = mutableListOf<@Composable (isFirst: Boolean, isLast: Boolean) -> Unit>()

        // 主题选择
        items.add { isFirst, isLast ->
            ExpressiveSettingsBoxItem(
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Text(stringResource(R.string.themeLabel), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.height(8.dp))
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(ThemeMode.entries) { mode ->
                        FilterChip(
                            selected = state.themeMode == mode,
                            onClick = { viewModel.setThemeMode(mode) },
                            label = {
                                Text(when(mode) {
                                    ThemeMode.System -> stringResource(R.string.themeSystem)
                                    ThemeMode.Light -> stringResource(R.string.themeLight)
                                    ThemeMode.Dark -> stringResource(R.string.themeDark)
                                })
                            },
                            leadingIcon = {
                                if (state.themeMode == mode) Icon(Icons.Filled.Check, null, modifier = Modifier.size(16.dp)) else null
                            }
                        )
                    }
                }
            }
        }

        // 动态取色
        if (true || isDynamicColorSupported()) {
            items.add { isFirst, isLast ->
                ExpressiveSettingsSwitchItem(
                    headline = stringResource(R.string.useDynamicColorLabel),
                    supporting = stringResource(R.string.useDynamicColorDesc),
                    checked = state.useDynamicColor,
                    onCheckedChange = { viewModel.setUseDynamicColor(it) },
                    isFirst = isFirst,
                    isLast = isLast,
                    containerColor = containerColor,
                    hazeState = hazeState,
                    enableHaze = enableHaze
                )
            }
        }

        // OLED 纯黑
        items.add { isFirst, isLast ->
            ExpressiveSettingsSwitchItem(
                headline = stringResource(R.string.oledPureBlackLabel),
                supporting = stringResource(R.string.oledPureBlackDesc),
                checked = state.oledPureBlack,
                onCheckedChange = { viewModel.setOledPureBlack(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        // 主题颜色选择
        items.add { isFirst, isLast ->
            ExpressiveSettingsBoxItem(
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze,
                overlay = {
                    if (state.useDynamicColor) {
                        Box(
                            modifier = Modifier
                                .matchParentSize()
                                .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.85f))
                                .clickable(enabled = false) { },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = stringResource(R.string.dynamicColorEnabledHint),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurface,
                                textAlign = TextAlign.Center,
                                modifier = Modifier.padding(horizontal = 16.dp)
                            )
                        }
                    }
                }
            ) {
                Text(stringResource(R.string.themeColorLabel), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.height(8.dp))
                val isSeedColorEnabled = !state.useDynamicColor
                val displayColor = if (state.useDynamicColor) {
                    MaterialTheme.colorScheme.primary.toArgb().toLong() and 0xFFFFFFFF
                } else {
                    state.seedColor
                }
                ColorSelectorWithPicker(
                    selectedColor = displayColor,
                    presetColors = seedColors,
                    onColorSelected = { viewModel.setSeedColor(it) },
                    enabled = isSeedColorEnabled,
                    disabledHint = stringResource(R.string.dynamicColorEnabledHint),
                    modifier = Modifier.fillMaxWidth()
                )
            }
        }

        // Palette Style
        items.add { isFirst, isLast ->
            ExpressiveSettingsBoxItem(
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Text(stringResource(R.string.paletteStyleLabel), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Text(stringResource(R.string.paletteStyleDesc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(8.dp))
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(PaletteStyle.entries) { style ->
                        FilterChip(
                            selected = state.paletteStyle == style,
                            onClick = { viewModel.setPaletteStyle(style) },
                            label = { Text(style.name) },
                            leadingIcon = {
                                if (state.paletteStyle == style) Icon(Icons.Filled.Check, null, modifier = Modifier.size(16.dp)) else null
                            }
                        )
                    }
                }
            }
        }

        // Expressive Shapes
        items.add { isFirst, isLast ->
            ExpressiveSettingsSwitchItem(
                headline = stringResource(R.string.useExpressiveShapesLabel),
                supporting = stringResource(R.string.useExpressiveShapesDesc),
                checked = state.useExpressiveShapes,
                onCheckedChange = { viewModel.setUseExpressiveShapes(it) },
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        // 可视化样式
        items.add { isFirst, isLast ->
            ExpressiveSettingsBoxItem(
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Text(stringResource(R.string.visualizerStyleLabel), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.height(8.dp))
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(VisualizerStyle.entries) { style ->
                        FilterChip(
                            selected = state.visualizerStyle == style,
                            onClick = { viewModel.setVisualizerStyle(style) },
                            label = {
                                Text(when(style) {
                                    VisualizerStyle.VolumeRing -> stringResource(R.string.visualizerStyleVolumeRing)
                                    VisualizerStyle.Ripple -> stringResource(R.string.visualizerStyleRipple)
                                    VisualizerStyle.Bars -> stringResource(R.string.visualizerStyleBars)
                                    VisualizerStyle.Wave -> stringResource(R.string.visualizerStyleWave)
                                    VisualizerStyle.Glow -> stringResource(R.string.visualizerStyleGlow)
                                    VisualizerStyle.Particles -> stringResource(R.string.visualizerStyleParticles)
                                })
                            },
                            leadingIcon = {
                                if (state.visualizerStyle == style) Icon(Icons.Filled.Check, null, modifier = Modifier.size(16.dp)) else null
                            }
                        )
                    }
                }
            }
        }

        // 背景设置
        items.add { isFirst, isLast ->
            ExpressiveSettingsBoxItem(
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Text(stringResource(R.string.backgroundSettingsLabel), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Spacer(Modifier.height(8.dp))

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    Button(
                        onClick = { viewModel.pickBackgroundImage() },
                        modifier = Modifier.weight(1f)
                    ) {
                        Text(stringResource(R.string.selectBackgroundImage))
                    }
                    if (state.backgroundSettings.hasCustomBackground) {
                        OutlinedButton(
                            onClick = { viewModel.clearBackgroundImage() },
                            modifier = Modifier.weight(1f)
                        ) {
                            Text(stringResource(R.string.clearBackgroundImage))
                        }
                    }
                }

                if (state.backgroundSettings.hasCustomBackground) {
                    Spacer(Modifier.height(8.dp))
                    Text("${stringResource(R.string.backgroundBrightnessLabel)}: ${(state.backgroundSettings.brightness * 100).toInt()}%", style = MaterialTheme.typography.bodySmall)
                    Slider(
                        value = state.backgroundSettings.brightness,
                        onValueChange = { viewModel.setBackgroundBrightness(it) },
                        valueRange = 0f..1f
                    )

                    Text("${stringResource(R.string.backgroundBlurLabel)}: ${state.backgroundSettings.blurRadius.toInt()}px", style = MaterialTheme.typography.bodySmall)
                    Slider(
                        value = state.backgroundSettings.blurRadius,
                        onValueChange = { viewModel.setBackgroundBlur(it) },
                        valueRange = 0f..50f
                    )

                    Text("${stringResource(R.string.cardOpacityLabel)}: ${(state.backgroundSettings.cardOpacity * 100).toInt()}%", style = MaterialTheme.typography.bodySmall)
                    Slider(
                        value = state.backgroundSettings.cardOpacity,
                        onValueChange = { viewModel.setCardOpacity(it) },
                        valueRange = 0f..1f
                    )

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text(stringResource(R.string.enableHazeEffectLabel), style = MaterialTheme.typography.bodySmall)
                            Text(stringResource(R.string.enableHazeEffectDesc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Switch(
                            checked = state.backgroundSettings.enableHazeEffect,
                            onCheckedChange = { viewModel.setEnableHazeEffect(it) }
                        )
                    }
                }
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            items.forEachIndexed { index, item ->
                item(index == 0, index == items.size - 1)
            }
        }
    }
}

private fun LazyListScope.audioSettingsItems(
    state: AppUiState,
    viewModel: MainViewModel,
    containerColor: Color,
    enableHaze: Boolean,
    hazeState: HazeState?
) {
    item {
        val items = mutableListOf<@Composable (isFirst: Boolean, isLast: Boolean) -> Unit>()

        // 自动配置
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = { viewModel.setAutoConfig(!state.isAutoConfig) },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Column(modifier = Modifier.weight(1f).padding(end = 16.dp)) {
                        Text(
                            stringResource(R.string.autoConfigLabel),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Medium,
                            color = MaterialTheme.colorScheme.onSurface
                        )
                        Spacer(Modifier.height(4.dp))
                        Text(
                            stringResource(R.string.autoConfigDesc),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Switch(
                        checked = state.isAutoConfig,
                        onCheckedChange = null // Handled by row click
                    )
                }
            }
        }
        val manualSettingsEnabled = !state.isAutoConfig

        // 采样率
        items.add { isFirst, isLast ->
            ExpressiveAudioDropdownItem(
                headline = stringResource(R.string.sampleRateLabel),
                selected = "${state.sampleRate.value} Hz",
                options = SampleRate.entries.map { "${it.value} Hz" },
                onSelect = { index -> viewModel.setSampleRate(SampleRate.entries[index]) },
                enabled = manualSettingsEnabled,
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        // 声道数
        items.add { isFirst, isLast ->
            ExpressiveAudioDropdownItem(
                headline = stringResource(R.string.channelCountLabel),
                selected = state.channelCount.label,
                options = ChannelCount.entries.map { it.label },
                onSelect = { index -> viewModel.setChannelCount(ChannelCount.entries[index]) },
                enabled = manualSettingsEnabled,
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        // 音频格式
        items.add { isFirst, isLast ->
            val formats = availableAudioFormats()
            ExpressiveAudioDropdownItem(
                headline = stringResource(R.string.audioFormatLabel),
                selected = state.audioFormat.label,
                options = formats.map { it.label },
                onSelect = { index -> viewModel.setAudioFormat(formats[index]) },
                enabled = manualSettingsEnabled,
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        // 音频源
        items.add { isFirst, isLast ->
            ExpressiveAudioSourceItem(
                viewModel = viewModel,
                isFirst = isFirst,
                isLast = isLast,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            )
        }

        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            items.forEachIndexed { index, item ->
                item(index == 0, index == items.size - 1)
            }
        }
    }
}

/**
 * 音频下拉选择项
 */
@Composable
private fun ExpressiveAudioDropdownItem(
    headline: String,
    selected: String,
    options: List<String>,
    onSelect: (Int) -> Unit,
    enabled: Boolean = true,
    isFirst: Boolean = false,
    isLast: Boolean = false,
    containerColor: Color = MaterialTheme.colorScheme.surfaceContainerLow,
    hazeState: HazeState? = null,
    enableHaze: Boolean = false
) {
    var expanded by remember { mutableStateOf(false) }

    ExpressiveListItem(
        isFirst = isFirst,
        isLast = isLast,
        onClick = if (enabled) { { expanded = true } } else null,
        containerColor = containerColor,
        hazeState = hazeState,
        enableHaze = enableHaze
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 18.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                text = headline,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f),
                modifier = Modifier.weight(1f).padding(end = 16.dp)
            )
            Box {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Text(
                        text = selected,
                        style = MaterialTheme.typography.bodyLarge,
                        color = if (enabled) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                    )
                    Icon(
                        imageVector = Icons.Default.ArrowDropDown,
                        contentDescription = null,
                        tint = if (enabled) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
                    )
                }
                DropdownMenu(
                    expanded = expanded,
                    onDismissRequest = { expanded = false },
                    shape = MaterialTheme.shapes.extraLarge
                ) {
                    options.forEachIndexed { index, option ->
                        DropdownMenuItem(
                            text = { Text(option) },
                            onClick = { onSelect(index); expanded = false },
                            trailingIcon = {
                                if (option == selected) Icon(Icons.Default.Check, contentDescription = null)
                            }
                        )
                    }
                }
            }
        }
    }
}

/**
 * 音频源选择项
 */
@Composable
private fun ExpressiveAudioSourceItem(
    viewModel: MainViewModel,
    isFirst: Boolean = false,
    isLast: Boolean = false,
    containerColor: Color = MaterialTheme.colorScheme.surfaceContainerLow,
    hazeState: HazeState? = null,
    enableHaze: Boolean = false
) {
    val state by viewModel.uiState.collectAsState()
    var expanded by remember { mutableStateOf(false) }
    val audioSourceOptions = getAudioSourceOptions()
    val currentSource = audioSourceOptions.find { it.name == state.androidAudioSourceName } ?: audioSourceOptions.firstOrNull()

    ExpressiveListItem(
        isFirst = isFirst,
        isLast = isLast,
        onClick = { expanded = true },
        containerColor = containerColor,
        hazeState = hazeState,
        enableHaze = enableHaze
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp, vertical = 18.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = stringResource(R.string.audioSourceLabel),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface
            )
            Spacer(Modifier.weight(1f))
            if (audioSourceOptions.isNotEmpty() && currentSource != null) {
                Box {
                    Row(
                        modifier = Modifier.widthIn(max = 180.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        Text(
                            text = if (currentSource.labelRes != null) stringResource(currentSource.labelRes)
                            else currentSource.label ?: currentSource.name,
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.primary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                        Icon(
                            imageVector = Icons.Default.ArrowDropDown,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary
                        )
                    }
                    DropdownMenu(
                        expanded = expanded,
                        onDismissRequest = { expanded = false },
                        shape = MaterialTheme.shapes.extraLarge
                    ) {
                        audioSourceOptions.forEach { source ->
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        if (source.labelRes != null) stringResource(source.labelRes)
                                        else source.label ?: source.name
                                    )
                                },
                                onClick = { viewModel.setAndroidAudioSource(source.name); expanded = false },
                                trailingIcon = {
                                    if (currentSource == source) Icon(Icons.Default.Check, contentDescription = null)
                                }
                            )
                        }
                    }
                }
            }
        }
    }
}

private fun LazyListScope.aboutSettingsItems(
    state: AppUiState,
    viewModel: MainViewModel,
    containerColor: Color,
    enableHaze: Boolean,
    hazeState: HazeState?,
    onShowLicenseDialog: () -> Unit,
    onShowContributorsDialog: () -> Unit,
    onShowSponsorsDialog: () -> Unit
) {
    item {
        val uriHandler = LocalUriHandler.current

        val items = mutableListOf<@Composable (isFirst: Boolean, isLast: Boolean) -> Unit>()

        // 开发者
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = null,
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Rounded.Person, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.developerLabel), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(4.dp))
                        Text("LanRhyme、ChinsaaWei、ChouChiu", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        // GitHub 仓库
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = { uriHandler.openUri("https://github.com/MicYou-Dev/MicYou") },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Rounded.Language, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.githubRepoLabel), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "https://github.com/MicYou-Dev/MicYou",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary,
                            textDecoration = TextDecoration.Underline
                        )
                    }
                }
            }
        }

        // 贡献者
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = { onShowContributorsDialog() },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Rounded.People, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.contributorsLabel), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(4.dp))
                        Text(stringResource(R.string.contributorsDesc), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        // 赞助者
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = { onShowSponsorsDialog() },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Rounded.Favorite, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.sponsorsLabel), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(4.dp))
                        Text(stringResource(R.string.sponsorsDesc), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        // 版本
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = { viewModel.checkUpdateManual() },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.weight(1f)) {
                        Icon(Icons.Rounded.Info, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                        Spacer(Modifier.width(16.dp))
                        Column {
                            Text(stringResource(R.string.versionLabel), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                            Spacer(Modifier.height(4.dp))
                            Text(getAppVersion(), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                    TextButton(onClick = { viewModel.checkUpdateManual() }) {
                        Text(stringResource(R.string.checkUpdate))
                    }
                }
            }
        }

        // 开源许可证
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = { onShowLicenseDialog() },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.Rounded.Description, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.openSourceLicense), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(4.dp))
                        Text(stringResource(R.string.viewLibraries), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        // 导出日志
        items.add { isFirst, isLast ->
            ExpressiveListItem(
                isFirst = isFirst,
                isLast = isLast,
                onClick = {
                    viewModel.exportLog { path ->
                        if (path != null) {
                            val message = "Log exported to: $path"
                            viewModel.showSnackbar(message)
                        } else {
                            val message = "Log export failed"
                            viewModel.showSnackbar(message)
                        }
                    }
                },
                containerColor = containerColor,
                hazeState = hazeState,
                enableHaze = enableHaze
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 20.dp, vertical = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(Icons.AutoMirrored.Rounded.TextSnippet, null, modifier = Modifier.size(24.dp), tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.exportLog), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.height(4.dp))
                        Text(stringResource(R.string.exportLogDesc), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }

        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            items.forEachIndexed { index, item ->
                item(index == 0, index == items.size - 1)
            }
        }
    }

    // 底部软件介绍卡片
    item {
        Spacer(Modifier.height(12.dp))
        Card(
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.7f)
            ),
            shape = MaterialTheme.shapes.large
        ) {
            Column(modifier = Modifier.padding(16.dp)) {
                Text(stringResource(R.string.softwareIntro), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSecondaryContainer)
                Spacer(Modifier.height(8.dp))
                Text(
                    stringResource(R.string.introText),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSecondaryContainer
                )
            }
        }
    }
}

@Composable
private fun MirrorCdkDialog(
    cdk: String,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit
) {
    val uriHandler = LocalUriHandler.current
    var inputCdk by remember { mutableStateOf(cdk) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.mirrorCdkLabel)) },
        text = {
            Column {
                Text(
                    text = stringResource(R.string.mirrorCdkDesc),
                    style = MaterialTheme.typography.bodyMedium
                )
                Spacer(Modifier.height(16.dp))
                OutlinedTextField(
                    value = inputCdk,
                    onValueChange = { inputCdk = it },
                    placeholder = { Text(stringResource(R.string.mirrorCdkPlaceholder)) },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true
                )
                Spacer(Modifier.height(12.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.Start
                ) {
                    val langLabel = stringResource(R.string.languageLabel)
                    Text(
                        text = stringResource(R.string.mirrorCdkGetLink),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        textDecoration = TextDecoration.Underline,
                        modifier = Modifier.clickable {
                            val url = if (langLabel.contains("中文") || langLabel.contains("简体") || langLabel.contains("繁體") || langLabel.contains("粤语")) {
                                "https://mirrorchyan.com/zh/get-start"
                            } else {
                                "https://mirrorchyan.com/en/get-start"
                            }
                            uriHandler.openUri(url)
                        }
                    )

                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(inputCdk) },
                enabled = inputCdk.isNotBlank()
            ) {
                Text(stringResource(R.string.ok))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.cancel))
            }
        }
    )
}
