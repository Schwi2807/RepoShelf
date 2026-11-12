// Job wording shared by the jobs popover and the repo header (FR-7, FR-9, FR-10).

import type { FetchOutcome, JobInfo, JobKind, Progress } from '../ipc/types';
import { formatBytes, plural } from './format';

/** Short fetch result for the repo header: "3 new commits", "No new commits, 1 new tag". */
export function fetchResultText(newCommits: number, newTags: string[]): string {
  const parts = [newCommits === 0 ? 'No new commits' : plural(newCommits, 'new commit')];
  if (newTags.length > 0) parts.push(plural(newTags.length, 'new tag'));
  return parts.join(', ');
}

function names(xs: string[], max = 4): string {
  return xs.length <= max ? xs.join(', ') : `${xs.slice(0, max).join(', ')} and ${xs.length - max} more`;
}

/** Full fetch report for the popover: commits, new tags by name, local tags kept, refs skipped. */
export function fetchReport(o: FetchOutcome): string[] {
  const out = [fetchResultText(o.newCommits, []) + (o.newTags.length > 0 ? `, new tag${o.newTags.length === 1 ? '' : 's'} ${names(o.newTags)}` : '') + '.'];
  if (o.keptLocalTags.length > 0) out.push(`Kept local tags ${names(o.keptLocalTags)} (the remote has different ones; nothing was overwritten).`);
  if (o.skippedRefs.length > 0) out.push(`Skipped ${plural(o.skippedRefs.length, 'ref')} Windows cannot store: ${names(o.skippedRefs)}.`);
  return out;
}

/** Clone report (FR-9): skipped refs are listed by name. */
export function cloneReport(skippedRefs: string[]): string[] {
  if (skippedRefs.length === 0) return ['Cloned.'];
  return ['Cloned.', `Skipped ${plural(skippedRefs.length, 'ref')} Windows cannot store: ${names(skippedRefs, 6)}.`];
}

/** Per-kind heading: "Clone", "Fetch", "Fetch all clones" (+ v0.2.0 "GitHub info", "Download"). */
export function kindLabel(k: JobKind): string {
  switch (k.type) {
    case 'clone':
      return 'Clone';
    case 'fetch':
      return 'Fetch';
    case 'fetchAll':
      return 'Fetch all clones';
    case 'githubSync':
      return 'GitHub info';
    case 'releaseDownload':
      return 'Download';
  }
}

/** v0.2.0 (M11 minimal; M12 owns the wording): GitHub / auto-download notes of a finished job. */
export function notesReport(notes: string[]): string[] {
  return notes;
}

export function progressText(p: Progress | null, state: JobInfo['state']): string {
  if (state === 'waiting') return 'Waiting for a free slot…';
  if (state === 'done') return 'Done';
  if (state === 'cancelled') return 'Cancelled';
  if (state === 'failed') return 'Failed';
  if (!p) return 'Starting…';
  switch (p.type) {
    case 'connecting':
      return 'Connecting…';
    case 'receiving':
      return `Receiving objects: ${p.total > 0 ? Math.floor((p.received / p.total) * 100) : 0}% (${p.received}/${p.total}), ${formatBytes(p.bytes)}`;
    case 'resolving':
      return `Resolving deltas: ${p.total > 0 ? Math.floor((p.done / p.total) * 100) : 0}% (${p.done}/${p.total})`;
    case 'finalizing':
      return 'Finishing…';
    case 'downloading':
      return p.total > 0
        ? `Downloading: ${Math.floor((p.bytes / p.total) * 100)}% (${formatBytes(p.bytes)} of ${formatBytes(p.total)})`
        : `Downloading: ${formatBytes(p.bytes)}`;
  }
}

/** Progress fraction 0..1, or null for an indeterminate bar. */
export function progressFraction(p: Progress | null): number | null {
  if (!p) return null;
  if (p.type === 'receiving' && p.total > 0) return p.received / p.total;
  if (p.type === 'resolving' && p.total > 0) return p.done / p.total;
  if (p.type === 'downloading' && p.total > 0) return Math.min(1, p.bytes / p.total);
  if (p.type === 'finalizing') return 1;
  return null;
}
