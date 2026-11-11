import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackendForTests, type Backend } from '../ipc/bridge';
import type { RepoSummary, SortKey } from '../ipc/types';
import { AUTO_HIDE_MS, applyJobUpdate, jobsStore, resetJobsForTests } from './jobs';
import { filterRepos, sortRepos } from './library';
import { recentRepos } from './recent';
import { nextTheme, resolveTheme } from './theme';

function repo(id: number, p: Partial<RepoSummary>): RepoSummary {
  return {
    id,
    owner: 'o',
    name: `r${id}`,
    source: 'imported',
    path: `C:\\r${id}`,
    description: null,
    topics: [],
    homepage: null,
    originUrl: null,
    originHost: null,
    defaultBranch: 'main',
    primaryLanguage: null,
    lastCommitAt: null,
    addedAt: 0,
    clonedAt: null,
    lastOpenedAt: null,
    lastFetchedAt: null,
    status: { type: 'ok' },
    ...p,
  };
}

const REPOS: RepoSummary[] = [
  repo(1, { name: 'parsekit', owner: 'northwind', lastCommitAt: 300, addedAt: 10 }),
  repo(2, { name: 'lumen', description: 'Parser-free UI kit', lastCommitAt: 100, addedAt: 30, source: 'cloned' }),
  repo(3, { name: 'quill', topics: ['rust', 'Parsing'], lastCommitAt: null, addedAt: 200, source: 'cloned' }),
  repo(4, { name: 'beacon', owner: 'local', lastCommitAt: 200, addedAt: 20 }),
];

describe('filterRepos (D-27)', () => {
  it('"pars" matches name, description and topics, case-insensitively', () => {
    expect(filterRepos(REPOS, 'pars').map((r) => r.id)).toEqual([1, 2, 3]);
    expect(filterRepos(REPOS, 'PARS').map((r) => r.id)).toEqual([1, 2, 3]);
    expect(filterRepos(REPOS, '  ').map((r) => r.id)).toEqual([1, 2, 3, 4]);
    expect(filterRepos(REPOS, 'local/').map((r) => r.id)).toEqual([4]);
  });
  it('combines with the source filter', () => {
    expect(filterRepos(REPOS, 'pars', 'cloned').map((r) => r.id)).toEqual([2, 3]);
    expect(filterRepos(REPOS, '', 'imported').map((r) => r.id)).toEqual([1, 4]);
  });
});

describe('sortRepos', () => {
  const cases: [SortKey, boolean, number[]][] = [
    // quill has no commit → addedAt 200, ties with beacon (200) → by name in both directions
    ['updated', true, [1, 4, 3, 2]],
    ['updated', false, [2, 4, 3, 1]],
    ['name', false, [4, 2, 1, 3]],
    ['name', true, [3, 1, 2, 4]],
    ['added', true, [3, 2, 4, 1]],
    ['added', false, [1, 4, 2, 3]],
  ];
  it.each(cases)('%s desc=%s', (key, desc, ids) => {
    expect(sortRepos(REPOS, { key, desc }).map((r) => r.id)).toEqual(ids);
  });
  it('does not mutate the input', () => {
    const before = REPOS.map((r) => r.id);
    sortRepos(REPOS, { key: 'name', desc: false });
    expect(REPOS.map((r) => r.id)).toEqual(before);
  });
});

describe('recentRepos (D-28)', () => {
  it('top 5 by lastOpenedAt, never-opened left out', () => {
    const many = Array.from({ length: 8 }, (_, i) => repo(i + 1, { lastOpenedAt: i === 2 ? null : (i + 1) * 10 }));
    expect(recentRepos(many).map((r) => r.id)).toEqual([8, 7, 6, 5, 4]);
  });
});

describe('theme', () => {
  it('resolves system via prefers-color-scheme', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(nextTheme('system')).toBe('light');
    expect(nextTheme('light')).toBe('dark');
    expect(nextTheme('dark')).toBe('system');
  });
});

describe('jobs auto-hide', () => {
  const dismissed: number[] = [];
  beforeEach(() => {
    vi.useFakeTimers();
    dismissed.length = 0;
    const b: Backend = {
      invoke: ((cmd: string, args: { jobId?: number }) => {
        if (cmd === 'jobs_dismiss') dismissed.push(args.jobId ?? -1);
        return Promise.resolve(cmd === 'library_refresh' ? [] : null);
      }) as Backend['invoke'],
      listen: () => () => undefined,
    };
    setBackendForTests(b);
    resetJobsForTests();
  });
  afterEach(() => {
    vi.useRealTimers();
    setBackendForTests(null);
    resetJobsForTests();
  });

  const job = (id: number, state: 'running' | 'done' | 'failed') => ({
    id,
    kind: { type: 'fetchAll' as const },
    state,
    progress: null,
    title: `job ${id}`,
    result: null,
    error: state === 'failed' ? 'Could not reach example.com' : null,
    startedAt: 0,
  });

  it('hides done rows after 5 s, keeps failures', async () => {
    applyJobUpdate(job(1, 'running'));
    applyJobUpdate(job(2, 'running'));
    applyJobUpdate(job(1, 'done'));
    applyJobUpdate(job(2, 'failed'));
    expect(jobsStore.get().map((j) => j.id)).toEqual([1, 2]);
    vi.advanceTimersByTime(AUTO_HIDE_MS - 1);
    expect(jobsStore.get().map((j) => j.id)).toEqual([1, 2]);
    vi.advanceTimersByTime(2);
    expect(jobsStore.get().map((j) => j.id)).toEqual([2]);
    await vi.runAllTimersAsync();
    expect(dismissed).toContain(1);
    expect(dismissed).not.toContain(2);
    expect(jobsStore.get().map((j) => j.id)).toEqual([2]);
  });
});
