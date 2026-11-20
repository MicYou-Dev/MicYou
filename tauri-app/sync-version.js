import { existsSync, readFileSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { execFileSync } from 'child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const gradlePropsPath = resolve(__dirname, '..', 'gradle.properties');
const tauriConfigPath = resolve(__dirname, 'src-tauri', 'tauri.conf.json');
const cargoTomlPath = resolve(__dirname, 'src-tauri', 'Cargo.toml');
const workspaceCargoTomlPath = resolve(__dirname, 'Cargo.toml');
const packageJsonPath = resolve(__dirname, 'package.json');
const installerIssPath = resolve(__dirname, 'src-tauri', 'installer.iss');
// The in-app "What's New" panel reads this file. It lives next to
// gradle.properties so a release updates the version and the announcement in
// the same obvious place.
const releaseNotesPath = resolve(__dirname, '..', 'release-notes.json');

function readGradleVersion() {
  const content = readFileSync(gradlePropsPath, 'utf-8');
  const versionMatch = content.match(/project\.version=(.+)/);
  const versionCodeMatch = content.match(/project\.version\.code=(.+)/);
  
  if (!versionMatch) throw new Error('project.version not found in gradle.properties');
  
  return {
    version: versionMatch[1].trim(),
    versionCode: versionCodeMatch ? versionCodeMatch[1].trim() : '1'
  };
}

function toSemver(version) {
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)(.*)?$/);
  if (match) {
    const [, major, minor, patch, pre] = match;
    if (pre && pre.length > 0) {
      const cleanPre = pre.replace(/[^a-zA-Z0-9]/g, '').toLowerCase();
      return `${major}.${minor}.${patch}-${cleanPre}`;
    }
    return `${major}.${minor}.${patch}`;
  }
  return version;
}

function updateTauriConfig(version) {
  const config = JSON.parse(readFileSync(tauriConfigPath, 'utf-8'));
  if (config.version === version) return;

  config.version = version;
  writeFileSync(tauriConfigPath, JSON.stringify(config, null, 2) + '\n');
  console.log(`Updated tauri.conf.json: ${version}`);
}

function updateCargoToml(version) {
  const content = readFileSync(cargoTomlPath, 'utf-8');
  const updated = content.replace(/^version\s*=\s*".*"$/m, `version = "${version}"`);
  if (updated === content) return;

  writeFileSync(cargoTomlPath, updated);
  console.log(`Updated Cargo.toml: ${version}`);
}

function updateWorkspaceCargoToml(version) {
  const content = readFileSync(workspaceCargoTomlPath, 'utf-8');
  const updated = content.replace(/^version\s*=\s*".*"$/m, `version = "${version}"`);
  if (updated === content) return;

  writeFileSync(workspaceCargoTomlPath, updated);
  console.log(`Updated workspace Cargo.toml: ${version}`);
}

function updatePackageJson(version) {
  const pkg = JSON.parse(readFileSync(packageJsonPath, 'utf-8'));
  if (pkg.version === version) return;

  pkg.version = version;
  writeFileSync(packageJsonPath, JSON.stringify(pkg, null, 2) + '\n');
  console.log(`Updated package.json: ${version}`);
}

function updateInstallerIss(version) {
  const content = readFileSync(installerIssPath, 'utf-8');
  const updated = content.replace(/#define MyAppVersion ".*"/, `#define MyAppVersion "${version}"`);
  if (updated === content) return;

  writeFileSync(installerIssPath, updated);
  console.log(`Updated installer.iss: ${version}`);
}

function updateCargoLock() {
  // Cargo.lock records the local micyou-app package version. Refresh it after
  // rewriting Cargo.toml so subsequent cargo-about --locked runs stay reproducible.
  execFileSync('cargo', ['metadata', '--format-version', '1'], {
    cwd: __dirname,
    stdio: ['ignore', 'ignore', 'inherit']
  });
  console.log('Updated Cargo.lock');
}

try {
  const { version, versionCode } = readGradleVersion();
  const semver = toSemver(version);
  
  console.log(`Gradle version: ${version} (code: ${versionCode})`);
  console.log(`Semver: ${semver}`);
  
  updateTauriConfig(semver);
  updateCargoToml(semver);
  updateWorkspaceCargoToml(semver);
  updateCargoLock();
  updatePackageJson(semver);
  updateInstallerIss(semver);

  if (!existsSync(releaseNotesPath)) {
    console.warn(`Missing release notes: ${releaseNotesPath}`);
  } else {
    console.log(`Release notes: ${releaseNotesPath} (update them for ${semver})`);
  }

  console.log('Version sync completed!');
} catch (error) {
  console.error('Failed to sync version:', error.message);
  process.exit(1);
}
