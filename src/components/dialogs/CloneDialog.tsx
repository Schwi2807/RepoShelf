// Clone dialog — M5 skeleton: URL input, Clone → `clone_start` (progress shows in the jobs popover), backend
// message on rejection. M6.3 adds debounced live validation (`clone_preview`), host + target preview.

import { Download } from 'lucide-react';
import { useState } from 'react';
import { cloneStart } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { Button } from '../ui/Button';
import { Dialog } from '../ui/Dialog';

export function CloneDialog({ onClose }: { onClose: () => void }) {
  const [input, setInput] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await cloneStart(input);
      onClose();
    } catch (e) {
      setError(errorText(e));
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
          <Button variant="primary" icon={Download} disabled={busy || input.trim().length === 0} onClick={submit}>
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
            onChange={(e) => setInput(e.target.value)}
            placeholder="https://github.com/owner/repo or owner/repo"
            autoComplete="off"
            spellCheck={false}
          />
          {error ? (
            <p className="field__error">{error}</p>
          ) : (
            <p className="field__hint">Public https:// repositories only. The clone is kept as a bare archive.</p>
          )}
        </div>
      </form>
    </Dialog>
  );
}
