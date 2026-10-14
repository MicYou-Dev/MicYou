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

import androidx.compose.foundation.Canvas
import com.lanrhyme.micyou.R
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import com.lanrhyme.micyou.theme.colorToHSV
import com.lanrhyme.micyou.theme.hsvToColor
import com.lanrhyme.micyou.theme.PresetColors
import com.lanrhyme.micyou.ui.ShardTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.res.stringResource

/**
 * HSV 颜色选择器组件
 * 提供色相环、饱和度/明度选择区域和预览功能
 */

@Composable
fun HsvColorPickerDialog(
    initialColor: Color,
    onColorSelected: (Color) -> Unit,
    onDismiss: () -> Unit
) {
    var currentColor by remember { mutableStateOf(initialColor) }
    var hsv by remember { mutableStateOf(FloatArray(3)) }

    LaunchedEffect(initialColor) {
        colorToHSV(initialColor.toArgb(), hsv)
    }

    Dialog(onDismissRequest = onDismiss) {
        Card(
            modifier = Modifier
                .widthIn(max = 320.dp)
                .wrapContentHeight()
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp),
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surface
            )
        ) {
            Column(
                modifier = Modifier
                    .padding(20.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text(
                    stringResource(R.string.colorPickerTitle),
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface
                )

                Spacer(modifier = Modifier.height(16.dp))

                // 饱和度/明度选择区域
                SaturationValuePanel(
                    hue = hsv[0],
                    saturation = hsv[1],
                    value = hsv[2],
                    onSaturationValueChanged = { sat, v ->
                        hsv[1] = sat
                        hsv[2] = v
                        currentColor = Color(hsvToColor(hsv))
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(140.dp)
                )

                Spacer(modifier = Modifier.height(16.dp))

                // 色相条
                HueSlider(
                    hue = hsv[0],
                    onHueChanged = { newHue ->
                        hsv[0] = newHue
                        currentColor = Color(hsvToColor(hsv))
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(24.dp)
                )

                Spacer(modifier = Modifier.height(16.dp))

                // 颜色预览和 HEX 值
                ColorPreview(
                    color = currentColor,
                    modifier = Modifier.fillMaxWidth(),
                    onColorChange = { newColor ->
                        currentColor = newColor
                        colorToHSV(newColor.toArgb(), hsv)
                    }
                )

                Spacer(modifier = Modifier.height(20.dp))

                // 按钮
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End
                ) {
                    TextButton(onClick = onDismiss) {
                        Text(stringResource(R.string.cancel))
                    }
                    Spacer(modifier = Modifier.width(12.dp))
                    Button(
                        onClick = {
                            onColorSelected(currentColor)
                            onDismiss()
                        }
                    ) {
                        Text(stringResource(R.string.confirm))
                    }
                }
            }
        }
    }
}

/**
 * 条状色相选择器
 */
@Composable
fun HueSlider(
    hue: Float,
    onHueChanged: (Float) -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .pointerInput(Unit) {
                detectDragGestures { change, _ ->
                    val newHue = ((change.position.x / size.width) * 360f).coerceIn(0f, 360f)
                    onHueChanged(newHue)
                }
            }
            .pointerInput(Unit) {
                detectTapGestures { offset ->
                    val newHue = ((offset.x / size.width) * 360f).coerceIn(0f, 360f)
                    onHueChanged(newHue)
                }
            }
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            val width = size.width
            val height = size.height

            // 绘制色相渐变条
            for (x in 0..width.toInt() step 2) {
                val hueValue = (x / width) * 360f
                val color = Color.hsv(hueValue, 1f, 1f)
                drawLine(
                    color = color,
                    start = Offset(x.toFloat(), 0f),
                    end = Offset(x.toFloat(), height),
                    strokeWidth = 2f
                )
            }

            // 绘制选择指示器
            val indicatorX = (hue / 360f) * width
            val indicatorY = height / 2f

            // 外圈白色
            drawCircle(
                color = Color.White,
                radius = 10f,
                center = Offset(indicatorX, indicatorY),
                style = Stroke(width = 3f)
            )
            // 内圈黑色
            drawCircle(
                color = Color.Black,
                radius = 10f,
                center = Offset(indicatorX, indicatorY),
                style = Stroke(width = 1f)
            )
            // 中心填充当前色相
            drawCircle(
                color = Color.hsv(hue, 1f, 1f),
                radius = 6f,
                center = Offset(indicatorX, indicatorY)
            )
        }
    }
}

