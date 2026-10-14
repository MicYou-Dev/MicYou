import { copyFileSync, mkdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = path.dirname(fileURLToPath(import.meta.url));
const debug = process.argv.includes('--debug');

function hostTargetTriple() {
  const details = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const target = details.match(/^host:\s*(\S+)$/m)?.[1];
  if (!target) throw new Error('Unable to determine the Rust host target triple');
  return target;
}

function cargoTargetDirectory() {
  const metadata = execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
    cwd: appDir,
    encoding: 'utf8',
  });
  return JSON.parse(metadata).target_directory;
}

const target = process.env.TAURI_ENV_TARGET_TRIPLE || hostTargetTriple();
const profile = debug ? 'debug' : 'release';
const extension = target.includes('windows') ? '.exe' : '';
const outputDir = path.join(appDir, 'src-tauri', 'binaries');
const sidecars = ['micyou-cli', 'micyou-tui'];
mkdirSync(outputDir, { recursive: true });

const cargoArgs = [
  'build',
  '--locked',
  '--target',
  target,
  '-p',
  'micyou-cli',
  '-p',
  'micyou-tui',
];
if (!debug) cargoArgs.push('--release');

console.log('[sidecars] Building CLI and TUI for ' + target + ' (' + profile + ')');
execFileSync('cargo', cargoArgs, {
  cwd: appDir,
  stdio: 'inherit',
  env: {
    ...process.env,
    // Both terminal crates depend on the desktop core library. Disable sidecar
    // validation only for this bootstrap build; the following Tauri build sees
    // the prepared binaries and validates/bundles them normally.
    TAURI_CONFIG: JSON.stringify({ bundle: { externalBin: [] } }),
  },
});

const targetDir = cargoTargetDirectory();

for (const binary of sidecars) {
  const source = path.join(targetDir, target, profile, binary + extension);
  const destination = path.join(outputDir, binary + '-' + target + extension);
  copyFileSync(source, destination);
  console.log('[sidecars] Prepared ' + path.relative(appDir, destination));
}
