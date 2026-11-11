// Pure helpers for the commit page (FR-27, FR-28): why a file card starts collapsed, how its path reads, the +/−
// mini bar.

import type { ChangeKind, FileChange } from '../../ipc/types';

/**
 * Why a card starts collapsed (null = show the inline diff):
 * - `binary`: never diffed (no Load button) · `large`: a side is > 1 MB, never rendered (no Load button)
 * - `generated`: lockfile / minified / dist — hidden, "Load diff" reveals it (inline if present, else fetched)
 * - `deferred`: the commit is over the file cap, diffs are on demand · `lines`: no inline diff (> 2,000 changed
 *   lines, or the commit's 20,000-line budget ran out) — "Load diff" fetches it with `force`.
 */
export type Collapse = 'binary' | 'large' | 'generated' | 'deferred' | 'lines' | null;

export function collapseReason(f: FileChange, hasInline: boolean, deferred: boolean): Collapse {
  if (f.binary) return 'binary';
  if (f.tooLarge) return 'large';
  if (f.generated) return 'generated';
  if (hasInline) return null;
  return deferred ? 'deferred' : 'lines';
}

/** Collapsed reasons that offer "Load diff". */
export function canLoad(c: Collapse): boolean {
  return c === 'generated' || c === 'deferred' || c === 'lines';
}

export function filePath(f: FileChange): string {
  return f.newPath ?? f.oldPath ?? '';
}

/** `old → new` for renames / copies, else the one path. */
export function pathLabel(f: FileChange): { from: string | null; to: string } {
  const to = filePath(f);
  if ((f.kind === 'renamed' || f.kind === 'copied') && f.oldPath && f.oldPath !== to) return { from: f.oldPath, to };
  return { from: null, to };
}

export const KIND_LABEL: Record<ChangeKind, string | null> = {
  added: 'Added',
  deleted: 'Deleted',
  modified: null,
  renamed: 'Renamed',
  copied: 'Copied',
  typeChange: 'Type changed',
};

/** Five blocks like GitHub's diffstat: filled in proportion to additions vs deletions. */
export function miniBlocks(additions: number, deletions: number, n = 5): ('add' | 'del' | 'none')[] {
  const total = additions + deletions;
  if (total === 0) return Array<'none'>(n).fill('none');
  const filled = Math.min(n, total);
  let adds = Math.round((filled * additions) / total);
  if (additions > 0 && adds === 0) adds = 1;
  if (deletions > 0 && adds === filled) adds = filled - 1;
  return Array.from({ length: n }, (_, i) => (i < adds ? 'add' : i < filled ? 'del' : 'none'));
}
