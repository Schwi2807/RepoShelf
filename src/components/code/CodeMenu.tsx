// The "Code" button (S3): GitHub's clone/download menu replaced by Copy clone URL / Open folder, plus a note when the
// repository is a bare archive.

import { Archive, Code2, FolderOpen, Link2 } from 'lucide-react';
import { useState } from 'react';
import { repoOpenFolder } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import type { RepoDetail, RepoSummary } from '../../ipc/types';
import { copyText } from '../ui/CopyButton';
import { Dropdown, type MenuEntry } from '../ui/Dropdown';

export function CodeMenu({ summary, detail }: { summary: RepoSummary; detail: RepoDetail | null }) {
  const [msg, setMsg] = useState<string | null>(null);
  const items: MenuEntry[] = [];
  if (summary.originUrl) {
    items.push({
      key: 'clone-url',
      label: 'Copy clone URL',
      icon: Link2,
      onSelect: () => {
        void copyText(summary.originUrl ?? '').then((ok) => {
          setMsg(ok ? 'Clone URL copied' : 'Could not copy');
          setTimeout(() => setMsg(null), 2000);
        });
      },
    });
  }
  items.push({
    key: 'open',
    label: 'Open folder',
    icon: FolderOpen,
    disabled: summary.status.type === 'missing',
    onSelect: () => {
      setMsg(null);
      repoOpenFolder(summary.id).catch((e: unknown) => setMsg(errorText(e)));
    },
  });
  if (detail?.isBare) {
    items.push('separator', {
      key: 'bare',
      note: (
        <span className="row">
          <Archive size={14} aria-hidden="true" /> Archived as a bare repository (no working files).
        </span>
      ),
    });
  }
  items.push('separator', { key: 'path', note: <span className="mono break-all">{summary.path}</span> });
  return (
    <div className="code-menu">
      <Dropdown text="Code" icon={Code2} variant="primary" align="right" header="Local repository" items={items} />
      {msg && (
        <span className="code-menu__msg small muted" role="status">
          {msg}
        </span>
      )}
    </div>
  );
}
