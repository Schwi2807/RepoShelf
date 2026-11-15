// Mock of the v0.3.0 Explore commands (M10a). Online repositories are served from the same in-memory repo model as
// the shelf (an `owner/name` that is a github.com repo on the mock shelf, else a generated repo), so every screen
// can be built against it. Behaves like the backend where the UI depends on it: validated addresses, "Explore is
// off", the quota header (60 anonymous / 5,000 with a token, 1 per distinct request, saved answers are free),
// `rateLimited` with a reset time (switch `?ratelimit` starts at 2 left), saved data while limited, a token that is
// never returned, Save to shelf = the clone job.

import type { ArgsOf, Commands, RetOf } from '../commands';
import { ipcError } from '../errors';
import type {
  ExploreRepo,
  ExploreSearch,
  ExploreStatus,
  JobId,
  Quota,
  Release,
  Settings,
} from '../types';
import { genericRepo } from './fixtures';
import { MockRepo } from './repoModel';

const PAGE = 35;
const OWNER_RE = /^(?!-)[A-Za-z0-9-]{1,39}$/;
const REPO_RE = /^(?!-)[A-Za-z0-9._-]{1,100}$/;
/** Names that behave like GitHub answers: 404 (private or missing). */
const MISSING = new Set(['missing', 'private', 'deleted']);
const EXTRA_REPOS: [string, string, string, string][] = [
  ['rust-lang', 'rustlings', 'Small exercises to get you used to reading and writing Rust code!', 'Rust'],
  ['sharkdp', 'bat', 'A cat(1) clone with wings.', 'Rust'],
  ['BurntSushi', 'ripgrep', 'Recursively searches directories for a regex pattern while respecting your gitignore.', 'Rust'],
  ['microsoft', 'vscode', 'Visual Studio Code', 'TypeScript'],
  ['python', 'cpython', 'The Python programming language', 'Python'],
  ['golang', 'go', 'The Go programming language', 'Go'],
];

export interface ExploreHost {
  repos: Map<number, MockRepo>;
  settings: () => Settings;
  /** The mock `clone_start` (Save to shelf = the clone job). */
  cloneStart: (url: string, autoDownload: boolean | null) => JobId;
  now: () => number;
}

type H = { [K in keyof Commands as K extends `explore_${string}` ? K : never]: (a: ArgsOf<K>) => RetOf<K> };

export class MockExplore {
  tokenSaved = false;
  private limit = 60;
  private remaining: number;
  private resetAt: number;
  private limitedUntil: number | null = null;
  private readonly seen = new Set<string>();
  private stale: { fetchedAt: number; reason: 'rateLimited' | 'offline' } | null = null;
  private readonly online = new Map<string, MockRepo>();
  private nextId = 900;

  constructor(
    private readonly host: ExploreHost,
    ratelimit: boolean,
  ) {
    this.remaining = ratelimit ? 2 : 60;
    this.resetAt = host.now() + 3600;
  }

  private guard(): void {
    if (!this.host.settings().exploreEnabled) {
      throw ipcError('refused', 'Explore is turned off (Settings → Network → Explore).');
    }
  }

  /** One request's worth of quota; the same request again is served from the saved answers for free. */
  private spend(key: string): void {
    this.guard();
    if (this.seen.has(key)) return;
    if (this.limitedUntil !== null && this.limitedUntil > this.host.now()) {
      // a saved answer would have been served; nothing is saved for a new request
      const mins = Math.max(1, Math.ceil((this.limitedUntil - this.host.now()) / 60));
      throw ipcError('rateLimited', `GitHub's request limit is used up; it resets in about ${mins} min`);
    }
    this.seen.add(key);
    this.remaining = Math.max(0, this.remaining - 1);
    if (this.remaining === 0) this.limitedUntil = this.resetAt;
  }

