// "Showing N changed files with A additions and D deletions" (S6) + the merge / root / large-commit notes and a
// collapsible file list whose entries jump to their card (`?file=N`).

import { FileDiff, GitMerge, Hourglass, ListTree, Sprout } from 'lucide-react';
import { useState } from 'react';
import type { CommitDetail } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount, plural, shortOid } from '../../util/format';
import { Badge } from '../ui/bits';
import { Button } from '../ui/Button';
import { useExplore } from '../layout/repoContext';
import { KIND_LABEL, pathLabel } from './diffs';
import { MiniBar } from './MiniBar';

export function summaryText(d: Pick<CommitDetail, 'files' | 'additions' | 'deletions' | 'diffsDeferred'>): string {
  const files = plural(d.files.length, 'changed file');
  if (d.diffsDeferred) return `Showing ${files}`;
  return `Showing ${files} with ${plural(d.additions, 'addition')} and ${plural(d.deletions, 'deletion')}`;
}

export function DiffSummary({
  d,
  owner,
  repo,
  onPick,
}: {
  d: CommitDetail;
  owner: string;
  repo: string;
  /** Jump to the card of file `index`. */
  onPick: (index: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const explore = useExplore();
  const parent = d.parentCompared;
  return (
    <div className="stack stack--tight">
      <div className="dsum">
        <FileDiff size={16} aria-hidden="true" className="dsum__icon" />
        <span className="dsum__text" data-testid="diff-summary">
          {summaryText(d)}
          {!d.diffsDeferred && d.files.length > 0 && <MiniBar additions={d.additions} deletions={d.deletions} numbers={false} />}
        </span>
        <span className="spacer" />
        {d.files.length > 0 && (
          <Button size="sm" icon={ListTree} aria-expanded={open} onClick={() => setOpen((o) => !o)}>
            {open ? 'Hide file list' : 'File list'}
          </Button>
        )}
      </div>
      {d.isMerge && parent && (
        <div className="notice" data-testid="merge-note">
          <GitMerge size={16} aria-hidden="true" />
          <span>
            This is a merge commit. Showing changes from the first parent,{' '}
            <Link to={href('commit', { explore, owner, repo, oid: parent })} className="mono">
              {shortOid(parent)}
            </Link>
            .
          </span>
        </div>
      )}
      {d.commit.parents.length === 0 && (
        <div className="notice" data-testid="root-note">
          <Sprout size={16} aria-hidden="true" />
          <span>This is the first commit of the history: every file is compared with an empty tree.</span>
        </div>
      )}
      {d.diffsDeferred && (
        <div className="notice notice--attention" data-testid="deferred-note">
          <Hourglass size={16} aria-hidden="true" />
          <span>
            {formatCount(d.files.length)} files changed — more than RepoShelf shows at once. Line counts are skipped and each diff
            loads when you ask for it.
          </span>
        </div>
      )}
      {open && (
        <ol className="box dfiles" aria-label="Changed files">
          {d.files.map((f) => {
            const l = pathLabel(f);
            const kind = KIND_LABEL[f.kind];
            return (
              <li key={f.index} className="dfiles__row">
                <button type="button" className="link-button dfiles__path mono" onClick={() => onPick(f.index)} title={l.to}>
                  {l.from ? `${l.from} → ${l.to}` : l.to}
                </button>
                {kind && <Badge>{kind}</Badge>}
                <span className="spacer" />
                {!d.diffsDeferred && !f.binary && <MiniBar additions={f.additions} deletions={f.deletions} />}
                {f.binary && <span className="muted small">binary</span>}
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
}
