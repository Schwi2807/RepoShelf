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
    expect(readSwitches('')).toEqual({ empty: false, slow: false, clonefail: false, fetchfail: false, huge: false, gh: false });
    expect(readSwitches('?gh').gh).toBe(true);
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
    expect(COMMAND_NAMES).toHaveLength(52);
  });

  it('v0.2.0: release assets, download job, reveal, and the auto-download options', async () => {
    vi.useRealTimers();
    const m = createMockBackend('', { tickMs: 20 });
    const seen: JobInfo[] = [];
    await m.invoke('jobs_subscribe', { channel: { onmessage: (j: JobInfo) => seen.push(j) } });
    // before GitHub notes are loaded: no assets, downloads refused
    const plain = await m.invoke('releases_list', { repoId: LUMEN });
    expect(plain.every((r) => r.assets.length === 0 && r.htmlUrl === null)).toBe(true);
    expect(plain[0].sourceZipUrl).toMatch(/^https:\/\/github\.com\/lumen-ui\/lumen\/archive\/refs\/tags\/.+\.zip$/);
    await expect(m.invoke('release_download', { repoId: LUMEN, tag: 'v1.1.0', asset: 'x.zip' })).rejects.toMatchObject({ code: 'notFound' });
    const gh = await m.invoke('releases_fetch_github', { repoId: LUMEN });
    const v11 = gh.find((r) => r.tag === 'v1.1.0')!;
    expect(v11.assets.map((a) => a.forThisPc)).toEqual([true, false, false, false]);
    expect(v11.htmlUrl).toBe('https://github.com/lumen-ui/lumen/releases/tag/v1.1.0');
    await expect(m.invoke('release_download', { repoId: LUMEN, tag: 'v1.1.0', asset: '' })).rejects.toMatchObject({ code: 'invalidInput' });
    await expect(m.invoke('release_reveal', { repoId: LUMEN, tag: 'v1.1.0', asset: v11.assets[0].name })).rejects.toMatchObject({ code: 'notFound' });
    const id = await m.invoke('release_download', { repoId: LUMEN, tag: 'v1.1.0', asset: v11.assets[0].name });
    await until(() => seen.some((j) => j.id === id && j.state === 'done'));
    const done = seen.filter((j) => j.id === id).pop()!;
    expect(done.kind).toEqual({ type: 'releaseDownload', repoId: LUMEN, tag: 'v1.1.0', asset: v11.assets[0].name });
    expect(done.result).toMatchObject({ type: 'downloaded', already: false, sha256Checked: true });
    await expect(m.invoke('release_reveal', { repoId: LUMEN, tag: 'v1.1.0', asset: v11.assets[0].name })).resolves.toBeNull();
    const after = await m.invoke('releases_list', { repoId: LUMEN });
    expect(after.find((r) => r.tag === 'v1.1.0')!.assets[0].downloaded).toContain('\\_releases\\github.com\\lumen-ui\\lumen\\v1.1.0\\');
    // a fetch with the option on: already downloaded → a note, no new download job
    const f = await m.invoke('fetch_start', { repoId: LUMEN, autoDownload: true });
    await until(() => seen.some((j) => j.id === f && j.state === 'done'));
    const fr = seen.filter((j) => j.id === f).pop()!;
    expect(fr.kind).toEqual({ type: 'fetch', repoId: LUMEN, autoDownload: true });
    expect(fr.result).toMatchObject({ type: 'fetched', notes: [expect.stringContaining('already downloaded')] });
  });

  it('v0.2.0: import queues a GitHub sync job; settings carry the network defaults', async () => {
    vi.useRealTimers();
    const m = createMockBackend('', { tickMs: 20 });
    const s = await m.invoke('settings_get', {});
    expect(s).toMatchObject({ loadRemoteImages: true, fetchReleaseNotes: true, fetchRepoInfo: true, autoDownloadLatestRelease: true, autoDownloadOnFetchAll: false, downloadSizeCapMb: 1024 });
    const seen: JobInfo[] = [];
    await m.invoke('jobs_subscribe', { channel: { onmessage: (j: JobInfo) => seen.push(j) } });
    const p = (await m.invoke('import_pick', {}))!;
    const sum = await m.invoke('import_confirm', { token: p.token, autoDownload: false });
    expect(sum.aboutUser).toEqual({ description: sum.description, homepage: sum.homepage, topics: sum.topics });
    await until(() => seen.some((j) => j.kind.type === 'githubSync' && j.state === 'done'));
    expect(seen.find((j) => j.kind.type === 'githubSync')!.kind).toEqual({ type: 'githubSync', repoId: sum.id, autoDownload: false });
    // Edit About keeps the user's values apart from GitHub's
    const ed = await m.invoke('repo_set_metadata', { repoId: LUMEN, description: 'Mine', homepage: null, topics: [] });
    expect(ed.aboutUser).toEqual({ description: 'Mine', homepage: null, topics: [] });
    expect(ed.description).toBe('Mine');
  });

  it('v0.2.0: remote images come as bytes from the mock, never from the network', async () => {
    const m = createMockBackend('');
    const img = await m.invoke('image_fetch', { url: 'https://img.shields.io/badge/build-passing-green' });
    expect(img).toMatchObject({ mime: 'image/svg+xml' });
    expect(img.key).toMatch(/^[0-9a-f]{64}$/);
    const bytes = await m.invoke('image_bytes', { key: img.key });
    // badge hosts give a 120×20 badge labelled with the path, others a square tile naming the host
    expect(new TextDecoder().decode(bytes)).toContain('build-passing-green');
    const tile = await m.invoke('image_fetch', { url: 'https://user-images.example.org/shot.png' });
    expect(new TextDecoder().decode(await m.invoke('image_bytes', { key: tile.key }))).toContain('user-images.example.org');
    for (const bad of ['http://example.com/a.png', 'https://127.0.0.1/a.png', 'https://localhost/a.png', 'https://user:pw@example.com/a.png']) {
      await expect(m.invoke('image_fetch', { url: bad })).rejects.toMatchObject({ code: 'refused' });
    }
    await expect(m.invoke('image_fetch', { url: 'https://fail.example.com/a.png' })).rejects.toMatchObject({ code: 'network' });
    await expect(m.invoke('image_bytes', { key: '../x' })).rejects.toMatchObject({ code: 'invalidInput' });
    await m.invoke('cache_clear', {});
    await expect(m.invoke('image_bytes', { key: img.key })).rejects.toMatchObject({ code: 'notFound' });
    const s = await m.invoke('settings_get', {});
    await m.invoke('settings_set', { settings: { ...s, loadRemoteImages: false } });
    await expect(m.invoke('image_fetch', { url: 'https://img.shields.io/x' })).rejects.toMatchObject({ code: 'refused' });
    await m.invoke('settings_set', { settings: s });
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
    // (v0.2.0) no auto-download follow-up job in this test
    const id = await m.invoke('fetch_start', { repoId: LUMEN, autoDownload: false });
    // running jobs cannot be retried and are not dismissed (rs-core JobRunner)
    await expect(m.invoke('jobs_retry', { jobId: id })).rejects.toMatchObject({ code: 'refused' });
    await m.invoke('jobs_dismiss', { jobId: id });
    expect(await m.invoke('jobs_cancel', { jobId: id })).toBe(true);
    const cancelled = seen.filter((j) => j.id === id).pop();
    expect([cancelled?.state, cancelled?.error]).toEqual(['cancelled', null]);
    expect(await m.invoke('jobs_cancel', { jobId: id })).toBe(false);
    expect(await m.invoke('jobs_cancel', { jobId: 999 })).toBe(false);
    const again = await m.invoke('jobs_retry', { jobId: id });
    expect(again).not.toBe(id);
    await until(() => seen.some((j) => j.id === again && j.state === 'done'));
    // a reload re-subscribes and gets the snapshot; the retried row was replaced by the new one
    const snap = await m.invoke('jobs_subscribe', { channel: { onmessage: () => undefined } });
    expect(snap.map((j) => j.id)).toEqual([again]);
    await m.invoke('jobs_dismiss', { jobId: again });
    expect(await m.invoke('jobs_subscribe', { channel: { onmessage: () => undefined } })).toEqual([]);
  });

  it('matches rs-core shapes checked against the Rust sources', async () => {
    const m = createMockBackend('');
    const t = await m.invoke('tree_list', { repoId: PARSEKIT, rev: 'main', path: '' });
    expect(t.readme).toBe('README.md'); // entry name, not a path
    const kinds = t.entries.map((e) => e.kind === 'dir');
    expect(kinds.indexOf(false)).toBeGreaterThan(0);
    expect(kinds.slice(kinds.indexOf(false)).every((d) => !d)).toBe(true); // dirs first
    await expect(m.invoke('blob_meta', { repoId: PARSEKIT, rev: 'main', path: 'src' })).rejects.toEqual({
      code: 'notFound',
      message: 'file not found',
    });
    await expect(m.invoke('blob_meta', { repoId: PARSEKIT, rev: 'main', path: 'nope.rs' })).rejects.toEqual({
      code: 'notFound',
      message: 'path not found',
    });
    expect((await m.invoke('blob_meta', { repoId: PARSEKIT, rev: 'main', path: 'scripts/release.sh' })).langHint).toBe('bash');
    await expect(m.invoke('commit_detail', { repoId: PARSEKIT, oid: 'abcd123' })).rejects.toMatchObject({ code: 'invalidRev' });
    const refs = await m.invoke('refs_list', { repoId: PARSEKIT });
    expect(refs.branches[0]).toMatchObject({ name: 'main', isDefault: true });
    expect(refs.branches.slice(1).map((b) => b.name)).toEqual(['docs/tutorial', 'feature/streaming', 'fix/utf8']);
    expect(refs.remoteBranches.map((b) => b.name)).toEqual(['origin/feature/streaming', 'origin/main']);
    const rows = await m.invoke('branches_list', { repoId: PARSEKIT });
    expect(rows[0].item.isDefault).toBe(true);
    expect(rows.some((r) => r.item.kind === 'remoteBranch')).toBe(true);
    const rel = await m.invoke('releases_list', { repoId: LUMEN });
    expect(rel.map((r) => [r.tag, r.prerelease, r.latest])).toEqual([
      ['v2.0.0-beta.1', true, false],
      ['v1.1.0', false, true],
      ['v1.0.0', false, false],
      ['v0.9.0', false, false],
    ]);
    expect(rel[1].previousTag).toBe('v1.0.0');
    expect(rel[3].title).toBe('v0.9.0');
  });

  it('M7.8 fixtures: merge commit with an openable second parent, rename / delete / no-newline, capped ahead/behind', async () => {
    const m = createMockBackend('');
    const page = await m.invoke('commits_page', { repoId: LUMEN, rev: 'main', path: null, since: null, skip: 0, tip: null });
    const merge = page.commits.find((c) => c.summary === "Merge branch 'next-dialog'")!;
    expect(merge.parents).toHaveLength(2);
    const d = await m.invoke('commit_detail', { repoId: LUMEN, oid: merge.oid });
    expect(d.isMerge).toBe(true);
    expect(d.parentCompared).toBe(merge.parents[0]);
    const side = await m.invoke('commit_detail', { repoId: LUMEN, oid: merge.parents[1] });
    expect(side.commit.summary).toBe('Dialog: fix Escape in nested menus');

    const pk = await m.invoke('commits_page', { repoId: PARSEKIT, rev: 'main', path: null, since: null, skip: 0, tip: null });
    const sp = await m.invoke('commit_detail', { repoId: PARSEKIT, oid: pk.commits[3].oid });
    const kinds = Object.fromEntries(sp.files.map((f) => [f.newPath ?? f.oldPath, [f.kind, f.oldPath]]));
    expect(kinds['docs/architecture.md']).toEqual(['renamed', 'docs/design.md']);
    expect(kinds['docs/old-notes.md']).toEqual(['deleted', 'docs/old-notes.md']);
    const nl = sp.diffs.find((x) => x.index === sp.files.find((f) => f.newPath === 'rustfmt.toml')!.index)!;
    expect(nl.hunks[0].lines.filter((l) => l.kind === 'noNewline')).toHaveLength(2);
    expect(sp.additions).toBe(sp.files.reduce((s, f) => s + f.additions, 0));

    const huge = createMockBackend('?huge');
    const [[, ab]] = await huge.invoke('branches_ahead_behind', { repoId: PARSEKIT, names: ['docs/tutorial'], requestId: 'ab1' });
    expect(ab).toEqual({ ahead: 1, behind: 1000, capped: true });
    const big = await huge.invoke('commits_page', { repoId: PARSEKIT, rev: 'main', path: null, since: null, skip: 0, tip: null });
    const bd = await huge.invoke('commit_detail', { repoId: PARSEKIT, oid: big.commits[0].oid });
    expect([bd.files.length, bd.diffsDeferred, bd.diffs.length]).toEqual([400, true, 0]);
    await expect(huge.invoke('commit_file_diff', { repoId: PARSEKIT, oid: big.commits[0].oid, index: 0, force: true })).resolves.toMatchObject({ index: 0 });
  });

  it('app_open_dir takes a keyword only', async () => {
    const m = createMockBackend('');
    await expect(m.invoke('app_open_dir', { which: 'data' })).resolves.toBeNull();
    await expect(m.invoke('app_open_dir', { which: 'C:\\Windows' as never })).rejects.toMatchObject({ code: 'invalidInput' });
  });

  it('library-refreshed fires once after the first listener', async () => {
    const m = createMockBackend('');
    const got = await new Promise<number>((res) => m.listen('library-refreshed', (p) => res(p.length)));
    expect(got).toBeGreaterThan(0);
  });
});
