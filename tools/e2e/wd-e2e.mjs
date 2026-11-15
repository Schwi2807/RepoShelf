// Real-app e2e for RepoShelf (M8.3): drives the e2e build (`target\e2e\release\reposhelf.exe`) through tauri-driver
// + msedgedriver against SCRATCH data made by run-e2e.ps1. Never run it against real data.
//
//   node tools/e2e/wd-e2e.mjs <config.json>
//
// config.json (written by run-e2e.ps1): { app, port, result, shots, archive, openLog, trashLog, upstream, upstreamUrl, gitEnv,
// network, networkRepos }. Writes the step list to `result` (JSON array of { step, ok, error?, ... }).
//
// Sessions: A = cold-ish start timing only; B = warm start timing + the whole smoke (imports via the e2e picker
// queue, every screen in light + dark with screenshots, security pass, hostile IPC, NFR-P1/P2 timings, file-seam
// clone → fetch → fast-forward → remove). C (only with network: true) = public clones + fetch.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const cfg = JSON.parse(readFileSync(process.argv[2], 'utf8').replace(/^\uFEFF/, ''));

// ---------------------------------------------------------------------------------------------------------------
// M13 network seam (D-113): every HTTP request of the e2e exe is answered from this routes file (RS_E2E_NET) and
// logged (RS_E2E_NET_LOG); git for the mapped github.com repo goes to the local upstream (RS_E2E_GIT_MAP). Expected
// failures have explicit failing routes, so the run must end with zero "unrouted" requests.
const ARCH = process.arch === 'arm64' ? 'aarch64' : 'x86_64';
const WIN = (name, ver) => `${name}-${ver}-${ARCH}-pc-windows-msvc.zip`;
const LINUX = (name, ver) => `${name}-${ver}-x86_64-unknown-linux-gnu.tar.gz`;
// An empty zip (end-of-central-directory record only): a real archive, nothing to run or unpack.
const ZIP = Buffer.concat([Buffer.from([0x50, 0x4b, 0x05, 0x06]), Buffer.alloc(18)]);
const TAR = Buffer.from('not really a tarball (e2e)\n');
const sha = (b) => createHash('sha256').update(b).digest('hex');
const BADGE = '<svg xmlns="http://www.w3.org/2000/svg" width="90" height="20"><rect width="90" height="20" fill="#4c1"/></svg>';
const NET = {
  basicDesc: 'Basic fixture as described on GitHub (canned)',
  demoDesc: 'Demo upstream as described on GitHub (canned)',
  basicWin: WIN('basic', '0.1.0'),
  basicLinux: LINUX('basic', '0.1.0'),
  demoWin: WIN('demo-upstream', '1.1.0'),
  demoLinux: LINUX('demo-upstream', '1.1.0'),
  badge: 'https://img.shields.io/badge/a-b-green',
  avatar: 'https://avatars.githubusercontent.com/u/583231?v=4',
};
// v0.3.0 (M10c): canned GitHub for Explore. `e2e/explored` is browsed online AND saved to the shelf (RS_E2E_GIT_MAP maps
// it to the local upstream), `e2e/limited` answers GitHub's rate-limit 403, `e2e/missing` a 404. The routes follow the
// exact URLs the backend builds (D-124): a cold repository page = repo + resolve + root listing + README + languages.
const X = {
  owner: 'e2e',
  name: 'explored',
  c1: '1'.repeat(40),
  c2: '2'.repeat(40),
  t1: 'a'.repeat(40),
  t2: 'b'.repeat(40),
  bReadme: 'c'.repeat(40),
  bSrc: 'd'.repeat(40),
  bMain: 'e'.repeat(40),
  bBig: 'f'.repeat(40),
  readme: '# Explored online (canned)\n\nHello from **GitHub**, read through the Explore seam.\n',
  main: 'fn main() {\n    println!("online");\n}\n',
  bigSize: 1572864,
  reset: Math.floor(Date.now() / 1000) + 3600,
  deskDesc: 'Explore fixture as described on GitHub (canned)',
};
const b64 = (s) => Buffer.from(s).toString('base64').replace(/(.{60})/g, '$1\n') + '\n';
function exploreRoutes(r) {
  const API = `https://api.github.com/repos/${X.owner}/${X.name}`;
  const hdr = (extra = {}, remaining = 55, limit = 60, resource = 'core') => ({
    'Content-Type': 'application/json; charset=utf-8',
    'X-RateLimit-Limit': String(limit), 'X-RateLimit-Remaining': String(remaining), 'X-RateLimit-Reset': String(X.reset), 'X-RateLimit-Resource': resource,
    ...extra,
  });
  const put = (url, v, extra = {}, status = 200, rem = 55) => (r[url] = { status, body: JSON.stringify(v), headers: hdr(extra, rem) });
  const sig = (date) => ({ name: 'Mona Octocat', email: 'mona@example.com', date });
  const commit = (sha, msg, date, parents, tree) => ({
    sha, node_id: 'C_x',
    commit: { author: sig(date), committer: sig(date), message: msg, tree: { sha: tree, url: 'https://api.github.com/x' }, comment_count: 0 },
    author: { login: 'octocat', id: 1, avatar_url: 'https://avatars.githubusercontent.com/u/1?v=4' },
    parents: parents.map((p) => ({ sha: p, url: 'u', html_url: 'h' })),
  });
  const c2 = commit(X.c2, 'Add main\n\nSecond commit body.', '2023-11-14T22:13:20Z', [X.c1], X.t2);
  const c1 = commit(X.c1, 'Initial', '2023-11-14T22:12:20Z', [], X.t1);
  const repo = (owner, name, desc) => ({
    id: 1, name, full_name: `${owner}/${name}`, private: false,
    owner: { login: owner, id: 9, avatar_url: 'https://avatars.githubusercontent.com/u/9?v=4' },
    html_url: `https://github.com/${owner}/${name}`, description: desc, fork: false,
    created_at: '2020-09-13T12:26:40Z', updated_at: '2023-11-14T22:13:20Z', pushed_at: '2023-11-14T22:13:20Z',
    homepage: null, size: 2048, stargazers_count: 120, language: 'Rust', forks_count: 7, open_issues_count: 3,
    license: { key: 'mit', name: 'MIT License', spdx_id: 'MIT' }, topics: ['explore', 'e2e'], default_branch: 'main', archived: false,
  });
  const entry = (name, path, sha, size, type) => ({ type, size, name, path, sha, html_url: 'h', download_url: null });
  const file = (name, path, sha, text) => ({ ...entry(name, path, sha, Buffer.byteLength(text), 'file'), content: b64(text), encoding: 'base64' });
  const link = '<https://api.github.com/repositories/1/commits?sha=main&per_page=1&page=2>; rel="next", <https://api.github.com/repositories/1/commits?sha=main&per_page=1&page=2>; rel="last"';
  put(API, repo(X.owner, X.name, X.deskDesc), { ETag: '"x-repo-1"' });
  for (const sha of ['main', X.c2]) put(`${API}/commits?sha=${sha}&per_page=1`, [c2], { Link: link });
  put(`${API}/contents?ref=${X.c2}`, [
    entry('README.md', 'README.md', X.bReadme, Buffer.byteLength(X.readme), 'file'),
    entry('big.csv', 'big.csv', X.bBig, X.bigSize, 'file'),
    entry('src', 'src', X.bSrc, 0, 'dir'),
  ]);
  put(`${API}/contents/src?ref=${X.c2}`, [entry('main.rs', 'src/main.rs', X.bMain, Buffer.byteLength(X.main), 'file')]);
  put(`${API}/contents/README.md?ref=${X.c2}`, file('README.md', 'README.md', X.bReadme, X.readme));
  put(`${API}/contents/src/main.rs?ref=${X.c2}`, file('main.rs', 'src/main.rs', X.bMain, X.main));
  // GitHub refuses the plain JSON contents call for a file over 1 MB (403 too_large, not a rate limit): D-126
  put(`${API}/contents/big.csv?ref=${X.c2}`, { message: 'This API returns blobs up to 1 MB in size. The requested blob is too large to fetch via the API, but you can use the Git Data API to request blobs up to 100 MB in size.', errors: [{ resource: 'Blob', field: 'data', code: 'too_large' }] }, {}, 403);
  put(`${API}/commits?sha=${X.c2}&per_page=35&page=1`, [c2, c1]);
  put(`${API}/commits?sha=${X.c2}&per_page=35&page=1&path=src%2Fmain.rs`, [c2]);
  put(`${API}/commits?sha=${X.c2}&per_page=35&page=1&path=big.csv`, [c1]);
  put(`${API}/commits/${X.c2}`, {
    ...c2, stats: { additions: 3, deletions: 0, total: 3 },
    files: [{ sha: X.bMain, filename: 'src/main.rs', status: 'added', additions: 3, deletions: 0, changes: 3, patch: '@@ -0,0 +1,3 @@\n+fn main() {\n+    println!("online");\n+}' }],
  });
  put(`${API}/branches?per_page=100`, [{ name: 'dev', commit: { sha: X.c1, url: 'u' }, protected: false }, { name: 'main', commit: { sha: X.c2, url: 'u' }, protected: true }]);
  put(`${API}/tags?per_page=100`, [{ name: 'v1.0.0', commit: { sha: X.c1, url: 'u' }, zipball_url: 'z', tarball_url: 't' }]);
  put(`${API}/releases?per_page=100`, [
    { tag_name: 'v1.0.0', name: 'Explored 1.0.0', body: '# Notes\n\nOnline release notes.', draft: false, prerelease: false, published_at: '2023-11-14T22:12:20Z', target_commitish: 'main',
      html_url: `https://github.com/${X.owner}/${X.name}/releases/tag/v1.0.0`,
      assets: [{ name: WIN('explored', '1.0.0'), size: 22, download_count: 4, content_type: 'application/zip',
        browser_download_url: `https://github.com/${X.owner}/${X.name}/releases/download/v1.0.0/${WIN('explored', '1.0.0')}` }] },
  ]);
  put(`${API}/languages`, { Rust: 900, Shell: 100 });
  // search: its own bucket (10 a minute without a token)
  const sh = (extra = {}) => ({ ...hdr(extra, 9, 10, 'search') });
  r['https://api.github.com/search/repositories?q=explored&per_page=30&page=1'] = {
    body: JSON.stringify({ total_count: 2, incomplete_results: false, items: [repo(X.owner, X.name, X.deskDesc), repo('other', 'explored-tools', 'Another match (canned)')] }),
    headers: sh(),
  };
  // the rate-limit answer: 403 with the quota used up, resets in an hour
  r['https://api.github.com/repos/e2e/limited'] = { status: 403, body: JSON.stringify({ message: 'API rate limit exceeded for 203.0.113.9.' }), headers: hdr({}, 0) };
  r['https://api.github.com/repos/e2e/missing'] = { status: 404, body: JSON.stringify({ message: 'Not Found' }), headers: hdr({}, 55) };
  // after saving a token: the free quota probe (5,000 an hour)
  r['https://api.github.com/rate_limit'] = { body: JSON.stringify({ resources: { core: { limit: 5000, remaining: 4999, reset: X.reset } } }), headers: hdr({}, 4999, 5000) };
}
function routes() {
  const r = {};
  const json = (url, v, etag) => (r[url] = { body: JSON.stringify(v), headers: { 'Content-Type': 'application/json; charset=utf-8', ...(etag ? { ETag: etag } : {}) } });
  const bin = (url, buf, type) => (r[url] = { bodyHex: buf.toString('hex'), headers: { 'Content-Type': type } });
  const dl = (o, n, tag, file) => `https://github.com/${o}/${n}/releases/download/${tag}/${file}`;
  const asset = (o, n, tag, file, buf, type, digest = true) => ({
    name: file, size: buf.length, download_count: 7, content_type: type, browser_download_url: dl(o, n, tag, file),
    ...(digest ? { digest: 'sha256:' + sha(buf) } : {}),
  });
  json('https://api.github.com/repos/example/basic', { description: NET.basicDesc, homepage: 'https://example.com/basic', topics: ['fixture', 'e2e'], html_url: 'https://github.com/example/basic' }, '"basic-info-1"');
  json('https://api.github.com/repos/example/basic/releases?per_page=100', [{
    tag_name: 'v0.1.0', name: 'Basic 0.1.0 (GitHub)', draft: false, prerelease: false, html_url: 'https://github.com/example/basic/releases/tag/v0.1.0',
    body: `Notes from GitHub.\n\nThanks to <a href="https://github.com/octo-dev"><img src="${NET.avatar}" alt="@octo-dev" width="20"></a> for the fix.\n`,
    assets: [
      asset('example', 'basic', 'v0.1.0', NET.basicWin, ZIP, 'application/zip'),
      asset('example', 'basic', 'v0.1.0', NET.basicLinux, TAR, 'application/gzip'),
      asset('example', 'basic', 'v0.1.0', 'SHA256SUMS', Buffer.from('x\n'), 'text/plain', false),
    ],
  }], '"basic-rel-1"');
  // Windows zip via GitHub's redirect to its CDN (every hop re-checked by the production Net); the tarball direct.
  r[dl('example', 'basic', 'v0.1.0', NET.basicWin)] = { status: 302, headers: { Location: 'https://objects.githubusercontent.com/e2e/basic-win.zip?sig=1' } };
  bin('https://objects.githubusercontent.com/e2e/basic-win.zip?sig=1', ZIP, 'application/octet-stream');
  bin(dl('example', 'basic', 'v0.1.0', NET.basicLinux), TAR, 'application/octet-stream');
  json('https://api.github.com/repos/e2e/demo-upstream', { description: NET.demoDesc, homepage: null, topics: ['demo'], html_url: 'https://github.com/e2e/demo-upstream' });
  json('https://api.github.com/repos/e2e/demo-upstream/releases?per_page=100', [
    { tag_name: 'v1.1.0', name: 'Demo 1.1.0', draft: false, prerelease: false, html_url: 'https://github.com/e2e/demo-upstream/releases/tag/v1.1.0', body: 'Second release.',
      assets: [asset('e2e', 'demo-upstream', 'v1.1.0', NET.demoWin, ZIP, 'application/zip'), asset('e2e', 'demo-upstream', 'v1.1.0', NET.demoLinux, TAR, 'application/gzip')] },
    { tag_name: 'v1.0.0', name: 'Demo 1.0.0', draft: false, prerelease: false, body: 'First release.', assets: [] },
  ]);
  bin(dl('e2e', 'demo-upstream', 'v1.1.0', NET.demoWin), ZIP, 'application/octet-stream');
  bin(dl('e2e', 'demo-upstream', 'v1.1.0', NET.demoLinux), TAR, 'application/octet-stream');
  // README images: a badge that loads, and the expected failures (fall back to the chip).
  r[NET.badge] = { body: BADGE, headers: { 'Content-Type': 'image/svg+xml;charset=utf-8' } };
  r['https://fail.example.com/missing.png'] = { status: 404, headers: { 'Content-Type': 'text/html' }, body: 'not found' };
  r['https://images.example.com/tracker.png'] = { status: 404, headers: { 'Content-Type': 'text/html' }, body: 'not found' };
  exploreRoutes(r);
  return r;
}
if (cfg.netRoutes) writeFileSync(cfg.netRoutes, JSON.stringify({ routes: routes() }, null, 2));
const netLog = () => (cfg.netLog && existsSync(cfg.netLog) ? readFileSync(cfg.netLog, 'utf8').split(/\r?\n/).filter(Boolean) : []);
const releaseFile = (o, n, tag, file) => join(cfg.archive, '_releases', 'github.com', o, n, tag, file);
const waitFile = async (p, ms = 30000) => {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    if (existsSync(p)) return statSync(p).size;
    await new Promise((r) => setTimeout(r, 200));
  }
  return null;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
