#!/usr/bin/env node
// Portable build of RepoShelf (M9.2): the release app as one exe. Nothing is installed; data still lives in the normal
// app data folder (%LOCALAPPDATA%\dev.schwi.reposhelf, archive Documents\RepoShelf) unless RS_DATA_DIR /
// RS_ARCHIVE_DIR are set.
//
//   npm run build:portable   ->   target/release/portable/RepoShelf_<version>_x64-portable.exe
//
// Shipped build rules (D-14, D-26, D-84): the tauri CLI only (it embeds dist/), never `--features e2e`, never
// `cargo build --workspace`; the copied exe must not contain any test-seam env name.

import { execSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..');
const { version, productName } = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const ext = process.platform === 'win32' ? '.exe' : '';

if (process.env.CARGO_TARGET_DIR) {
  throw new Error(`CARGO_TARGET_DIR is set (${process.env.CARGO_TARGET_DIR}): unset it so the shipped exe lands in target/release`);
}

// `tauri build` runs `npm run build` (the UI) first and embeds dist/.
execSync('npx tauri build --no-bundle', { cwd: root, stdio: 'inherit' });

const app = join(root, 'target', 'release', `reposhelf${ext}`);
if (!existsSync(app)) throw new Error(`missing ${app}`);

const bytes = readFileSync(app);
for (const marker of ['RS_TEST_FILE_CLONE', 'RS_E2E_']) {
  if (bytes.includes(Buffer.from(marker, 'latin1'))) throw new Error(`test seam "${marker}" found in ${app}`);
}

const outDir = join(root, 'target', 'release', 'portable');
mkdirSync(outDir, { recursive: true });
const out = join(outDir, `${productName}_${version}_x64-portable${ext}`);
copyFileSync(app, out);
console.log(`${out} (${(statSync(out).size / 1024 / 1024).toFixed(1)} MB)`);