  private addr(owner: string, name: string): void {
    if (!OWNER_RE.test(owner)) throw ipcError('invalidInput', `Not a GitHub user or organisation name: ${JSON.stringify(owner.slice(0, 60))}`);
    if (!REPO_RE.test(name) || name === '.' || name === '..' || name.toLowerCase().endsWith('.git')) {
      throw ipcError('invalidInput', `Not a GitHub repository name: ${JSON.stringify(name.slice(0, 60))}`);
    }
  }

  private shelfMatch(owner: string, name: string): MockRepo | undefined {
    return [...this.host.repos.values()].find((r) => {
      const m = /^https:\/\/github\.com\/([^/]+)\/([^/]+?)(\.git)?$/.exec(r.def.summary.originUrl ?? '');
      return m !== null && m[1].toLowerCase() === owner.toLowerCase() && m[2].toLowerCase() === name.toLowerCase();
    });
  }

  /** The repository model behind `owner/name`; counts one request for `key`. */
  private repo(owner: string, name: string, key: string): MockRepo {
    this.addr(owner, name);
    this.spend(key);
    if (MISSING.has(name.toLowerCase())) {
      throw ipcError('notFound', 'repository, file or page on GitHub (private repositories need a token) not found');
    }
    const id = `${owner}/${name}`.toLowerCase();
    const known = this.shelfMatch(owner, name) ?? this.online.get(id);
    if (known) return known;
    const r = new MockRepo(genericRepo(this.nextId++, owner, name, 'cloned', `online:${id}`, `https://github.com/${owner}/${name}.git`));
    this.online.set(id, r);
    return r;
  }

