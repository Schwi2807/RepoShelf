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
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const cfg = JSON.parse(readFileSync(process.argv[2], 'utf8').replace(/^\uFEFF/, ''));
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

async function main() {
  // ---- Session A: start timing only (first launch after the build: cold-ish file cache) ----
  let s = await newSession();
  await startTiming(s, 'A.start');
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
      need(byText('.dialog__footer button', 'Import'), 'Import button').click();
      await until(() => !dialog(), 10000);
      await until(() => location.hash.split('/').length >= 3, 10000);
      await settle();
      return { summary: summary.slice(0, 160), route: location.hash };
    `);
    imported.push(r.route);
  }
  record('imports', { routes: imported }, imported.length === cfg.imports.length && imported.every(Boolean) ? undefined : 'some imports failed');

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
  const log3 = readFileSync(cfg.openLog, 'utf8');
  record('no browser opened', { log: log3.trim() }, /^url\t/m.test(log3) ? 'a URL was opened' : undefined);

  // Imports, every screen in both themes, worker highlighting, security README: drained before the probes so any
  // console noise from the hostile calls is told apart from the app's own.
  await drain(s, 'browse');

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
