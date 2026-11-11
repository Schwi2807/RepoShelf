// Missing / corrupt repo banner (FR-4, NFR-O3). "Locate…" re-points a missing repo (`repo_locate`).

import { FilePen, FolderSearch, OctagonAlert, Trash2, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { href, navigate } from '../../router/router';
import { RemoveDialog } from '../dialogs/RemoveDialog';
import { repoLocate } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { IpcError, RepoSummary } from '../../ipc/types';
import { applySummaryAndRefresh } from '../../state/repoCache';
import { Button } from '../ui/Button';

/** D-16: shown only when Settings → "Note uncommitted changes" is on and the imported work tree differs. */
export function DirtyNote() {
  return (
    <div className="notice notice--attention" data-testid="dirty-note">
      <FilePen size={16} aria-hidden="true" />
      <span>The working folder has uncommitted changes. RepoShelf shows the committed history only.</span>
    </div>
  );
}

export function RepoBanner({ summary, error }: { summary: RepoSummary; error?: IpcError }) {
  const [msg, setMsg] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  const missing = summary.status.type === 'missing' || error?.code === 'repoMissing';
  const corrupt = summary.status.type === 'corrupt' || error?.code === 'repoCorrupt';
  if (!missing && !corrupt && !error) return null;
  if (missing) {
    return (
      <div className="banner banner--attention" role="alert">
        <TriangleAlert size={16} aria-hidden="true" />
        <div className="banner__body">
          <strong>The repository folder is missing.</strong>
          <div className="mono muted">{summary.path}</div>
          {msg && <div className="field__error">{msg}</div>}
        </div>
        <Button
          size="sm"
          icon={FolderSearch}
          onClick={async () => {
            setMsg(null);
            try {
              const r = await repoLocate(summary.id);
              if (r) {
                applySummaryAndRefresh(r);
              }
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
        {removing && <RemoveDialog summary={summary} onClose={() => setRemoving(false)} onRemoved={() => navigate(href('dashboard'))} />}
      </div>
    );
  }
  const message = summary.status.type === 'corrupt' ? summary.status.message : (error?.message ?? 'Unknown error');
  return (
    <div className="banner banner--danger" role="alert">
      <OctagonAlert size={16} aria-hidden="true" />
      <div className="banner__body">
        <strong>{corrupt ? 'This repository is damaged.' : 'This repository could not be read.'}</strong>
        <div className="muted">{message}</div>
      </div>
    </div>
  );
}
