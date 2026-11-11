// Import dialog — M5 skeleton: "Choose folder…" → `import_pick` (native dialog in Rust) → summary → Import →
// `import_confirm(token)`. M6.3 completes the summary (duplicate / non-repo cases, wording).

import { FolderOpen, FolderPlus } from 'lucide-react';
import { useState } from 'react';
import { importConfirm, importPick } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { ImportPreview } from '../../ipc/types';
import { upsertRepo } from '../../state/library';
import { Button } from '../ui/Button';
import { Dialog } from '../ui/Dialog';

export function ImportDialog({ onClose }: { onClose: () => void }) {
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pick = async () => {
    setError(null);
    setBusy(true);
    try {
      const p = await importPick();
      if (p) setPreview(p);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const confirm = async () => {
    if (!preview) return;
    setBusy(true);
    try {
      upsertRepo(await importConfirm(preview.token));
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Dialog
      title="Import a local repository"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" icon={FolderPlus} disabled={busy || !preview || preview.duplicateOf !== null} onClick={confirm}>
            Import
          </Button>
        </>
      }
    >
      <p className="muted">RepoShelf only reads imported repositories; the folder stays where it is.</p>
      <div className="row mt-3">
        <Button icon={FolderOpen} onClick={pick} disabled={busy}>
          Choose folder…
        </Button>
      </div>
      {preview && (
        <div className="box mt-3">
          <div className="box__body">
            <div>
              <strong>{preview.name}</strong> {preview.isBare && <span className="badge">bare</span>}
            </div>
            <div className="mono muted">{preview.path}</div>
            {preview.originUrl && <div className="muted">origin: {preview.originUrl}</div>}
            {preview.duplicateOf !== null && <p className="field__error">This repository is already on the shelf.</p>}
          </div>
        </div>
      )}
      {error && <p className="field__error">{error}</p>}
    </Dialog>
  );
}
