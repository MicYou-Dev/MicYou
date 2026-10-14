// Compiles the Icon Composer project in src-tauri/icons into the two assets the
// macOS bundle needs: icon.icns for systems before macOS 26, and Assets.car for
// the layered Liquid Glass icon that those systems resolve through the
// CFBundleIconName key declared in src-tauri/Info.plist.
//
// actool is only shipped with Xcode, so environments without it keep the
// committed output instead of failing the build. Regeneration is skipped while
// the output is newer than the project because actool is not reproducible: it
// stamps every run into Assets.car, which would otherwise show up as a
// meaningless diff after each build. Pass --force to rebuild regardless.

import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = path.dirname(fileURLToPath(import.meta.url));
const srcTauriDir = path.join(appDir, 'src-tauri');
const iconsDir = path.join(srcTauriDir, 'icons');

// actool names the compiled assets after the project, and macOS matches that
// name against CFBundleIconName, so the project file name, Info.plist and the
// checks below all have to agree on this one string.
const iconName = 'MicYou';
const projectDir = path.join(iconsDir, iconName + '.icon');
const icnsPath = path.join(iconsDir, 'icon.icns');
const carPath = path.join(iconsDir, 'Assets.car');
const infoPlistPath = path.join(srcTauriDir, 'Info.plist');
const configPath = path.join(srcTauriDir, 'tauri.conf.json');
const force = process.argv.includes('--force');

function modificationTime(target) {
  try {
    return statSync(target).mtimeMs;
  } catch {
    return 0;
  }
}

function newestModificationTime(directory) {
  let newest = 0;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const fullPath = path.join(directory, entry.name);
    const time = entry.isDirectory() ? newestModificationTime(fullPath) : modificationTime(fullPath);
    newest = Math.max(newest, time);
  }
  return newest;
}

function hasActool() {
  try {
    execFileSync('xcrun', ['--find', 'actool'], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

function plistValue(file, key) {
  return execFileSync('plutil', ['-extract', key, 'raw', '-o', '-', file], {
    encoding: 'utf8',
  }).trim();
}

// actool has to compile the catalog for the same macOS floor the bundle declares,
// so the floor is read from the config instead of being repeated here: two copies
// would drift apart again, and a catalog built for a newer system than the app
// claims to support only shows up when macOS rejects the icon. Tauri itself falls
// back to 10.13 while the key is absent.
function macosMinimumSystemVersion() {
  let config;
  try {
    config = JSON.parse(readFileSync(configPath, 'utf8'));
  } catch (error) {
    throw new Error('could not read the macOS bundle config ' + configPath + ': ' + error);
  }
  return config.bundle?.macOS?.minimumSystemVersion || '10.13';
}

if (process.platform !== 'darwin') {
  console.log('[icon] Skipping ' + iconName + '.icon: Icon Composer projects only build on macOS');
  process.exit(0);
}

const sourceTime = newestModificationTime(projectDir);
if (!force && modificationTime(icnsPath) > sourceTime && modificationTime(carPath) > sourceTime) {
  console.log('[icon] Up to date');
  process.exit(0);
}

const hasCommittedOutput = modificationTime(icnsPath) > 0 && modificationTime(carPath) > 0;
if (!hasActool()) {
  if (hasCommittedOutput) {
    console.log('[icon] actool is unavailable, keeping the committed icon assets');
    process.exit(0);
  }
  throw new Error('actool is required to build the app icon (install Xcode)');
}

// actool writes CFBundleIconFile with the bare asset name, but the bundle ships
// the .icns under Tauri's own name, so only CFBundleIconName is declared in
// Info.plist. Refuse to produce a pair the system cannot resolve instead.
const declaredName = plistValue(infoPlistPath, 'CFBundleIconName');
const deploymentTarget = macosMinimumSystemVersion();

const workDir = mkdtempSync(path.join(tmpdir(), 'micyou-icon-'));
try {
  const partialPlistPath = path.join(workDir, 'partial.plist');
  console.log(
    '[icon] Compiling ' +
      path.relative(appDir, projectDir) +
      ' with actool for macOS ' +
      deploymentTarget,
  );
  const compile = spawnSync(
    'xcrun',
    [
      'actool',
      projectDir,
      '--compile',
      workDir,
      '--app-icon',
      iconName,
      '--output-partial-info-plist',
      partialPlistPath,
      '--platform',
      'macosx',
      '--minimum-deployment-target',
      deploymentTarget,
      '--include-all-app-icons',
    ],
    { encoding: 'utf8' },
  );

  if (compile.status !== 0) {
    // Keep the full output when the compile actually failed.
    if (compile.stdout) process.stdout.write(compile.stdout);
    if (compile.stderr) process.stderr.write(compile.stderr);
    throw new Error('actool exited with code ' + compile.status);
  }

  // When an older macOS runs a newer Xcode, dyld reports symbol overrides in
  // the SDK's CoreMedia and MediaToolbox stubs on every actool invocation.
  // They say nothing about the icon, so they are dropped from the log while
  // every other actool diagnostic is still surfaced.
  const noise = (compile.stderr || '')
    .split('\n')
    .filter((line) => line !== '' && !/^dyld\[\d+\]: /.test(line));
  // actool's stdout is a plist describing the files it just wrote, which the
  // script reports itself below.
  for (const line of noise) console.warn('[icon] ' + line);

  const compiledName = plistValue(partialPlistPath, 'CFBundleIconName');
  if (compiledName !== declaredName) {
    throw new Error(
      'Info.plist declares CFBundleIconName ' +
        declaredName +
        ' but actool compiled ' +
        compiledName +
        '; rename the project, the plist key or the icon name so they match',
    );
  }

  copyFileSync(path.join(workDir, iconName + '.icns'), icnsPath);
  copyFileSync(path.join(workDir, 'Assets.car'), carPath);
  console.log(
    '[icon] Wrote ' +
      path.relative(appDir, icnsPath) +
      ' and ' +
      path.relative(appDir, carPath),
  );
} finally {
  rmSync(workDir, { recursive: true, force: true });
}
