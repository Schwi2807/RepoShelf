// Scans files for a secret string (M10c): `node scan-secret.mjs <secret> <control> <path>...` where each path is a file or
// a folder (walked recursively). The secret is searched as UTF-8/ASCII and as UTF-16LE bytes (SQLite, WAL, LevelDB,
// git objects, logs, JSON). `control` is a string that MUST be found at least once (proof that the scanner reads the DB and
// the other files). Prints one JSON object: { files, bytes, hits: [path], controlHits, unreadable }. Exit 1 on a hit
// or when the control was not found. The secret itself is never printed.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const [secret, control, ...roots] = process.argv.slice(2);
if (!secret || !control || roots.length === 0) {
  console.error('usage: node scan-secret.mjs <secret> <control> <path>...');
  process.exit(2);
}
const needles = [Buffer.from(secret, 'utf8'), Buffer.from(secret, 'utf16le')];
const controls = [Buffer.from(control, 'utf8'), Buffer.from(control, 'utf16le')];
const out = { files: 0, bytes: 0, hits: [], controlHits: 0, unreadable: [] };

function scan(path) {
  let st;
  try {
    st = statSync(path);
  } catch (e) {
    out.unreadable.push(path);
    return;
  }
  if (st.isDirectory()) {
    let names = [];
    try {
      names = readdirSync(path);
    } catch {
      out.unreadable.push(path);
    }
    for (const n of names) scan(join(path, n));
    return;
  }
  let buf;
  try {
    buf = readFileSync(path);
  } catch {
    out.unreadable.push(path);
    return;
  }
  out.files++;
  out.bytes += buf.length;
  if (needles.some((n) => buf.includes(n))) out.hits.push(path);
  if (controls.some((n) => buf.includes(n))) out.controlHits++;
}

for (const r of roots) scan(r);
console.log(JSON.stringify(out));
process.exit(out.hits.length === 0 && out.controlHits > 0 ? 0 : 1);