/**
 * 饱和度/明度选择面板
 */
@Composable
fun SaturationValuePanel(
    hue: Float,
    saturation: Float,
    value: Float,
    onSaturationValueChanged: (Float, Float) -> Unit,
    modifier: Modifier = Modifier
) {
    Box(modifier = modifier.clip(RoundedCornerShape(8.dp))) {
        Canvas(
            modifier = Modifier
                .fillMaxSize()
                .pointerInput(Unit) {
                    detectDragGestures { change, _ ->
                        val sat = (change.position.x / size.width).coerceIn(0f, 1f)
    val v = (1f - change.position.y / size.height).coerceIn(0f, 1f)
                        onSaturationValueChanged(sat, v)
                    }
                }
                .pointerInput(Unit) {
                    detectTapGestures { offset ->
                        val sat = (offset.x / size.width).coerceIn(0f, 1f)
    val v = (1f - offset.y / size.height).coerceIn(0f, 1f)
                        onSaturationValueChanged(sat, v)
                    }
                }
        ) {
            val width = size.width
            val height = size.height

            // 绘制基础色相渐变（从左到右：白色 -> 纯色）
            // 使用最大明度(V=1)来绘制，这样面板背景不会随选择改变
            for (x in 0..width.toInt() step 2) {
                val sat = x / width
                val color = Color.hsv(hue, sat, 1f) // 始终使用 V=1
                drawLine(
                    color = color,
                    start = Offset(x.toFloat(), 0f),
                    end = Offset(x.toFloat(), height),
                    strokeWidth = 2f
                )
            }

            // 绘制明度渐变遮罩（从上到下：透明 -> 黑色）
            // 这样无论选择什么颜色，面板左上角始终是白色
            drawRect(
                brush = Brush.verticalGradient(
                    colors = listOf(
                        Color.Transparent,  // 顶部保持原色（白色到纯色）
                        Color.Black          // 底部变为黑色
                    ),
                    startY = 0f,
                    endY = height
                )
            )

            // 绘制选择指示器
            val indicatorX = saturation * width
            val indicatorY = (1f - value) * height

            drawCircle(
                color = Color.White,
                radius = 8f,
                center = Offset(indicatorX, indicatorY),
                style = Stroke(width = 3f)
            )
            drawCircle(
                color = Color.Black,
                radius = 8f,
                center = Offset(indicatorX, indicatorY),
                style = Stroke(width = 1f)
            )
        }
    }
}

/**
 * 颜色预览组件
 */
