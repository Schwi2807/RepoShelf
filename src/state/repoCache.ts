// Keeps the library store and the query cache in step after a mutation (Edit About, Locate…, Remove, fast-forward).
// RepoLayout reads its summary from the `repo_by_slug` query, not from the library store, so both are updated.

import { evict, invalidate, prime, qk } from '../ipc/query';
import type { RepoSummary } from '../ipc/types';
import { removeRepoLocal, upsertRepo } from './library';

export function slugKey(owner: string, name: string): string {
  return qk('repo_by_slug', owner.toLowerCase(), name.toLowerCase());
}

/** A changed summary: library row, slug cache, repo detail refetched. */
export function applySummary(r: RepoSummary): void {
  upsertRepo(r);
  prime(slugKey(r.owner, r.name), r);
  evict(qk('repo_detail', r.id));
}

/** The repo's files may have changed (locate, fast-forward): refetch every ref-keyed entry. */
export function applySummaryAndRefresh(r: RepoSummary): void {
  applySummary(r);
  invalidate();
}

/** Removed from the shelf. */
export function forgetRepo(r: RepoSummary): void {
  removeRepoLocal(r.id);
  evict(slugKey(r.owner, r.name));
  evict(qk('repo_detail', r.id));
}
