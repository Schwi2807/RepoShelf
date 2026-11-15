// Save to shelf / Open in shelf (v0.3.0 M10b, D-125, D-130): the button of every Explore repository page. Save starts the
// existing bare-clone job for `https://github.com/<owner>/<name>.git` in the background (progress from the jobs
// store, cancellable, the jobs tray shows it too); when the job is done — or the backend says the repository is already
// on the shelf — the same place offers "Open in shelf". The link comes from the library entry with the id the
// backend answered (`onShelf`, or the finished job's `repoId`), never from the GitHub name: a shelf slug can differ.

import { Archive, ChevronDown, CircleCheck, Download, FolderOpen, X } from 'lucide-react';
import { useState } from 'react';
import type { JobId } from '../../ipc/types';
import { exploreSave } from '../../source';
import { errorText, toIpcError } from '../../ipc/errors';
import { invalidate } from '../../ipc/query';
import type { ExploreRepo, JobInfo } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { cancelJob, isActive, useJobs } from '../../state/jobs';
import { libraryStore, refreshLibrary } from '../../state/library';
import { useStore } from '../../state/store';
import { progressFraction, progressText } from '../../util/jobText';
import { Button } from '../ui/Button';
import { Spinner } from '../ui/bits';
import { Dropdown } from '../ui/Dropdown';

export type SaveRepo = Pick<ExploreRepo, 'owner' | 'name' | 'cloneUrl' | 'onShelf'>;

export interface SaveState {
  /** repository id on the shelf, once known */
  shelfId: number | null;
  /** the clone job to show (running, or failed and not dismissed) */
  job: JobInfo | undefined;
  busy: boolean;
  error: string | null;
  start: (autoDownload: boolean | null) => void;
}

/** The clone job for this repository (the newest one), whichever page started it: the id this page got, or the URL. */
function cloneJobFor(jobs: JobInfo[], cloneUrl: string, id: JobId | null): JobInfo | undefined {
  const url = cloneUrl.toLowerCase();
  let found: JobInfo | undefined;
  for (const j of jobs) if (j.id === id || (j.kind.type === 'clone' && j.kind.url.toLowerCase() === url)) found = j;
  return found;
}

export function useSaveToShelf(repo: SaveRepo): SaveState {
  const jobs = useJobs();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [jobId, setJobId] = useState<JobId | null>(null);
  const job = cloneJobFor(jobs, repo.cloneUrl, jobId);
  const doneId = job?.state === 'done' && job.result?.type === 'cloned' ? job.result.repoId : null;

  const start = (autoDownload: boolean | null) => {
    setBusy(true);
    setError(null);
    exploreSave(repo.owner, repo.name, autoDownload).then(
      (id) => {
        setJobId(id);
        setBusy(false);
      },
      (e: unknown) => {
        setBusy(false);
        if (toIpcError(e).code === 'duplicate') {
          // already on the shelf (another window, or saved before): the repository answer carries the id now
          invalidate();
          void refreshLibrary();
          return;
        }
        setError(errorText(e));
      },
    );
  };
  return { shelfId: repo.onShelf ?? doneId, job, busy, error, start };
}

/** Where "Open in shelf" leads: the library entry with that id (its slug), else the GitHub name. */
export function shelfHref(shelfId: number, owner: string, name: string): string {
  const s = libraryStore.get().repos?.find((r) => r.id === shelfId);
  return href('code', { owner: s?.owner ?? owner, repo: s?.name ?? name });
}

export function OpenInShelf({ repo, shelfId, size = 'sm' }: { repo: SaveRepo; shelfId: number; size?: 'sm' | 'md' }) {
  useStore(libraryStore);
  return (
    <Link to={shelfHref(shelfId, repo.owner, repo.name)} className={`btn btn--primary${size === 'sm' ? ' btn--sm' : ''}`} data-testid="explore-open-in-shelf">
      <FolderOpen size={16} aria-hidden="true" />
      Open in shelf
    </Link>
  );
}

/** The Save to shelf control: split button while idle, progress + Cancel while the clone runs, Open in shelf after. */
export function SaveToShelfButton({ repo, size = 'sm' }: { repo: SaveRepo; size?: 'sm' | 'md' }) {
  const s = useSaveToShelf(repo);
  if (s.shelfId !== null) {
    return (
      <span className="row" data-testid="explore-saved">
        <span className="badge badge--success" title="This repository is on your shelf">
          <CircleCheck size={12} aria-hidden="true" />
          On your shelf
        </span>
        <OpenInShelf repo={repo} shelfId={s.shelfId} size={size} />
      </span>
    );
  }
  const j = s.job;
  if (j !== undefined && isActive(j)) {
    const f = progressFraction(j.progress);
    return (
      <span className="row save-progress" data-testid="explore-saving">
        <Spinner size={14} label="Saving to the shelf" />
        <span className="small">
          Saving to shelf{f !== null ? ` ${Math.round(f * 100)}%` : '…'}
          <span className="muted"> {progressText(j.progress, j.state)}</span>
        </span>
        <Button size="sm" icon={X} label="Cancel saving to the shelf" onClick={() => void cancelJob(j.id)} />
      </span>
    );
  }
  const failed = j?.state === 'failed';
  return (
    <span className="row row--wrap">
      {(failed || s.error) && (
        <span className="field__error small" role="alert" data-testid="explore-save-error">
          {s.error ?? j?.error ?? 'Saving failed.'}
        </span>
      )}
      <span className="btn-split">
        <Button
          variant="primary"
          size={size}
          icon={Archive}
          disabled={s.busy}
          data-testid="explore-save"
          onClick={() => s.start(null)}
        >
          {failed || s.error ? 'Try again' : 'Save to shelf'}
        </Button>
        <Dropdown
          label="Save options"
          icon={ChevronDown}
          size={size}
          align="right"
          caret={false}
          variant="primary"
          header="Clone this repository onto your shelf (bare, in the background)"
          items={[
            { key: 'save-dl', label: 'Save and download the latest release', icon: Download, onSelect: () => s.start(true) },
            { key: 'save-only', label: 'Save without downloading a release', icon: Archive, onSelect: () => s.start(false) },
          ]}
        />
      </span>
    </span>
  );
}
