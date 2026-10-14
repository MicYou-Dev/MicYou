#!/usr/bin/env node
// Renders the monochrome menu bar icon macOS wants from the committed app glyph.
//
// The menu bar draws a template image from its alpha channel alone and tints it to
// match the bar, so the macOS tray needs a transparent glyph instead of the coloured
// application icon. This renders `src/shared/assets/app_icon.svg`, whose paths use
// `currentColor` and therefore come out black in a standalone render.
//
// Two details are load-bearing. The tray layer sizes every tray image to a fixed 18pt
// height and ignores the pixel count and any DPI tag, so the pixel dimensions only
// decide how much detail sits behind those 18 points (36px is 2x). And the glyph only
// covers ~66% of its 512x512 canvas, so rendering the canvas as-is would show a ~12pt
// icon: the view box below is cropped to the glyph's measured bounds plus a small
// margin, which brings it back to ~16.8pt of the 18pt box.
//
// Usage: bun build-tray-icon.js [--force]
//
// Run with --force after editing the SVG. The size check and the source timestamp
// decide whether anything has to happen.
//
// The rendered PNG is committed, so a machine without the tooling still builds.

import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const appDir = path.dirname(fileURLToPath(import.meta.url))
const sourcePath = path.join(appDir, 'src', 'shared', 'assets', 'app_icon.svg')
const outputPath = path.join(appDir, 'src-tauri', 'icons', 'tray-icon-template.png')

// The tray layer gives every tray image an 18pt height, so 36px is a 2x representation.
const SIZE = 36
// Guards the crop below: a different canvas would need a different view box.
const SOURCE_SIZE = 512
const SOURCE_TAG = `width="${SOURCE_SIZE}" height="${SOURCE_SIZE}" viewBox="0 0 ${SOURCE_SIZE} ${SOURCE_SIZE}"`
// The glyph's alpha bounding box measures 337x337 from (76,99); this is that box grown
// by ~6% and re-centred on it, so the icon keeps a little breathing room in the bar.
const CROP = { x: 64, y: 88, size: 360 }

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

// sips reports what the file actually carries, which is what the tray will show.
function readTags(file) {
  const probe = run('sips', ['-g', 'pixelWidth', '-g', 'pixelHeight', file])
  const read = (key) => {
    const match = probe.match(new RegExp(`${key}:\\s*([0-9.]+)`))
    return match ? Math.round(Number(match[1])) : NaN
  }
  return { width: read('pixelWidth'), height: read('pixelHeight') }
}

function hasExpectedSize(tags) {
  return tags.width === SIZE && tags.height === SIZE
}

if (process.platform !== 'darwin') {
  console.log('[tray-icon] Skipped (macOS only)')
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
  // The glyph is an input to the PNG, so a newer source outranks a correct size.
  const fresh = statSync(outputPath).mtimeMs >= statSync(sourcePath).mtimeMs
  if (tags && hasExpectedSize(tags) && fresh) {
    console.log(`[tray-icon] Up to date (${tags.width}x${tags.height}) - pass --force after editing the SVG`)
    process.exit(0)
  }
}

for (const tool of ['rsvg-convert', 'sips']) {
  if (!toolAvailable(tool)) {
    if (existsSync(outputPath)) {
      console.warn(`[tray-icon] ${tool} is not available - keeping the committed PNG`)
      process.exit(0)
    }
    throw new Error(`${tool} is not available and ${path.relative(appDir, outputPath)} does not exist`)
  }
}

const source = readFileSync(sourcePath, 'utf8')
if (!source.includes(SOURCE_TAG)) {
  throw new Error(`${path.relative(appDir, sourcePath)} does not open with ${SOURCE_TAG} - re-measure CROP`)
}
if (source.split(SOURCE_TAG).length !== 2) {
  throw new Error(`expected exactly one ${SOURCE_TAG} in ${path.relative(appDir, sourcePath)}`)
}
// Rewriting only the opening tag leaves fill="none" and currentColor untouched.
const cropped = source.replace(
  SOURCE_TAG,
  `width="${SIZE}" height="${SIZE}" viewBox="${CROP.x} ${CROP.y} ${CROP.size} ${CROP.size}"`,
)

const tempDir = mkdtempSync(path.join(tmpdir(), 'micyou-tray-icon-'))
try {
  const svg = path.join(tempDir, 'tray-icon.svg')
  const rendered = path.join(tempDir, 'tray-icon-template.png')
  writeFileSync(svg, cropped)
  run('rsvg-convert', ['-w', String(SIZE), '-h', String(SIZE), '-o', rendered, svg])

  const tags = readTags(rendered)
  if (!hasExpectedSize(tags)) {
    throw new Error(`expected ${SIZE}x${SIZE}, got ${tags.width}x${tags.height}`)
  }

  copyFileSync(rendered, outputPath)
  console.log(`[tray-icon] Rendered ${tags.width}x${tags.height}`)
} finally {
  rmSync(tempDir, { recursive: true, force: true })
}
