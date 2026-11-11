// Remove from shelf (FR-36, S10): imported → unregister only, the folder is untouched; cloned → unregister and move
// the archive folder to the Recycle Bin (the backend trashes only inside the archive dir, D-50).

import { Trash2 } from 'lucide-react';
import { useState } from 'react';
import { repoRemove } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { RemoveResult, RepoSummary } from '../../ipc/types';
import { forgetRepo } from '../../state/repoCache';
import { Button } from '../ui/Button';
import { Dialog } from '../ui/Dialog';

export function RemoveDialog({
  summary,
  onClose,
  onRemoved,
}: {
  summary: RepoSummary;
  onClose: () => void;
  /** Called after the backend removed it (before the stores are updated). */
  onRemoved?: (r: RemoveResult) => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const cloned = summary.source === 'cloned';

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await repoRemove(summary.id);
      onRemoved?.(r);
      forgetRepo(summary);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={`Remove ${summary.owner}/${summary.name} from the shelf?`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="danger" icon={Trash2} disabled={busy} onClick={remove}>
            {cloned ? 'Remove and move to Recycle Bin' : 'Remove from shelf'}
          </Button>
        </>
      }
    >
      {cloned ? (
        <div className="stack stack--tight">
          <p>RepoShelf forgets this clone and moves its archive folder to the Recycle Bin:</p>
          <p className="mono path-box">{summary.path}</p>
          <p className="muted">You can restore it from the Recycle Bin, but RepoShelf will not list it again until you clone or import it.</p>
        </div>
      ) : (
        <div className="stack stack--tight">
          <p>RepoShelf only forgets this repository (unregister only). The folder stays untouched:</p>
          <p className="mono path-box">{summary.path}</p>
          <p className="muted">Nothing on disk is changed. You can import it again at any time.</p>
        </div>
      )}
      {error && (
        <p className="field__error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
