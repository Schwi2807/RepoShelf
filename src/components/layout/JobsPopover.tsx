// Jobs indicator (S0, FR-7, FR-9, FR-10, NFR-P7): button with active-count badge; popover rows per job with a
// per-kind heading, progress from the `Progress` variants, Cancel (waiting and running), Retry (failed only — the
// backend refuses others), Dismiss. Done rows vanish after 5 s (jobs store); failures stay with the backend's
// message. Results: clones list the refs Windows could not store (FR-9) and link to the new repo; fetches report
// new commits, new tags, kept local tags and skipped refs. v0.2.0: GitHub info (`githubSync`) and release downloads
// (`releaseDownload`, with "Show in folder" — the only way back to a file whose tag has no release card, D-80) plus
// the GitHub notes of clone / fetch / import.

import {
  Activity,
  Archive,
  CircleAlert,
  CircleCheck,
  CircleSlash,
  Download,
  FolderOpen,
  Globe,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  X,
  type LucideIcon,
} from 'lucide-react';
import { useRef, useState } from 'react';
import { releaseReveal } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { JobInfo } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { cancelJob, dismissJob, isActive, retryJob, useJobs } from '../../state/jobs';
import { useLibrary } from '../../state/library';
import { cloneReport, downloadReport, fetchReport, kindLabel, notesReport, progressFraction, progressText } from '../../util/jobText';
import { Button } from '../ui/Button';
import { useDismiss } from '../ui/Popover';

const STATE_ICON: Record<JobInfo['state'], LucideIcon> = {
  waiting: LoaderCircle,
  running: LoaderCircle,
  done: CircleCheck,
  failed: CircleAlert,
  cancelled: CircleSlash,
};

/** What the job works on, without the kind word the backend puts in front ("Clone owner/name" → "owner/name"). */
export function jobTarget(j: JobInfo): string {
  if (j.kind.type === 'fetchAll') return 'every clone on the shelf';
  if (j.kind.type === 'releaseDownload') return `${j.kind.asset} (${j.kind.tag})`;
  const t = j.title.replace(/^(clone|fetch|github info|download)\s+/i, '');
  return t.length > 0 ? t : j.kind.type === 'clone' ? j.kind.url : j.title;
}

const KIND_ICON: Record<JobInfo['kind']['type'], LucideIcon> = {
  clone: Archive,
  fetch: RefreshCw,
  fetchAll: RefreshCw,
  githubSync: Globe,
  releaseDownload: Download,
};

export function resultLines(j: JobInfo): string[] {
  const r = j.result;
  if (!r) return ['Done.'];
  switch (r.type) {
    case 'cloned':
      return [...cloneReport(r.skippedRefs), ...notesReport(r.notes)];
    case 'fetched':
      return [...fetchReport(r), ...notesReport(r.notes)];
    case 'synced':
      return r.notes.length > 0 ? notesReport(r.notes) : ['GitHub info updated.'];
    case 'downloaded':
      return [downloadReport(r)];
  }
}

function RevealButton({ repoId, tag, file, onError }: { repoId: number; tag: string; file: string; onError: (e: unknown) => void }) {
  return (
    <Button size="sm" icon={FolderOpen} onClick={() => releaseReveal(repoId, tag, file).catch(onError)}>
      Show in folder
    </Button>
  );
}

export function JobRow({ job }: { job: JobInfo }) {
  const [actionError, setActionError] = useState<string | null>(null);
  const lib = useLibrary();
  const f = progressFraction(job.progress);
  const active = isActive(job);
  const Icon = STATE_ICON[job.state];
  const KindIcon = KIND_ICON[job.kind.type];
  const res = job.result;
  const cloned = job.state === 'done' && res?.type === 'cloned' ? lib.repos?.find((r) => r.id === res.repoId) : undefined;
  const downloaded = job.state === 'done' && res?.type === 'downloaded' ? res : undefined;

  const run = (p: Promise<unknown>) => {
    setActionError(null);
    p.catch((e: unknown) => setActionError(errorText(e)));
  };

  return (
    <li className={`job job--${job.state}`} data-state={job.state} data-kind={job.kind.type}>
      <div className="job__title">
        <Icon size={16} aria-hidden="true" className={`job__icon job__icon--${job.state}${active ? ' spin' : ''}`} />
        <span className="job__kind">
          <KindIcon size={12} aria-hidden="true" />
          {kindLabel(job.kind)}
        </span>
        <span className="truncate job__target" title={job.kind.type === 'clone' ? job.kind.url : job.title}>
          {jobTarget(job)}
        </span>
      </div>
      {job.state === 'done' ? (
        <div className="job__status job__status--done" aria-live="polite">
          {resultLines(job).map((l, i) => (
            <div key={i}>{l}</div>
          ))}
        </div>
      ) : (
        <div className="job__status" aria-live="polite">
          {progressText(job.progress, job.state)}
        </div>
      )}
      {active && (
        <div
          className={`progress${f === null ? ' progress--indeterminate' : ''}`}
          role="progressbar"
          aria-label={`${kindLabel(job.kind)} progress`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={f === null ? undefined : Math.round(f * 100)}
        >
          <div className="progress__bar" style={f === null ? undefined : { width: `${Math.round(f * 100)}%` }} />
        </div>
      )}
      {job.state === 'failed' && <div className="job__error">{job.error ?? 'The job failed.'}</div>}
      {actionError && <div className="job__error">{actionError}</div>}
      <div className="job__actions">
        {cloned && (
          <Link to={href('code', { owner: cloned.owner, repo: cloned.name })} className="btn btn--sm">
            Open repository
          </Link>
        )}
        {downloaded && (
          <RevealButton repoId={downloaded.repoId} tag={downloaded.tag} file={downloaded.file} onError={(e) => setActionError(errorText(e))} />
        )}
        {active && (
          <Button size="sm" icon={X} onClick={() => run(cancelJob(job.id))}>
            Cancel
          </Button>
        )}
        {job.state === 'failed' && (
          <Button size="sm" icon={RotateCcw} onClick={() => run(retryJob(job.id))}>
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

export function JobsList({ jobs }: { jobs: JobInfo[] }) {
  if (jobs.length === 0) return <div className="menu__note jobs__empty">No background jobs (clones, fetches, downloads) running.</div>;
  return (
    <ul className="menu jobs__list scroll-y">
      {jobs.map((j) => (
        <JobRow key={j.id} job={j} />
      ))}
    </ul>
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
        {(active > 0 || failed > 0) && <span className={`gh__count${active === 0 ? ' gh__count--failed' : ''}`}>{active > 0 ? active : '!'}</span>}
      </button>
      {open && (
        <div className="popover popover--right jobs" role="dialog" aria-label="Background jobs">
          <div className="popover__header">Background jobs</div>
          <JobsList jobs={jobs} />
        </div>
      )}
    </div>
  );
}
