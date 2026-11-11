// NFR-S3: every external link goes through this confirmation (full URL shown) and then `open_external` (Rust
// re-validates; http/https only). `mailto:` is shown as text to copy — the backend never opens it (D-58).

import { ExternalLink, Mail } from 'lucide-react';
import { useState } from 'react';
import { openExternal } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { createStore, useStore } from '../../state/store';
import { Button } from './Button';
import { CopyButton } from './CopyButton';
import { Dialog } from './Dialog';

const pendingUrl = createStore<string | null>(null);

/** Classify a link target. Only these two kinds may ever leave the app. */
export function externalKind(url: string): 'web' | 'mail' | null {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return null;
  }
  if ((u.protocol === 'http:' || u.protocol === 'https:') && u.hostname.length > 0 && !u.username && !u.password) return 'web';
  if (u.protocol === 'mailto:') return 'mail';
  return null;
}

/** Ask before opening `url` outside the app. Ignored for anything that is not http(s)/mailto. */
export function confirmExternal(url: string): void {
  if (externalKind(url) === null) return;
  pendingUrl.set(url);
}

/** Mounted once in App. */
export function ExternalLinkHost() {
  const url = useStore(pendingUrl);
  if (url === null) return null;
  return <ConfirmExternalLink url={url} onClose={() => pendingUrl.set(null)} />;
}

export function ConfirmExternalLink({ url, onClose }: { url: string; onClose: () => void }) {
  const kind = externalKind(url);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const address = kind === 'mail' ? decodeURIComponent(url.replace(/^mailto:/i, '').split('?')[0]) : url;
  return (
    <Dialog
      title={kind === 'mail' ? 'Email address' : 'Open external link?'}
      onClose={onClose}
      footer={
        kind === 'mail' ? (
          <>
            <CopyButton text={address} variant="default" size="md">
              Copy address
            </CopyButton>
            <Button onClick={onClose}>Close</Button>
          </>
        ) : (
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button
              variant="primary"
              icon={ExternalLink}
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  await openExternal(url);
                  onClose();
                } catch (e) {
                  setError(errorText(e));
                  setBusy(false);
                }
              }}
            >
              Open in browser
            </Button>
          </>
        )
      }
    >
      {kind === 'mail' ? (
        <p className="row">
          <Mail size={16} aria-hidden="true" />
          <span className="mono">{address}</span>
        </p>
      ) : (
        <>
          <p className="muted">This link leaves RepoShelf and opens in your default browser:</p>
          <p className="mono mt-2 break-all">
            {url}
          </p>
        </>
      )}
      {error && <p className="field__error">{error}</p>}
    </Dialog>
  );
}
