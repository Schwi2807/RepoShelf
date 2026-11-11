// File toolbar (S4, FR-22/23): view switch (Preview for markdown · Code · Raw `?plain=1`), "N lines (M sloc) · size",
// Copy link to the selected lines (`#L10-L20` form), Copy raw text, Save a copy (`blob_save_as`, native dialog).

import { Check, Download, Link2 } from 'lucide-react';
import { useState } from 'react';
import { blobSaveAs } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { BlobMeta } from '../../ipc/types';
import { copyLocation } from '../../router/router';
import { Link } from '../../router/Link';
import { formatBytes, formatCount } from '../../util/format';
import { Button } from '../ui/Button';
import { CopyButton } from '../ui/CopyButton';

export type FileMode = 'preview' | 'code' | 'raw';

export interface FileToolbarProps {
  repoId: number;
  commit: string;
  meta: BlobMeta;
  mode: FileMode;
  /** Modes offered, with their hrefs. */
  modes: { mode: FileMode; label: string; to: string }[];
  /** Raw text for Copy (null when not loaded / not text). */
  text: string | null;
  hasSelection: boolean;
}

export function FileToolbar({ repoId, commit, meta, mode, modes, text, hasSelection }: FileToolbarProps) {
  const [saved, setSaved] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [linkCopied, setLinkCopied] = useState(false);

  const stats: string[] = [];
  if (meta.lines !== null) {
    stats.push(`${formatCount(meta.lines)} ${meta.lines === 1 ? 'line' : 'lines'}${meta.sloc !== null ? ` (${formatCount(meta.sloc)} sloc)` : ''}`);
  }
  stats.push(formatBytes(meta.size));

  return (
    <div className="filebar">
      {modes.length > 1 && (
        <div className="segmented" role="tablist" aria-label="View">
          {modes.map((m) => (
            <Link key={m.mode} to={m.to} replace role="tab" aria-selected={m.mode === mode} className="segmented__item">
              {m.label}
            </Link>
          ))}
        </div>
      )}
      <span className="filebar__stats">{stats.join(' · ')}</span>
      <span className="spacer" />
      {saved && (
        <span className="small muted truncate filebar__note" role="status" title={saved}>
          Saved to {saved}
        </span>
      )}
      {error && (
        <span className="field__error filebar__note" role="alert">
          {error}
        </span>
      )}
      {hasSelection && (
        <Button
          size="sm"
          icon={linkCopied ? Check : Link2}
          onClick={async () => {
            try {
              await copyLocation();
              setLinkCopied(true);
              setTimeout(() => setLinkCopied(false), 1500);
            } catch {
              setError('Could not copy the link.');
            }
          }}
        >
          {linkCopied ? 'Copied' : 'Copy link'}
        </Button>
      )}
      <div className="btn-group">
        {text !== null && <CopyButton text={text} label="Copy raw file" variant="default" />}
        <Button
          size="sm"
          icon={Download}
          label="Save a copy…"
          onClick={async () => {
            setError(null);
            setSaved(null);
            try {
              const p = await blobSaveAs(repoId, commit, meta.path);
              if (p) setSaved(p);
            } catch (e) {
              setError(errorText(e));
            }
          }}
        />
      </div>
    </div>
  );
}
