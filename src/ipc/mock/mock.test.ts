import { describe, expect, it, vi } from 'vitest';
import { COMMAND_NAMES } from '../commands';
import type { JobInfo } from '../types';
import { createMockBackend } from './index';
import { readSwitches } from './switches';

const PARSEKIT = 1;
const LUMEN = 2;

async function until(cond: () => boolean, ms = 8000): Promise<void> {
  const t0 = Date.now();
  while (!cond()) {
    if (Date.now() - t0 > ms) throw new Error('timeout');
    await new Promise((r) => setTimeout(r, 10));
  }
}

describe('switches', () => {
  it('parses the search string', () => {
    expect(readSwitches('?slow&huge')).toMatchObject({ slow: true, huge: true, empty: false });
    expect(readSwitches('')).toEqual({ empty: false, slow: false, clonefail: false, fetchfail: false, huge: false });
    expect(readSwitches('?empty=0').empty).toBe(false);
  });

  it('?empty → no repos', async () => {
    const m = createMockBackend('?empty');
    expect(await m.invoke('library_list', {})).toEqual([]);
    expect((await m.invoke('app_info', {})).repoCount).toBe(0);
    expect((await createMockBackend('').invoke('library_list', {})).length).toBeGreaterThanOrEqual(5);
  });

  it('?slow delays every call by ~600 ms', async () => {
    const fast = createMockBackend('');
    let t0 = Date.now();
    await fast.invoke('app_info', {});
    expect(Date.now() - t0).toBeLessThan(200);
    const slow = createMockBackend('?slow');
    t0 = Date.now();
    await slow.invoke('app_info', {});
    expect(Date.now() - t0).toBeGreaterThanOrEqual(580);
  });

  it('?huge → 1,000-entry dir, 10k-commit history, 400-file commit', async () => {
    const m = createMockBackend('?huge');
    const t = await m.invoke('tree_list', { repoId: PARSEKIT, rev: 'main', path: 'generated' });
    expect(t.entries).toHaveLength(1000);
    const n = await m.invoke('commit_count', { repoId: PARSEKIT, rev: 'main', requestId: 'c1' });
    expect(n).toBe(10_000);
    const tip = (await m.invoke('rev_resolve', { repoId: PARSEKIT, rev: 'main' })).commit;
    const d = await m.invoke('commit_detail', { repoId: PARSEKIT, oid: tip });
    expect(d.files).toHaveLength(400);
    expect(d.diffsDeferred).toBe(true);
    const normal = createMockBackend('');
    await expect(normal.invoke('tree_list', { repoId: PARSEKIT, rev: 'main', path: 'generated' })).rejects.toMatchObject({
      code: 'notFound',
    });
  });

  it('?clonefail fails clone jobs, ?fetchfail fails fetch jobs', async () => {
    for (const [search, start, failed] of [
      ['?clonefail', 'clone', true],
      ['?fetchfail', 'fetch', true],
      ['', 'clone', false],
      ['', 'fetch', false],
    ] as const) {
      const m = createMockBackend(search, { tickMs: 1 });
      const seen: JobInfo[] = [];
      await m.invoke('jobs_subscribe', { channel: { onmessage: (j) => seen.push(j) } });
      const id =
        start === 'clone'
          ? await m.invoke('clone_start', { input: 'example-org/new-thing' })
          : await m.invoke('fetch_start', { repoId: LUMEN });
      await until(() => seen.some((j) => j.id === id && (j.state === 'done' || j.state === 'failed')));
      const last = seen.filter((j) => j.id === id).pop()!;
      expect(last.state).toBe(failed ? 'failed' : 'done');
      if (failed) expect(last.error).toMatch(/Could not reach/);
      expect(seen.some((j) => j.id === id && j.progress?.type === 'receiving')).toBe(true);
    }
  });
});

