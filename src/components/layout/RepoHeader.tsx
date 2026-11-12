// S2 repo header row 1 (FR-10, FR-11, FR-12, FR-36): book icon, `owner / name`, source badge, origin link (confirmed),
// Fetch (clones directly; fetchable imported repos after the D-3 confirm) with the "N new commits" result; for
// github.com origins a split menu picks "with / without the latest release download" (v0.2.0 F4, else the setting),
// Fast-forward when `canFastForward`, Open folder, "…" → Copy location / Remove from shelf.

import { Archive, BookMarked, ChevronDown, Download, Ellipsis, FastForward, FolderOpen, Globe, HardDrive, Link2, RefreshCw, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { fastForward, fetchStart, repoOpenFolder } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { invalidate } from '../../ipc/query';
import type { JobId, RepoDetail, RepoSummary } from '../../ipc/types';
import { copyLocation, href, navigate } from '../../router/router';
import { Link } from '../../router/Link';
import { useJobs } from '../../state/jobs';
import { refreshLibrary } from '../../state/library';
import { useSettings } from '../../state/settings';
import { isGithubUrl } from '../dialogs/AutoDownloadCheck';
import { shortOid } from '../../util/format';
import { fetchResultText } from '../../util/jobText';
import { FetchConfirmDialog } from '../dialogs/FetchConfirmDialog';
import { RemoveDialog } from '../dialogs/RemoveDialog';
import { Button } from '../ui/Button';
import { Badge, Spinner } from '../ui/bits';
import { confirmExternal, externalKind } from '../ui/ConfirmExternalLink';
import { Dropdown } from '../ui/Dropdown';

export function SourceBadge({ summary }: { summary: RepoSummary }) {
  return summary.source === 'cloned' ? (
    <Badge tone="accent" icon={Archive} title="Managed clone (bare archive)">
      Cloned
    </Badge>
  ) : (
    <Badge icon={HardDrive} title={`Imported from ${summary.path}`}>
      Imported
    </Badge>
  );
}

export function RepoHeader({ summary, detail }: { summary: RepoSummary; detail: RepoDetail | null }) {
  const [msg, setMsg] = useState<{ text: string; error: boolean } | null>(null);
  const [jobId, setJobId] = useState<JobId | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [ffBusy, setFfBusy] = useState(false);
  const jobs = useJobs();
  const autoDefault = useSettings().settings.autoDownloadLatestRelease;
  const missing = summary.status.type === 'missing';
  const canFetch = detail?.fetchable === true && !missing;
  const job = jobId === null ? undefined : jobs.find((j) => j.id === jobId);
  const running = job !== undefined && (job.state === 'waiting' || job.state === 'running');

  useEffect(() => {
    if (!job) return;
    if (job.state === 'done' && job.result?.type === 'fetched') {
      setMsg({ text: fetchResultText(job.result.newCommits, job.result.newTags), error: false });
      setJobId(null);
    } else if (job.state === 'failed') {
      setMsg({ text: job.error ?? 'Fetch failed.', error: true });
      setJobId(null);
    } else if (job.state === 'cancelled') {
      setMsg({ text: 'Fetch cancelled.', error: false });
      setJobId(null);
    }
  }, [job]);

  // F4: the Fetch menu's choice (null = the setting), kept across the imported-repo confirm.
  const [autoDl, setAutoDl] = useState<boolean | null>(null);
  const github = isGithubUrl(summary.originUrl);
  const startFetch = (auto: boolean | null = autoDl) => {
    setMsg(null);
    fetchStart(summary.id, github ? auto : null).then(setJobId, (e: unknown) => setMsg({ text: errorText(e), error: true }));
  };
  const requestFetch = (auto: boolean | null) => {
    setAutoDl(auto);
    if (summary.source === 'imported') setConfirming(true);
    else startFetch(auto);
  };

  const origin = summary.originUrl;
  return (
    <div className="rh__row">
      <h1 className="rh__title">
        <BookMarked size={18} aria-hidden="true" className="muted" />
        <Link to={href('dashboard', {}, { q: summary.owner })}>{summary.owner}</Link>
        <span className="rh__sep">/</span>
        <strong>
          <Link to={href('code', { owner: summary.owner, repo: summary.name })}>{summary.name}</Link>
        </strong>
        <SourceBadge summary={summary} />
      </h1>
      {origin && externalKind(origin) === 'web' && (
        <a
          className="rh__origin small"
          href={origin}
          title={`Open ${origin} in your browser`}
          onClick={(e) => {
            e.preventDefault();
            confirmExternal(origin.replace(/\.git$/, ''));
          }}
        >
          <Globe size={14} aria-hidden="true" />
          {summary.originHost}
        </a>
      )}
      <div className="rh__spacer" />
      <div className="rh__actions">
        {msg && (
          <span className={msg.error ? 'field__error rh__msg' : 'rh__msg small muted'} role={msg.error ? 'alert' : 'status'}>
            {msg.text}
          </span>
        )}
        {detail?.canFastForward === true && (
          <Button
            size="sm"
            icon={FastForward}
            disabled={ffBusy}
            title={`Move ${summary.defaultBranch ?? 'the default branch'} forward to origin`}
            onClick={async () => {
              setFfBusy(true);
              setMsg(null);
              try {
                const oid = await fastForward(summary.id);
                setMsg({ text: `Fast-forwarded to ${shortOid(oid)}`, error: false });
                invalidate();
                void refreshLibrary();
              } catch (e) {
                setMsg({ text: errorText(e), error: true });
              } finally {
                setFfBusy(false);
              }
            }}
          >
            Fast-forward
          </Button>
        )}
        {canFetch && (
          <div className={github ? 'btn-split' : undefined}>
            <Button size="sm" icon={running ? undefined : RefreshCw} disabled={running} onClick={() => requestFetch(null)}>
              {running ? (
                <>
                  <Spinner size={14} label="Fetching" /> Fetching…
                </>
              ) : (
                'Fetch'
              )}
            </Button>
            {github && !running && (
              <Dropdown
                label="Fetch options"
                icon={ChevronDown}
                size="sm"
                align="right"
                caret={false}
                header={`Release download: ${autoDefault ? 'on' : 'off'} by default (Settings)`}
                items={[
                  { key: 'fetch-dl', label: 'Fetch and download the latest release', icon: Download, onSelect: () => requestFetch(true) },
                  { key: 'fetch-only', label: 'Fetch without downloading a release', icon: RefreshCw, onSelect: () => requestFetch(false) },
                ]}
              />
            )}
          </div>
        )}
        <Button
          size="sm"
          icon={FolderOpen}
          disabled={missing}
          onClick={() => {
            setMsg(null);
            repoOpenFolder(summary.id).catch((e: unknown) => setMsg({ text: errorText(e), error: true }));
          }}
        >
          Open folder
        </Button>
        <Dropdown
          label="More repository actions"
          icon={Ellipsis}
          size="sm"
          align="right"
          caret={false}
          items={[
            { key: 'copy', label: 'Copy location', icon: Link2, onSelect: () => void copyLocation() },
            'separator',
            { key: 'remove', label: 'Remove from shelf…', icon: Trash2, danger: true, onSelect: () => setRemoving(true) },
          ]}
        />
      </div>
      {confirming && (
        <FetchConfirmDialog summary={summary} isBare={detail?.isBare ?? false} onConfirm={() => startFetch()} onClose={() => setConfirming(false)} />
      )}
      {removing && (
        <RemoveDialog summary={summary} onClose={() => setRemoving(false)} onRemoved={() => navigate(href('dashboard'))} />
      )}
    </div>
  );
}
