// Jobs indicator (S0, FR-7, NFR-P7): button with active-count badge; popover rows with progress, Cancel, Retry,
// Dismiss. Done rows vanish after 5 s (jobs store); failures stay with the backend's message.

import { Activity, CircleAlert, CircleCheck, CircleSlash, LoaderCircle, RotateCcw, X } from 'lucide-react';
import { useRef, useState } from 'react';
import type { JobInfo, Progress } from '../../ipc/types';
import { cancelJob, dismissJob, isActive, retryJob, useJobs } from '../../state/jobs';
import { formatBytes } from '../../util/format';
import { Button } from '../ui/Button';
import { useDismiss } from '../ui/Popover';

export function progressText(p: Progress | null, state: JobInfo['state']): string {
  if (state === 'waiting') return 'Waiting…';
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
  }
}

function fraction(p: Progress | null): number | null {
  if (!p) return null;
  if (p.type === 'receiving' && p.total > 0) return p.received / p.total;
  if (p.type === 'resolving' && p.total > 0) return p.done / p.total;
  if (p.type === 'finalizing') return 1;
  return null;
}

function resultText(j: JobInfo): string | null {
  const r = j.result;
  if (!r) return null;
  if (r.type === 'cloned') {
    return r.skippedRefs.length > 0
      ? `Cloned. ${r.skippedRefs.length} refs could not be stored on Windows: ${r.skippedRefs.join(', ')}`
      : 'Cloned.';
  }
  const n = r.newCommits;
  return `${n === 0 ? 'No' : n} new commit${n === 1 ? '' : 's'}${r.newTags.length > 0 ? `, ${r.newTags.length} new tag(s)` : ''}.`;
}

function JobRow({ job }: { job: JobInfo }) {
  const f = fraction(job.progress);
  const active = isActive(job);
  const Icon =
    job.state === 'done' ? CircleCheck : job.state === 'failed' ? CircleAlert : job.state === 'cancelled' ? CircleSlash : LoaderCircle;
  return (
    <li className="job">
      <div className="job__title">
        <Icon
          size={16}
          aria-hidden="true"
          className={`job__icon job__icon--${job.state}${active ? ' spin' : ''}`}
        />
        <span className="truncate">{job.title}</span>
      </div>
      <div className="job__status" aria-live="polite">
        {job.state === 'done' ? (resultText(job) ?? 'Done') : progressText(job.progress, job.state)}
      </div>
      {active && (
        <div className={`progress${f === null ? ' progress--indeterminate' : ''}`} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={f === null ? undefined : Math.round(f * 100)}>
          <div className="progress__bar" style={f === null ? undefined : { width: `${Math.round(f * 100)}%` }} />
        </div>
      )}
      {job.state === 'failed' && job.error && <div className="job__error">{job.error}</div>}
      <div className="job__actions">
        {active && (
          <Button size="sm" icon={X} onClick={() => void cancelJob(job.id)}>
            Cancel
          </Button>
        )}
        {(job.state === 'failed' || job.state === 'cancelled') && (
          <Button size="sm" icon={RotateCcw} onClick={() => void retryJob(job.id)}>
            Retry
          </Button>
        )}
        {!active && (
          <Button size="sm" variant="invisible" onClick={() => dismissJob(job.id)}>
            Dismiss
          </Button>
        )}
      </div>
    </li>
  );
}

export function JobsButton() {
  const jobs = useJobs();
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const btn = useRef<HTMLButtonElement>(null);
  useDismiss(open, () => setOpen(false), [wrap], btn);
  const active = jobs.filter(isActive).length;
  const failed = jobs.filter((j) => j.state === 'failed').length;
  const label = active > 0 ? `Background jobs: ${active} running` : failed > 0 ? `Background jobs: ${failed} failed` : 'Background jobs';
  return (
    <div className="dropdown" ref={wrap}>
      <button
        ref={btn}
        type="button"
        className="gh__btn"
        aria-label={label}
        title={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <Activity size={16} aria-hidden="true" />
        {(active > 0 || failed > 0) && <span className="gh__count">{active > 0 ? active : '!'}</span>}
      </button>
      {open && (
        <div className="popover popover--right jobs" role="dialog" aria-label="Background jobs">
          <div className="popover__header">Background jobs</div>
          {jobs.length === 0 ? (
            <div className="menu__note jobs__empty">No clones or fetches running.</div>
          ) : (
            <ul className="menu jobs__list scroll-y">
              {jobs.map((j) => (
                <JobRow key={j.id} job={j} />
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