const save = () => writeFileSync(cfg.result, JSON.stringify(results, null, 2));
mkdirSync(cfg.shots, { recursive: true });

const wd = async (method, path, body) => {
  const r = await fetch(`http://127.0.0.1:${cfg.port}${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  });
  return r.json();
};

// Wall-clock start (epoch ms) of the e2e exe's newest process, from Windows (CreationDate).
function processStartEpoch() {
  const exe = cfg.app.replace(/'/g, "''");
  const ps =
    `$p = Get-CimInstance Win32_Process -Filter "Name='reposhelf.exe'" | Where-Object { $_.ExecutablePath -eq '${exe}' } | ` +
    `Sort-Object CreationDate | Select-Object -Last 1; if ($p) { [DateTimeOffset]::new($p.CreationDate).ToUnixTimeMilliseconds() }`;
  const out = execFileSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', ps], { encoding: 'utf8' }).trim();
  return out ? Number(out) : null;
}

async function newSession() {
  for (let i = 0; i < 60; i++) {
    try {
      const s = await wd('POST', '/session', {
        capabilities: { alwaysMatch: { browserName: 'wry', 'tauri:options': { application: cfg.app } } },
      });
      const id = s.value?.sessionId;
      if (id) {
        await wd('POST', `/session/${id}/timeouts`, { script: 300000 });
        return id;
      }
      if (i === 59) throw new Error(JSON.stringify(s).slice(0, 400));
    } catch (e) {
      if (i === 59) throw e;
    }
    await sleep(500);
  }
  throw new Error('no session');
}

// Helpers available inside every evaluated snippet.
const HELPERS = `
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const until = async (f, ms = 15000, step = 50) => { const t0 = Date.now(); while (Date.now() - t0 < ms) { const v = f(); if (v) return v; await sleep(step); } return null; };
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const txt = (el) => (el ? el.innerText.replace(/\\s+/g, ' ').trim() : null);
  const byText = (sel, t, r = document) => $$(sel, r).find((e) => e.innerText.replace(/\\s+/g, ' ').trim().includes(t));
  const dialog = () => $('.dialog[role="dialog"]');
  const go = async (hash) => { if (location.hash !== hash) location.hash = hash; await sleep(80); };
  const settle = async (ms = 15000) => { await until(() => !$('.shimmer, [aria-busy="true"]'), ms); await sleep(300); };
  const setValue = (el, v) => { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(el, v); el.dispatchEvent(new Event('input', { bubbles: true })); };
  const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
  const need = (v, what) => { if (!v) throw new Error('missing: ' + what + ' (at ' + location.hash + ')'); return v; };
  // v0.3.0: every link of an Explore page body stays under #/explore/ (except Open in shelf and Settings)
  const badLinks = () => $$('main a[href^="#"]').filter((a) => a.dataset.testid !== 'explore-open-in-shelf').map((a) => a.getAttribute('href')).filter((h) => !h.startsWith('#/explore/') && h !== '#/settings' && !h.startsWith('#user-content-'));
  const quotaText = () => txt($('[data-testid="explore-quota-remaining"]'));
  const setTheme = async (want) => {
    for (let i = 0; i < 4; i++) {
      const b = need($('button[aria-label^="Theme:"]'), 'theme toggle');
      if (b.getAttribute('aria-label').startsWith('Theme: ' + want)) break;
      b.click();
      await sleep(200);
    }
    return document.documentElement.dataset.theme;
  };
  const menuItem = async (trigger, label) => { need(trigger, 'menu trigger for ' + label).click(); const it = need(await until(() => byText('[role="menuitem"]', label), 3000), 'menu item ' + label); it.click(); };
`;

async function exec(session, body) {
  const r = await wd('POST', `/session/${session}/execute/sync`, {
    script: `return (async () => { ${HELPERS} try { ${body} } catch (e) { return { error: String(e && e.stack || e).slice(0, 800) }; } })();`,
    args: [],
  });
  if (r.value && r.value.error && typeof r.value.error === 'string' && r.value.message) return { error: `${r.value.error}: ${r.value.message}`.slice(0, 800) };
  return r.value ?? {};
}

async function step(session, name, body) {
  const t0 = Date.now();
  let v;
  try {
    v = await exec(session, body);
  } catch (e) {
    v = { error: String(e) };
  }
  const row = { step: name, ms: Date.now() - t0, ...(v && typeof v === 'object' ? v : { value: v }) };
  results.push(row);
  save();
  console.log(`${row.error ? 'FAIL' : 'ok  '} ${name}${row.error ? ': ' + String(row.error).split('\n')[0] : ''}`);
  return row;
}

// Node-side check recorded like a step.
function record(name, data, error) {
  const row = { step: name, ...data, ...(error ? { error } : {}) };
  results.push(row);
  save();
  console.log(`${error ? 'FAIL' : 'ok  '} ${name}${error ? ': ' + error : ''}`);
  return row;
}

async function shot(session, name) {
  const r = await wd('GET', `/session/${session}/screenshot`);
  if (typeof r.value !== 'string') throw new Error('screenshot failed: ' + JSON.stringify(r).slice(0, 200));
  const file = join(cfg.shots, `${name}.png`);
  writeFileSync(file, Buffer.from(r.value, 'base64'));
  return file;
}

// Collector for console errors/warnings, CSP violations, uncaught errors and alert() calls. Installed before any app
// script through CDP when the driver allows it (then the page is reloaded), otherwise injected into the live page.
const COLLECTOR = `(() => {
  if (window.__e2e) return;
  const c = (window.__e2e = { csp: [], errors: [], warns: [], alerts: [] });
  document.addEventListener('securitypolicyviolation', (e) => c.csp.push({ dir: e.violatedDirective, uri: String(e.blockedURI).slice(0, 200), src: String(e.sourceFile || '').slice(0, 200), sample: String(e.sample || '').slice(0, 80) }));
  window.addEventListener('error', (e) => c.errors.push('error: ' + String(e.message).slice(0, 300)));
  window.addEventListener('unhandledrejection', (e) => c.errors.push('rejection: ' + String(e.reason && (e.reason.message || JSON.stringify(e.reason)) || e.reason).slice(0, 300)));
  const oe = console.error.bind(console), ow = console.warn.bind(console);
  console.error = (...a) => { c.errors.push('console.error: ' + a.map(String).join(' ').slice(0, 300)); oe(...a); };
  console.warn = (...a) => { c.warns.push('console.warn: ' + a.map(String).join(' ').slice(0, 300)); ow(...a); };
  window.alert = (m) => { c.alerts.push(String(m)); };
  // The default buffer (250) would drop entries during the screens pass; drain() also clears it.
  try { performance.setResourceTimingBufferSize(10000); } catch {}
})();`;

async function installCollector(session) {
  const cdp = await wd('POST', `/session/${session}/ms/cdp/execute`, {
    cmd: 'Page.addScriptToEvaluateOnNewDocument',
    params: { source: COLLECTOR },
  }).catch((e) => ({ value: { error: String(e) } }));
  if (cdp.value && !cdp.value.error) {
    await exec(session, `location.reload(); return {};`).catch(() => {});
    await sleep(1500);
    const v = await exec(session, `await until(() => window.__e2e && document.querySelector('.gh'), 20000); return { installed: !!window.__e2e };`);
    return { mode: 'cdp', ...v };
  }
  await exec(session, `${COLLECTOR}; return {};`);
  return { mode: 'page', cdpError: JSON.stringify(cdp.value ?? cdp).slice(0, 200) };
}

// Everything the collector saw, over every document of session B (a reload starts a fresh `window.__e2e` and a fresh
// resource-timing list, so the page is drained into these before each reload and at the end).
const acc = { csp: [], errors: [], warns: [], alerts: [], resources: new Set(), drains: [] };

async function drain(session, label) {
  const v = await exec(session, `
    const c = window.__e2e;
    if (!c) return { missing: true };
    const res = performance.getEntriesByType('resource').map((e) => e.name);
    performance.clearResourceTimings();
    return { csp: c.csp.splice(0), errors: c.errors.splice(0), warns: c.warns.splice(0), alerts: c.alerts.splice(0), res };
  `);
  const tag = (xs) => (xs ?? []).map((x) => ({ at: label, ...(typeof x === 'string' ? { msg: x } : x) }));
  acc.csp.push(...tag(v.csp));
  acc.errors.push(...tag(v.errors));
  acc.warns.push(...tag(v.warns));
  acc.alerts.push(...tag(v.alerts));
  for (const r of v.res ?? []) acc.resources.add(r);
  const row = { label, missing: !!v.missing, csp: v.csp?.length ?? 0, errors: v.errors?.length ?? 0, warns: v.warns?.length ?? 0, resources: v.res?.length ?? 0 };
  acc.drains.push(row);
  record(`drain ${label}`, row, v.error ? String(v.error) : v.missing ? 'collector missing in this document' : undefined);
}

// ---------------------------------------------------------------------------------------------------------------

const START_STEP = `
  const rows = await until(() => performance.getEntriesByName('rs:dashboard-rows', 'mark')[0], 30000, 20);
  if (!rows) return { error: 'dashboard rows never rendered (no rs:dashboard-rows mark)' };
  await sleep(300);
  return {
    rowsEpoch: performance.timeOrigin + rows.startTime,
    sinceNavigationMs: Math.round(rows.startTime),
    rowCount: $$('.repo-row').length,
    title: document.title,
    theme: document.documentElement.dataset.theme,
    origin: location.origin,
    // F1: nothing to go back or forward to in a fresh window
    backDisabled: $('button[aria-label^="Back"]')?.disabled ?? null,
    forwardDisabled: $('button[aria-label^="Forward"]')?.disabled ?? null,
  };
`;

async function startTiming(session, name) {
  const row = await step(session, name, START_STEP);
  const proc = processStartEpoch();
  if (proc && row.rowsEpoch) {
    row.processStartToRowsMs = Math.round(row.rowsEpoch - proc);
    save();
  }
  return row;
}

// Screenshot of the current page in both themes (the theme is Light again afterwards).
async function themedShot(session, id, prep = '') {
  for (const theme of ['Light', 'Dark']) {
    await exec(session, `await setTheme(${JSON.stringify(theme)}); const jb = $('button[aria-label^="Background jobs"]'); if (jb && jb.getAttribute('aria-expanded') === 'true') jb.click(); window.scrollTo(0, 0); ${prep} await sleep(400); return {};`);
    try {
      await shot(session, `${id}-${theme.toLowerCase()}`);
    } catch (e) {
      record(`shot ${id} ${theme}`, {}, String(e).slice(0, 200));
    }
  }
  await exec(session, `await setTheme('Light'); return {};`);
}

// v0.3.0 Explore, M10c. Every answer is canned (`exploreRoutes`); `e2e/explored` is browsed online and then saved to the
// shelf through the git map. The token is a fake: it must not appear in the page, the backend's answers, the DB or any log.
async function exploreFlows(s) {
  const T = cfg.token;
  const api = (from) => netLog().slice(from).filter((l) => (l.split('\t')[1] ?? '').startsWith('https://api.github.com/'));
  const urls = (lines) => lines.map((l) => l.split('\t')[1]);
  const hasAuth = (l) => l.split('\t')[3] === 'auth';
  const N = (re) => netLog().filter((l) => re.test(l.split('\t')[1] ?? ''));
  const o0 = netLog().length;

  // Settings → Explore defaults (the Settings checks know exploreEnabled / githubTokenSaved since v0.3.0)
  await step(s, 'X0 Explore settings defaults', `
    await go('#/settings');
    const box = need(await until(() => $('[data-testid="explore-settings"]'), 10000), 'Explore settings');
    await sleep(300);
    const st = await invoke('settings_get', {});
    const status = await invoke('explore_status', {});
    const input = $('[data-testid="explore-token-input"]');
    const r = {
      enabledBox: $('[data-testid="explore-enabled"]')?.checked ?? null,
      exploreEnabled: st.exploreEnabled, githubTokenSaved: st.githubTokenSaved,
      statusEnabled: status.enabled, statusTokenSaved: status.tokenSaved, statusAvailable: status.available, limitedUntil: status.limitedUntil, cacheEntries: status.cacheEntries,
      inputType: input?.type ?? null, savedShown: !!$('[data-testid="explore-token-saved"]'), nav: !!$('[data-testid="nav-explore"]'),
      keys: ['exploreEnabled', 'githubTokenSaved'].filter((k) => k in st),
    };
    const ok = r.enabledBox === true && r.exploreEnabled === true && r.githubTokenSaved === false && r.statusEnabled && !r.statusTokenSaved && r.statusAvailable
      && r.limitedUntil == null && r.inputType === 'password' && !r.savedShown && r.nav && r.keys.length === 2;
    return { ...r, ...(ok ? {} : { error: 'Explore defaults wrong: ' + JSON.stringify(r) }) };
  `);

  // search → open
  const s0 = netLog().length;
  await step(s, 'X1 Explore: nav entry, search on submit, results, quota', `
    await go('#/');
    await until(() => $('.repo-row'));
    need($('[data-testid="nav-explore"]'), 'Explore nav entry').click();
    await until(() => location.hash.startsWith('#/explore'), 5000);
    const input = need(await until(() => $('[data-testid="explore-search-input"]'), 10000), 'search input');
    setValue(input, 'explored');
    await sleep(400);
    const before = !!$('[data-testid="explore-results"]');
    need($('[data-testid="explore-search-form"]'), 'search form').requestSubmit();
    need(await until(() => $('[data-testid="explore-results"]'), 15000), 'results');
    await settle();
    const rows = $$('[data-testid="explore-result"]');
    const names = rows.map((r) => r.dataset.repo);
    const row = need(rows.find((r) => r.dataset.repo === 'e2e/explored'), 'result e2e/explored');
    const r = {
      route: location.hash, resultsBeforeSubmit: before, names, quota: quotaText(),
      rowText: txt(row).slice(0, 160), hasSave: !!$('[data-testid="explore-save"]', row), open: $('[data-testid="explore-open"]', row)?.getAttribute('href') ?? null,
      bad: badLinks(),
    };
    const ok = !before && names.length === 2 && /9 of 10 searches left/.test(r.quota ?? '') && r.hasSave && r.open === '#/explore/e2e/explored' && !r.bad.length && /Rust|★|120/.test(r.rowText);
    return { ...r, ...(ok ? {} : { error: 'search page wrong: ' + JSON.stringify(r) }) };
  `);
  await themedShot(s, 'm10c-explore-search');
  {
    const search = netLog().slice(s0).filter((l) => l.includes('/search/repositories'));
    record('X1 one search request (on submit)', { search }, search.length === 1 && search[0].endsWith('\t200') ? undefined : `expected one canned search request, got ${search.length}`);
  }

  // the repository page
  const c0 = netLog().length;
  await step(s, 'X2 Explore repository page (cold): files, README, About, quota, links', `
    need($('[data-testid="explore-result"][data-repo="e2e/explored"] [data-testid="explore-open"]'), 'Open').click();
    await until(() => location.hash === '#/explore/e2e/explored', 5000);
    need(await until(() => $('[data-testid="explore-header"]'), 15000), 'explore header');
    await until(() => $$('[data-testid="file-table"] [data-kind]').length >= 3, 15000);
    const md = need(await until(() => $('.markdown'), 15000), 'README');
    await until(() => /Hello from GitHub/.test(md.innerText), 10000);
    await settle();
    const r = {
      header: txt($('[data-testid="explore-header"]'))?.slice(0, 120), names: $$('[data-testid="file-table"] [data-kind]').map((e) => txt(e)?.slice(0, 40)),
      readme: txt(md).slice(0, 100), about: txt($('[data-testid="explore-about"]'))?.slice(0, 140) ?? null, quota: quotaText(),
      save: !!$('[data-testid="explore-save"]'), hint: !!$('[data-testid="explore-files-hint"]'), bad: badLinks(),
      tokenHint: !!byText('.xbar a', 'Add a token'),
    };
    const ok = r.header.includes('e2e') && r.names.length === 3 && /Hello from GitHub/.test(r.readme) && r.about?.includes(${JSON.stringify(X.deskDesc)}) && /55 of 60/.test(r.quota ?? '')
      && r.save && r.hint && !r.bad.length && r.tokenHint;
    return { ...r, ...(ok ? {} : { error: 'repository page wrong: ' + JSON.stringify(r) }) };
  `);
  {
    const lines = api(c0);
    const P = '/repos/e2e/explored';
    const want = [P, `${P}/commits?sha=main&per_page=1`, `${P}/contents?ref=${X.c2}`, `${P}/contents/README.md?ref=${X.c2}`, `${P}/languages`];
    const got = urls(lines).map((u) => u.replace('https://api.github.com', ''));
    const missing = want.filter((w) => !got.includes(w));
    record('X2 a cold repository page costs 5 requests (D-124)', { got }, got.length !== 5 || missing.length ? `expected ${want.join(', ')}; got ${got.join(', ')}` : lines.some(hasAuth) ? 'anonymous request carried Authorization' : undefined);
  }
  await themedShot(s, 'm10c-explore-code');

  await step(s, 'X3 Explore: folder, file, history, commit diff, branches, tags, releases', `
    const out = {};
    await go('#/explore/e2e/explored/tree/main/src');
    await until(() => byText('[data-testid="file-table"] [data-kind]', 'main.rs'), 15000);
    out.tree = { rows: $$('[data-testid="file-table"] [data-kind]').map((e) => txt(e)?.slice(0, 30)), crumbs: txt($('.crumbs'))?.slice(0, 60), bad: badLinks() };
    await go('#/explore/e2e/explored/blob/main/src/main.rs');
    const fb = need(await until(() => $('.filebox'), 15000), 'file box');
    await until(() => /println/.test(fb.innerText), 10000);
    out.file = { text: /println/.test(fb.innerText), saveCopy: /Save a copy/.test(document.body.innerText), bad: badLinks() };
    await go('#/explore/e2e/explored/commits/main');
    await until(() => $$('.crow').length >= 2, 15000);
    await settle();
    out.commits = { rows: $$('.crow').length, first: txt($('.crow a.crow__summary')), bad: badLinks() };
    need($('.crow a.crow__summary'), 'first commit link').click();
    const card = need(await until(() => $('.dcard'), 15000), 'diff card');
    await until(() => /println/.test(card.innerText), 10000);
    out.commit = { head: !!$('.chead'), diff: /println/.test(card.innerText), route: location.hash, bad: badLinks() };
    await go('#/explore/e2e/explored/branches');
    await until(() => $$('[data-testid="online-branches"] .brow').length >= 2, 15000);
    out.branches = { names: $$('[data-testid="online-branches"] .brow').map((e) => e.dataset.branch ?? txt(e)?.slice(0, 20)), bad: badLinks() };
    await go('#/explore/e2e/explored/tags');
    await until(() => $$('[data-testid="online-tags"] .trow').length >= 1, 15000);
    out.tags = { rows: $$('[data-testid="online-tags"] .trow').length, bad: badLinks() };
    await go('#/explore/e2e/explored/releases');
    const rc = need(await until(() => $('.rcard'), 15000), 'release card');
    await settle();
    out.releases = { cards: $$('.rcard').length, assets: $$('.rasset').length, download: $$('.rasset button').some((b) => /^Download/.test(b.getAttribute('aria-label') || txt(b))), notes: /Online release notes/.test(rc.innerText), bad: badLinks() };
    const bad = [].concat(...Object.values(out).map((v) => v.bad ?? []));
    const ok = out.tree.rows.length === 1 && out.file.text && !out.file.saveCopy && out.commits.rows === 2 && out.commit.head && out.commit.diff && out.branches.names.length === 2
      && out.tags.rows === 1 && out.releases.cards === 1 && out.releases.assets >= 1 && !out.releases.download && out.releases.notes && !bad.length;
    return { ...out, ...(ok ? {} : { error: 'online browsing wrong: ' + JSON.stringify(out).slice(0, 600) }) };
  `);

  const big0 = netLog().length;
  await step(s, 'X4 Explore: file over 1 MB = Too large to preview online', `
    await go('#/explore/e2e/explored/blob/main/big.csv');
    const ph = need(await until(() => $('[data-placeholder="large"]'), 15000), 'too-large placeholder');
    await settle();
    const r = { text: txt(ph)?.slice(0, 120), save: !!$('[data-testid="explore-save"]', ph), content: !!$('.filebox pre, .filebox code') };
    return { ...r, ...(r.text?.includes('Too large to preview online — Save to shelf') && r.save && !r.content ? {} : { error: 'placeholder wrong: ' + JSON.stringify(r) }) };
  `);
  {
    const big = netLog().slice(big0).filter((l) => l.includes('/contents/big.csv'));
    record('X4 the contents call of the big file was refused by GitHub (403 too_large), nothing else fetched', { big },
      big.length === 1 && big[0].includes('\t403') ? undefined : `expected one 403 contents request, got ${JSON.stringify(big)}`);
  }
  await themedShot(s, 'm10c-explore-toolarge');

  await step(s, 'X5 Explore: unknown repository (404) says so with the token hint', `
    await go('#/explore/e2e/missing');
    const err = need(await until(() => $('[data-testid="explore-error"]'), 15000), 'error state');
    const r = { code: err.dataset.code, text: txt(err)?.slice(0, 200), limit: !!$('[data-testid="explore-rate-limit"]') };
    return { ...r, ...(r.code === 'notFound' && /token/i.test(r.text ?? '') && !r.limit ? {} : { error: 'not-found state wrong: ' + JSON.stringify(r) }) };
  `);

  // Save to shelf → Open in shelf (the existing clone job through the git map; split-button option "without downloading")
  const sv0 = netLog().length;
  await step(s, 'X6 Explore: Save to shelf (without release) -> Open in shelf', `
    await go('#/explore/e2e/explored');
    await until(() => $('[data-testid="explore-save"]'), 15000);
    await settle();
    await menuItem(need($('button[aria-label="Save options"]'), 'Save options'), 'Save without downloading');
    const sawProgress = !!(await until(() => $('[data-testid="explore-saving"]') || $('[data-testid="explore-saved"]'), 15000));
    const saved = need(await until(() => $('[data-testid="explore-saved"]'), 60000), 'On your shelf state');
    const open = need($('[data-testid="explore-open-in-shelf"]', saved), 'Open in shelf');
    const href = open.getAttribute('href');
    open.click();
    await until(() => location.hash === '#/e2e/explored' && $('.rh__title'), 15000);
    await settle();
    const lib = (await invoke('library_list', {})).filter((r) => r.owner === 'e2e' && r.name === 'explored');
    const r = { sawProgress, href, route: location.hash, onShelf: lib.length, files: $$('[data-testid="file-table"] .ftable__row').length, notExplore: !$('[data-testid="explore-header"]') };
    return { ...r, ...(href === '#/e2e/explored' && r.route === '#/e2e/explored' && r.onShelf === 1 && r.notExplore ? {} : { error: 'save/open wrong: ' + JSON.stringify(r) }) };
  `);
  {
    const log = netLog().slice(sv0);
    const dl = log.filter((l) => l.includes('/releases/download/'));
    record('X6 saving without the release option downloaded nothing', { lines: log.map((l) => l.split('\t').slice(0, 3).join(' ')) }, dl.length ? 'a release was downloaded: ' + dl.join(', ') : undefined);
  }

  // rate limit: the 403 with the quota used up; saved pages keep opening
  const rl0 = netLog().length;
  await step(s, 'X7 Explore: rate limit reached (403, remaining 0) -> state with reset time', `
    await go('#/explore/e2e/limited');
    // the page that failed has no strip: the full-page state says when the limit resets
    const err = need(await until(() => $('[data-testid="explore-error"]'), 15000), 'rate-limit state');
    await sleep(300);
    const status = await invoke('explore_status', {});
    const r = { text: txt(err)?.slice(0, 200), code: err.dataset.code ?? null, limitedUntil: status.limitedUntil };
    const ok = /rate limit reached.*Resets at \\d{1,2}[:.]\\d{2}/i.test(r.text ?? '') && r.code === 'rateLimited' && r.limitedUntil > Date.now() / 1000;
    return { ...r, ...(ok ? {} : { error: 'rate-limit state wrong: ' + JSON.stringify(r) }) };
  `);
  await themedShot(s, 'm10c-explore-ratelimit');
  {
    const before = netLog().length;
    await step(s, 'X7 Explore: while limited, a saved page still opens with the banner (no request)', `
      await go('#/explore/e2e/explored');
      await until(() => $('[data-testid="explore-header"]'), 15000);
      await until(() => $$('[data-testid="file-table"] [data-kind]').length >= 3, 15000);
      await settle();
      const bn = $('[data-testid="explore-rate-limit"]');
      const r = { banner: !!bn, bannerText: txt(bn)?.slice(0, 160) ?? null, error: !!$('[data-testid="explore-error"]'), rows: $$('[data-testid="file-table"] [data-kind]').length, readme: !!$('.markdown') };
      return { ...r, ...(r.banner && /Rate limit reached.*resets at \\d{1,2}[:.]\\d{2}/.test(r.bannerText ?? '') && !r.error && r.rows === 3 && r.readme ? {} : { error: 'saved page while limited: ' + JSON.stringify(r) }) };
    `);
    await themedShot(s, 'm10c-explore-banner');
    const lines = netLog().slice(before);
    record('X7 only the limited call reached GitHub (one request while limited)', { sinceLimit: netLog().slice(rl0).map((l) => l.split('\t').slice(0, 3).join(' ')), after: lines.length },
      netLog().slice(rl0).filter((l) => l.includes('/repos/e2e/limited')).length === 1 && lines.length === 0 ? undefined : 'unexpected requests while limited');
  }

  // token: save (fake in-memory store), never shown, request now carries Authorization, identity reset, Remove
  const tk0 = netLog().length;
  await step(s, 'X8 Explore token: save -> Token saved (check), field gone, the secret nowhere in the page', `
    const T = ${JSON.stringify(T)};
    await go('#/settings');
    const input = need(await until(() => $('[data-testid="explore-token-input"]'), 10000), 'token input');
    const before = { limited: (await invoke('explore_status', {})).limitedUntil };
    setValue(input, T);
    await sleep(200);
    const saveBtn = need($('[data-testid="explore-token-save"]'), 'Save token');
    const enabledBeforeClick = !saveBtn.disabled;
    saveBtn.click();
    const saved = need(await until(() => $('[data-testid="explore-token-saved"]'), 10000), 'Token saved state');
    await sleep(500);
    const st = await invoke('settings_get', {});
    const status = await invoke('explore_status', {});
    const hay = {
      html: document.documentElement.outerHTML, text: document.body.innerText, href: location.href, state: JSON.stringify(history.state),
      local: JSON.stringify({ ...localStorage }), session: JSON.stringify({ ...sessionStorage }), inputs: $$('input, textarea').map((i) => i.value).join('|'),
      settings: JSON.stringify(st), status: JSON.stringify(status), msg: txt($('[data-testid="explore-token-msg"]')) ?? '',
    };
    const leaks = Object.entries(hay).filter(([, v]) => v.includes(T)).map(([k]) => k);
    const r = {
      savedText: txt(saved)?.slice(0, 60), inputGone: !$('[data-testid="explore-token-input"]'), removeShown: !!$('[data-testid="explore-token-remove"]'), enabledBeforeClick,
      settingsSaved: st.githubTokenSaved, statusSaved: status.tokenSaved, limitedBefore: before.limited != null, limitedAfter: status.limitedUntil, core: status.core ? status.core.remaining + '/' + status.core.limit : null,
      leaks, msg: hay.msg.slice(0, 80),
    };
    const ok = /Token saved/.test(r.savedText ?? '') && r.inputGone && r.removeShown && r.enabledBeforeClick && r.settingsSaved === true && r.statusSaved === true
      && r.limitedBefore && r.limitedAfter == null && r.core === '4999/5000' && !leaks.length;
    return { ...r, ...(ok ? {} : { error: 'token save wrong: ' + JSON.stringify(r) }) };
  `);
  await themedShot(s, 'm10c-explore-settings', "document.querySelector('[data-testid=\"explore-settings\"]').scrollIntoView({ block: 'start' });");
  await step(s, 'X8 Explore token: with a token the page loads again (limit state reset), quota strip says token in use', `
    await go('#/explore/e2e/explored');
    await until(() => $('[data-testid="explore-header"]'), 15000);
    await until(() => $$('[data-testid="file-table"] [data-kind]').length >= 3, 15000);
    await settle();
    const bar = txt($('[data-testid="explore-quota"]')) ?? '';
    const r = { bar: bar.slice(0, 160), limited: !!$('[data-testid="explore-rate-limit"]'), error: !!$('[data-testid="explore-error"]'), rows: $$('[data-testid="file-table"] [data-kind]').length, tokenInUse: /token in use/.test(bar), text: document.body.innerText.includes(${JSON.stringify(T)}) };
    return { ...r, ...(r.tokenInUse && !r.limited && !r.error && r.rows === 3 && !r.text ? {} : { error: 'page with token wrong: ' + JSON.stringify(r) }) };
  `);
  {
    const lines = api(tk0);
    const probe = lines.filter((l) => l.split('\t')[1] === 'https://api.github.com/rate_limit');
    const repo = lines.filter((l) => l.split('\t')[1] === 'https://api.github.com/repos/e2e/explored');
    record('X8 after saving, GitHub requests carry Authorization (only to api.github.com)', { probe: probe.length, repo: repo.length, requests: lines.length },
      probe.length !== 1 || !hasAuth(probe[0]) ? 'the /rate_limit probe was not sent with the token' : !repo.length || !repo.every(hasAuth) ? 'the repo request after the token was not authenticated'
        : netLog().slice(tk0).some((l) => hasAuth(l) && !(l.split('\t')[1] ?? '').startsWith('https://api.github.com/')) ? 'Authorization sent to another host' : undefined);
  }
  const rm0 = netLog().length;
  await step(s, 'X8 Explore token: Remove brings the field back, the saved flag is false', `
    await go('#/settings');
    need(await until(() => $('[data-testid="explore-token-remove"]'), 10000), 'Remove').click();
    const input = need(await until(() => $('[data-testid="explore-token-input"]'), 10000), 'token input again');
    await sleep(400);
    const st = await invoke('settings_get', {});
    const status = await invoke('explore_status', {});
    const r = { empty: input.value === '', savedGone: !$('[data-testid="explore-token-saved"]'), settingsSaved: st.githubTokenSaved, statusSaved: status.tokenSaved, msg: txt($('[data-testid="explore-token-msg"]'))?.slice(0, 60), cacheEntries: status.cacheEntries };
    return { ...r, ...(r.empty && r.savedGone && r.settingsSaved === false && r.statusSaved === false && r.cacheEntries === 0 ? {} : { error: 'token remove wrong: ' + JSON.stringify(r) }) };
  `);
  await step(s, 'X8 Explore token: after Remove, requests are anonymous again', `
    await go('#/explore/e2e/explored');
    await until(() => $$('[data-testid="file-table"] [data-kind]').length >= 3, 15000);
    await settle();
    return { bar: txt($('[data-testid="explore-quota"]'))?.slice(0, 120) };
  `);
  {
    const lines = api(rm0);
    record('X8 after Remove, requests carry no Authorization', { requests: lines.length }, !lines.length ? 'no request after Remove (the cache should have been cleared)' : lines.some(hasAuth) ? 'a request after Remove was authenticated' : undefined);
  }

  // Explore off / on
  const off0 = netLog().length;
  await step(s, 'X9 Settings -> Explore off: nav entry gone, pages say so, no request; back on', `
    await go('#/settings');
    const cb = need(await until(() => $('[data-testid="explore-enabled"]'), 10000), 'enable box');
    cb.click();
    await until(() => !$('[data-testid="nav-explore"]'), 5000);
    await sleep(300);
    const off = await invoke('settings_get', {});
    await go('#/explore/e2e/explored');
    const msg = need(await until(() => $('[data-testid="explore-off"]'), 10000), 'explore-off page');
    await go('#/explore/search?q=explored');
    await until(() => $('[data-testid="explore-off"]'), 5000);
    await go('#/settings');
    need(await until(() => $('[data-testid="explore-enabled"]'), 10000), 'enable box').click();
    await until(() => $('[data-testid="nav-explore"]'), 5000);
    await sleep(300);
    const on = await invoke('settings_get', {});
    const r = { offSaved: off.exploreEnabled, onSaved: on.exploreEnabled, offText: txt(msg)?.slice(0, 100), navBack: !!$('[data-testid="nav-explore"]') };
    return { ...r, ...(r.offSaved === false && r.onSaved === true && r.navBack ? {} : { error: 'off/on wrong: ' + JSON.stringify(r) }) };
  `);
  record('X9 Explore off made no GitHub request', { requests: netLog().slice(off0).length }, netLog().slice(off0).length ? 'requests while Explore was off: ' + netLog().slice(off0).join(' | ') : undefined);

  // Clear Explore cache
  await step(s, 'X10 Settings -> Clear Explore cache empties the saved answers', `
    await go('#/explore/e2e/explored');
    await until(() => $$('[data-testid="file-table"] [data-kind]').length >= 3, 15000);
    await settle();
    await go('#/settings');
    const size = need(await until(() => $('[data-testid="explore-cache-size"]'), 10000), 'cache size');
    await sleep(400);
    const before = txt(size);
    need($('[data-testid="explore-cache-clear"]'), 'Clear Explore cache').click();
    const msg = need(await until(() => $('[data-testid="explore-cache-msg"]'), 10000), 'cleared message');
    await sleep(300);
    const status = await invoke('explore_status', {});
    const r = { before: before.slice(0, 60), msg: txt(msg), after: txt($('[data-testid="explore-cache-size"]'))?.slice(0, 60), entries: status.cacheEntries };
    return { ...r, ...(/[1-9]\\d* saved answers?/.test(r.before) && /Cleared [1-9]\\d* saved answers?/.test(r.msg) && /^0 saved answers/.test(r.after) && r.entries === 0 ? {} : { error: 'cache clear wrong: ' + JSON.stringify(r) }) };
  `);

  // the fake token must be in no log written so far (the DB and every file are scanned after the run by run-e2e.ps1)
  {
    const logs = [netLog().join('\n'), existsSync(cfg.openLog) ? readFileSync(cfg.openLog, 'utf8') : '', existsSync(cfg.trashLog) ? readFileSync(cfg.trashLog, 'utf8') : ''];
    record('X11 the token is in none of the e2e logs', { logs: logs.length }, logs.some((l) => l.includes(T)) ? 'token found in a log' : undefined);
    const unrouted = N(/./).filter((l) => l.endsWith('\tunrouted'));
    record('X11 every Explore request had a canned route', { unrouted }, unrouted.length ? 'unrouted: ' + unrouted.join(', ') : undefined);
  }
}

async function main() {
  // ---- Session A: start timing only (first launch after the build: cold-ish file cache) ----
  let s = await newSession();
  const a = await startTiming(s, 'A.start');
  record('F1 Back/Forward disabled in a fresh window', { back: a.backDisabled, forward: a.forwardDisabled },
    a.backDisabled === true && a.forwardDisabled === true ? undefined : 'Back/Forward should both be present and disabled');
  await wd('DELETE', `/session/${s}`);
  await sleep(1500);

  // ---- Session B: warm start + smoke ----
  s = await newSession();
  const start = await startTiming(s, 'B.start');
  const p1 = start.processStartToRowsMs;
  record('NFR-P1 start to dashboard rows', { ms: p1, rows: start.rowCount, budgetMs: 1500 },
    p1 == null ? 'no timing' : p1 >= 1500 ? `${p1} ms >= 1500 ms` : start.rowCount < 50 ? `only ${start.rowCount} rows` : undefined);

  const col = await installCollector(s);
  record('collector', col, col.mode === 'cdp' && !col.installed ? 'collector not installed after reload' : undefined);

  // Opener seam: Settings → Open data folder is logged, not opened (app_open_dir in the built exe).
  await step(s, 'settings.open-data-dir', `
    await go('#/settings');
    const open = need(await until(() => byText('button', 'Open')), 'Open button');
    open.click();
    await sleep(600);
    return { info: txt($('main')).slice(0, 200) };
  `);
  const log1 = existsSync(cfg.openLog) ? readFileSync(cfg.openLog, 'utf8') : '';
  record('app_open_dir logged', { log: log1.trim() }, /^folder\t.*data/m.test(log1) ? undefined : 'no folder line for the data dir');

  // ---- Imports through the e2e picker queue (RS_E2E_PICK) ----
  const imported = [];
  for (const want of cfg.imports) {
    const r = await step(s, `import ${want}`, `
      await go('#/');
      await menuItem($('button[aria-label="Add a repository"]'), 'Import folder');
      const d = need(await until(dialog, 3000), 'import dialog');
      need(byText('button', 'Choose', d), 'Choose folder button').click();
      const sum = need(await until(() => $('[data-testid="import-summary"]'), 10000), 'import summary');
      const summary = txt(sum);
      await sleep(150);
      // F4/D-112: "Download the latest release" only for github.com origins; left untouched (= the setting, on).
      const box = $('[data-testid="autodl"] input', d);
      const autodl = box ? box.checked : null;
      need(byText('.dialog__footer button', 'Import'), 'Import button').click();
      await until(() => !dialog(), 10000);
      await until(() => location.hash.split('/').length >= 3, 10000);
      await settle();
      return { summary: summary.slice(0, 160), route: location.hash, autodl };
    `);
    imported.push(r.route);
    const wantBox = want === 'basic' ? true : null;
    if (!r.error && r.autodl !== wantBox) record(`import ${want} auto-download option`, { autodl: r.autodl }, `expected ${wantBox}, got ${r.autodl}`);
  }
  record('imports', { routes: imported }, imported.length === cfg.imports.length && imported.every(Boolean) ? undefined : 'some imports failed');

  // F4 + F6: importing basic (github.com origin) queued the GitHub sync (info + releases) and, with the option left
  // on, the download of the Windows asset into the scratch release store — all answered by the canned transport.
  {
    const f = releaseFile('example', 'basic', 'v0.1.0', NET.basicWin);
    const size = await waitFile(f);
    const log = netLog();
    const has = (u) => log.some((l) => l.startsWith(`get\t${u}\t`));
    const want = ['https://api.github.com/repos/example/basic', 'https://api.github.com/repos/example/basic/releases?per_page=100',
      `https://github.com/example/basic/releases/download/v0.1.0/${NET.basicWin}`, 'https://objects.githubusercontent.com/e2e/basic-win.zip?sig=1'];
    const missing = want.filter((u) => !has(u));
    record('import basic: GitHub sync + auto-download (canned)', { file: f, size, requests: log.length, missing },
      size !== ZIP.length ? `downloaded file missing or wrong size (${size})` : missing.length ? 'requests missing: ' + missing.join(', ')
        : has(NET.avatar) ? 'a contributor avatar was fetched' : undefined);
  }

  // ---- Every screen, light + dark, with screenshots (S0–S10) ----
  const first = await step(s, 'oids', `
    await go('#/example/basic/commits/main');
    const basicHead = need(await until(() => $('.crow[data-oid]')), 'basic commits').dataset.oid;
    await go('#/local/huge-commit/commits/main');
    await until(() => $$('.crow[data-oid]').length === 2);
    const huge = $$('.crow[data-oid]')[0].dataset.oid;
    await go('#/local/merges/commits/main');
    await until(() => $('.crow[data-oid]'));
    await settle();
    const crow = (t) => $$('.crow[data-oid]').find((e) => e.innerText.includes(t))?.dataset.oid ?? null;
    // m1 merges a side branch with a real change; m3 (the tip) is treesame with its first parent.
    return { basicHead, huge, mergeM1: need(crow('m1 merge side'), 'm1'), mergeM3: need(crow('m3 merge rev'), 'm3') };
  `);
  const screens = [
    ['s01-dashboard', '#/', '.repo-row'],
    ['s03-code', '#/example/basic', '[data-testid="file-table"] .ftable__row'],
    ['s04-file', '#/example/basic/blob/main/web/util.ts', '[class*="hljs-"]'],
    ['s04-image', '#/local/binaries/blob/main/img/evil.svg', 'img[src^="blob:"]'],
    ['s05-commits', '#/local/long-history/commits/main', '.crow'],
    ['s06-commit', `#/example/basic/commit/${first.basicHead}`, '.dcard'],
    ['s06-merge', `#/local/merges/commit/${first.mergeM1}`, '.dcard'],
    ['s06-merge-treesame', `#/local/merges/commit/${first.mergeM3}`, 'main h1, main h2'],
    ['s07-branches', '#/example/basic/branches', '.brow'],
    ['s08-tags', '#/local/releases/tags', '.trow'],
    ['s09-releases', '#/local/releases/releases', '.rcard'],
    ['s10-settings', '#/settings', 'main'],
    ['security-readme', '#/local/security-readme', '.markdown'],
    // v0.2.0: GitHub-like release card with assets (canned GitHub list), centred README with a remote badge
    ['m13-release-assets', '#/example/basic/releases', '.rcard[data-tag="v0.1.0"] .rasset'],
    ['m13-align-readme', '#/local/align', '.markdown img[src^="blob:"]'],
  ];
  for (const theme of ['Light', 'Dark']) {
    const t = await step(s, `theme ${theme}`, `return { resolved: await setTheme('${theme}') };`);
    if (t.resolved !== theme.toLowerCase()) record(`theme ${theme} check`, t, `data-theme is ${t.resolved}`);
    for (const [id, hash, ready] of screens) {
      const r = await step(s, `screen ${id} ${theme}`, `
        await go(${JSON.stringify(hash)});
        need(await until(() => $(${JSON.stringify(ready)}), 15000), ${JSON.stringify(ready)});
        await settle();
        window.scrollTo(0, 0);
        await sleep(200);
        return { route: location.hash, h1: txt($('h1'))?.slice(0, 80) ?? null, notFound: !!byText('h1, h2', 'not found') };
      `);
      if (!r.error) {
        r.shot = await shot(s, `${id}-${theme.toLowerCase()}`);
        save();
      }
    }
    // S10 dialogs: Import and Clone (no pick is taken; the queue stays as it is).
    for (const [id, label] of [['s10-import', 'Import folder'], ['s10-clone', 'Clone URL']]) {
      const r = await step(s, `screen ${id} ${theme}`, `
        await go('#/');
        await until(() => $('.repo-row'));
        await menuItem($('button[aria-label="Add a repository"]'), ${JSON.stringify(label)});
        const d = need(await until(dialog, 3000), 'dialog');
        if (${JSON.stringify(id)} === 's10-clone') {
          setValue(need($('#clone-url', d), 'clone url'), ${JSON.stringify(cfg.upstreamUrl)});
          need(await until(() => $('[data-testid="clone-preview"]'), 5000), 'clone preview');
        }
        await sleep(300);
        return { title: txt($('.dialog__title', d)) };
      `);
      if (!r.error) r.shot = await shot(s, `${id}-${theme.toLowerCase()}`);
      await exec(s, `byText('.dialog__footer button', 'Cancel')?.click(); await until(() => !dialog(), 3000); return {};`);
      save();
    }
  }
  await step(s, 'theme back to Light', `return { resolved: await setTheme('Light') };`);
  await step(s, 'dashboard Recent lists opened repos', `
    await go('#/');
    await until(() => $('.repo-row'));
    const sec = $$('section.sidebar__section').find((x) => txt(x.querySelector('h2')) === 'Recent');
    const items = sec ? $$('a', sec).map(txt) : [];
    return { items, ...(items.length === 5 ? {} : { error: 'Recent has ' + items.length + ' items' }) };
  `);

  // ---- Checks against the real backend (M7 handoff) ----
  await step(s, 'code tab', `
    await go('#/example/basic');
    await until(() => $('[data-testid="file-table"] .ftable__row'));
    await settle();
    const md = need($('.markdown'), 'readme body');
    return {
      files: $$('[data-testid="file-table"] .ftable__row').length,
      readmeTable: !!md.querySelector('table'),
      readmeCodeHighlighted: !!md.querySelector('code [class*="hljs-"]'),
      readmeImage: md.querySelector('img')?.getAttribute('src')?.slice(0, 5) ?? null,
      languages: txt(byText('section, aside, div', 'Languages'))?.slice(0, 80) ?? null,
    };
  `);
  await step(s, 'file view highlighting (worker)', `
    await go('#/example/basic/blob/main/web/util.ts');
    const span = await until(() => $('[class*="hljs-"]'), 10000);
    return { highlighted: !!span, spans: $$('[class*="hljs-"]').length, ...(span ? {} : { error: 'no hljs spans: worker highlighting failed' }) };
  `);
  await step(s, 'commits path filter page 2 (chained skip)', `
    await go('#/local/long-history/commits/main?path=src%2Ff01.rs');
    await until(() => $$('.crow[data-oid]').length === 35, 15000);
    const p1 = $$('.crow[data-oid]').map((e) => e.dataset.oid);
    need(byText('.pager a, .pager button', 'Older'), 'Older').click();
    await until(() => $('.crow[data-oid]') && $('.crow[data-oid]').dataset.oid !== p1[0], 15000);
    await settle();
    const p2 = $$('.crow[data-oid]').map((e) => e.dataset.oid);
    const overlap = p2.filter((o) => p1.includes(o)).length;
    const summaries = $$('.crow .crow__summary').slice(0, 3).map(txt);
    return { page1: p1.length, page2: p2.length, overlap, route: location.hash, summaries, ...(p2.length === 35 && overlap === 0 ? {} : { error: 'page 2 wrong' }) };
  `);
  await step(s, 'commit ?file=250 on the 400-file commit', `
    await go('#/local/huge-commit/commit/${first.huge}?file=250');
    const card = await until(() => document.getElementById('diff-250'), 15000);
    await sleep(800);
    const top = card ? Math.round(card.getBoundingClientRect().top) : null;
    return { cards: $$('.dcard').length, top, summary: txt($('.dsummary, [class*="dsummary"]'))?.slice(0, 120) ?? null, ...(card && top >= 0 && top < 200 ? {} : { error: 'card 250 not scrolled into view (top ' + top + ')' }) };
  `);
  await step(s, 'branches ahead/behind (HEAD branch base)', `
    await go('#/example/basic/branches');
    await until(() => $('.brow[data-branch="feature"]'));
    await settle();
    const row = $('.brow[data-branch="feature"]');
    const ab = txt(row.querySelector('.brow__ab'));
    const note = $$('p, span').map(txt).find((t) => t && t.startsWith('Compared with') || t && t.includes('compared with')) ?? null;
    return { feature: ab, compared: note?.slice(0, 80) ?? null, ...(/1/.test(ab ?? '') ? {} : { error: 'feature should be 1 ahead: ' + ab }) };
  `);
  await step(s, 'GitHub notes button (github.com origin, feature on; not clicked)', `
    await go('#/example/basic/releases');
    await until(() => $('.rcard'));
    const b = byText('button', 'GitHub release notes');
    await go('#/local/releases/releases');
    await until(() => $('.rcard'));
    const none = !byText('button', 'GitHub release notes');
    return { onGithubOrigin: !!b, hiddenWithoutOrigin: none, cards: $$('.rcard').length, ...(b && none ? {} : { error: 'button visibility wrong' }) };
  `);
  await step(s, 'open folder (imported) logged', `
    await go('#/example/basic');
    need(await until(() => byText('.rh__actions button', 'Open folder')), 'Open folder').click();
    await sleep(500);
    return {};
  `);
  const log2 = readFileSync(cfg.openLog, 'utf8');
  record('repo_open_folder logged', { log: log2.trim() }, /^folder\t.*basic/m.test(log2) ? undefined : 'no folder line for basic');

  // ---- Security pass (NFR-S1..S7) ----
  await step(s, 'security README', `
    await go('#/local/security-readme');
    const md = need(await until(() => $('.markdown'), 15000), 'readme body');
    await settle();
    const hrefs = $$('a', md).map((a) => a.getAttribute('href'));
    const bad = hrefs.filter((h) => h && /^(javascript|data|vbscript|file):/i.test(h));
    const remoteImgs = $$('img', md).filter((i) => /^https?:/i.test(i.getAttribute('src') || ''));
    const chip = byText('a, span, button', 'Remote image');
    const r = {
      scripts: $$('script, iframe, object, embed, form', md).length,
      onAttrs: $$('*', md).filter((e) => [...e.attributes].some((a) => a.name.startsWith('on'))).length,
      styleAttrs: $$('[style]', md).length,
      badHrefs: bad,
      remoteImgs: remoteImgs.length,
      remoteChip: !!chip,
      alerts: window.__e2e ? window.__e2e.alerts.length : null,
      route: location.hash,
    };
    if (r.scripts || r.onAttrs || r.styleAttrs || bad.length || r.remoteImgs || !r.remoteChip || r.alerts) r.error = 'hostile content survived: ' + JSON.stringify(r);
    return r;
  `);
  await step(s, 'external link → confirm dialog (cancelled)', `
    await go('#/example/basic');
    const a = need(await until(() => $('a.rh__origin')), 'origin link');
    a.click();
    const d = need(await until(dialog, 3000), 'confirm dialog');
    const shown = txt(d).slice(0, 200);
    need(byText('.dialog__footer button', 'Cancel', d), 'Cancel').click();
    await until(() => !dialog(), 3000);
    return { shown, stillInApp: location.origin, hash: location.hash };
  `);
  await step(s, 'svg with script shown as an image only', `
    await go('#/local/security-readme/blob/main/svg-with-script.svg');
    const img = need(await until(() => $('img[src^="blob:"]'), 10000), 'svg img');
    await sleep(300);
    return { tag: img.tagName, inlineSvg: $$('main svg script').length, alerts: window.__e2e?.alerts.length ?? null };
  `);

  // ---- v0.2.0 (M11/M12) in the real app ----
  await step(s, 'F1 Back/Forward buttons', `
    await go('#/example/basic');
    await until(() => $('.rh__title'));
    await go('#/example/basic/commits/main');
    await until(() => $('.crow[data-oid]'));
    const back = need($('button[aria-label^="Back"]'), 'Back button');
    const fwd = need($('button[aria-label^="Forward"]'), 'Forward button');
    const before = { back: back.disabled, fwd: fwd.disabled };
    back.click();
    const r1 = await until(() => location.hash === '#/example/basic' && $('.rh__title'), 5000);
    await sleep(200);
    const mid = { hash: location.hash, back: $('button[aria-label^="Back"]').disabled, fwd: $('button[aria-label^="Forward"]').disabled };
    $('button[aria-label^="Forward"]').click();
    const r2 = await until(() => location.hash === '#/example/basic/commits/main' && $('.crow[data-oid]'), 5000);
    await sleep(200);
    const end = { hash: location.hash, fwd: $('button[aria-label^="Forward"]').disabled };
    const ok = !before.back && before.fwd && r1 && !mid.back && !mid.fwd && r2 && end.fwd;
    return { before, mid, end, ...(ok ? {} : { error: 'Back/Forward did not move or enable correctly' }) };
  `);
  const imgReqs0 = netLog().length;
  await step(s, 'F2/F3 centred README with remote image (align)', `
    await go('#/local/align');
    const md = need(await until(() => $('.markdown'), 15000), 'readme body');
    await settle();
    for (const c of $$('a.md-remote-image', md)) { c.scrollIntoView({ block: 'center' }); await sleep(150); }
    const badge = await until(() => $('img[alt="badge"][src^="blob:"]', md), 10000);
    await until(() => $$('img', md).every((i) => i.complete), 5000);
    window.scrollTo(0, 0);
    md.scrollIntoView({ block: 'start' });
    await sleep(300);
    const m = md.getBoundingClientRect();
    const mc = m.left + m.width / 2;
    const centre = (r) => Math.round(r.left + r.width / 2 - mc);
    const textRect = (el) => { const rg = document.createRange(); rg.selectNodeContents(el); return rg.getBoundingClientRect(); };
    const h1 = need($('h1[align="center"]', md), 'h1 align=center');
    const logo = need($('img[alt="Logo"]', md), 'logo');
    const table = need($('table[align="center"]', md), 'table align=center');
    const side = need($('img[alt="Side"]', md), 'side image');
    const h2 = need($('h2[align="right"]', md), 'h2 align=right');
    const sr = side.getBoundingClientRect();
    const r = {
      h1TextOffset: centre(textRect(h1)),
      logoOffset: centre(logo.getBoundingClientRect()),
      logoWidth: Math.round(logo.getBoundingClientRect().width),
      tableOffset: centre(table.getBoundingClientRect()),
      badgeOffset: badge ? centre(badge.getBoundingClientRect()) : null,
      h2RightGap: Math.round(m.right - textRect(h2).right),
      sideFloat: getComputedStyle(side).float,
      sideRightGap: Math.round(m.right - sr.right),
      // content box (GitHub adds 20 px padding beside a floated image, D-110)
      sideWidth: Math.round(sr.width - parseFloat(getComputedStyle(side).paddingLeft) - parseFloat(getComputedStyle(side).paddingRight)),
      badgeFromBlob: !!badge,
      blobImgs: $$('img[src^="blob:"]', md).length,
      remoteImgs: $$('img', md).filter((i) => /^https?:/i.test(i.getAttribute('src') || '')).length,
      chips: $$('a.md-remote-image', md).map((c) => txt(c)),
      justifyAlign: $$('[align="justify"]', md).length,
      styleAttrs: $$('[style]', md).length,
    };
    const near = (v, d = 3) => v != null && Math.abs(v) <= d;
    const ok = near(r.h1TextOffset) && near(r.logoOffset) && r.logoWidth === 120 && near(r.tableOffset) && near(r.badgeOffset)
      && near(r.h2RightGap, 4) && r.sideFloat === 'right' && near(r.sideRightGap, 1) && r.sideWidth === 200 && r.badgeFromBlob
      && r.remoteImgs === 0 && r.chips.length === 1 && r.chips[0].includes('fail.example.com') && !r.justifyAlign && !r.styleAttrs;
    return { ...r, ...(ok ? {} : { error: 'README layout/images not GitHub-like: ' + JSON.stringify(r) }) };
  `);
  {
    const log = netLog().slice(imgReqs0);
    const badge = log.filter((l) => l.startsWith(`get\t${NET.badge}\t`)).length;
    const fail = log.filter((l) => l.startsWith('get\thttps://fail.example.com/missing.png\t')).length;
    record('README images fetched in Rust (canned)', { badge, fail, lines: log.slice(0, 10) },
      badge > 1 ? 'badge fetched more than once (cache?)' : badge + fail === 0 && !netLog().some((l) => l.includes(NET.badge)) ? 'no image request seen' : undefined);
  }
  await step(s, 'F6 About: GitHub values vs user edits', `
    await go('#/example/basic');
    const desc = () => txt($('.about__desc'));
    const gh = await until(() => desc() === ${JSON.stringify(NET.basicDesc)} && $('[data-testid="about-from-github"]'), 10000);
    if (!gh) return { error: 'GitHub description not shown: ' + desc() };
    const topics = $$('.about a.topic, .about .topic, .about .chip').map(txt).slice(0, 6);
    const edit = async (value) => {
      need($('button[aria-label="Edit repository details"]'), 'Edit About').click();
      const d = need(await until(dialog, 3000), 'About dialog');
      const ta = need($('textarea', d), 'description field');
      const placeholder = ta.placeholder;
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(ta, value);
      ta.dispatchEvent(new Event('input', { bubbles: true }));
      need(byText('button', 'Save changes', d), 'Save changes').click();
      await until(() => !dialog(), 5000);
      await sleep(300);
      return placeholder;
    };
    const placeholder = await edit('My own words (e2e)');
    const mine = await until(() => desc() === 'My own words (e2e)', 5000);
    const id = (await invoke('repo_by_slug', { owner: 'example', name: 'basic' })).id;
    const sum = (await invoke('library_list', {})).find((r) => r.id === id);
    await edit('');
    const back = await until(() => desc() === ${JSON.stringify(NET.basicDesc)}, 5000);
    const r = { placeholder, topics, userSaved: sum?.aboutUser?.description ?? null, githubKept: sum?.aboutGithub?.description ?? null, mineShown: !!mine, githubBack: !!back };
    const ok = r.placeholder.includes(${JSON.stringify(NET.basicDesc)}) && r.mineShown && r.githubBack && r.userSaved === 'My own words (e2e)' && r.githubKept === ${JSON.stringify(NET.basicDesc)};
    return { ...r, ...(ok ? {} : { error: 'About merge wrong: ' + JSON.stringify(r) }) };
  `);
  await step(s, 'F4 release card + assets (basic, canned GitHub list)', `
    await go('#/example/basic/releases');
    const card = need(await until(() => $('.rcard[data-tag="v0.1.0"]'), 15000), 'v0.1.0 card');
    await until(() => $$('.rasset', card).length >= 3, 10000);
    await settle();
    const rows = $$('.rasset', card).map((r) => ({ name: r.dataset.asset ?? txt($('.rasset__name', r)), pc: r.classList.contains('rasset--pc'), size: txt($('.rasset__size', r)), count: txt($('.rasset__count', r)), buttons: $$('button', r).map((b) => b.getAttribute('aria-label') || txt(b)) }));
    // F5: the avatar inside the contributor's link became "@user" text in that link (D-97)
    const mention = $$('.markdown a', card).find((a) => txt(a) === '@octo-dev' && a.getAttribute('href') === 'https://github.com/octo-dev');
    const r = {
      title: txt($('h2, h3', card))?.slice(0, 80) ?? null,
      tagChip: $('.rcard__tagchip', card)?.getAttribute('href') ?? null,
      oidLink: $('.rcard__oid', card)?.getAttribute('href') ?? null,
      ghLink: $('.rcard__gh', card)?.getAttribute('href') ?? null,
      mention: mention ? txt(mention) : null,
      avatarImgs: $$('img', card).filter((i) => /avatars/.test(i.getAttribute('src') || '')).length,
      rows,
    };
    const win = rows.find((x) => x.name === ${JSON.stringify(NET.basicWin)});
    const ok = r.tagChip && /\\/commit\\/[0-9a-f]{40}$/.test(r.oidLink ?? '') && r.ghLink === 'https://github.com/example/basic/releases/tag/v0.1.0'
      && r.mention === '@octo-dev' && !r.avatarImgs && win && win.pc && win.buttons.some((b) => /Show in folder/.test(b));
    return { ...r, ...(ok ? {} : { error: 'release card wrong: ' + JSON.stringify(r).slice(0, 500) }) };
  `);
  const opens0 = readFileSync(cfg.openLog, 'utf8');
  await step(s, 'F4 Download (button) + Show in folder', `
    const card = need($('.rcard[data-tag="v0.1.0"]'), 'card');
    const row = (n) => $('.rasset[data-asset="' + n + '"]', card);
    need(byText('button', 'Show in folder', row(${JSON.stringify(NET.basicWin)})), 'Show in folder (auto-downloaded)').click();
    await sleep(300);
    need($('button[aria-label="Download ' + ${JSON.stringify(NET.basicLinux)} + '"]', card), 'Download button').click();
    const shown = await until(() => byText('button', 'Show in folder', row(${JSON.stringify(NET.basicLinux)})), 20000);
    if (!shown) {
      const rr = row(${JSON.stringify(NET.basicLinux)});
      const diag = { rowText: txt(rr), buttons: rr ? $$('button', rr).map((b) => b.getAttribute('aria-label') || txt(b)) : null, cardConnected: card.isConnected };
      await go('#/example/basic');
      await sleep(500);
      await go('#/example/basic/releases');
      await until(() => $('.rcard[data-tag="v0.1.0"] .rasset'), 10000);
      await settle();
      const r2 = $('.rcard[data-tag="v0.1.0"] .rasset[data-asset="' + ${JSON.stringify(NET.basicLinux)} + '"]');
      diag.afterRevisit = r2 ? $$('button', r2).map((b) => b.getAttribute('aria-label') || txt(b)) : null;
      return { error: 'Show in folder did not appear after the download: ' + JSON.stringify(diag) };
    }
    shown.click();
    await sleep(300);
    return { downloaded: txt(row(${JSON.stringify(NET.basicLinux)}))?.slice(0, 120) };
  `);
  {
    const f = releaseFile('example', 'basic', 'v0.1.0', NET.basicLinux);
    const size = await waitFile(f, 5000);
    const reveals = readFileSync(cfg.openLog, 'utf8').slice(opens0.length).split(/\r?\n/).filter((l) => l.startsWith('reveal\t'));
    const inStore = reveals.filter((l) => l.toLowerCase().includes(join(cfg.archive, '_releases').toLowerCase()));
    record('downloads land in the scratch release store; Show in folder reaches the opener', { file: f, size, reveals },
      size !== TAR.length ? `tarball missing or wrong size (${size})` : inStore.length !== 2 ? `expected 2 reveal lines inside _releases, got ${inStore.length}` : undefined);
  }
  await step(s, 'F6 Network settings toggles (persist, images off = chips, no request)', `
    const boxes = async () => { await go('#/settings'); const n = need(await until(() => $('[data-testid="network-settings"]'), 10000), 'Network section'); await sleep(200); return n; };
    let net = await boxes();
    const state = () => Object.fromEntries($$('input[type="checkbox"]', $('[data-testid="network-settings"]')).map((b) => [b.name, b.checked]));
    const defaults = state();
    const cap = $('#setting-cap')?.value ?? null;
    $('input[name="loadRemoteImages"]', net).click();
    $('input[name="autoDownloadOnFetchAll"]', net).click();
    await sleep(400);
    const saved = await invoke('settings_get', {});
    return { defaults, cap, saved: { loadRemoteImages: saved.loadRemoteImages, autoDownloadOnFetchAll: saved.autoDownloadOnFetchAll } };
  `).then((r) => {
    const want = { loadRemoteImages: true, fetchReleaseNotes: true, fetchRepoInfo: true, autoDownloadLatestRelease: true, autoDownloadOnFetchAll: false };
    const same = (x, y) => Object.keys(y).length === Object.keys(x ?? {}).length && Object.keys(y).every((k) => x[k] === y[k]);
    const bad = r.error ? null : !same(r.defaults, want) ? 'defaults ' + JSON.stringify(r.defaults)
      : r.cap !== '1024' ? 'size cap ' + r.cap
        : r.saved.loadRemoteImages !== false || r.saved.autoDownloadOnFetchAll !== true ? 'not saved: ' + JSON.stringify(r.saved) : undefined;
    if (bad) record('network settings check', { defaults: r.defaults, cap: r.cap, saved: r.saved }, bad);
  });
  // A reload starts a fresh collector and resource list: drain first (imports, screens, the v0.2.0 steps so far).
  await drain(s, 'browse');
  await exec(s, `location.hash = '#/settings'; location.reload(); return {};`).catch(() => {});
  await sleep(1500);
  if (col.mode !== 'cdp') await exec(s, `${COLLECTOR}; return {};`);
  await step(s, 'network settings kept after reload', `
    await until(() => $('.gh'), 20000);
    await go('#/settings');
    const net = need(await until(() => $('[data-testid="network-settings"]'), 10000), 'Network section');
    await sleep(300);
    const st = Object.fromEntries($$('input[type="checkbox"]', net).map((b) => [b.name, b.checked]));
    return { st, ...(st.loadRemoteImages === false && st.autoDownloadOnFetchAll === true ? {} : { error: 'not kept: ' + JSON.stringify(st) }) };
  `);
  const offReqs = netLog().length;
  await step(s, 'images off: chips only', `
    await go('#/local/align');
    const md = need(await until(() => $('.markdown'), 15000), 'readme body');
    await settle();
    for (const c of $$('a.md-remote-image', md)) { c.scrollIntoView({ block: 'center' }); await sleep(150); }
    await sleep(800);
    const r = { chips: $$('a.md-remote-image', md).length, remoteBlob: !!$('img[alt="badge"][src^="blob:"]', md), localBlob: $$('img[src^="blob:"]', md).length };
    return { ...r, ...(r.chips === 2 && !r.remoteBlob && r.localBlob >= 3 ? {} : { error: 'with images off: ' + JSON.stringify(r) }) };
  `);
  record('images off: no image request', { newRequests: netLog().slice(offReqs) }, netLog().length !== offReqs ? 'requests made with images off' : undefined);
  await step(s, 'settings back to defaults', `
    await go('#/settings');
    const net = need(await until(() => $('[data-testid="network-settings"]'), 10000), 'Network section');
    $('input[name="loadRemoteImages"]', net).click();
    $('input[name="autoDownloadOnFetchAll"]', net).click();
    await sleep(400);
    const st = await invoke('settings_get', {});
    return { st, ...(st.loadRemoteImages && !st.autoDownloadOnFetchAll ? {} : { error: 'not restored' }) };
  `);

  const log3 = readFileSync(cfg.openLog, 'utf8');
  record('no browser opened', { log: log3.trim() }, /^url\t/m.test(log3) ? 'a URL was opened' : undefined);

  // Imports, every screen in both themes, worker highlighting, security README: drained before the probes so any
  // console noise from the hostile calls is told apart from the app's own.
  await drain(s, 'browse 2');

  // ---- Hostile IPC from the webview (PoC criterion 4) ----
  await step(s, 'hostile IPC refused', `
    const id = (await invoke('repo_by_slug', { owner: 'example', name: 'basic' })).id;
    const table = [
      ['tree_list', { repoId: id, rev: 'main', path: '../x' }, 'invalidPath'],
      ['tree_list', { repoId: id, rev: 'main', path: 'C:\\\\Windows' }],
      ['tree_list', { repoId: id, rev: 'main', path: '/etc' }, 'invalidPath'],
      ['tree_list', { repoId: id, rev: '..', path: '' }, 'invalidRev'],
      ['tree_list', { repoId: id, rev: '@{-1}', path: '' }, 'invalidRev'],
      ['blob_text', { repoId: id, rev: 'main', path: 'a\\\\..\\\\..\\\\b' }],
      ['blob_bytes', { repoId: id, rev: 'refs/../x', path: 'README.md' }, 'invalidRev'],
      ['commit_detail', { repoId: id, oid: 'HEAD' }, 'invalidRev'],
      ['commits_page', { repoId: id, rev: 'main', path: null, since: '--upload-pack=x', skip: 0, tip: null }, 'invalidRev'],
      ['repo_detail', { repoId: 987654 }, 'notFound'],
      ['open_external', { url: 'javascript:alert(1)' }],
      ['open_external', { url: 'file:///C:/Windows/win.ini' }],
      ['open_external', { url: 'mailto:x@example.com' }],
      ['clone_preview', { input: 'http://github.com/o/r' }],
      ['clone_preview', { input: 'ssh://git@github.com/o/r' }],
      ['clone_preview', { input: 'git@github.com:o/r.git' }],
      ['clone_preview', { input: 'https://u:p@github.com/o/r' }],
      ['clone_preview', { input: 'C:\\\\Windows' }],
      ['app_open_dir', { which: 'C:\\\\' }],
      ['plugin:window|set_title', { label: 'main', value: 'x' }],
      ['plugin:fs|read_text_file', { path: 'C:\\\\Windows\\\\win.ini' }],
      ['plugin:shell|execute', { program: 'cmd', args: ['/c', 'echo'] }],
      ['plugin:dialog|open', { options: {} }],
      ['plugin:opener|open_url', { url: 'https://example.com/' }],
      ['plugin:opener|open_path', { path: 'C:\\\\' }],
    ];
    const out = [];
    const defaultWant = (cmd) =>
      cmd.startsWith('plugin:') ? 'not allowed by ACL' : cmd === 'app_open_dir' ? 'invalidInput' : /^(open_external|clone_preview)$/.test(cmd) ? 'invalidUrl' : 'invalidPath';
    for (const [cmd, args, want0] of table) {
      const want = want0 ?? defaultWant(cmd);
      try {
        const v = await invoke(cmd, args);
        out.push({ cmd, want, accepted: true, value: JSON.stringify(v).slice(0, 80) });
      } catch (e) {
        const got = (e && (e.code || e.kind)) || String(e).slice(0, 80);
        out.push({ cmd, want, refused: got, ok: got === want || String(got).includes(want) });
      }
    }
    const wrong = out.filter((o) => !o.ok);
    return { probes: out.length, refused: out.filter((o) => o.refused).length, out, ...(wrong.length ? { error: 'not refused as expected: ' + JSON.stringify(wrong) } : {}) };
  `);
  const log4 = readFileSync(cfg.openLog, 'utf8');
  record('hostile IPC opened nothing', {}, log4 !== log3 ? 'opener log changed: ' + log4 : undefined);

  await drain(s, 'hostile IPC');

  // ---- NFR-P2: dashboard → click the 1,000-entry repo → file table rendered (fresh page, nothing cached) ----
  await exec(s, `location.hash = '#/'; location.reload(); return {};`).catch(() => {});
  await sleep(1500);
  if (col.mode !== 'cdp') await exec(s, `${COLLECTOR}; return {};`);
  for (const pass of ['first', 'second']) {
    const r = await step(s, `NFR-P2 1,000-entry dir (${pass} visit)`, `
      await until(() => $('.gh'), 20000);
      await go('#/');
      const link = need(await until(() => $$('a.repo-row__name').find((a) => a.getAttribute('href')?.endsWith('/local/wide-dir')), 20000), 'wide-dir row');
      await sleep(300);
      const t0 = performance.now();
      link.click();
      const t = await new Promise((res) => {
        const tick = () => {
          const n = document.querySelectorAll('[data-testid="file-table"] .ftable__row').length;
          if (n >= 20) res(performance.now() - t0); else if (performance.now() - t0 > 10000) res(null); else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
      await settle();
      // Virtualised above 500 rows: only the rows on screen are in the DOM.
      const rows = $$('[data-testid="file-table"] .ftable__row').length;
      const names = $$('[data-testid="file-table"] .ftable__row').slice(0, 3).map(txt);
      return { ms: t == null ? null : Math.round(t), renderedRows: rows, firstRows: names, budgetMs: 300, ...(t == null ? { error: 'table never rendered' } : t >= 300 ? { error: Math.round(t) + ' ms >= 300 ms' } : {}) };
    `);
    void r;
  }

  // ---- File-seam clone → fetch → fast-forward → remove (D-14, D-20) ----
  const clone = await step(s, 'clone (file seam)', `
    await go('#/');
    await until(() => $('.repo-row'));
    await menuItem($('button[aria-label="Add a repository"]'), 'Clone URL');
    const d = need(await until(dialog, 3000), 'clone dialog');
    setValue(need($('#clone-url', d), 'clone url'), ${JSON.stringify(cfg.upstreamUrl)});
    const prev = need(await until(() => $('[data-testid="clone-preview"]'), 5000), 'clone preview');
    const preview = txt(prev);
    need(byText('.dialog__footer button', 'Clone'), 'Clone button').click();
    await until(() => !dialog(), 5000);
    $('button[aria-label^="Background jobs"]').click();
    const done = need(await until(() => $('li.job[data-kind="clone"][data-state="done"], li.job[data-kind="clone"][data-state="failed"]'), 60000), 'clone job finished');
    const job = txt(done);
    if (done.dataset.state !== 'done') return { error: 'clone failed: ' + job };
    // The link appears once the library refresh after the job has the new repo (done rows hide after 5 s).
    const open = need(await until(() => byText('a', 'Open repository', done), 4500), 'Open repository');
    open.click();
    await until(() => $('.rh__title'), 10000);
    await settle();
    return { preview: preview.slice(0, 200), job: job.slice(0, 200), route: location.hash, files: $$('[data-testid="file-table"] .ftable__row').length };
  `);
  if (!clone.error) {
    // Upstream moves on: one more commit and a tag (isolated git config).
    const git = (...a) =>
      execFileSync('git', ['-C', cfg.upstream, '-c', 'user.name=e2e', '-c', 'user.email=e2e@example.com', '-c', 'commit.gpgsign=false', '-c', 'tag.gpgsign=false', ...a], {
        encoding: 'utf8',
        env: { ...process.env, ...cfg.gitEnv },
      }).trim();
    git('commit', '--allow-empty', '-q', '-m', 'After the clone');
    git('tag', 'v1.1.0');
    const newHead = git('rev-parse', 'HEAD');
    record('upstream moved', { newHead });
    await step(s, 'fetch (file seam)', `
      need(await until(() => byText('.rh__actions button', 'Fetch')), 'Fetch button').click();
      const msg = need(await until(() => { const m = $('.rh__msg'); return m && /new commit|up to date|failed|error/i.test(m.innerText) ? m : null; }, 60000), 'fetch result');
      const text = txt(msg);
      return { text, ...(/1 new commit/.test(text) ? {} : { error: 'unexpected fetch result: ' + text }) };
    `);
    await step(s, 'fast-forward', `
      const b = need(await until(() => byText('.rh__actions button', 'Fast-forward'), 15000), 'Fast-forward button');
      b.click();
      const msg = need(await until(() => { const m = $('.rh__msg'); return m && /Fast-forwarded|error|diverged/i.test(m.innerText) ? m : null; }, 15000), 'ff result');
      const text = txt(msg);
      await settle();
      const latest = $('.crow[data-oid]')?.dataset.oid ?? null;
      return { text, ...(text.includes(${JSON.stringify(newHead.slice(0, 7))}) ? {} : { error: 'fast-forward did not reach ' + ${JSON.stringify(newHead.slice(0, 7))} + ': ' + text }) };
    `);
    const rm = await step(s, 'remove cloned (trash seam)', `
      const route = location.hash;
      const id = (await invoke('repo_by_slug', { owner: route.split('/')[1], name: route.split('/')[2] })).id;
      const path = (await invoke('library_list', {})).find((r) => r.id === id)?.path ?? null;
      await menuItem($('button[aria-label="More repository actions"]'), 'Remove from shelf');
      const d = need(await until(dialog, 3000), 'remove dialog');
      need(byText('.dialog__footer button', 'Recycle Bin', d), 'Remove button').click();
      await until(() => location.hash === '#/' || location.hash === '', 15000);
      await sleep(500);
      const still = (await invoke('library_list', {})).some((r) => r.id === id);
      return { path, stillListed: still, route: location.hash };
    `);
    const gone = rm.path ? !existsSync(rm.path) : false;
    // D-91: the e2e trash seam deletes the scratch clone and logs it; nothing reaches the real Recycle Bin.
    const trashLines = existsSync(cfg.trashLog) ? readFileSync(cfg.trashLog, 'utf8').split(/\r?\n/).filter(Boolean) : [];
    const logged = trashLines.length === 1 && trashLines[0].startsWith('trash\t');
    record('clone folder trashed', { path: rm.path, gone, trashLog: trashLines },
      !(rm.path && gone && !rm.stillListed) ? 'clone folder or entry still there'
        : !logged ? 'trash seam did not log exactly one deletion (real Recycle Bin used?)' : undefined);
  }

  // ---- v0.2.0: github.com clone through the git map (local upstream) → Clone option off → Fetch ▾ with download ----
  const demoUrl = 'https://github.com/e2e/demo-upstream';
  const before = netLog().length;
  const gclone = await step(s, 'clone github.com (git map) with the release option off', `
    await go('#/');
    await until(() => $('.repo-row'));
    await menuItem($('button[aria-label="Add a repository"]'), 'Clone URL');
    const d = need(await until(dialog, 3000), 'clone dialog');
    setValue(need($('#clone-url', d), 'clone url'), ${JSON.stringify(demoUrl)});
    need(await until(() => $('[data-testid="clone-preview"]'), 5000), 'clone preview');
    const box = need(await until(() => $('[data-testid="autodl"] input', d), 3000), 'release option (github.com)');
    const checkedByDefault = box.checked;
    box.click();
    await sleep(150);
    const unchecked = !box.checked;
    need(byText('.dialog__footer button', 'Clone'), 'Clone button').click();
    await until(() => !dialog(), 5000);
    // the jobs popover may still be open from the file-seam clone: a second click would close it
    const jb = $('button[aria-label^="Background jobs"]');
    if (jb.getAttribute('aria-expanded') !== 'true') jb.click();
    const done = need(await until(() => $$('li.job[data-kind="clone"]').find((j) => /done|failed/.test(j.dataset.state) && j.innerText.includes('e2e/demo-upstream')), 60000), 'clone job finished');
    const job = txt(done);
    if (done.dataset.state !== 'done') return { error: 'clone failed: ' + job };
    need(await until(() => byText('a', 'Open repository', done), 4500), 'Open repository').click();
    await until(() => location.hash === '#/e2e/demo-upstream' && $('.rh__title'), 10000);
    await settle();
    const desc = await until(() => txt($('.about__desc')) === ${JSON.stringify(NET.demoDesc)}, 10000);
    return { checkedByDefault, unchecked, job: job.slice(0, 200), route: location.hash, githubDesc: !!desc,
      ...(checkedByDefault && unchecked && desc ? {} : { error: 'option/About wrong: ' + JSON.stringify({ checkedByDefault, unchecked, desc: txt($('.about__desc')) }) }) };
  `);
  {
    const log = netLog().slice(before);
    const downloads = log.filter((l) => l.includes('/releases/download/'));
    record('clone: GitHub sync ran, no release download (option off)', { lines: log },
      gclone.error ? undefined : !log.some((l) => l.startsWith('get\thttps://api.github.com/repos/e2e/demo-upstream\t')) ? 'no GitHub info request'
        : downloads.length ? 'downloaded although the option was off: ' + downloads.join(', ') : existsSync(releaseFile('e2e', 'demo-upstream', 'v1.1.0', NET.demoWin)) ? 'file exists' : undefined);
  }
  if (!gclone.error) {
    await step(s, 'Fetch menu: Fetch and download the latest release', `
      await menuItem(need(await until(() => $('button[aria-label="Fetch options"]'), 5000), 'Fetch options'), 'Fetch and download the latest release');
      const msg = need(await until(() => { const m = $('.rh__msg'); return m && /new commit|up to date|failed|error/i.test(m.innerText) ? m : null; }, 60000), 'fetch result');
      return { text: txt(msg), ...(/failed|error/i.test(txt(msg)) ? { error: 'fetch: ' + txt(msg) } : {}) };
    `);
    const f = releaseFile('e2e', 'demo-upstream', 'v1.1.0', NET.demoWin);
    const size = await waitFile(f);
    record('fetch menu: latest release for this PC downloaded', { file: f, size }, size !== ZIP.length ? `missing or wrong size (${size})` : undefined);
    const o0 = readFileSync(cfg.openLog, 'utf8').length;
    await step(s, 'releases of the clone: Downloaded + Show in folder', `
      await go('#/e2e/demo-upstream/releases');
      const card = need(await until(() => $('.rcard[data-tag="v1.1.0"]'), 15000), 'v1.1.0 card');
      const row = need(await until(() => $('.rasset[data-asset="' + ${JSON.stringify(NET.demoWin)} + '"]', card), 10000), 'win asset row');
      const show = await until(() => byText('button', 'Show in folder', $('.rasset[data-asset="' + ${JSON.stringify(NET.demoWin)} + '"]')), 10000);
      if (!show) {
        const id = (await invoke('repo_by_slug', { owner: 'e2e', name: 'demo-upstream' })).id;
        const list = await invoke('releases_list', { repoId: id });
        const v = list.find((x) => x.tag === 'v1.1.0');
        return { error: 'no Show in folder: ' + JSON.stringify({ id, row: txt(row), backend: v ? v.assets.map((a) => [a.name, a.downloaded]) : null, tags: list.map((x) => x.tag) }) };
      }
      show.click();
      await sleep(300);
      return { latest: !!byText('span, .badge, .label', 'Latest', card), row: txt(row).slice(0, 120) };
    `);
    const rev = readFileSync(cfg.openLog, 'utf8').slice(o0).split(/\r?\n/).filter((l) => l.startsWith('reveal\t'));
    record('clone download revealed', { rev }, rev.length === 1 && rev[0].toLowerCase().endsWith(NET.demoWin.toLowerCase()) ? undefined : 'no reveal line for the download');
  }

  // ---- v0.3.0 (M10a/M10b): Explore in the real app, all answers canned (RS_E2E_NET); the e2e build has the in-memory
  // credential store, so the token never reaches the real Credential Manager ----
  await exploreFlows(s);

  // ---- Console / CSP: positive control, then the collected messages ----
  await step(s, 'CSP collector positive control', `
    const before = window.__e2e.csp.length;
    const sc = document.createElement('script');
    sc.textContent = 'window.__e2eInline = 1;';
    document.body.appendChild(sc);
    await sleep(300);
    sc.remove();
    const seen = window.__e2e.csp.length - before;
    const ran = window.__e2eInline === 1;
    window.__e2e.csp.splice(before);
    return { violationsSeen: seen, inlineRan: ran, ...(seen > 0 && !ran ? {} : { error: 'control failed: seen ' + seen + ', ran ' + ran }) };
  `);
  await drain(s, 'final');

  // M13 (D-113): the Rust side made requests only through the canned transport, and every one had a route.
  {
    const log = netLog();
    const unrouted = log.filter((l) => l.endsWith('\tunrouted'));
    const api = log.filter((l) => /^get\thttps:\/\/api\.github\.com\/.*\t200$/.test(l)).length;
    record('Rust-side requests all canned', { requests: log.length, apiOk: api, unrouted },
      !cfg.netLog ? 'no net log configured' : api === 0 ? 'no canned api.github.com answer seen: was the network seam active?' : unrouted.length ? 'unrouted: ' + unrouted.join(', ') : undefined);
  }

  // ---- Over every document of session B: console / CSP, then requests (NFR-O1 proxy) ----
  {
    // Tauri warns "Couldn't find callback id" when the harness itself reloads during an IPC call; anything else fails.
    const warns = acc.warns.filter((w) => !String(w.msg).includes("Couldn't find callback id"));
    const bad = acc.csp.length || acc.errors.length || acc.alerts.length || warns.length;
    record('console and CSP clean', { drains: acc.drains, csp: acc.csp, errors: acc.errors, warns: acc.warns, alerts: acc.alerts },
      bad ? 'console/CSP: ' + JSON.stringify({ csp: acc.csp, errors: acc.errors, warns, alerts: acc.alerts }).slice(0, 700) : undefined);
    const ok = (u) => /^(https?:\/\/tauri\.localhost\/|http:\/\/ipc\.localhost\/|ipc:|blob:|data:)/.test(u);
    const all = [...acc.resources];
    const foreign = all.filter((u) => !ok(u));
    const chunks = all.filter((u) => /\/assets\/.+\.js$/.test(u)).length;
    record('no foreign requests', { resources: all.length, jsChunks: chunks, foreign },
      foreign.length ? 'foreign requests: ' + foreign.join(', ') : (acc.drains.find((d) => d.label === 'browse')?.resources ?? 0) < 20 ? 'fewer than 20 requests in the browse drain: the collector missed the browse phase' : undefined);
  }
  await wd('DELETE', `/session/${s}`);

  // ---- Session C (opt-in): public clones + fetch over the network ----
  if (cfg.network) {
    await sleep(1500);
    s = await newSession();
    await step(s, 'C.start', START_STEP);
    for (const repo of cfg.networkRepos) {
      await step(s, `network clone ${repo}`, `
        await go('#/');
        await menuItem($('button[aria-label="Add a repository"]'), 'Clone URL');
        const d = need(await until(dialog, 3000), 'clone dialog');
        setValue($('#clone-url', d), ${JSON.stringify(repo)});
        need(await until(() => $('[data-testid="clone-preview"]'), 8000), 'preview');
        need(byText('.dialog__footer button', 'Clone'), 'Clone').click();
        await until(() => !dialog(), 5000);
        $('button[aria-label^="Background jobs"]').click();
        const done = need(await until(() => $$('li.job[data-kind="clone"]').find((j) => /done|failed/.test(j.dataset.state) && j.innerText.includes(${JSON.stringify(repo.split('/')[1])})), 600000, 500), 'clone finished');
        const job = txt(done);
        if (done.dataset.state !== 'done') return { error: 'clone failed: ' + job };
        byText('a', 'Open repository', done)?.click();
        await until(() => $('.rh__title'), 10000);
        await settle(30000);
        need(await until(() => byText('.rh__actions button', 'Fetch')), 'Fetch').click();
        const msg = await until(() => { const m = $('.rh__msg'); return m && /new commit|up to date|failed|error|Could not/i.test(m.innerText) ? m : null; }, 300000, 500);
        return { job: job.slice(0, 300), fetch: txt(msg) };
      `);
    }
    await wd('DELETE', `/session/${s}`);
  }
}

try {
  await main();
} catch (e) {
  record('harness', {}, String(e && e.stack || e).slice(0, 600));
}
save();
const failed = results.filter((r) => r.error).length;
console.log(`${results.length} steps, ${failed} failed`);
process.exit(failed ? 1 : 0);
