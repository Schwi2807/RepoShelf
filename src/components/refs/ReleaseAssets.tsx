// GitHub-like "Assets" list of a release card (v0.2.0 F4, D-111): one row per GitHub release file (name, size,
// download count, "for this PC" marker) with Download (Rust, into the release store, progress from the jobs tray) /
// Show in folder (once downloaded) / Open on GitHub (ConfirmExternalLink), then GitHub's "Source code (zip)" and
// "(tar.gz)" rows (open externally). Files are known only after a GitHub releases fetch; nothing is run or unpacked.

import { CircleCheck, Download, ExternalLink, FileArchive, FolderOpen, Monitor, Package, RotateCcw, X } from 'lucide-react';
import { useState } from 'react';
import { releaseDownload, releaseReveal } from '../../ipc/api';
import { errorText, toIpcError } from '../../ipc/errors';
import type { JobInfo, Release, ReleaseAsset } from '../../ipc/types';
import { cancelJob, isActive, useJobs } from '../../state/jobs';
import { formatBytes, formatCount } from '../../util/format';
import { progressFraction, progressText } from '../../util/jobText';
import { Badge } from '../ui/bits';
import { Button } from '../ui/Button';
import { confirmExternal } from '../ui/ConfirmExternalLink';

/** The newest `releaseDownload` job for this file (jobs store order = start order). */
export function downloadJob(jobs: JobInfo[], repoId: number, tag: string, asset: string): JobInfo | undefined {
  let found: JobInfo | undefined;
  for (const j of jobs) {
    if (j.kind.type === 'releaseDownload' && j.kind.repoId === repoId && j.kind.tag === tag && j.kind.asset === asset) found = j;
  }
  return found;
}

/** `repoId` null = a repository seen online (Explore): the files can only be opened on GitHub; saving the repository
 * to the shelf is what gives a release download. */
function AssetRow({ repoId, tag, asset, job }: { repoId: number | null; tag: string; asset: ReleaseAsset; job: JobInfo | undefined }) {
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const running = job !== undefined && isActive(job);
  const failed = job?.state === 'failed';
  const f = running ? progressFraction(job.progress) : null;

  const download = async () => {
    if (repoId === null) return;
    setErr(null);
    setBusy(true);
    try {
      await releaseDownload(repoId, tag, asset.name);
    } catch (e) {
      setErr(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const reveal = async () => {
    if (repoId === null) return;
    setErr(null);
    try {
      await releaseReveal(repoId, tag, asset.name);
    } catch (e) {
      setErr(toIpcError(e).code === 'notFound' ? 'The downloaded file is gone (moved or deleted). Download it again.' : errorText(e));
    }
  };

  return (
    <li className={`rasset${asset.forThisPc ? ' rasset--pc' : ''}`} data-asset={asset.name}>
      <div className="rasset__main">
        <Package size={16} aria-hidden="true" className="rasset__icon" />
        <span className="rasset__name mono break-all" title={asset.contentType ?? undefined}>
          {asset.name}
        </span>
        {asset.forThisPc && (
          <Badge tone="accent" icon={Monitor} title="The file RepoShelf picks for this PC (Windows, this architecture)">
            For this PC
          </Badge>
        )}
        {asset.downloaded !== null && (
          <Badge tone="success" icon={CircleCheck} title={asset.downloaded}>
            Downloaded
          </Badge>
        )}
      </div>
      <span className="rasset__size muted small">{formatBytes(asset.size)}</span>
      <span className="rasset__count muted small" title="Downloads on GitHub">
        {asset.downloadCount !== null ? `${formatCount(asset.downloadCount)} ${asset.downloadCount === 1 ? 'download' : 'downloads'}` : ''}
      </span>
      <div className="rasset__actions">
        {repoId === null ? null : running ? (
          <Button size="sm" icon={X} onClick={() => void cancelJob(job.id)} aria-label={`Cancel the download of ${asset.name}`}>
            Cancel
          </Button>
        ) : asset.downloaded !== null ? (
          <Button size="sm" icon={FolderOpen} onClick={() => void reveal()}>
            Show in folder
          </Button>
        ) : (
          <Button size="sm" icon={failed ? RotateCcw : Download} disabled={busy} onClick={() => void download()} aria-label={`Download ${asset.name}`}>
            {failed ? 'Retry' : 'Download'}
          </Button>
        )}
        <Button size="sm" variant="invisible" icon={ExternalLink} label={`Open ${asset.name} on GitHub`} onClick={() => confirmExternal(asset.url)} />
      </div>
      {running && (
        <div className="rasset__progress">
          <div
            className={`progress${f === null ? ' progress--indeterminate' : ''}`}
            role="progressbar"
            aria-label={`Download of ${asset.name}`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={f === null ? undefined : Math.round(f * 100)}
          >
            <div className="progress__bar" style={f === null ? undefined : { width: `${Math.round(f * 100)}%` }} />
          </div>
          <span className="muted small">{progressText(job.progress, job.state)}</span>
        </div>
      )}
      {failed && !running && (
        <p className="rasset__error field__error" role="alert">
          {job.error ?? 'The download failed.'}
        </p>
      )}
      {err && (
        <p className="rasset__error field__error" role="alert">
          {err}
        </p>
      )}
    </li>
  );
}

function SourceRow({ label, url }: { label: string; url: string }) {
  return (
    <li className="rasset rasset--source">
      <div className="rasset__main">
        <FileArchive size={16} aria-hidden="true" className="rasset__icon" />
        <a
          href={url}
          className="rasset__name"
          onClick={(e) => {
            e.preventDefault();
            confirmExternal(url);
          }}
        >
          <strong>Source code</strong> ({label})
        </a>
      </div>
    </li>
  );
}

/** `ghLoaded`: a GitHub release list is cached for this repo (some release carries `htmlUrl`). */
export function ReleaseAssets({ rel, repoId, github, ghLoaded }: { rel: Release; repoId: number | null; github: boolean; ghLoaded: boolean }) {
  const jobs = useJobs();
  const sources = [
    rel.sourceZipUrl ? { label: 'zip', url: rel.sourceZipUrl } : null,
    rel.sourceTarUrl ? { label: 'tar.gz', url: rel.sourceTarUrl } : null,
  ].filter((x): x is { label: string; url: string } => x !== null);
  if (!github || (rel.assets.length === 0 && sources.length === 0)) return null;
  const n = rel.assets.length + sources.length;
  return (
    <details className="rassets" open>
      <summary className="rassets__summary">
        <span className="rassets__title">Assets</span>
        <span className="counter">{n}</span>
      </summary>
      <ul className="rassets__list">
        {rel.assets.map((a) => (
          <AssetRow key={a.name} repoId={repoId} tag={rel.tag} asset={a} job={repoId === null ? undefined : downloadJob(jobs, repoId, rel.tag, a.name)} />
        ))}
        {sources.map((s) => (
          <SourceRow key={s.label} label={s.label} url={s.url} />
        ))}
      </ul>
      {rel.assets.length === 0 && (
        <p className="rassets__note muted small">
          {ghLoaded
            ? rel.htmlUrl
              ? 'GitHub lists no files for this release.'
              : 'GitHub has no release for this tag (only the source code archives).'
            : 'Release files show up after the GitHub release info is loaded (button above, or on Clone / Fetch / Import).'}
        </p>
      )}
    </details>
  );
}
