import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = path.dirname(fileURLToPath(import.meta.url));
const resourcesDir = path.join(appDir, 'src-tauri', 'resources');
const generatedDir = path.join(appDir, 'src', 'generated');
const outputFile = path.join(generatedDir, 'third-party-licenses.html');
const bunLockFile = path.join(appDir, 'bun.lock');

await mkdir(generatedDir, { recursive: true });

// cargo-about takes ~20 s, so the report is only rebuilt when an input changes.
// The fingerprint of the inputs is stored on the first line of the report.
async function inputFingerprint() {
  const crateDirs = (await readdir(path.join(appDir, 'crates'))).map((dir) => path.join('crates', dir));
  const resources = (await readdir(resourcesDir)).filter((file) => /^LICENSE-.+\.txt$/i.test(file));
  const inputs = [
    'generate-licenses.js',
    'about.toml',
    'about.hbs',
    'Cargo.lock',
    'bun.lock',
    'package.json',
    'Cargo.toml',
    path.join('src-tauri', 'Cargo.toml'),
    ...crateDirs.map((dir) => path.join(dir, 'Cargo.toml')),
    ...resources.map((file) => path.join('src-tauri', 'resources', file)),
  ].sort();
  const hash = createHash('sha256');
  for (const input of inputs) {
    hash.update(input);
    hash.update(await readFile(path.join(appDir, input)).catch(() => ''));
  }
  return hash.digest('hex');
}

const fingerprint = await inputFingerprint();
const stamp = `<!-- licenses-inputs: ${fingerprint} -->`;
const force = process.argv.includes('--force');
const previous = await readFile(outputFile, 'utf8').catch(() => '');
if (!force && previous.startsWith(stamp)) {
  console.log('[licenses] Inputs unchanged, keeping the existing report (--force to rebuild)');
  process.exit(0);
}

const escapeHtml = (value) =>
  String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');

async function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: appDir,
      stdio: options.capture ? ['ignore', 'pipe', 'pipe'] : 'inherit',
    });
    let stdout = '';
    let stderr = '';
    if (options.capture) {
      child.stdout.on('data', (chunk) => (stdout += chunk));
      child.stderr.on('data', (chunk) => (stderr += chunk));
    }
    child.once('error', reject);
    child.once('exit', (code, signal) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        const error = new Error(
          `${command} exited with ${signal ? `signal ${signal}` : `code ${code}`}${stderr ? `: ${stderr.trim()}` : ''}`,
        );
        error.exitCode = code;
        reject(error);
      }
    });
  });
}

async function cargoAboutVersion() {
  try {
    const { stdout } = await run('cargo', ['about', '--version'], { capture: true });
    return stdout.trim().match(/cargo-about\s+(\S+)/)?.[1] ?? null;
  } catch (error) {
    if (error.exitCode !== undefined || error.code === 'ENOENT') return null;
    throw error;
  }
}

async function configuredCargoAboutVersion() {
  const { stdout } = await run(
    'cargo',
    ['metadata', '--no-deps', '--format-version', '1'],
    { capture: true },
  );
  const metadata = JSON.parse(stdout);
  const version = metadata.metadata?.tools?.['cargo-about'];
  if (typeof version !== 'string' || !/^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/.test(version)) {
    throw new Error('Cargo.toml must declare workspace.metadata.tools.cargo-about');
  }
  return version;
}

async function ensureCargoAbout() {
  const expected = await configuredCargoAboutVersion();
  const installed = await cargoAboutVersion();
  if (installed === expected) {
    console.log(`[licenses] cargo-about ${installed} ready`);
    return;
  }

  console.log(
    installed
      ? `Updating cargo-about from ${installed} to ${expected}...`
      : `Installing cargo-about ${expected}...`,
  );
  await run('cargo', [
    'install',
    'cargo-about',
    '--version',
    expected,
    '--locked',
    '--features',
    'cli',
    '--force',
  ]);
  console.log(`[licenses] cargo-about ${expected} installed`);
}

async function generateRustReport() {
  console.log('[licenses] Generating Rust dependency report...');
  await ensureCargoAbout();

  await run('cargo', [
    'about',
    'generate',
    'about.hbs',
    '--workspace',
    '--all-features',
    '--locked',
    '--fail',
    '--output-file',
    path.relative(appDir, outputFile),
  ]);

  const report = await readFile(outputFile, 'utf8');
  console.log('[licenses] Rust dependency report generated');
  return report;
}

async function readLicenseFiles(packageDirectory) {
  const files = await readdir(packageDirectory).catch(() => []);
  const licenseFiles = files
    .filter((file) => /^(licen[cs]e|copying|notice)(\..*)?$/i.test(file))
    .sort((left, right) => left.localeCompare(right));

  const texts = await Promise.all(
    licenseFiles.map(async (file) => {
      const text = await readFile(path.join(packageDirectory, file), 'utf8').catch(() => '');
      return text.trim() ? `${file}\n\n${text.trim()}` : '';
    }),
  );
  return texts.filter(Boolean).join('\n\n');
}

// bun.lock is JSON with trailing commas. Package keys are install paths:
// "name" is hoisted, "parent/name" is nested under parent's node_modules.
async function readBunLock() {
  const text = await readFile(bunLockFile, 'utf8');
  return JSON.parse(text.replace(/,(\s*[}\]])/g, '$1'));
}