@Composable
fun ColorPreview(
    color: Color,
    modifier: Modifier = Modifier,
    onColorChange: ((Color) -> Unit)? = null
) {
    val hsv = remember(color) {
        FloatArray(3).apply {
            colorToHSV(color.toArgb(), this)
        }
    }
    var hexInput by remember(color) {
        mutableStateOf(
            String.format("#%06X", color.toArgb() and 0xFFFFFF)
        )
    }
    var inputError by remember { mutableStateOf(false) }
    var isEditing by remember { mutableStateOf(false) }
    val hexString = remember(color) {
        String.format("#%06X", color.toArgb() and 0xFFFFFF)
    }

    fun parseHexInput(input: String): Int? {
        val trimmed = input.trim()
    val hexPattern = Regex("^#([0-9A-Fa-f]{6})$")
    val match = hexPattern.matchEntire(trimmed) ?: return null
        return try {
            match.groupValues[1].toLong(16).toInt() or 0xFF000000.toInt()
        } catch (e: NumberFormatException) {
            null
        }
    }

    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(color)
                .border(2.dp, MaterialTheme.colorScheme.outline, RoundedCornerShape(8.dp))
        )

        Spacer(modifier = Modifier.width(16.dp))

        Column(modifier = Modifier.weight(1f)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (isEditing && onColorChange != null) {
                    ShardTextField(
                        value = hexInput,
                        onValueChange = { newValue ->
                            hexInput = newValue.uppercase()
    val parsed = parseHexInput(newValue)
                            inputError = parsed == null && newValue.isNotEmpty()
                            if (parsed != null) {
                                val newColor = Color(parsed)
                                onColorChange(newColor)
                            }
                        },
                        modifier = Modifier.weight(1f),
                        textStyle = MaterialTheme.typography.titleMedium,
                        isError = inputError,
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii),
                        keyboardActions = KeyboardActions(
                            onDone = {
                                val parsed = parseHexInput(hexInput)
                                if (parsed != null) {
                                    inputError = false
                                    isEditing = false
                                } else {
                                    hexInput = String.format("#%06X", color.toArgb() and 0xFFFFFF)
                                    inputError = false
                                }
                            }
                        ),
                        label = "HEX",
                        supportingText = if (inputError) {
                            { Text(stringResource(R.string.hexFormatError), style = MaterialTheme.typography.labelSmall) }
                        } else null
                    )
                } else {
                    Text(
                        hexString,
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                        modifier = Modifier.clickable(enabled = onColorChange != null) {
                            if (onColorChange != null) {
                                hexInput = hexString
                                isEditing = true
                            }
                        }
                    )
                }
            }

            Spacer(modifier = Modifier.height(4.dp))

            Text(
                "H: ${hsv[0].toInt()}° S: ${(hsv[1] * 100).toInt()}% V: ${(hsv[2] * 100).toInt()}%",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

/**
 * 颜色选择器按钮组件
 */
@Composable
fun ColorPickerButton(
    color: Color,
    onColorSelected: (Color) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true
) {
    var showDialog by remember { mutableStateOf(false) }

    Box(
        modifier = modifier
            .size(40.dp)
            .clip(CircleShape)
            .background(color)
            .border(
                width = 2.dp,
                color = if (enabled) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.outline.copy(alpha = 0.3f),
                shape = CircleShape
            )
            .then(if (enabled) {
                Modifier.pointerInput(Unit) {
                    detectTapGestures { showDialog = true }
                }
            } else Modifier)
            .alpha(if (enabled) 1f else 0.5f)
    )

    if (showDialog) {
        HsvColorPickerDialog(
            initialColor = color,
            onColorSelected = onColorSelected,
            onDismiss = { showDialog = false }
        )
    }
}

/**
 * 扩展颜色列表，添加自定义颜色选项 - 使用网格布局（每行5个）
 */
@Composable
fun ColorSelectorWithPicker(
    selectedColor: Long,
    presetColors: List<Long>,
    onColorSelected: (Long) -> Unit,
    enabled: Boolean = true,
    disabledHint: String = "Dynamic color is enabled",
    modifier: Modifier = Modifier
) {
    var customColor by remember { mutableStateOf<Long?>(null) }

    // 将预设颜色和自定义颜色按钮合并为一个列表
    val allColors = presetColors + (customColor ?: selectedColor)
    val isCustomSelected = customColor != null && selectedColor == customColor

    // 使用网格布局显示颜色
    ColorGrid(
        colors = allColors,
        selectedColor = selectedColor,
        onColorSelected = { colorHex ->
            if (enabled) {
                if (colorHex == allColors.last() && isCustomSelected) {
                    // 点击的是自定义颜色按钮
                    onColorSelected(colorHex)
                } else {
                    onColorSelected(colorHex)
                }
            }
        },
        onCustomColorClick = { /* 自定义颜色按钮单独处理 */ },
        enabled = enabled,
        columns = 5,
        modifier = modifier
    )
}

/**
 * 颜色网格布局组件
 */
@Composable
private fun ColorGrid(
    colors: List<Long>,
    selectedColor: Long,
    onColorSelected: (Long) -> Unit,
    onCustomColorClick: () -> Unit,
    enabled: Boolean = true,
    columns: Int = 5,
    modifier: Modifier = Modifier
) {
    val itemSize = 40.dp
    val spacing = 12.dp

    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(spacing)
    ) {
        // 将颜色分组，每行 columns 个
        colors.chunked(columns).forEachIndexed { rowIndex, rowColors ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(spacing, Alignment.Start)
            ) {
                rowColors.forEachIndexed { index, colorHex ->
                    val isSelected = selectedColor == colorHex
                    val isLastItem = rowIndex == colors.chunked(columns).size - 1 &&
                                     index == rowColors.size - 1

                    if (isLastItem) {
                        // 最后一个位置显示自定义颜色按钮
                        CustomColorOption(
                            color = Color(colorHex.toInt()),
                            isSelected = isSelected,
                            onColorSelected = { color ->
                                val colorLong = color.toArgb().toLong() and 0xFFFFFFFF
                                onColorSelected(colorLong)
                            },
                            enabled = enabled
                        )
                    } else {
                        ColorOption(
                            color = Color(colorHex.toInt()),
                            isSelected = isSelected,
                            onClick = { if (enabled) onColorSelected(colorHex) },
                            enabled = enabled
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ColorOption(
    color: Color,
    isSelected: Boolean,
    onClick: () -> Unit,
    enabled: Boolean = true
) {
    Box(
        modifier = Modifier
            .size(40.dp)
            .clip(CircleShape)
            .background(color)
            .border(
                width = if (isSelected) 3.dp else 2.dp,
                color = when {
                    isSelected -> MaterialTheme.colorScheme.primary
                    !enabled -> MaterialTheme.colorScheme.outline.copy(alpha = 0.5f)
                    else -> MaterialTheme.colorScheme.outline
                },
                shape = CircleShape
            )
            .then(if (enabled) Modifier.pointerInput(Unit) {
                detectTapGestures { onClick() }
            } else Modifier),
        contentAlignment = Alignment.Center
    ) {
        if (isSelected) {
            Icon(
                imageVector = Icons.Default.Check,
                contentDescription = null,
                tint = if (color.luminance() > 0.5f) Color.Black else Color.White,
                modifier = Modifier.size(20.dp)
            )
        }
    }
}

@Composable
private fun CustomColorOption(
    color: Color,
    isSelected: Boolean,
    onColorSelected: (Color) -> Unit,
    enabled: Boolean = true
) {
    var showDialog by remember { mutableStateOf(false) }

    Box(
        modifier = Modifier
            .size(40.dp)
            .clip(CircleShape)
            .background(
                brush = Brush.linearGradient(
                    colors = listOf(
                        Color.Red,
                        Color.Yellow,
                        Color.Green,
                        Color.Cyan,
                        Color.Blue,
                        Color.Magenta,
                        Color.Red
                    )
                )
            )
            .border(
                width = if (isSelected) 3.dp else 2.dp,
                color = when {
                    isSelected -> MaterialTheme.colorScheme.primary
                    !enabled -> MaterialTheme.colorScheme.outline.copy(alpha = 0.5f)
                    else -> MaterialTheme.colorScheme.outline
                },
                shape = CircleShape
            )
            .then(if (enabled) Modifier.pointerInput(Unit) {
                detectTapGestures { showDialog = true }
            } else Modifier),
        contentAlignment = Alignment.Center
    ) {
        // 中心显示当前自定义颜色
        Box(
            modifier = Modifier
                .size(28.dp)
                .clip(CircleShape)
                .background(color)
        )
    }

    if (showDialog) {
        HsvColorPickerDialog(
            initialColor = color,
            onColorSelected = { selectedColor ->
                onColorSelected(selectedColor)
            },
            onDismiss = { showDialog = false }
        )
    }
}