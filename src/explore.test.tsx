// M10b (v0.3.0 frontend): Explore routes, nav entry and the off switch, search, repo pages through the source layer,
// quota header, rate-limit state, Save to shelf / Open in shelf, the too-large placeholder and Settings → Explore
// (token never shown). Sweeps: every in-page link of an Explore page stays under #/explore/ (except Open in shelf and
// Settings), and no Explore page sends a repo id.

import { describe, expect, it } from 'vitest';
import { href, parse } from './router/router';
import { boot, click, go, installHarness, settle, typeInto } from './test/appHarness';

installHarness();

type Invoke = (cmd: string, args: Record<string, unknown>) => Promise<unknown>;

function spy(backend: { invoke: unknown }) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  const orig = (backend.invoke as Invoke).bind(backend);
  (backend as { invoke: Invoke }).invoke = (cmd, args) => {
    calls.push({ cmd, args });
    return orig(cmd, args);
  };
  return calls;
}

const q = (host: HTMLElement, id: string) => host.querySelector(`[data-testid="${id}"]`);
const quotaText = (host: HTMLElement) => q(host, 'explore-quota-remaining')?.textContent ?? '';

/** Links of the page body (not the global header). */
function pageLinks(host: HTMLElement): string[] {
  return [...host.querySelectorAll('main a[href^="#"]')]
    .filter((a) => a.getAttribute('data-testid') !== 'explore-open-in-shelf')
    .map((a) => a.getAttribute('href')!);
}

function expectExploreLinks(host: HTMLElement) {
  const bad = pageLinks(host).filter((h) => !h.startsWith('#/explore/') && h !== '#/settings' && !h.startsWith('#user-content-'));
  expect(bad).toEqual([]);
}

describe('Explore routes', () => {
  it('parse and href keep the explore flag; the search page has two spellings', () => {
    expect(parse('#/explore/octo/demo')).toMatchObject({ name: 'code', params: { owner: 'octo', repo: 'demo', explore: true } });
    expect(parse('#/explore/octo/demo/tree/main/src/lib')).toMatchObject({
      name: 'tree',
      params: { owner: 'octo', repo: 'demo', ref: 'main', path: 'src/lib', explore: true },
    });
    expect(parse('#/explore/octo/demo/commit/abc')).toMatchObject({ name: 'commit', params: { oid: 'abc', explore: true } });
    expect(parse('#/explore/search?q=rust+cli')).toMatchObject({ name: 'exploreSearch', query: { q: 'rust cli' } });
    expect(parse('#/explore')).toMatchObject({ name: 'exploreSearch' });
    expect(parse('#/explore/just-one').name).toBe('notFound');
    expect(parse('#/northwind/parsekit').params.explore).toBeUndefined();
    expect(href('tree', { explore: true, owner: 'octo', repo: 'demo', ref: 'feature/x', path: 'a b' })).toBe('#/explore/octo/demo/tree/feature%2Fx/a%20b');
    expect(href('code', { explore: false, owner: 'octo', repo: 'demo' })).toBe('#/octo/demo');
    expect(href('exploreSearch', {}, { q: 'ripgrep', sort: null })).toBe('#/explore/search?q=ripgrep');
    // `explore/search/x` is a repository called x of the owner "search"
    expect(parse('#/explore/search/x')).toMatchObject({ name: 'code', params: { owner: 'search', repo: 'x', explore: true } });
  });
});

describe('Explore entry and the off switch', () => {
  it('shows the nav entry; turning Explore off removes it and makes no Explore request', async () => {
    const { host, backend } = await boot('', '#/settings');
    await settle(100);
    expect(q(host, 'nav-explore')?.getAttribute('href')).toBe('#/explore/search');
    const calls = spy(backend);
    await click(q(host, 'explore-enabled'));
    await settle(100);
    expect(q(host, 'nav-explore')).toBeNull();
    await go('#/explore/octo/demo', 300);
    expect(q(host, 'explore-off')).not.toBeNull();
    await go('#/explore/search?q=rust', 300);
    expect(q(host, 'explore-off')).not.toBeNull();
    expect(calls.filter((c) => c.cmd.startsWith('explore_') && c.cmd !== 'explore_status')).toEqual([]);
    // the Settings section stays usable while off (D-119)
    await go('#/settings', 200);
    expect(q(host, 'explore-token-input')).not.toBeNull();
  });
});

