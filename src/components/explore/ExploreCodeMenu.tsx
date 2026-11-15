// The "Code" button of an Explore repository: Copy clone URL / Open on GitHub, and a note that the files are read
// from GitHub (Save to shelf in the header keeps a local copy). Twin of the shelf's CodeMenu.

import { Code2, ExternalLink, Link2 } from 'lucide-react';
import { useState } from 'react';
import type { ExploreRepo } from '../../ipc/types';
import { confirmExternal } from '../ui/ConfirmExternalLink';
import { copyText } from '../ui/CopyButton';
import { Dropdown, type MenuEntry } from '../ui/Dropdown';

export function ExploreCodeMenu({ repo }: { repo: ExploreRepo }) {
  const [msg, setMsg] = useState<string | null>(null);
  const items: MenuEntry[] = [
    {
      key: 'clone-url',
      label: 'Copy clone URL',
      icon: Link2,
      onSelect: () => {
        void copyText(repo.cloneUrl).then((ok) => {
          setMsg(ok ? 'Clone URL copied' : 'Could not copy');
          setTimeout(() => setMsg(null), 2000);
        });
      },
    },
    { key: 'gh', label: 'Open on GitHub', icon: ExternalLink, onSelect: () => confirmExternal(repo.htmlUrl) },
    'separator',
    { key: 'note', note: <span className="small">Read from GitHub. Use Save to shelf for a copy on this PC.</span> },
  ];
  return (
    <div className="code-menu">
      <Dropdown text="Code" icon={Code2} variant="primary" align="right" header="GitHub repository" items={items} />
      {msg && (
        <span className="code-menu__msg small muted" role="status">
          {msg}
        </span>
      )}
    </div>
  );
}
