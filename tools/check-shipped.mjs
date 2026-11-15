#!/usr/bin/env node
// String check of the SHIPPED exes (M13 marker check + M10c credential-store check).
//
//   node tools/check-shipped.mjs <shipped.exe>... [--e2e <e2e.exe>]
//
// Every shipped exe (target\release\reposhelf.exe, the portable copy) must
//   - contain none of the test-seam env names (RS_TEST_FILE_CLONE, RS_E2E_ - which covers RS_E2E_NET / RS_E2E_GIT_MAP),
//   - contain the REAL credential store: `windows-native-keyring-store` is compiled in and used (D-117). A build with
//     the `testkit` feature unified in silently gets the in-memory fake (the token would show as saved and vanish at
//     the next start), so the real store's failure text and its crate must be present, and the fake's must not.
// The e2e exe is the control (`--e2e`): it must contain the seam names and NOT the real store (it has the fake).
// Exit 1 on any miss; prints one line per check.

import { readFileSync } from 'node:fs';

const args = process.argv.slice(2);
const e2eAt = args.indexOf('--e2e');
const e2e = e2eAt >= 0 ? args.splice(e2eAt, 2)[1] : null;
if (args.length === 0) {
  console.error('usage: node tools/check-shipped.mjs <shipped.exe>... [--e2e <e2e.exe>]');
  process.exit(2);
}

const has = (buf, s) => buf.includes(Buffer.from(s, 'latin1'));
const SEAMS = ['RS_TEST_FILE_CLONE', 'RS_E2E_', 'RS_E2E_NET', 'RS_E2E_GIT_MAP', 'RS_E2E_TRASH_LOG'];
// the text of `cred::fail` (only built where WindowsStore is reachable), and the store crate's own path in panic locations
const REAL = ['Windows Credential Manager failed'];
const CRATE = 'windows-native-keyring-store';
let bad = 0;
const say = (ok, msg) => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${msg}`);
  if (!ok) bad++;
};

for (const f of args) {
  const b = readFileSync(f);
  const seams = SEAMS.filter((m) => has(b, m));
  say(seams.length === 0, `${f}: no test seam strings${seams.length ? ' (found ' + seams.join(', ') + ')' : ''}`);
  for (const r of REAL) say(has(b, r), `${f}: real credential store present ("${r}")`);
  console.log(`info ${f}: "${CRATE}" text present: ${has(b, CRATE)}`);
}
if (e2e) {
  const b = readFileSync(e2e);
  for (const m of SEAMS) say(has(b, m), `${e2e}: positive control, ${m} present in the e2e exe`);
  for (const r of REAL) say(!has(b, r), `${e2e}: negative control, the e2e exe has the in-memory fake (no "${r}")`);
}
process.exit(bad ? 1 : 0);