describe('Explore repo pages', () => {
  it('a cold repository page costs 5 requests, shows the quota and links only inside Explore', async () => {
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/explore/octo/demo', 900);
    expect(host.querySelector('[data-testid="explore-header"]')?.textContent).toContain('octo');
    expect(host.querySelectorAll('[data-testid="file-table"] [data-kind]').length).toBeGreaterThan(0);
    expect(host.querySelector('#readme')).not.toBeNull();
    expect(host.querySelector('[data-testid="latest-commit"]')?.textContent).toMatch(/Commits/);
    expect(quotaText(host)).toContain('55 of 60');
    expect(q(host, 'explore-files-hint')).not.toBeNull();
    expect(q(host, 'explore-save')).not.toBeNull();
    expectExploreLinks(host);
    // never an id or a path; no local command
    for (const c of calls) {
      expect(c.args).not.toHaveProperty('repoId');
      expect(['repo_by_slug', 'repo_detail', 'tree_list', 'rev_resolve', 'commit_latest']).not.toContain(c.cmd);
    }
    // the same page again is free
    await go('#/explore/octo/demo/tree/main', 500);
    expect(quotaText(host)).toContain('55 of 60');
  });

  it('browses code, a file, history, a commit, branches, tags and releases (lumen is also on the shelf)', async () => {
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/explore/lumen-ui/lumen', 900);
    expect(q(host, 'explore-saved')).not.toBeNull();
    expect(q(host, 'explore-open-in-shelf')?.getAttribute('href')).toBe('#/lumen-ui/lumen');

    await go('#/explore/lumen-ui/lumen/tree/main/src', 600);
    expect(host.querySelector('[data-testid="file-table"]')?.textContent).toContain('index.ts');
    expect(host.querySelector('.crumbs')?.textContent).toContain('src');
    expectExploreLinks(host);

    await go('#/explore/lumen-ui/lumen/blob/main/src/index.ts', 600);
    expect(host.querySelector('.filebox')?.textContent).toMatch(/lines|Raw/);
    expect(host.textContent).not.toContain('Save a copy');
    expectExploreLinks(host);

    await go('#/explore/lumen-ui/lumen/commits/main', 600);
    const rows = host.querySelectorAll('.crow');
    expect(rows.length).toBeGreaterThan(0);
    expectExploreLinks(host);
    await click(rows[0].querySelector('a.crow__summary'));
    await settle(600);
    expect(host.querySelector('.chead')).not.toBeNull();
    expectExploreLinks(host);

    await go('#/explore/lumen-ui/lumen/branches', 500);
    expect(host.querySelectorAll('[data-testid="online-branches"] .brow').length).toBeGreaterThan(0);
    expectExploreLinks(host);
    await go('#/explore/lumen-ui/lumen/tags', 500);
    expect(host.querySelectorAll('[data-testid="online-tags"] .trow').length).toBeGreaterThan(0);
    expectExploreLinks(host);
    await go('#/explore/lumen-ui/lumen/releases', 600);
    expect(host.querySelectorAll('.rcard').length).toBeGreaterThan(0);
    expect(host.querySelector('.rasset')).not.toBeNull();
    expect([...host.querySelectorAll('.rasset button')].map((b) => b.textContent)).not.toContain('Download');
    expectExploreLinks(host);

    for (const c of calls) expect(c.args).not.toHaveProperty('repoId');
  });

  it('a tag route reaches the backend as the short tag name', async () => {
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/explore/lumen-ui/lumen/tree/refs%2Ftags%2Fv2.0.0-beta.1', 900);
    const revs = calls.filter((c) => ['explore_resolve', 'explore_tree', 'explore_commit_latest'].includes(c.cmd)).map((c) => c.args.rev);
    expect(revs.length).toBeGreaterThan(2);
    expect(new Set(revs)).toEqual(new Set(['v2.0.0-beta.1']));
    expect(host.querySelector('.refswitch')?.textContent).toContain('v2.0.0-beta.1');
    expect(host.querySelector('[data-testid="file-table"] [data-kind]')).not.toBeNull();
  });

  it('files over 1 MB show the placeholder with Save to shelf and no content request', async () => {
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/explore/octo/demo/blob/main/data/big-dump.csv', 700);
    const ph = host.querySelector('[data-placeholder="large"]');
    expect(ph?.textContent).toContain('Too large to preview online — Save to shelf');
    expect(ph?.querySelector('[data-testid="explore-save"]')).not.toBeNull();
    expect(calls.filter((c) => c.cmd === 'explore_blob_text')).toEqual([]);
  });

  it('an unknown or private repository says so, with the token hint', async () => {
    const { host } = await boot('', '#/');
    await go('#/explore/octo/private', 600);
    const err = q(host, 'explore-error');
    expect(err?.getAttribute('data-code')).toBe('notFound');
    expect(err?.textContent).toContain('token');
  });
});

describe('Save to shelf', () => {
  it('starts the clone job, then offers Open in shelf from the library entry', async () => {
    const { host, backend } = await boot('', '#/');
    const calls = spy(backend);
    await go('#/explore/octo/demo', 800);
    await click(q(host, 'explore-save'));
    await settle(1500);
    expect(calls.filter((c) => c.cmd === 'explore_save_to_shelf').map((c) => c.args)).toEqual([{ owner: 'octo', name: 'demo', autoDownload: null }]);
    const open = q(host, 'explore-open-in-shelf');
    expect(open).not.toBeNull();
    expect(open!.getAttribute('href')).toBe('#/octo/demo');
    expect(q(host, 'explore-save')).toBeNull();
  });
});

