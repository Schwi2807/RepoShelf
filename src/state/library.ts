// Library store (FR-1..3): the DB list first (fast), then a background `library_refresh` (missing/corrupt status,
// R-9) and the startup `library-refreshed` event. Search / filter / sort live here once (D-27).

import { libraryList, libraryRefresh, newRequestId } from '../ipc/api';
import { onEvent } from '../ipc/bridge';
import { errorText } from '../ipc/errors';
import type { RepoSummary, SortSpec, SourceFilter } from '../ipc/types';
import { createStore, useStore } from './store';

export interface LibraryState {
  repos: RepoSummary[] | null;
  error: string | null;
  refreshing: boolean;
}

export const libraryStore = createStore<LibraryState>({ repos: null, error: null, refreshing: false });

let started: Promise<void> | null = null;

/** Load once (main.tsx). Safe under StrictMode (module-level). */
export function startLibrary(): Promise<void> {
  started ??= (async () => {
    void onEvent('library-refreshed', (repos) => libraryStore.set((p) => ({ ...p, repos })));
    try {
      const repos = await libraryList();
      libraryStore.set((p) => ({ ...p, repos, error: null }));
    } catch (e) {
      libraryStore.set((p) => ({ ...p, error: errorText(e), repos: p.repos ?? [] }));
    }
    void refreshLibrary();
  })();
  return started;
}

/** Re-read summaries (status, last commit). */
export async function refreshLibrary(): Promise<void> {
  libraryStore.set((p) => ({ ...p, refreshing: true }));
  try {
    const repos = await libraryRefresh(newRequestId());
    libraryStore.set({ repos, error: null, refreshing: false });
  } catch (e) {
    libraryStore.set((p) => ({ ...p, refreshing: false, error: p.repos ? p.error : errorText(e) }));
  }
}

/** Replace / insert one summary (after import, Edit About, locate). */
export function upsertRepo(r: RepoSummary): void {
  libraryStore.set((p) => {
    const list = p.repos ?? [];
    const i = list.findIndex((x) => x.id === r.id);
    const next = i >= 0 ? list.map((x) => (x.id === r.id ? r : x)) : [...list, r];
    return { ...p, repos: next };
  });
}

export function removeRepoLocal(id: number): void {
  libraryStore.set((p) => ({ ...p, repos: (p.repos ?? []).filter((x) => x.id !== id) }));
}

export function useLibrary(): LibraryState {
  return useStore(libraryStore);
}

/** Case-insensitive match on owner/name, description and topics. Empty query → all. */
export function filterRepos(repos: RepoSummary[], query: string, source: SourceFilter = 'all'): RepoSummary[] {
  const q = query.trim().toLowerCase();
  return repos.filter((r) => {
    if (source !== 'all' && r.source !== source) return false;
    if (q.length === 0) return true;
    if (`${r.owner}/${r.name}`.toLowerCase().includes(q)) return true;
    if (r.description?.toLowerCase().includes(q)) return true;
    return r.topics.some((t) => t.toLowerCase().includes(q));
  });
}

function sortValue(r: RepoSummary, key: SortSpec['key']): number | string {
  switch (key) {
    case 'updated':
      return r.lastCommitAt ?? r.addedAt;
    case 'added':
      return r.addedAt;
    case 'name':
      return r.name.toLowerCase();
  }
}

/** Stable sort by the setting; ties by name then id. */
export function sortRepos(repos: RepoSummary[], spec: SortSpec): RepoSummary[] {
  const dir = spec.desc ? -1 : 1;
  return [...repos].sort((a, b) => {
    const va = sortValue(a, spec.key);
    const vb = sortValue(b, spec.key);
    if (va < vb) return -1 * dir;
    if (va > vb) return 1 * dir;
    const na = `${a.name}/${a.owner}`.toLowerCase();
    const nb = `${b.name}/${b.owner}`.toLowerCase();
    if (na !== nb) return na < nb ? -1 : 1;
    return a.id - b.id;
  });
}

/** Tests only. */
export function resetLibraryForTests(): void {
  started = null;
  libraryStore.set({ repos: null, error: null, refreshing: false });
}
