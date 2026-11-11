// Commit list paging (FR-26, D-9, D-37, D-77). The URL carries `?page=N` and, from page 2 on, the pinned `?tip=` so
// every page re-walks from the same commit. Without a path filter a page starts at walk position (N-1)·35; with a
// path filter the start of page N is the `skip` page N-1 returned, so pages 1..N-1 are walked first (each page is
// cached here, keyed by the pinned tip, so Older/Newer and reloads cost one request per new page).

import { commitsPage } from '../../ipc/api';
import type { CommitsPage } from '../../ipc/types';
import type { Query } from '../../router/routes';
import { href } from '../../router/router';

export const COMMITS_PER_PAGE = 35;

export interface CommitsQuery {
  path: string;
  since: string;
  page: number;
  tip: string | null;
}

export function readCommitsQuery(q: Query): CommitsQuery {
  const n = Number.parseInt(q.page ?? '1', 10);
  const tip = q.tip && /^[0-9a-fA-F]{40}$/.test(q.tip) ? q.tip.toLowerCase() : null;
  return { path: (q.path ?? '').replace(/^\/+|\/+$/g, ''), since: q.since ?? '', page: Number.isFinite(n) && n > 1 ? n : 1, tip };
}

/** Commits route href; page 1 leaves `page` out, `tip` is kept whenever given. */
export function commitsHref(owner: string, repo: string, ref: string, q: Partial<CommitsQuery> = {}): string {
  return href(
    'commits',
    { owner, repo, ref },
    { path: q.path || null, since: q.since || null, tip: q.tip ?? null, page: q.page && q.page > 1 ? q.page : null },
  );
}

/** Newer / Older hrefs for the page being shown; both keep the pinned tip (and path / since). */
export function pagerHrefs(
  owner: string,
  repo: string,
  ref: string,
  q: CommitsQuery,
  shown: Pick<CommitsPage, 'tip' | 'hasMore' | 'capped'>,
): { newer: string | null; older: string | null } {
  const base = { path: q.path, since: q.since, tip: shown.tip };
  return {
    newer: q.page > 1 ? commitsHref(owner, repo, ref, { ...base, page: q.page - 1 }) : null,
    older: shown.hasMore || shown.capped ? commitsHref(owner, repo, ref, { ...base, page: q.page + 1 }) : null,
  };
}

const pageCache = new Map<string, Promise<CommitsPage>>();
const PAGE_CACHE_MAX = 120;

function cachedPage(repoId: number, rev: string, path: string | null, since: string | null, skip: number, tip: string | null): Promise<CommitsPage> {
  const key = JSON.stringify([repoId, tip ?? rev, path, since, skip]);
  let p = pageCache.get(key);
  if (!p) {
    p = commitsPage(repoId, rev, { path, since, skip, tip });
    p.catch(() => pageCache.delete(key));
    pageCache.set(key, p);
    if (pageCache.size > PAGE_CACHE_MAX) pageCache.delete(pageCache.keys().next().value as string);
  }
  return p;
}

/**
 * Page `q.page` of the list. `rev` is the resolved commit of the route ref (used when no tip is pinned yet).
 * A page past the end comes back empty with `hasMore: false`.
 */
export async function loadCommitsPage(repoId: number, rev: string, q: CommitsQuery): Promise<CommitsPage> {
  const path = q.path || null;
  const since = q.since || null;
  if (path === null) {
    return cachedPage(repoId, rev, null, since, (q.page - 1) * COMMITS_PER_PAGE, q.tip);
  }
  let skip = 0;
  let tip = q.tip;
  for (let p = 1; ; p++) {
    const res = await cachedPage(repoId, rev, path, since, skip, tip);
    if (p === q.page) return res;
    if (!res.hasMore && !res.capped) return { ...res, commits: [], hasMore: false, capped: false };
    tip = res.tip;
    skip = res.skip;
  }
}

/** Tests only. */
export function resetCommitsPageCache(): void {
  pageCache.clear();
}
