#!/usr/bin/env node
/**
 * Run `npm run tauri dev` with an isolated app data folder so wizard / first-run
 * testing never touches %APPDATA%\com.grimoire.app (or the normal install path).
 *
 * Usage:
 *   node scripts/wizard-sandbox.mjs              # reuse sandbox (continue where you left off)
 *   node scripts/wizard-sandbox.mjs --fresh      # wipe sandbox, simulate brand-new install
 *   node scripts/wizard-sandbox.mjs --migration  # fresh sandbox + fake preview vault to migrate
 *
 * Sandbox files live under scripts/.local-sandboxes/ (gitignored).
 */

import { spawn } from 'node:child_process';
import { mkdir, rm, writeFile, mkdir as mkdirp } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');
const sandboxesRoot = path.join(repoRoot, 'scripts', '.local-sandboxes');
const sandboxDir = path.join(sandboxesRoot, 'wizard-default');
const legacyDir = path.join(sandboxesRoot, 'wizard-legacy-preview');

const args = new Set(process.argv.slice(2));
const fresh = args.has('--fresh');
const migration = args.has('--migration');

if (args.has('--help') || args.has('-h')) {
  console.log(`Grimoire wizard sandbox (isolated app data)

  npm run tauri:wizard-sandbox              Reuse sandbox folder
  npm run tauri:wizard-sandbox:fresh        Delete sandbox, first-run wizard
  npm run tauri:wizard-sandbox:migration    Fake preview vault → migration banner

  Sandbox:  ${sandboxDir}
  Your normal install folder is NOT used when GRIMOIRE_APP_DATA_DIR is set.
`);
  process.exit(0);
}

await mkdir(sandboxesRoot, { recursive: true });

if (fresh || migration) {
  await rm(sandboxDir, { recursive: true, force: true });
  console.log(`Cleared sandbox: ${sandboxDir}`);
}

await mkdir(sandboxDir, { recursive: true });

const env = {
  ...process.env,
  GRIMOIRE_APP_DATA_DIR: sandboxDir,
};

if (migration) {
  await rm(legacyDir, { recursive: true, force: true });
  await mkdirp(legacyDir, { recursive: true });
  await mkdirp(path.join(legacyDir, 'lancedb', 'probe'), { recursive: true });
  await writeFile(path.join(legacyDir, 'grimoire.db'), 'sandbox-legacy-db-marker\n');
  await writeFile(path.join(legacyDir, 'lancedb', 'probe', 'x.dat'), 'x');
  env.GRIMOIRE_LEGACY_MIGRATION_FROM = legacyDir;
  console.log(`Seeded fake preview vault: ${legacyDir}`);
}

console.log('');
console.log('=== Grimoire wizard sandbox ===');
console.log(`App data (isolated): ${sandboxDir}`);
console.log('Your production install is NOT touched.');
console.log('Look for a log line: GRIMOIRE_APP_DATA_DIR is set');
console.log('');

const child = spawn('npm', ['run', 'tauri', 'dev'], {
  cwd: repoRoot,
  env,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
