// Import dialog (FR-5, S10): "Choose folder…" → `import_pick` (native dialog in Rust) → summary (name, path, bare,
// origin, duplicate) or the backend's refusal (not a repository…) → Import → `import_confirm(token)` (D-18) → open
// the imported repo.

import { FolderGit2, FolderOpen, FolderPlus, Info, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { importConfirm, importPick } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { ImportPreview } from '../../ipc/types';
import { href, navigate } from '../../router/router';
import { Link } from '../../router/Link';
import { useLibrary } from '../../state/library';
import { applySummary } from '../../state/repoCache';
import { Button } from '../ui/Button';
import { Badge } from '../ui/bits';
import { Dialog } from '../ui/Dialog';

export function ImportDialog({ onClose }: { onClose: () => void }) {
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const { repos } = useLibrary();
  const dup = preview?.duplicateOf != null ? (repos ?? []).find((r) => r.id === preview.duplicateOf) : undefined;

  const pick = async () => {
    setError(null);
    setBusy(true);
    try {
      const p = await importPick();
      if (p) setPreview(p);
    } catch (e) {
      setPreview(null);
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const confirm = async () => {
    if (!preview) return;
    setBusy(true);
    setError(null);
    try {
      const r = await importConfirm(preview.token);
      applySummary(r);
      onClose();
      navigate(href('code', { owner: r.owner, repo: r.name }));
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
      <p className="dialog__lead">
        Pick a folder that contains a git repository (a work tree or a bare repository). RepoShelf only reads it: the
        folder stays where it is and nothing inside is changed.
      </p>
      <div className="row mt-3">
        <Button icon={FolderOpen} onClick={pick} disabled={busy}>
          {preview ? 'Choose another folder…' : 'Choose folder…'}
        </Button>
      </div>
      {preview && (
        <div className="import-summary mt-3" data-testid="import-summary">
          <div className="import-summary__head">
            <FolderGit2 size={20} aria-hidden="true" />
            <strong>{preview.name}</strong>
            {preview.isBare ? <Badge>bare repository</Badge> : <Badge>work tree</Badge>}
          </div>
          <dl className="kv-grid">
            <dt>Folder</dt>
            <dd className="mono break-all">{preview.path}</dd>
            <dt>Origin</dt>
            <dd className="mono break-all">{preview.originUrl ?? <span className="muted">none (shown as local/{preview.name})</span>}</dd>
          </dl>
          {preview.duplicateOf !== null ? (
            <p className="notice notice--attention" role="alert">
              <TriangleAlert size={16} aria-hidden="true" />
              <span>
                This folder is already on the shelf
                {dup && (
                  <>
                    {' '}
                    as{' '}
                    <Link to={href('code', { owner: dup.owner, repo: dup.name })} onClick={onClose}>
                      {dup.owner}/{dup.name}
                    </Link>
                  </>
                )}
                .
              </span>
            </p>
          ) : (
            <p className="notice">
              <Info size={16} aria-hidden="true" />
              <span>Read-only: browsing never writes to this folder. Fetch (optional, asks first) is the only exception.</span>
            </p>
          )}
        </div>
      )}
      {error && (
        <p className="notice notice--danger mt-3" role="alert">
          <TriangleAlert size={16} aria-hidden="true" />
          <span>{error}</span>
        </p>
      )}
    </Dialog>
  );
}
