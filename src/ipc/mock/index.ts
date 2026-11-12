// Browser mock backend (FR-37, ARCH §7.4). Implements every command of `commands.ts` (the handler table is typed
// from it, so a missing command fails typecheck) over in-memory fixtures. Behaves like the real backend where the
// UI depends on it: rejections are `{ code, message }`, cancelled requestIds reject with `cancelled`, commits page
// by tip + skip, `blob_bytes` gives an ArrayBuffer, `jobs_subscribe` returns a snapshot and then pushes updates.

import type { Backend } from '../bridge';
import type { ArgsOf, CommandName, Commands, Events, RetOf } from '../commands';
import { ipcError, isIpcError } from '../errors';
import type { AppInfo, ImportPreview, JobKind, JobResult, RepoSummary, Settings } from '../types';
import { mockClonePreview } from './cloneUrl';
import { buildFixtures, genericRepo } from './fixtures';
import { MockJobs } from './mockJobs';
import { effectiveAbout, MockRepo } from './repoModel';
import { readSwitches, SLOW_MS, type Switches } from './switches';

export { readSwitches } from './switches';

const DATA_DIR = 'C:\\Users\\demo\\AppData\\Local\\dev.schwi.reposhelf';
const ARCHIVE_DIR = 'C:\\Users\\demo\\Documents\\RepoShelf';
const SETTINGS_KEY = 'reposhelf-mock-settings';
/** Extra latency of the "compute" commands even without `?slow`, so shimmers are visible briefly. */
const COMPUTE_MS = 180;
const COMPUTE: ReadonlySet<CommandName> = new Set<CommandName>([
  'tree_last_commits',
  'commit_count',
  'languages',
  'file_last_commit',
  'branches_ahead_behind',
  'library_refresh',
]);

type Handlers = { [K in CommandName]: (args: ArgsOf<K>) => RetOf<K> | Promise<RetOf<K>> };

const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  sort: { key: 'updated', desc: true },
  sourceFilter: 'all',
  showDirtyNote: false,
  loadRemoteImages: true,
  fetchReleaseNotes: true,
  fetchRepoInfo: true,
  autoDownloadLatestRelease: true,
  autoDownloadOnFetchAll: false,
  downloadSizeCapMb: 1024,
};

/** Mock `image_fetch` (F2): an SVG placeholder naming the host, so nothing ever leaves the page. */
/** Mock remote image: a badge (shields-like hosts or `/badge` paths, 120×20, labelled with the last path segment)
 * or a square picture tile naming the host (160×160). */
function placeholderImage(url: string): ArrayBuffer {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    u = new URL('https://unknown.invalid/');
  }
  const clean = (s: string) => s.replace(/[^a-z0-9.-]/gi, ' ').trim().slice(0, 40);
  const host = clean(u.hostname);
  let svg: string;
  if (/shields|badge/i.test(u.hostname + u.pathname)) {
    const label = clean(decodeURIComponent(u.pathname.split('/').filter(Boolean).pop() ?? host)) || host;
    svg =
      `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="20" viewBox="0 0 120 20">` +
      `<rect width="120" height="20" rx="3" fill="#1b6a80"/>` +
      `<text x="60" y="14" font-size="11" text-anchor="middle" fill="#fff" font-family="sans-serif">${label}</text></svg>`;
  } else {
    svg =
      `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="160" viewBox="0 0 160 160">` +
      `<rect width="160" height="160" rx="12" fill="#2a8a8f"/><circle cx="80" cy="66" r="30" fill="#e8f4f2"/>` +
      `<text x="80" y="134" font-size="12" text-anchor="middle" fill="#fff" font-family="sans-serif">${host}</text></svg>`;
  }
  return new TextEncoder().encode(svg).buffer as ArrayBuffer;
}

/** 64 hex chars from a string (not a real sha-256; the key is opaque to the UI). */
function mockKey(s: string): string {
  let out = '';
  let h = 2166136261;
  for (let round = 0; round < 8; round++) {
    for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619) >>> 0;
    h = Math.imul(h ^ round, 16777619) >>> 0;
    out += h.toString(16).padStart(8, '0');
  }
  return out;
}

function loadSettings(): Settings {
  try {
    const raw = globalThis.localStorage?.getItem(SETTINGS_KEY);
    if (raw) return { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Partial<Settings>) };
  } catch {
    // ignore
  }
  return { ...DEFAULT_SETTINGS };
}

