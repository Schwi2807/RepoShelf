// "Recent" = top 5 by `lastOpenedAt` (D-28), derived from the library list.

import { useMemo } from 'react';
import type { RepoSummary } from '../ipc/types';
import { useLibrary } from './library';

export const RECENT_COUNT = 5;

export function recentRepos(repos: RepoSummary[], n = RECENT_COUNT): RepoSummary[] {
  return repos
    .filter((r) => r.lastOpenedAt !== null)
    .sort((a, b) => (b.lastOpenedAt ?? 0) - (a.lastOpenedAt ?? 0) || a.id - b.id)
    .slice(0, n);
}

export function useRecent(): RepoSummary[] {
  const { repos } = useLibrary();
  return useMemo(() => recentRepos(repos ?? []), [repos]);
}
