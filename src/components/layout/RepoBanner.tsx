// Missing / corrupt repo banner (FR-4, NFR-O3). "Locate…" re-points a missing repo (`repo_locate`).

import { FolderSearch, OctagonAlert, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { repoLocate } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { invalidate } from '../../ipc/query';
import type { IpcError, RepoSummary } from '../../ipc/types';
import { upsertRepo } from '../../state/library';
import { Button } from '../ui/Button';

export function RepoBanner({ summary, error }: { summary: RepoSummary; error?: IpcError }) {
  const [msg, setMsg] = useState<string | null>(null);
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
                upsertRepo(r);
                invalidate();
              }
            } catch (e) {
              setMsg(errorText(e));
            }
          }}
        >
          Locate…
        </Button>
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
