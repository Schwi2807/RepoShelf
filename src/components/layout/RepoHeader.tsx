// S2 repo header row 1 (FR-10, FR-11, FR-12, FR-36): book icon, `owner / name`, source badge, origin link (confirmed),
// Fetch (clones directly; fetchable imported repos after the D-3 confirm) with the "N new commits" result,
// Fast-forward when `canFastForward`, Open folder, "…" → Copy location / Remove from shelf.

import { Archive, BookMarked, Ellipsis, FastForward, FolderOpen, Globe, HardDrive, Link2, RefreshCw, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { fastForward, fetchStart, repoOpenFolder } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { invalidate } from '../../ipc/query';
import type { JobId, RepoDetail, RepoSummary } from '../../ipc/types';
import { copyLocation, href, navigate } from '../../router/router';
import { Link } from '../../router/Link';
import { useJobs } from '../../state/jobs';
import { refreshLibrary } from '../../state/library';
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

  const startFetch = () => {
    setMsg(null);
    fetchStart(summary.id).then(setJobId, (e: unknown) => setMsg({ text: errorText(e), error: true }));
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
          <Button
            size="sm"
            icon={running ? undefined : RefreshCw}
            disabled={running}
            onClick={() => (summary.source === 'imported' ? setConfirming(true) : startFetch())}
          >
            {running ? (
              <>
                <Spinner size={14} label="Fetching" /> Fetching…
              </>
            ) : (
              'Fetch'
            )}
          </Button>
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
        <FetchConfirmDialog summary={summary} isBare={detail?.isBare ?? false} onConfirm={startFetch} onClose={() => setConfirming(false)} />
      )}
      {removing && (
        <RemoveDialog summary={summary} onClose={() => setRemoving(false)} onRemoved={() => navigate(href('dashboard'))} />
      )}
    </div>
  );
}
