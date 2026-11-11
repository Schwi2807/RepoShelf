// Dashboard rows (S1, FR-1, FR-4): a normal repo row and the warning row for a missing folder.

import { FolderSearch, Globe, HardDrive, OctagonAlert, Trash2, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { repoLocate } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { RepoSummary } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { applySummaryAndRefresh } from '../../state/repoCache';
import { RemoveDialog } from '../dialogs/RemoveDialog';
import { SourceBadge } from '../layout/RepoHeader';
import { Button } from '../ui/Button';
import { Badge } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';
import { LangLabel, TopicChips } from './meta';

/** "github.com/northwind" for origins, "Local folder" otherwise. */
export function originText(r: RepoSummary): string {
  if (r.originHost) return `${r.originHost}/${r.owner}`;
  return 'Local folder';
}

function RepoName({ r }: { r: RepoSummary }) {
  return (
    <Link to={href('code', { owner: r.owner, repo: r.name })} className="repo-row__name">
      <span className="repo-row__owner">{r.owner} / </span>
      <strong>{r.name}</strong>
    </Link>
  );
}

export function RepoRow({ r }: { r: RepoSummary }) {
  const updated = r.lastCommitAt ?? r.addedAt;
  return (
    <div className="repo-row" data-repo-id={r.id}>
      <div className="repo-row__title">
        <RepoName r={r} />
        <SourceBadge summary={r} />
        {r.status.type === 'corrupt' && (
          <Badge tone="danger" icon={OctagonAlert} title={r.status.message}>
            Damaged
          </Badge>
        )}
        {r.status.type === 'empty' && <Badge>Empty</Badge>}
      </div>
      {r.description && <p className="repo-row__desc">{r.description}</p>}
      <TopicChips topics={r.topics} />
      <div className="repo-row__meta">
        {r.primaryLanguage && <LangLabel name={r.primaryLanguage} />}
        <span className="repo-row__origin">
          {r.originHost ? <Globe size={14} aria-hidden="true" /> : <HardDrive size={14} aria-hidden="true" />}
          {originText(r)}
        </span>
        <span>
          {r.lastCommitAt === null ? 'Added ' : 'Updated '}
          <RelativeTime secs={updated} />
        </span>
      </div>
    </div>
  );
}

/** FR-4: the folder is gone. Locate… re-points it; Remove from shelf unregisters it. */
export function MissingRow({ r }: { r: RepoSummary }) {
  const [msg, setMsg] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  return (
    <div className="repo-row repo-row--missing" data-repo-id={r.id}>
      <div className="repo-row__title">
        <RepoName r={r} />
        <SourceBadge summary={r} />
        <Badge tone="attention" icon={TriangleAlert}>
          Not found
        </Badge>
      </div>
      <p className="repo-row__desc">
        The folder <span className="mono break-all">{r.path}</span> is missing. It may have been moved, renamed or be on a
        drive that is not connected.
      </p>
      {msg && (
        <p className="field__error" role="alert">
          {msg}
        </p>
      )}
      <div className="row row--wrap">
        <Button
          size="sm"
          icon={FolderSearch}
          onClick={async () => {
            setMsg(null);
            try {
              const nr = await repoLocate(r.id);
              if (nr) applySummaryAndRefresh(nr);
            } catch (e) {
              setMsg(errorText(e));
            }
          }}
        >
          Locate…
        </Button>
        <Button size="sm" variant="danger" icon={Trash2} onClick={() => setRemoving(true)}>
          Remove from shelf
        </Button>
      </div>
      {removing && <RemoveDialog summary={r} onClose={() => setRemoving(false)} />}
    </div>
  );
}