function keySegments(key) {
  const parts = key.split('/');
  const names = [];
  for (let i = 0; i < parts.length; i++) {
    names.push(parts[i].startsWith('@') ? `${parts[i]}/${parts[++i]}` : parts[i]);
  }
  return names;
}

// Node resolution: look in the requiring package's own node_modules first,
// then walk up through its parents to the hoisted root.
function resolveKey(lockPackages, parentKey, name) {
  const scope = parentKey ? keySegments(parentKey) : [];
  for (let depth = scope.length; depth >= 0; depth--) {
    const key = [...scope.slice(0, depth), name].join('/');
    if (lockPackages[key]) return key;
  }
  return null;
}

async function generateNpmReport() {
  const lock = await readBunLock();
  const lockPackages = lock.packages ?? {};
  const root = lock.workspaces?.[''] ?? {};
  const queue = Object.keys({ ...root.dependencies, ...root.optionalDependencies })
    .map((name) => resolveKey(lockPackages, '', name))
    .filter(Boolean);
  const visited = new Set();
  const seen = new Set();
  const packages = [];

  while (queue.length) {
    const key = queue.shift();
    if (visited.has(key)) continue;
    visited.add(key);

    const [spec, , info = {}] = lockPackages[key];
    const deps = { ...info.dependencies, ...info.optionalDependencies, ...info.peerDependencies };
    for (const name of Object.keys(deps)) {
      const child = resolveKey(lockPackages, key, name);
      if (child) queue.push(child);
    }

    const packageDirectory = path.join(appDir, 'node_modules', ...keySegments(key).join('/node_modules/').split('/'));
    const manifest = JSON.parse(
      await readFile(path.join(packageDirectory, 'package.json'), 'utf8').catch(() => '{}'),
    );
    const name = spec.slice(0, spec.lastIndexOf('@'));
    const version = spec.slice(spec.lastIndexOf('@') + 1);
    // Optional platform packages that are not installed on this host have no manifest.
    if (!manifest.name) continue;
    const id = `${name}@${version}`;
    if (seen.has(id)) continue;
    seen.add(id);

    const license = typeof manifest.license === 'string' ? manifest.license : manifest.license?.type;
    packages.push({
      name,
      version,
      license: license ?? 'UNKNOWN',
      text: await readLicenseFiles(packageDirectory),
    });
  }

  packages.sort((left, right) =>
    left.name.localeCompare(right.name) || left.version.localeCompare(right.version),
  );
  console.log(`[licenses] Frontend production dependencies: ${packages.length}`);

  const rows = packages
    .map(
      ({ name, version, license }) => `<tr>
        <td><a href="https://www.npmjs.com/package/${encodeURIComponent(name)}" target="_blank" rel="noreferrer">${escapeHtml(name)}</a></td>
        <td>${escapeHtml(version)}</td>
        <td>${escapeHtml(license)}</td>
      </tr>`,
    )
    .join('\n');
  const details = packages
    .filter(({ text }) => text)
    .map(
      ({ name, version, text }) => `<details>
      <summary>${escapeHtml(name)} ${escapeHtml(version)}</summary>
      <pre>${escapeHtml(text)}</pre>
    </details>`,
    )
    .join('\n');

  return `<section class="npm-licenses">
  <div class="license-summary">
    <h3>Frontend dependencies</h3>
    <p>Generated from production packages in bun.lock.</p>
  </div>
  <table class="license-table">
    <thead><tr><th>Package</th><th>Version</th><th>License</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>
  <div class="license-texts">${details}</div>
</section>`;
}

async function generateAssetReport() {
  const licenseFiles = (await readdir(resourcesDir))
    .filter((file) => /^LICENSE-.+\.txt$/i.test(file))
    .sort((left, right) => left.localeCompare(right));

  const assetLicenses = await Promise.all(
    licenseFiles.map(async (file) => {
      const text = await readFile(path.join(resourcesDir, file), 'utf8');
      const name = file.replace(/^LICENSE-/i, '').replace(/\.txt$/i, '');
      const license = text.split(/\r?\n/, 1)[0].trim();
      return { name, license, text };
    }),
  );

  console.log(`[licenses] Bundled asset licenses: ${assetLicenses.length}`);
  if (!assetLicenses.length) return '';
  return `<section class="asset-licenses">
  <div class="license-summary"><h3>${assetLicenses
    .map(({ name }) => escapeHtml(name))
    .join(' / ')}</h3></div>
  <div class="license-texts">
    ${assetLicenses
      .map(
        ({ name, license, text }) => `<details>
      <summary>${escapeHtml(name)} — ${escapeHtml(license)}</summary>
      <pre>${escapeHtml(text)}</pre>
    </details>`,
      )
      .join('\n    ')}
  </div>
</section>`;
}

const [rustReport, npmReport, assetReport] = await Promise.all([
  generateRustReport(),
  generateNpmReport(),
  generateAssetReport(),
]);
await writeFile(outputFile, `${stamp}\n${assetReport}\n${npmReport}\n${rustReport}`);
console.log(`[licenses] Report written to ${path.relative(appDir, outputFile)}`);
