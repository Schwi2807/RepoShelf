// S2 repo header row 1: book icon, `owner / name`, source badge, Fetch / Open folder / "…". Fetch for imported
// repos (with the D-3 confirm), Fast-forward and Remove land in M6.4.

import { Archive, BookMarked, Ellipsis, FolderOpen, HardDrive, Link2, RefreshCw, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { fetchStart, repoOpenFolder } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { RepoDetail, RepoSummary } from '../../ipc/types';
import { copyLocation, href } from '../../router/router';
import { Link } from '../../router/Link';
import { Button } from '../ui/Button';
import { Badge } from '../ui/bits';
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
  const [msg, setMsg] = useState<string | null>(null);
  const canFetch = detail?.fetchable === true && summary.source === 'cloned';
  const missing = summary.status.type === 'missing';
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
      <div className="rh__spacer" />
      <div className="rh__actions">
        {msg && (
          <span className="field__error" role="alert">
            {msg}
          </span>
        )}
        {canFetch && (
          <Button
            size="sm"
            icon={RefreshCw}
            onClick={() => {
              setMsg(null);
              fetchStart(summary.id).catch((e: unknown) => setMsg(errorText(e)));
            }}
          >
            Fetch
          </Button>
        )}
        <Button
          size="sm"
          icon={FolderOpen}
          disabled={missing}
          onClick={() => {
            setMsg(null);
            repoOpenFolder(summary.id).catch((e: unknown) => setMsg(errorText(e)));
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
            { key: 'remove', label: 'Remove from shelf…', icon: Trash2, danger: true, disabled: true, onSelect: () => undefined },
          ]}
        />
      </div>
    </div>
  );
}
