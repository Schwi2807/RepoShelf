// What RepoLayout resolved for the current repo route (ARCH §7.3: slug → repoId, rev → ResolvedRev). Children use
// `resolved.commit` for oid-keyed (stable) queries.

import { createContext, useContext } from 'react';
import type { IpcError, RepoDetail, RepoSummary, ResolvedRev } from '../../ipc/types';

export interface RepoContextValue {
  summary: RepoSummary;
  repoId: number;
  owner: string;
  repo: string;
  detail: RepoDetail | null;
  detailError: IpcError | undefined;
  /** Decoded ref in use: route `:ref`, else HEAD's branch / default branch. null for an unborn/empty repo. */
  refName: string | null;
  /** True when `refName` came from the route. */
  refFromRoute: boolean;
  resolved: ResolvedRev | null;
  revError: IpcError | undefined;
  revLoading: boolean;
}

export const RepoContext = createContext<RepoContextValue | null>(null);

export function useRepo(): RepoContextValue {
  const v = useContext(RepoContext);
  if (!v) throw new Error('useRepo outside RepoLayout');
  return v;
}

/** Default ref for links when the route has none. */
export function defaultRef(summary: RepoSummary, detail: RepoDetail | null): string | null {
  const h = detail?.head;
  if (h?.type === 'branch') return h.name;
  if (h?.type === 'detached') return h.oid;
  if (h?.type === 'unborn') return h.name;
  return summary.defaultBranch;
}