describe('Explore search', () => {
  it('searches on submit only, lists results with Open and Save, and offers a direct open for owner/name', async () => {
    const { host, backend } = await boot('', '#/explore/search');
    await settle(200);
    const calls = spy(backend);
    const input = q(host, 'explore-search-input');
    await typeInto(input, 'ripgrep');
    await settle(300);
    expect(calls.filter((c) => c.cmd === 'explore_search')).toEqual([]);
    await typeInto(input, 'octo/demo');
    expect(q(host, 'explore-direct')?.getAttribute('href')).toBe('#/explore/octo/demo');
    await typeInto(input, 'ripgrep');
    await act_submit(host);
    await settle(600);
    expect(calls.filter((c) => c.cmd === 'explore_search')).toHaveLength(1);
    expect(location.hash).toBe('#/explore/search?q=ripgrep');
    const row = host.querySelector('[data-repo="BurntSushi/ripgrep"]');
    expect(row).not.toBeNull();
    expect(row!.querySelector('[data-testid="explore-open"]')?.getAttribute('href')).toBe('#/explore/BurntSushi/ripgrep');
    expect(row!.querySelector('[data-testid="explore-save"]')).not.toBeNull();
    expect(row!.textContent).toMatch(/★|Updated|Rust/);
    expect(quotaText(host)).toContain('of 10 searches left');
    expectExploreLinks(host);
  });
});

async function act_submit(host: HTMLElement) {
  const { act } = await import('react');
  await act(async () => {
    host.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  });
}

describe('Rate limit', () => {
  it('says when the limit resets and shows the failing page as such', async () => {
    const { host } = await boot('?ratelimit', '#/');
    await go('#/explore/octo/limited', 900);
    expect(q(host, 'explore-rate-limit')?.textContent).toMatch(/Rate limit reached.*resets at/);
    const err = q(host, 'explore-error');
    expect(err?.getAttribute('data-code')).toBe('rateLimited');
  });
});

describe('Settings → Explore', () => {
  it('the token is kept out of the page, history, URL and storage; Remove brings the field back', async () => {
    const { host, backend } = await boot('', '#/settings');
    await settle(200);
    const calls = spy(backend);
    const secret = 'ghp_Secret_Value-123';
    expect(q(host, 'explore-token-saved')).toBeNull();
    await typeInto(q(host, 'explore-token-input'), secret);
    await click(q(host, 'explore-token-save'));
    await settle(300);
    expect(q(host, 'explore-token-saved')?.textContent).toContain('Token saved');
    expect(q(host, 'explore-token-input')).toBeNull();
    expect(document.body.innerHTML).not.toContain('Secret_Value');
    expect(location.href).not.toContain('Secret');
    expect(JSON.stringify(history.state)).not.toContain('Secret');
    expect(JSON.stringify({ ...localStorage })).not.toContain('Secret');
    expect(JSON.stringify({ ...sessionStorage })).not.toContain('Secret');
    // the token goes only into the one token command
    expect(calls.filter((c) => JSON.stringify(c.args).includes('Secret')).map((c) => c.cmd)).toEqual(['explore_token_set']);
    expect(quotaText(host)).toBe(''); // not on a page with the strip
    await click(q(host, 'explore-token-remove'));
    await settle(300);
    expect(q(host, 'explore-token-input')).not.toBeNull();
    expect((q(host, 'explore-token-input') as HTMLInputElement).value).toBe('');
  });

  it('a refused token shows the error and leaves an empty field', async () => {
    const { host } = await boot('', '#/settings');
    await settle(200);
    await typeInto(q(host, 'explore-token-input'), 'ghp_rejected');
    await click(q(host, 'explore-token-save'));
    await settle(300);
    expect(q(host, 'explore-token-msg')?.textContent).toMatch(/did not accept/);
    expect((q(host, 'explore-token-input') as HTMLInputElement).value).toBe('');
    expect(document.body.innerHTML).not.toContain('ghp_rejected');
  });

  it('Clear Explore cache empties the saved answers and the quota strip stays consistent', async () => {
    const { host } = await boot('', '#/');
    await go('#/explore/octo/demo', 900);
    await go('#/settings', 400);
    expect(q(host, 'explore-cache-size')?.textContent).toMatch(/[1-9]\d* saved answers?/);
    await click(q(host, 'explore-cache-clear'));
    await settle(300);
    expect(q(host, 'explore-cache-msg')?.textContent).toMatch(/Cleared \d+ saved answers?/);
    expect(q(host, 'explore-cache-size')?.textContent).toMatch(/^0 saved answers/);
  });
});
