// Optional "Load GitHub release notes" (FR-31 opt, NFR-O1): only for github.com origins; asks first (one request to
// api.github.com), then replaces the cached `releases_list` with the answer. A build without the feature answers
// `unsupported`: the button then disappears for the session. `Release` carries no fetch date, so the date shown is
// the time of this session's load (D-80).

import { CloudDownload } from 'lucide-react';
import { useState, useSyncExternalStore } from 'react';
import { releasesFetchGithub } from '../../ipc/api';
import { errorText, toIpcError } from '../../ipc/errors';
import { prime, qk } from '../../ipc/query';
import type { RepoSummary } from '../../ipc/types';
import { formatDateTime } from '../../util/format';
import { Button } from '../ui/Button';
import { Dialog } from '../ui/Dialog';

interface GhState {
  unsupported?: boolean;
  fetchedAt?: number;
}

const state = new Map<number, GhState>();
const listeners = new Set<() => void>();
let version = 0;

function setState(repoId: number, s: GhState): void {
  state.set(repoId, { ...state.get(repoId), ...s });
  version += 1;
  for (const l of listeners) l();
}

function useGhState(repoId: number): GhState {
  useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => version,
  );
  return state.get(repoId) ?? {};
}

/** Tests only. */
export function resetGhNotesState(): void {
  state.clear();
  version += 1;
}

export function isGithub(summary: RepoSummary): boolean {
  return summary.originHost?.toLowerCase() === 'github.com';
}

export function GhNotesButton({ summary, anyGhNotes }: { summary: RepoSummary; anyGhNotes: boolean }) {
  const gh = useGhState(summary.id);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  if (!isGithub(summary) || gh.unsupported) return null;

  const load = async () => {
    setBusy(true);
    setErr(null);
    try {
      const rel = await releasesFetchGithub(summary.id);
      prime(qk('releases_list', summary.id), rel);
      setState(summary.id, { fetchedAt: Date.now() / 1000 });
      setConfirm(false);
    } catch (e) {
      if (toIpcError(e).code === 'unsupported') {
        setState(summary.id, { unsupported: true });
        setConfirm(false);
      } else {
        setErr(errorText(e));
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="ghnotes">
      {gh.fetchedAt ? (
        <span className="muted small" data-testid="gh-fetched">
          GitHub notes loaded {formatDateTime(gh.fetchedAt)}
        </span>
      ) : (
        anyGhNotes && <span className="muted small">Some notes come from GitHub (cached)</span>
      )}
      <Button size="sm" icon={CloudDownload} onClick={() => setConfirm(true)}>
        {gh.fetchedAt || anyGhNotes ? 'Reload GitHub release notes' : 'Load GitHub release notes'}
      </Button>
      {confirm && (
        <Dialog
          title="Load release notes from GitHub?"
          onClose={() => !busy && setConfirm(false)}
          footer={
            <>
              <Button onClick={() => setConfirm(false)} disabled={busy}>
                Cancel
              </Button>
              <Button variant="primary" icon={CloudDownload} onClick={() => void load()} disabled={busy}>
                {busy ? 'Loading…' : 'Load notes'}
              </Button>
            </>
          }
        >
          <p>
            RepoShelf will send <strong>one request to api.github.com</strong> for{' '}
            <span className="mono">
              {summary.owner}/{summary.name}
            </span>{' '}
            and keep the release notes it gets back. Nothing else leaves this computer.
          </p>
          {err && (
            <p className="field__error" role="alert">
              {err}
            </p>
          )}
        </Dialog>
      )}
    </div>
  );
}