describe('backend behaviour the UI relies on', () => {
  it('implements every command', () => {
    expect(COMMAND_NAMES).toHaveLength(47);
  });

  it('rejects with { code, message }', async () => {
    const m = createMockBackend('');
    await expect(m.invoke('repo_by_slug', { owner: 'no', name: 'pe' })).rejects.toEqual({
      code: 'notFound',
      message: 'repository not found',
    });
    await expect(m.invoke('rev_resolve', { repoId: PARSEKIT, rev: 'nope' })).rejects.toMatchObject({ code: 'notFound' });
    await expect(m.invoke('repo_detail', { repoId: 6 })).rejects.toMatchObject({ code: 'repoMissing' });
    await expect(m.invoke('rev_resolve', { repoId: 5, rev: 'main' })).rejects.toMatchObject({ code: 'notFound' });
    await expect(m.invoke('clone_preview', { input: 'http://example.com/a/b' })).rejects.toMatchObject({
      code: 'invalidUrl',
      message: 'Only https:// addresses are allowed (http:// is not encrypted).',
    });
    await expect(m.invoke('open_external', { url: 'javascript:alert(1)' })).rejects.toMatchObject({ code: 'invalidUrl' });
  });

  it('resolves refs: branch with a slash, tag, refs/tags/, short oid', async () => {
    const m = createMockBackend('');
    const b = await m.invoke('rev_resolve', { repoId: PARSEKIT, rev: 'feature/streaming' });
    expect(b.kind).toBe('branch');
    const t = await m.invoke('rev_resolve', { repoId: PARSEKIT, rev: 'refs/tags/v0.2.0' });
    expect(t.kind).toBe('tag');
    const c = await m.invoke('rev_resolve', { repoId: PARSEKIT, rev: b.commit.slice(0, 7) });
    expect(c.commit).toBe(b.commit);
    expect(c.kind).toBe('commit');
  });

  it('a cancelled requestId rejects with `cancelled` (also when cancelled before it started)', async () => {
    const m = createMockBackend('');
    const p = m.invoke('tree_last_commits', { repoId: PARSEKIT, rev: 'main', path: '', requestId: 'r-1' });
    await m.invoke('request_cancel', { requestId: 'r-1' });
    await expect(p).rejects.toEqual({ code: 'cancelled', message: 'cancelled' });
    await m.invoke('request_cancel', { requestId: 'r-2' });
    await expect(m.invoke('commit_count', { repoId: PARSEKIT, rev: 'main', requestId: 'r-2' })).rejects.toMatchObject({
      code: 'cancelled',
    });
    await expect(m.invoke('commit_count', { repoId: PARSEKIT, rev: 'main', requestId: 'bad id!' })).rejects.toMatchObject({
      code: 'invalidInput',
    });
  });

  it('pages commits over a pinned tip with skip', async () => {
    const m = createMockBackend('');
    const p1 = await m.invoke('commits_page', { repoId: PARSEKIT, rev: 'main', path: null, since: null, skip: 0, tip: null });
    expect(p1.commits).toHaveLength(35);
    expect(p1.hasMore).toBe(true);
    const p2 = await m.invoke('commits_page', { repoId: PARSEKIT, rev: 'main', path: null, since: null, skip: p1.skip, tip: p1.tip });
    expect(p2.tip).toBe(p1.tip);
    expect(p2.commits[0].oid).not.toBe(p1.commits[0].oid);
    expect(p2.commits[0].oid).toBe(p1.commits[34].parents[0]);
    const filtered = await m.invoke('commits_page', { repoId: PARSEKIT, rev: 'main', path: 'src/lib.rs', since: null, skip: 0, tip: null });
    expect(filtered.scanned).toBeGreaterThan(filtered.commits.length);
    const since = await m.invoke('commits_page', { repoId: PARSEKIT, rev: 'v0.3.0', path: null, since: 'v0.2.0', skip: 0, tip: null });
    expect(since.commits).toHaveLength(5);
    expect(since.hasMore).toBe(false);
  });

  it('blob_bytes gives an ArrayBuffer; meta carries the image mime', async () => {
    const m = createMockBackend('');
    const meta = await m.invoke('blob_meta', { repoId: PARSEKIT, rev: 'main', path: 'docs/logo.svg' });
    expect(meta.kind).toEqual({ type: 'image', mime: 'image/svg+xml' });
    const bytes = await m.invoke('blob_bytes', { repoId: PARSEKIT, rev: 'main', path: 'docs/screenshot.png' });
    expect(bytes).toBeInstanceOf(ArrayBuffer);
    expect(new Uint8Array(bytes).slice(1, 4)).toEqual(new Uint8Array([0x50, 0x4e, 0x47]));
  });

  it('README HTML is ammonia-shaped (rsrepo: links/images, user-content ids, language classes)', async () => {
    const m = createMockBackend('');
    const r = await m.invoke('readme_render', { repoId: PARSEKIT, rev: 'main', path: '' });
    expect(r?.html).toContain('src="rsrepo:/docs/logo.svg"');
    expect(r?.html).toContain('id="user-content-usage"');
    expect(r?.html).toContain('class="language-rust"');
    expect(r?.html).toContain('<table>');
  });

  it('jobs: snapshot, cancel, retry, dismiss', async () => {
    vi.useRealTimers();
    const m = createMockBackend('', { tickMs: 30 });
    const seen: JobInfo[] = [];
    const snap0 = await m.invoke('jobs_subscribe', { channel: { onmessage: (j) => seen.push(j) } });
    expect(snap0).toEqual([]);
    const id = await m.invoke('fetch_start', { repoId: LUMEN });
    expect(await m.invoke('jobs_cancel', { jobId: id })).toBe(true);
    expect(seen.filter((j) => j.id === id).pop()?.state).toBe('cancelled');
    const again = await m.invoke('jobs_retry', { jobId: id });
    expect(again).not.toBe(id);
    await until(() => seen.some((j) => j.id === again && j.state === 'done'));
    // a reload re-subscribes and gets the snapshot
    const snap = await m.invoke('jobs_subscribe', { channel: { onmessage: () => undefined } });
    expect(snap.map((j) => j.id)).toEqual([id, again]);
    await m.invoke('jobs_dismiss', { jobId: id });
    expect((await m.invoke('jobs_subscribe', { channel: { onmessage: () => undefined } })).map((j) => j.id)).toEqual([again]);
  });

  it('library-refreshed fires once after the first listener', async () => {
    const m = createMockBackend('');
    const got = await new Promise<number>((res) => m.listen('library-refreshed', (p) => res(p.length)));
    expect(got).toBeGreaterThan(0);
  });
});
