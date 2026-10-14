#!/usr/bin/env node
// Renders the macOS DMG background from the committed SVG.
//
// Finder sizes a disk-image background by the image's POINT size, which AppKit
// derives from the pixel dimensions and the DPI tag (points = pixels / (dpi / 72)).
// A 660x400 @72dpi image fills the window but is upscaled on a Retina screen, while
// a 1320x800 image without the tag would be read as 1320 points wide and overflow
// the window. Rendering at 2x and tagging it 144dpi gives the window exactly
// 660x400 points of layout with twice the pixels behind it.
//
// Usage: bun build-dmg-background.js [--force]
//
// Run with --force after editing the SVG. The tag check decides whether anything
// has to happen, because sips does not encode byte-identical output and an mtime
// rule would rewrite the file on every fresh clone.
//
// The rendered PNG is committed, so a machine without the tooling still builds.

import { copyFileSync, existsSync, mkdtempSync, rmSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const appDir = path.dirname(fileURLToPath(import.meta.url))
const packagingDir = path.join(appDir, 'src-tauri', 'packaging')
const sourcePath = path.join(packagingDir, 'dmg-background.svg')
const outputPath = path.join(packagingDir, 'dmg-background.png')

// The DMG window is 660x400 points; the PNG carries the same layout at 2x.
const WIDTH = 660
const HEIGHT = 400
const SCALE = 2
const DPI = 72 * SCALE

const force = process.argv.includes('--force')

function run(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8' })
  if (result.error || result.status !== 0) {
    const detail = result.error ? result.error.message : (result.stderr || '').trim()
    throw new Error(`${command} failed: ${detail}`)
  }
  return result.stdout || ''
}

function toolAvailable(command) {
  const result = spawnSync('which', [command], { encoding: 'utf8' })
  return !result.error && result.status === 0
}

// sips reports what a file actually carries, which is the only thing Finder sees.
function readTags(file) {
  const probe = run('sips', ['-g', 'pixelWidth', '-g', 'pixelHeight', '-g', 'dpiWidth', file])
  const read = (key) => {
    const match = probe.match(new RegExp(`${key}:\\s*([0-9.]+)`))
    return match ? Math.round(Number(match[1])) : NaN
  }
  return { width: read('pixelWidth'), height: read('pixelHeight'), dpi: read('dpiWidth') }
}

function isCorrect(tags) {
  return tags.width === WIDTH * SCALE && tags.height === HEIGHT * SCALE && tags.dpi === DPI
}

if (process.platform !== 'darwin') {
  console.log('[dmg-background] Skipped (macOS only)')
  process.exit(0)
}

const haveSips = toolAvailable('sips')
if (!force && haveSips && existsSync(outputPath)) {
  let tags = null
  try {
    tags = readTags(outputPath)
  } catch {
    tags = null
  }
  if (tags && isCorrect(tags)) {
    console.log(`[dmg-background] Up to date (${tags.width}x${tags.height} at ${tags.dpi}dpi) - pass --force after editing the SVG`)
    process.exit(0)
  }
}

for (const tool of ['rsvg-convert', 'sips']) {
  if (!toolAvailable(tool)) {
    if (existsSync(outputPath)) {
      console.warn(`[dmg-background] ${tool} is not available - keeping the committed PNG`)
      process.exit(0)
    }
    throw new Error(`${tool} is not available and ${path.relative(appDir, outputPath)} does not exist`)
  }
}

const tempDir = mkdtempSync(path.join(tmpdir(), 'micyou-dmg-background-'))
try {
  const rendered = path.join(tempDir, 'dmg-background.png')
  run('rsvg-convert', ['-w', String(WIDTH * SCALE), '-h', String(HEIGHT * SCALE), '-o', rendered, sourcePath])
  run('sips', ['-s', 'dpiWidth', String(DPI), '-s', 'dpiHeight', String(DPI), rendered])

  const tags = readTags(rendered)
  if (!isCorrect(tags)) {
    throw new Error(`expected ${WIDTH * SCALE}x${HEIGHT * SCALE} at ${DPI}dpi, got ${tags.width}x${tags.height} at ${tags.dpi}dpi`)
  }

  copyFileSync(rendered, outputPath)
  console.log(`[dmg-background] Rendered ${tags.width}x${tags.height} at ${tags.dpi}dpi`)
} finally {
  rmSync(tempDir, { recursive: true, force: true })
}
