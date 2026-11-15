// Mock Explore backend (v0.3.0 M10a): behaves like the real one where the UI will depend on it.
import { beforeEach, describe, expect, it } from 'vitest';
import { isIpcError, toIpcError } from '../errors';
import { createMockBackend } from './index';

const mk = (search = '') => createMockBackend(search, { tickMs: 20 });
const rejects = async (p: Promise<unknown>): Promise<{ code: string; message: string }> => {
  try {
    await p;
  } catch (e) {
    return e as { code: string; message: string };
  }
  throw new Error('expected a rejection');
};

describe('mock explore', () => {
  beforeEach(() => globalThis.localStorage?.clear());

  it('browses a repository online with the shelf wire types', async () => {
    const m = mk();
    const repo = await m.invoke('explore_repo', { owner: 'octo', name: 'demo' });
    expect(repo).toMatchObject({ fullName: 'octo/demo', cloneUrl: 'https://github.com/octo/demo.git', onShelf: null });
    const refs = await m.invoke('explore_refs', { owner: 'octo', name: 'demo' });
    expect(refs.refs.remoteBranches).toEqual([]);
    expect(refs.branchesTruncated || refs.tagsTruncated).toBe(false);
    const tree = await m.invoke('explore_tree', { owner: 'octo', name: 'demo', rev: 'HEAD', path: '' });
    expect(tree.entries.length).toBeGreaterThan(0);
    const page = await m.invoke('explore_commits', { owner: 'octo', name: 'demo', rev: 'main', path: null, skip: 0, tip: null });
    expect(page.commits.length).toBeGreaterThan(0);
    expect(await rejects(m.invoke('explore_commits', { owner: 'octo', name: 'demo', rev: 'main', path: null, skip: 7, tip: null }))).toMatchObject({ code: 'invalidInput' });
    // a repo on the mock shelf with a github.com origin is "on the shelf"
    const lumen = [...(await m.invoke('library_list', {}))].find((r) => r.originUrl?.startsWith('https://github.com/'))!;
    const [o, n] = lumen.originUrl!.replace('https://github.com/', '').replace(/\.git$/, '').split('/');
    expect((await m.invoke('explore_repo', { owner: o, name: n })).onShelf).toBe(lumen.id);
    expect(await rejects(m.invoke('explore_save_to_shelf', { owner: o, name: n }))).toMatchObject({ code: 'duplicate' });
  });

  it('validates addresses, revs, paths and oids like the backend', async () => {
    const m = mk();
    for (const [owner, name] of [['..', 'x'], ['a/b', 'x'], ['-x', 'x'], ['octo', '..'], ['octo', 'a/b'], ['octo', '-r'], ['octo', 'r.git'], ['', 'r']]) {
      expect(await rejects(m.invoke('explore_repo', { owner, name })), `${owner}/${name}`).toMatchObject({ code: 'invalidInput' });
    }
    expect(await rejects(m.invoke('explore_tree', { owner: 'octo', name: 'demo', rev: 'a..b', path: '' }))).toMatchObject({ code: 'invalidRev' });
    expect(await rejects(m.invoke('explore_tree', { owner: 'octo', name: 'demo', rev: 'main', path: 'a/../b' }))).toMatchObject({ code: 'invalidPath' });
    expect(await rejects(m.invoke('explore_commit', { owner: 'octo', name: 'demo', oid: 'main' }))).toMatchObject({ code: 'invalidRev' });
    expect(await rejects(m.invoke('explore_repo', { owner: 'octo', name: 'private' }))).toMatchObject({ code: 'notFound' });
    expect(await rejects(m.invoke('explore_search', { query: '', sort: null, page: 1 }))).toMatchObject({ code: 'invalidInput' });
  });

  it('the token is saved, flagged in settings, never returned; the UI cannot set the flag', async () => {
    const m = mk();
    expect((await m.invoke('settings_get', {})).githubTokenSaved).toBe(false);
    const s = await m.invoke('settings_set', { settings: { ...(await m.invoke('settings_get', {})), githubTokenSaved: true } });
    expect(s.githubTokenSaved).toBe(false);
    const st = await m.invoke('explore_token_set', { token: ' ghp_Secret_Value-123 ' });
    expect(st.tokenSaved).toBe(true);
    expect(st.core?.limit).toBe(5000);
    expect(JSON.stringify(st)).not.toContain('Secret');
    expect((await m.invoke('settings_get', {})).githubTokenSaved).toBe(true);
    expect(JSON.stringify(await m.invoke('settings_get', {}))).not.toContain('Secret');
    expect(await rejects(m.invoke('explore_token_set', { token: 'has space' }))).toMatchObject({ code: 'invalidInput' });
    expect(await rejects(m.invoke('explore_token_set', { token: 'ghp_rejected' }))).toMatchObject({ code: 'refused' });
    expect((await m.invoke('explore_token_remove', {})).tokenSaved).toBe(false);
  });

  it('exploreEnabled off refuses every call but status and token handling', async () => {
    const m = mk();
    const s = await m.invoke('settings_get', {});
    await m.invoke('settings_set', { settings: { ...s, exploreEnabled: false } });
    expect(await rejects(m.invoke('explore_repo', { owner: 'octo', name: 'demo' }))).toMatchObject({ code: 'refused' });
    expect(await rejects(m.invoke('explore_search', { query: 'bat', sort: null, page: 1 }))).toMatchObject({ code: 'refused' });
    expect(await rejects(m.invoke('explore_save_to_shelf', { owner: 'octo', name: 'demo' }))).toMatchObject({ code: 'refused' });
    expect((await m.invoke('explore_status', {})).enabled).toBe(false);
  });

  it('?ratelimit: the quota runs out, saved answers stay free, the error carries its own code', async () => {
    const m = mk('?ratelimit');
    await m.invoke('explore_repo', { owner: 'octo', name: 'a' });
    await m.invoke('explore_repo', { owner: 'octo', name: 'b' });
    const st = await m.invoke('explore_status', {});
    expect(st.core?.remaining).toBe(0);
    expect(st.limitedUntil).toBeGreaterThan(Date.now() / 1000);
    await m.invoke('explore_repo', { owner: 'octo', name: 'a' }); // saved answer
    const e = await rejects(m.invoke('explore_repo', { owner: 'octo', name: 'c' }));
    expect(e.code).toBe('rateLimited');
    expect(isIpcError(e)).toBe(true);
    expect(toIpcError(e).code).toBe('rateLimited');
    expect((await m.invoke('explore_cache_clear', {})).cacheEntries).toBe(0);
  });

  it('search pages and Save to shelf starts the clone job', async () => {
    const m = mk();
    const r = await m.invoke('explore_search', { query: 'rust', sort: 'stars', page: 1 });
    expect(r.items.length).toBeGreaterThan(0);
    expect(r.items[0].stars).toBeGreaterThanOrEqual(r.items[r.items.length - 1].stars);
    const id = await m.invoke('explore_save_to_shelf', { owner: 'sharkdp', name: 'bat', autoDownload: false });
    const snap = await m.invoke('jobs_subscribe', { channel: { onmessage: () => {} } });
    expect(snap.find((j) => j.id === id)?.kind).toMatchObject({ type: 'clone', url: 'https://github.com/sharkdp/bat.git', autoDownload: false });
  });
});
