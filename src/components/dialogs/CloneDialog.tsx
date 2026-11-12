// Clone dialog (FR-6, S10): URL input with debounced live validation (`clone_preview`, the backend's own rules and
// wording), the host shown for confirmation and the target folder preview; Clone → `clone_start`. Progress lives in
// the jobs popover (FR-7).

import { Download, Globe, TriangleAlert } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { clonePreview, cloneStart } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { ClonePreview } from '../../ipc/types';
import { Button } from '../ui/Button';
import { Spinner } from '../ui/bits';
import { Dialog } from '../ui/Dialog';
import { AutoDownloadCheck, isGithubUrl, useAutoDownload } from './AutoDownloadCheck';

export const PREVIEW_DEBOUNCE_MS = 250;

type Check =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'ok'; preview: ClonePreview }
  | { state: 'error'; message: string };

export function CloneDialog({ onClose }: { onClose: () => void }) {
  const [input, setInput] = useState('');
  const [check, setCheck] = useState<Check>({ state: 'idle' });
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const seq = useRef(0);
  const autoDl = useAutoDownload();

  useEffect(() => {
    const value = input.trim();
    seq.current += 1;
    const mine = seq.current;
    if (value.length === 0) {
      setCheck({ state: 'idle' });
      return;
    }
    setCheck({ state: 'checking' });
    const t = setTimeout(() => {
      clonePreview(value).then(
        (preview) => {
          if (seq.current === mine) setCheck({ state: 'ok', preview });
        },
        (e: unknown) => {
          if (seq.current === mine) setCheck({ state: 'error', message: errorText(e) });
        },
      );
    }, PREVIEW_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [input]);

  const submit = async () => {
    if (check.state !== 'ok' || busy) return;
    setBusy(true);
    setSubmitError(null);
    try {
      await cloneStart(input.trim(), isGithubUrl(check.preview.url) ? autoDl.value : null);
      onClose();
    } catch (e) {
      setSubmitError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Dialog
      title="Clone a repository"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" icon={Download} disabled={busy || check.state !== 'ok'} onClick={submit}>
            Clone
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="field">
          <label className="label" htmlFor="clone-url">
            Repository URL
          </label>
          <input
            id="clone-url"
            className="input"
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              setSubmitError(null);
            }}
            placeholder="https://github.com/owner/repo or owner/repo"
            autoComplete="off"
            spellCheck={false}
            aria-invalid={check.state === 'error'}
            aria-describedby="clone-url-status"
          />
          <div id="clone-url-status" className="clone-status" aria-live="polite">
            {check.state === 'idle' && <p className="field__hint">Public https:// repositories only. The clone is kept as a bare archive.</p>}
            {check.state === 'checking' && (
              <p className="field__hint row">
                <Spinner size={12} label="Checking" /> Checking…
              </p>
            )}
            {check.state === 'error' && (
              <p className="field__error row clone-status__error" role="alert">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{check.message}</span>
              </p>
            )}
          </div>
        </div>
      </form>
      {check.state === 'ok' && (
        <div className="clone-preview" data-testid="clone-preview">
          <div className="clone-preview__row">
            <Globe size={16} aria-hidden="true" />
            <span>
              Host: <strong className="clone-preview__host">{check.preview.host}</strong>
            </span>
          </div>
          <div className="clone-preview__row">
            <span className="muted">Repository</span>
            <span className="mono">
              {check.preview.owner}/{check.preview.name}
            </span>
          </div>
          <div className="clone-preview__row">
            <span className="muted">Clone URL</span>
            <span className="mono break-all">{check.preview.url}</span>
          </div>
          <div className="clone-preview__row">
            <span className="muted">Saved to</span>
            <span className="mono break-all clone-preview__target">{check.preview.targetDir}</span>
          </div>
          <p className="field__hint">Make sure the host is the one you expect. RepoShelf connects to it once to download the repository.</p>
        </div>
      )}
      {check.state === 'ok' && isGithubUrl(check.preview.url) && <AutoDownloadCheck checked={autoDl.checked} onChange={autoDl.set} />}
      {submitError && (
        <p className="field__error" role="alert">
          {submitError}
        </p>
      )}
    </Dialog>
  );
}