  private rev(rev: string): void {
    if (rev.length === 0 || /[\s\\:^~]|\.\.|@\{/.test(rev) || rev.startsWith('-')) {
      throw ipcError('invalidRev', `invalid ref: ${JSON.stringify(rev.slice(0, 60))}`);
    }
  }

  private oid(oid: string): string {
    if (!/^[0-9a-fA-F]{40}$/.test(oid)) throw ipcError('invalidRev', `invalid ref: not a full commit id: ${JSON.stringify(oid.slice(0, 60))}`);
    return oid.toLowerCase();
  }

  private path(p: string): void {
    if (p.startsWith('/') || p.includes('\\') || p.includes('\0') || /^[A-Za-z]:/.test(p) || p.split('/').some((s, _i, a) => s === '.' || s === '..' || (s === '' && (p !== '' || a.length > 1)))) {
      throw ipcError('invalidPath', `invalid path: ${p}`);
    }
  }

  private toRepo(r: MockRepo, owner: string, name: string): ExploreRepo {
    const s = r.def.summary;
    const h = [...`${owner}/${name}`].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
    const on = this.shelfMatch(owner, name);
    return {
      owner,
      name,
      fullName: `${owner}/${name}`,
      description: s.description,
      homepage: s.homepage,
      topics: s.topics,
      htmlUrl: `https://github.com/${owner}/${name}`,
      cloneUrl: `https://github.com/${owner}/${name}.git`,
      defaultBranch: r.defaultBranch(),
      stars: h % 90000,
      forks: h % 7000,
      openIssues: h % 300,
      language: s.primaryLanguage,
      license: r.def.license,
      sizeKb: 1000 + (h % 90000),
      createdAt: this.host.now() - 86400 * 900,
      updatedAt: this.host.now() - 86400 * (h % 30),
      pushedAt: this.host.now() - 86400 * (h % 30),
      archived: false,
      fork: false,
      isPrivate: false,
      ownerAvatarUrl: `https://avatars.githubusercontent.com/u/${h % 99999}?v=4`,
      onShelf: on ? on.id : null,
    };
  }

  private quota(resource: string, limit: number, remaining: number, resetAt: number): Quota {
    return { resource, limit, remaining, resetAt, seenAt: this.host.now() };
  }

  status(): ExploreStatus {
    const lim = this.tokenSaved ? 5000 : this.limit;
    const used = this.limit - this.remaining;
    const limited = this.limitedUntil !== null && this.limitedUntil > this.host.now() ? this.limitedUntil : null;
    return {
      enabled: this.host.settings().exploreEnabled,
      available: true,
      tokenSaved: this.tokenSaved,
      core: this.seen.size > 0 || this.tokenSaved ? this.quota('core', lim, Math.max(0, lim - used), this.resetAt) : null,
      search: null,
      limitedUntil: limited,
      searchLimitedUntil: null,
      stale: limited !== null && this.stale ? this.stale : null,
      cacheBytes: this.seen.size * 6200,
      cacheEntries: this.seen.size,
    };
  }

  private searchItems(): ExploreRepo[] {
    const out: ExploreRepo[] = [];
    for (const r of this.host.repos.values()) {
      const m = /^https:\/\/github\.com\/([^/]+)\/([^/]+?)(\.git)?$/.exec(r.def.summary.originUrl ?? '');
      if (m) out.push(this.toRepo(r, m[1], m[2]));
    }
    for (const [o, n, d, lang] of EXTRA_REPOS) {
      const r = new MockRepo(genericRepo(this.nextId++, o, n, 'cloned', `online:${o}/${n}`, `https://github.com/${o}/${n}.git`));
      r.def.summary.description = d;
      r.def.summary.primaryLanguage = lang;
      this.online.set(`${o}/${n}`.toLowerCase(), r);
      out.push(this.toRepo(r, o, n));
    }
    return out;
  }

  handlers(): H {
    const e = this;
    const k = (...p: (string | number | null | boolean)[]) => p.join('\u0001');
    return {
      explore_status: () => e.status(),
      explore_token_set: ({ token }) => {
        const t = token.trim();
        if (t.length === 0 || t.length > 255 || !/^[A-Za-z0-9_-]+$/.test(t)) {
          throw ipcError('invalidInput', "A GitHub token is 1–255 letters, digits, '_' or '-' with no spaces. Copy it again from github.com/settings/tokens.");
        }
        if (t === 'ghp_rejected') throw ipcError('refused', 'GitHub did not accept this token.');
        e.tokenSaved = true;
        e.limitedUntil = null;
        e.seen.clear();
        e.remaining = 5000;
        e.limit = 5000;
        return e.status();
      },
      explore_token_remove: () => {
        e.tokenSaved = false;
        e.limitedUntil = null;
        e.seen.clear();
        e.limit = 60;
        e.remaining = 60;
        return e.status();
      },
      explore_cache_clear: () => {
        e.seen.clear();
        return e.status();
      },
      explore_repo: ({ owner, name }) => {
        const r = e.repo(owner, name, k('repo', owner, name));
        return e.toRepo(r, owner, name);
      },
      explore_refs: ({ owner, name }) => {
        const r = e.repo(owner, name, k('refs', owner, name));
        return { refs: { ...r.refs(), remoteBranches: [] }, branchesTruncated: false, tagsTruncated: false };
      },
      explore_languages: ({ owner, name }) => {
        const r = e.repo(owner, name, k('lang', owner, name));
        return { tree: '', items: r.def.languages };
      },
      explore_resolve: ({ owner, name, rev }) => {
        e.rev(rev);
        const r = e.repo(owner, name, k('resolve', owner, name, rev));
        return r.resolve(rev === 'HEAD' ? (r.defaultBranch() ?? 'main') : rev).rev;
      },
      explore_commit_latest: ({ owner, name, rev }) => {
        e.rev(rev);
        const r = e.repo(owner, name, k('resolve', owner, name, rev));
        const res = r.resolve(rev === 'HEAD' ? (r.defaultBranch() ?? 'main') : rev);
        return { commit: res.chain[0].summary, count: res.chain.length };
      },
      explore_tree: ({ owner, name, rev, path }) => {
        e.rev(rev);
        e.path(path);
        const r = e.repo(owner, name, k('tree', owner, name, rev, path));
        return r.tree(rev === 'HEAD' ? (r.defaultBranch() ?? 'main') : rev, path);
      },
      explore_readme: ({ owner, name, rev, path }) => {
        e.rev(rev);
        e.path(path);
        return e.repo(owner, name, k('readme', owner, name, rev, path)).readme(rev === 'HEAD' ? 'main' : rev, path);
      },
      explore_render_file: ({ owner, name, rev, path }) => {
        e.rev(rev);
        e.path(path);
        return e.repo(owner, name, k('render', owner, name, rev, path)).markdownFile(rev, path);
      },
      explore_blob_meta: ({ owner, name, rev, path }) => {
        e.rev(rev);
        e.path(path);
        return e.repo(owner, name, k('blob', owner, name, rev, path)).meta(rev, path);
      },
      explore_blob_text: ({ owner, name, rev, path }) => {
        e.rev(rev);
        e.path(path);
        return e.repo(owner, name, k('blob', owner, name, rev, path)).blobText(rev, path);
      },
      explore_blob_bytes: ({ owner, name, rev, path }) => {
        e.rev(rev);
        e.path(path);
        return e.repo(owner, name, k('blob', owner, name, rev, path)).bytes(rev, path);
      },
      explore_commits: ({ owner, name, rev, path, skip, tip }) => {
        e.rev(rev);
        if (path !== null) e.path(path);
        if (tip !== null) e.oid(tip);
        if (skip % PAGE !== 0) throw ipcError('invalidInput', `skip must be a multiple of ${PAGE} for an online repository`);
        const r = e.repo(owner, name, k('commits', owner, name, rev, path, skip, tip));
        return r.page(rev, { path, since: null, skip, tip: tip?.toLowerCase() ?? null });
      },
      explore_commit: ({ owner, name, oid }) => {
        const o = e.oid(oid);
        return e.repo(owner, name, k('commit', owner, name, o)).detail(o);
      },
      explore_commit_file_diff: ({ owner, name, oid, index }) => {
        const o = e.oid(oid);
        return e.repo(owner, name, k('commit', owner, name, o)).fileDiff(o, index);
      },
      explore_releases: ({ owner, name }): Release[] => {
        return e.repo(owner, name, k('releases', owner, name)).releases(true).map((x) => ({ ...x, assets: x.assets.map((a) => ({ ...a, downloaded: null })) }));
      },
      explore_search: ({ query, sort, page }): ExploreSearch => {
        const q = query.trim();
        if (q.length === 0 || q.length > 256 || /[\u0000-\u001f]/.test(q)) throw ipcError('invalidInput', 'Enter 1–256 characters to search for.');
        if (page < 1 || page > 34) throw ipcError('invalidInput', 'GitHub only shows the first 34 search pages.');
        if (sort !== null && !['stars', 'forks', 'updated', 'help-wanted-issues'].includes(sort)) throw ipcError('invalidInput', 'Unknown sort order.');
        e.spend(k('search', q, sort, page));
        const words = q.toLowerCase().split(/\s+/).filter((w) => !w.includes(':'));
        let items = e.searchItems().filter((r) => words.every((w) => `${r.fullName} ${r.description ?? ''} ${r.topics.join(' ')}`.toLowerCase().includes(w)));
        if (sort === 'stars') items = items.sort((a, b) => b.stars - a.stars);
        if (sort === 'updated') items = items.sort((a, b) => (b.pushedAt ?? 0) - (a.pushedAt ?? 0));
        const slice = items.slice((page - 1) * 30, page * 30);
        return { query: q, totalCount: items.length, incomplete: false, page, hasMore: page * 30 < items.length, items: slice };
      },
      explore_save_to_shelf: ({ owner, name, autoDownload }) => {
        e.addr(owner, name);
        e.guard();
        if (e.shelfMatch(owner, name)) throw ipcError('duplicate', `already on the shelf: ${owner}/${name}`);
        return e.host.cloneStart(`https://github.com/${owner}/${name}.git`, autoDownload ?? null);
      },
    };
  }
}

