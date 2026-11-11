// Fetch confirm for imported repos (D-3, ARCH §0.2: the one write exception). Clones fetch without asking.

import { RefreshCw } from 'lucide-react';
import type { RepoSummary } from '../../ipc/types';
import { Button } from '../ui/Button';
import { Dialog } from '../ui/Dialog';

export function gitDirOf(summary: RepoSummary, isBare: boolean): string {
  if (isBare) return summary.path;
  const sep = summary.path.includes('\\') ? '\\' : '/';
  return summary.path.replace(/[\\/]+$/, '') + sep + '.git';
}

export function FetchConfirmDialog({
  summary,
  isBare,
  onConfirm,
  onClose,
}: {
  summary: RepoSummary;
  isBare: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog
      title="Fetch from origin?"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            icon={RefreshCw}
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            Fetch
          </Button>
        </>
      }
    >
      <div className="stack stack--tight">
        <p>
          This is an imported repository. Fetch writes <code>refs/remotes/origin/*</code>, new tags and <code>FETCH_HEAD</code> into
        </p>
        <p className="mono path-box">{gitDirOf(summary, isBare)}</p>
        <p>Branches, the index and your files are not touched.</p>
        {summary.originUrl && (
          <p className="muted small">
            From <span className="mono break-all">{summary.originUrl}</span>
          </p>
        )}
      </div>
    </Dialog>
  );
}
