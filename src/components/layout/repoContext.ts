// What RepoLayout / ExploreLayout resolved for the current repo route (ARCH §7.3: slug → repoId, rev → ResolvedRev).
// Children use `resolved.commit` for oid-keyed (stable) queries and read data through `src` (D-127). The fields that
// exist only for a repository on the shelf (summary, repoId, detail) live under `local`, so the compiler lists every
// place that would assume one for a GitHub repository seen online (`online`).

import { createContext, useContext } from 'react';
import type { ExploreRepo, IpcError, RepoDetail, RepoSummary, ResolvedRev } from '../../ipc/types';
import type { RepoSource } from '../../source';

export interface LocalRepo {
  summary: RepoSummary;
  repoId: number;
  detail: RepoDetail | null;
  detailError: IpcError | undefined;
}

export interface RepoContextValue {
  /** Where this page's data comes from (the shelf or GitHub). */
  src: RepoSource;
  /** Same as `src.explore`: links carry the `explore` route flag. */
  explore: boolean;
  owner: string;
  repo: string;
  defaultBranch: string | null;
  /** The repository has no commits (local unborn HEAD). Online, an empty repository is a resolve error. */
  unborn: boolean;
  /** Decoded ref in use: route `:ref`, else HEAD's branch / default branch. null for an unborn/empty repo. */
  refName: string | null;
  /** True when `refName` came from the route. */
  refFromRoute: boolean;
  resolved: ResolvedRev | null;
  revError: IpcError | undefined;
  revLoading: boolean;
  /** The shelf entry; null online. */
  local: LocalRepo | null;
  /** The GitHub answer; null on the shelf. */
  online: ExploreRepo | null;
}

export const RepoContext = createContext<RepoContextValue | null>(null);

export function useRepo(): RepoContextValue {
  const v = useContext(RepoContext);
  if (!v) throw new Error('useRepo outside RepoLayout');
  return v;
}

/** Link flag for the current page: true under Explore, false on the shelf and outside any repo layout. */
export function useExplore(): boolean {
  return useContext(RepoContext)?.explore ?? false;
}

/** The shelf entry of a page that exists only for shelf repositories (the layout guarantees it). */
export function useLocal(): LocalRepo {
  const v = useRepo();
  if (!v.local) throw new Error('useLocal on an Explore page');
  return v.local;
}

/** Default ref for links when the route has none. */
export function defaultRef(summary: RepoSummary, detail: RepoDetail | null): string | null {
  const h = detail?.head;
  if (h?.type === 'branch') return h.name;
  if (h?.type === 'detached') return h.oid;
  if (h?.type === 'unborn') return h.name;
  return summary.defaultBranch;
}