/** As rs-core `validate::oid`: exactly 40 hex digits. */
function fullOid(s: string): string {
  if (!/^[0-9a-fA-F]{40}$/.test(s)) throw ipcError('invalidRev', `invalid ref: not a full commit id: ${JSON.stringify(s.slice(0, 60))}`);
  return s.toLowerCase();
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export interface MockBackend extends Backend {
  readonly switches: Switches;
  /** Tests: the jobs runner. */
  readonly jobs: MockJobs;
}

export function createMockBackend(search: string, opts: { tickMs?: number } = {}): MockBackend {
  const sw = readSwitches(search);
  const repos = new Map<number, MockRepo>();
  for (const d of buildFixtures(sw)) repos.set(d.summary.id, new MockRepo(d));
  let nextRepoId = 100;
  let settings = loadSettings();
  let cacheBytes = 3_412_000;
  const cancelled = new Map<string, number>();
  const importTokens = new Map<string, string>();
  let importSeq = 0;
  const ghFetched = new Set<number>();
  // `?gh`: GitHub release lists already cached (assets visible at once)
  if (sw.gh) for (const r of repos.values()) if (r.githubUrl() !== null) ghFetched.add(r.id);
  const images = new Map<string, string>();
  const eventListeners = new Map<keyof Events, Set<(p: never) => void>>();
  let refreshedSent = false;

  const now = () => Math.floor(Date.now() / 1000);
  const list = (): RepoSummary[] => [...repos.values()].map((r) => r.def.summary);
  const get = (repoId: number): MockRepo => {
    const r = repos.get(repoId);
    if (!r) throw ipcError('notFound', 'repository not found');
    return r;
  };
  /** Like rs-core `repo::open`: missing folder → repoMissing. */
  const open = (repoId: number): MockRepo => {
    const r = get(repoId);
    if (r.def.summary.status.type === 'missing') {
      throw ipcError('repoMissing', `repository folder is missing: ${r.def.summary.path}`);
    }
    return r;
  };
  const appInfo = (): AppInfo => ({
    version: '0.2.0',
    dataDir: DATA_DIR,
    archiveDir: ARCHIVE_DIR,
    cacheBytes,
    repoCount: repos.size,
    archiveBytes: sw.empty ? 0 : 148_326_400,
  });
  const slug = (r: MockRepo) => `${r.def.summary.owner}/${r.def.summary.name}`;

  /**
   * v0.2.0 hook (D-103) after Clone / Import / Fetch of a github.com repo: release notes become "fetched" (when
   * a GitHub setting is on) and the auto-download (action option, else the setting) starts as its own
   * `releaseDownload` job when the latest release has a file marked for this PC. Returns the job notes.
   */
  const ghHook = (r: MockRepo, autoDownload: boolean | null, downloadAllowed = true): string[] => {
    if (r.githubUrl() === null) return [];
    if (settings.fetchReleaseNotes || settings.fetchRepoInfo || settings.autoDownloadLatestRelease) ghFetched.add(r.id);
    if (!downloadAllowed || !(autoDownload ?? settings.autoDownloadLatestRelease)) return [];
    const latest = r.releases(true).find((x) => x.latest);
    const a = latest?.assets.find((x) => x.forThisPc);
    if (!latest || !a) return ['No release download: no file for this PC'];
    if (a.downloaded) return [`Latest release ${latest.tag}: ${a.name} is already downloaded`];
    setTimeout(() => jobs.start({ type: 'releaseDownload', repoId: r.id, tag: latest.tag, asset: a.name }, `Download ${a.name} (${latest.tag})`), 0);
    return [];
  };

  const planner = (kind: JobKind): { run: () => JobResult } => ({
    run: () => {
      if (kind.type === 'clone') {
        const p = mockClonePreview(kind.url, ARCHIVE_DIR);
        if (sw.clonefail) throw `Could not reach ${p.host}: the connection timed out.`;
        const id = nextRepoId++;
        const r = new MockRepo(genericRepo(id, p.owner, p.name, 'cloned', p.targetDir, p.url));
        repos.set(id, r);
        const skipped = /invalid/i.test(p.name) ? ['refs/heads/aux', 'refs/tags/CON'] : [];
        return { type: 'cloned', repoId: id, skippedRefs: skipped, notes: ghHook(r, kind.autoDownload) };
      }
      if (kind.type === 'fetch') {
        const r = get(kind.repoId);
        if (sw.fetchfail) throw `Could not reach ${r.def.summary.originHost ?? 'the host'}: the connection timed out.`;
        r.def.summary.lastFetchedAt = now();
        const notes = ghHook(r, kind.autoDownload);
        // M7.7: clones report tags too (a new tag, a local tag kept because the remote moved it, a skipped ref)
        if (r.def.summary.source === 'cloned') {
          return {
            type: 'fetched',
            newCommits: 3,
            newTags: ['v2.0.0-beta.2'],
            keptLocalTags: ['v1.1.0'],
            skippedRefs: ['refs/remotes/origin/AUX'],
            updatedBranches: ['origin/main'],
            notes,
          };
        }
        return { type: 'fetched', newCommits: 3, newTags: [], keptLocalTags: [], skippedRefs: [], updatedBranches: ['origin/main'], notes };
      }
      if (kind.type === 'githubSync') {
        const r = get(kind.repoId);
        return { type: 'synced', repoId: r.id, notes: ghHook(r, kind.autoDownload) };
      }
      if (kind.type === 'releaseDownload') {
        const r = get(kind.repoId);
        const rel = r.releases(true).find((x) => x.tag === kind.tag);
        const a = rel?.assets.find((x) => x.name === kind.asset);
        if (!rel || !a) throw 'release file not found';
        if (a.size > settings.downloadSizeCapMb * 1024 * 1024) throw `${a.name} is larger than the download limit (${settings.downloadSizeCapMb} MB).`;
        const owner = r.def.summary.owner;
        const path = `${ARCHIVE_DIR}\\_releases\\github.com\\${owner}\\${r.def.summary.name}\\${kind.tag}\\${a.name}`;
        const already = a.downloaded !== null;
        r.downloaded.set(`${kind.tag}\n${a.name}`, path);
        return { type: 'downloaded', repoId: r.id, tag: kind.tag, file: a.name, path, size: a.size, sha256Checked: a.sha256 !== null, already };
      }
      if (sw.fetchfail) throw 'Fetch failed for 2 repositories:\nlumen-ui/lumen: git: timed out\ninkwell/quill: git: timed out';
      const notes: string[] = [];
      for (const r of repos.values()) {
        if (r.def.summary.source !== 'cloned') continue;
        r.def.summary.lastFetchedAt = now();
        notes.push(...ghHook(r, null, settings.autoDownloadOnFetchAll).map((n) => `${slug(r)}: ${n}`));
      }
      return { type: 'fetched', newCommits: 5, newTags: ['v2.0.0'], keptLocalTags: [], skippedRefs: [], updatedBranches: ['origin/main'], notes };
    },
  });

  const jobs = new MockJobs({ tickMs: opts.tickMs ?? (sw.slow ? 500 : 280), planner });

  const h: Handlers = {
    app_info: () => appInfo(),
    settings_get: () => settings,
    settings_set: ({ settings: s }) => {
      settings = { ...s };
      try {
        globalThis.localStorage?.setItem(SETTINGS_KEY, JSON.stringify(settings));
      } catch {
        // ignore
      }
      return settings;
    },
    cache_clear: () => {
      cacheBytes = 0;
      images.clear();
      return appInfo();
    },
    app_open_dir: ({ which }) => {
      if (which !== 'data' && which !== 'archive') {
        throw ipcError('invalidInput', 'unknown folder: expected "data" or "archive"');
      }
      return null;
    },
    library_list: () => list(),
    library_refresh: () => list(),
    repo_by_slug: ({ owner, name }) => {
      const r = [...repos.values()].find(
        (x) => x.def.summary.owner.toLowerCase() === owner.toLowerCase() && x.def.summary.name.toLowerCase() === name.toLowerCase(),
      );
      if (!r) throw ipcError('notFound', 'repository not found');
      return r.def.summary;
    },
    import_pick: () => {
      importSeq += 1;
      // M6.8: picks cycle through a new repo, a non-repo folder (refused) and an already-imported folder (duplicate).
      if (importSeq % 3 === 2) throw ipcError('notARepo', 'C:\\Users\\demo\\Pictures is not a git repository.');
      const path = importSeq % 3 === 0 ? 'C:\\Code\\parsekit' : 'C:\\Code\\notebook';
      const dup = [...repos.values()].find((r) => r.def.summary.path.toLowerCase() === path.toLowerCase());
      const token = `mock${importSeq}${Math.random().toString(16).slice(2, 10)}`;
      importTokens.set(token, path);
      const p: ImportPreview = {
        path,
        name: dup ? dup.def.summary.name : 'notebook',
        isBare: false,
        originUrl: dup ? dup.def.summary.originUrl : 'https://github.com/example-dev/notebook.git',
        duplicateOf: dup ? dup.id : null,
        token,
      };
      return p;
    },
    import_confirm: ({ token, autoDownload }) => {
      const path = importTokens.get(token);
      if (!path) throw ipcError('invalidInput', 'The import selection expired. Pick the folder again.');
      importTokens.delete(token);
      const dup = [...repos.values()].find((r) => r.def.summary.path.toLowerCase() === path.toLowerCase());
      if (dup) throw ipcError('duplicate', `already on the shelf: ${slug(dup)}`);
      const id = nextRepoId++;
      const r = new MockRepo(genericRepo(id, 'example-dev', 'notebook', 'imported', path, 'https://github.com/example-dev/notebook.git'));
      repos.set(id, r);
      // v0.2.0: GitHub info / auto-download as a follow-up job (like the backend; never blocks the import)
      if (settings.fetchReleaseNotes || settings.fetchRepoInfo || (autoDownload ?? settings.autoDownloadLatestRelease)) {
        jobs.start({ type: 'githubSync', repoId: id, autoDownload: autoDownload ?? null }, `GitHub info ${slug(r)}`);
      }
      return r.def.summary;
    },
    repo_locate: ({ repoId }) => {
      const r = get(repoId);
      const s = r.def.summary;
      const fresh = genericRepo(s.id, s.owner, s.name, s.source, s.path.replace(/^[A-Z]:/, 'D:'), s.originUrl);
      fresh.summary = { ...s, path: fresh.summary.path, status: { type: 'ok' } };
      const nr = new MockRepo(fresh);
      repos.set(repoId, nr);
      return nr.def.summary;
    },
    repo_remove: ({ repoId }) => {
      const r = get(repoId);
      repos.delete(repoId);
      return { trashed: r.def.summary.source === 'cloned' && r.def.summary.path.startsWith(ARCHIVE_DIR) };
    },
    repo_set_metadata: ({ repoId, description, homepage, topics }) => {
      const r = get(repoId);
      const desc = description?.trim() ?? '';
      if (desc.length > 350) throw ipcError('invalidInput', `Description is ${desc.length} characters; the limit is 350.`);
      const hp = homepage?.trim() ?? '';
      if (hp.length > 0 && !/^https?:\/\/[^\s/@]+(\/\S*)?$/i.test(hp)) {
        throw ipcError('invalidInput', 'Homepage must be an http:// or https:// address without a user name.');
      }
      const out: string[] = [];
      for (const raw of topics) {
        const t = raw.trim().toLowerCase();
        if (t.length === 0) continue;
        if (!/^[a-z0-9][a-z0-9-]{0,49}$/.test(t)) {
          throw ipcError(
            'invalidInput',
            `Topic ${JSON.stringify(raw)}: use lowercase letters, digits and '-', start with a letter or digit, at most 50 characters.`,
          );
        }
        if (!out.includes(t)) out.push(t);
      }
      if (out.length > 20) throw ipcError('invalidInput', `${out.length} topics; the limit is 20.`);
      const user = { description: desc.length > 0 ? desc : null, homepage: hp.length > 0 ? hp : null, topics: out };
      r.def.summary.aboutUser = user;
      Object.assign(r.def.summary, effectiveAbout(user, r.def.summary.aboutGithub));
      return r.def.summary;
    },
    repo_detail: ({ repoId }) => {
      const r = open(repoId);
      r.def.summary.lastOpenedAt = now();
      const refs = r.refs();
      return {
        // A copy, like the real IPC (serialised): the UI must not rely on the list's objects changing in place (D-87).
        summary: { ...r.def.summary },
        isBare: r.def.isBare,
        head: r.def.head,
        branchCount: refs.branches.length,
        tagCount: refs.tags.length,
        releaseCount: r.releases(false).length,
        license: r.def.license,
        fetchable: r.def.fetchable,
        canFastForward: r.def.canFastForward,
        dirty: settings.showDirtyNote && !r.def.isBare ? (r.def.dirty ?? false) : null,
      };
    },
    repo_open_folder: ({ repoId }) => {
      open(repoId);
      return null;
    },
    refs_list: ({ repoId }) => open(repoId).refs(),
    rev_resolve: ({ repoId, rev }) => open(repoId).resolve(rev).rev,
    tree_list: ({ repoId, rev, path }) => open(repoId).tree(rev, path),
    tree_last_commits: ({ repoId, rev, path }) => open(repoId).lastCommits(rev, path),
    commit_latest: ({ repoId, rev }) => {
      const res = open(repoId).resolve(rev);
      return { commit: res.chain[0].summary, count: res.chain.length > 5000 ? null : res.chain.length };
    },
    commit_count: ({ repoId, rev }) => open(repoId).count(rev),
    readme_render: ({ repoId, rev, path }) => open(repoId).readme(rev, path),
    markdown_render_file: ({ repoId, rev, path }) => open(repoId).markdownFile(rev, path),
    languages: ({ repoId, rev }) => {
      const r = open(repoId);
      return { tree: r.resolve(rev).rev.tree, items: r.def.languages };
    },
    blob_meta: ({ repoId, rev, path }) => open(repoId).meta(rev, path),
    blob_text: ({ repoId, rev, path }) => open(repoId).blobText(rev, path),
    blob_bytes: ({ repoId, rev, path }) => open(repoId).bytes(rev, path),
    blob_save_as: ({ repoId, rev, path }) => {
      open(repoId).meta(rev, path);
      return `C:\\Users\\demo\\Downloads\\${path.split('/').pop() ?? 'file'}`;
    },
    file_last_commit: ({ repoId, rev, path }) => open(repoId).fileLastCommit(rev, path),
    commits_page: ({ repoId, rev, path, since, skip, tip }) => {
      if (tip !== null) fullOid(tip);
      return open(repoId).page(rev, { path, since, skip, tip: tip?.toLowerCase() ?? null });
    },
    commit_detail: ({ repoId, oid }) => open(repoId).detail(fullOid(oid)),
    commit_file_diff: ({ repoId, oid, index }) => open(repoId).fileDiff(fullOid(oid), index),
    branches_list: ({ repoId }) => open(repoId).branches(),
    branches_ahead_behind: ({ repoId, names }) => open(repoId).aheadBehind(names),
    tags_list: ({ repoId }) => open(repoId).tags(),
    releases_list: ({ repoId }) => open(repoId).releases(ghFetched.has(repoId)),
    releases_fetch_github: ({ repoId }) => {
      const r = open(repoId);
      if (!r.def.summary.originUrl?.startsWith('https://github.com/')) {
        throw ipcError('refused', 'Release notes can only be loaded for repositories on github.com.');
      }
      ghFetched.add(repoId);
      return r.releases(true);
    },
    clone_preview: ({ input }) => mockClonePreview(input, ARCHIVE_DIR),
    release_download: ({ repoId, tag, asset }) => {
      const r = open(repoId);
      for (const v of [tag, asset]) {
        if (v.length === 0 || v.length > 512 || /[\u0000-\u001f]/.test(v)) throw ipcError('invalidInput', 'invalid release or file name');
      }
      if (!ghFetched.has(repoId)) throw ipcError('notFound', 'GitHub release list (load the release notes first) not found');
      const known = r.releases(true).some((x) => x.tag === tag && x.assets.some((a) => a.name === asset));
      if (!known) throw ipcError('notFound', 'release file not found');
      return jobs.start({ type: 'releaseDownload', repoId, tag, asset }, `Download ${asset} (${tag})`);
    },
    release_reveal: ({ repoId, tag, asset }) => {
      const r = open(repoId);
      if (!r.downloaded.has(`${tag}\n${asset}`)) throw ipcError('notFound', 'downloaded file not found');
      return null;
    },
    image_fetch: ({ url }) => {
      if (!settings.loadRemoteImages) throw ipcError('refused', 'Remote images are turned off (Settings → Network).');
      let u: URL;
      try {
        u = new URL(url);
      } catch {
        throw ipcError('invalidUrl', 'invalid image address');
      }
      const h = u.hostname.toLowerCase();
      if (u.protocol !== 'https:' || u.username || u.port || !h.includes('.') || /^[\d.]+$|^\[|localhost$|\.local$/.test(h)) {
        throw ipcError('refused', `Refused a non-public address: ${url}`);
      }
      // mock: hosts containing "fail" behave like an unreachable server (the UI falls back to the chip)
      if (h.includes('fail')) throw ipcError('network', `network: could not reach ${h}`);
      const key = mockKey(u.href);
      images.set(key, u.href);
      return { key, mime: 'image/svg+xml', size: placeholderImage(u.href).byteLength };
    },
    image_bytes: ({ key }) => {
      if (!/^[0-9a-f]{64}$/.test(key)) throw ipcError('invalidInput', 'bad image key');
      const h = images.get(key);
      if (h === undefined) throw ipcError('notFound', 'cached image not found');
      return placeholderImage(h);
    },
    clone_start: ({ input, autoDownload }) => {
      const p = mockClonePreview(input, ARCHIVE_DIR);
      const dup = [...repos.values()].find((r) => r.def.summary.originUrl === p.url);
      if (dup) throw ipcError('duplicate', `already on the shelf: ${slug(dup)}`);
      return jobs.start({ type: 'clone', url: p.url, autoDownload: autoDownload ?? null }, `Clone ${p.host}/${p.owner}/${p.name}`);
    },
    fetch_start: ({ repoId, autoDownload }) => {
      const r = open(repoId);
      if (!r.def.fetchable) throw ipcError('refused', 'This repository has no https:// origin to fetch from.');
      return jobs.start({ type: 'fetch', repoId, autoDownload: autoDownload ?? null }, `Fetch ${slug(r)}`);
    },
    fetch_all_start: () => jobs.start({ type: 'fetchAll' }, 'Fetch all clones'),
    fast_forward: ({ repoId }) => {
      const r = open(repoId);
      if (r.def.canFastForward !== true) throw ipcError('refused', 'The default branch is already up to date.');
      r.def.canFastForward = false;
      return r.refs().remoteBranches.find((b) => b.name === `origin/${r.defaultBranch()}`)?.target ?? r.mainline[0].summary.oid;
    },
    jobs_subscribe: ({ channel }) => jobs.subscribe(channel),
    jobs_cancel: ({ jobId }) => jobs.cancel(jobId),
    jobs_retry: ({ jobId }) => jobs.retry(jobId),
    jobs_dismiss: ({ jobId }) => {
      jobs.dismiss(jobId);
      return null;
    },
    request_cancel: ({ requestId }) => {
      cancelled.set(requestId, Date.now());
      return true;
    },
    open_external: ({ url }) => {
      let u: URL;
      try {
        u = new URL(url);
      } catch {
        throw ipcError('invalidUrl', 'Only http:// and https:// links can be opened.');
      }
      if ((u.protocol !== 'http:' && u.protocol !== 'https:') || u.username || u.password || !u.hostname.includes('.')) {
        throw ipcError('invalidUrl', 'Only http:// and https:// links can be opened.');
      }
      return null;
    },
  };

  async function invoke<K extends CommandName>(cmd: K, args: ArgsOf<K>): Promise<RetOf<K>> {
    const handler = h[cmd] as (a: Commands[K]['args']) => RetOf<K> | Promise<RetOf<K>>;
    if (!handler) throw ipcError('internal', `unknown command ${cmd}`);
    const requestId = (args as { requestId?: unknown }).requestId;
    if (requestId !== undefined && (typeof requestId !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(requestId))) {
      throw ipcError('invalidInput', 'invalid requestId');
    }
    const delay = (sw.slow ? SLOW_MS : 0) + (COMPUTE.has(cmd) ? COMPUTE_MS : 0);
    if (delay > 0) await sleep(delay);
    else await Promise.resolve();
    if (typeof requestId === 'string' && cancelled.has(requestId)) {
      cancelled.delete(requestId);
      throw ipcError('cancelled', 'cancelled');
    }
    // forget early cancels after 30 s, like the backend
    const cutoff = Date.now() - 30_000;
    for (const [k, t] of cancelled) if (t < cutoff) cancelled.delete(k);
    try {
      const out = await handler(args);
      return out instanceof ArrayBuffer ? out : (structuredClone(out) as RetOf<K>);
    } catch (e) {
      if (isIpcError(e)) throw e;
      throw ipcError('internal', e instanceof Error ? e.message : String(e));
    }
  }

  function listen<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): () => void {
    let set = eventListeners.get(event);
    if (!set) {
      set = new Set();
      eventListeners.set(event, set);
    }
    const fn = handler as (p: never) => void;
    set.add(fn);
    if (event === 'library-refreshed' && !refreshedSent) {
      refreshedSent = true;
      setTimeout(() => {
        const payload = structuredClone(list());
        for (const l of eventListeners.get('library-refreshed') ?? []) (l as (p: RepoSummary[]) => void)(payload);
      }, 400 + (sw.slow ? SLOW_MS : 0));
    }
    return () => {
      set.delete(fn);
    };
  }

  return { invoke, listen, switches: sw, jobs };
}
