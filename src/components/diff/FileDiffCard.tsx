// One changed file on the commit page (FR-27, FR-28, S6): header (chevron, +/− mini bar, change badge, path or
// `old → new`, copy path, View file) and the diff table — or a collapsed body that says why (binary, > 1 MB,
// generated, large / deferred) with "Load diff" where loading is possible (`commit_file_diff(force)`).
// Expanded/loaded state lives in the page (cards unmount while virtualised).

import { ChevronDown, ChevronRight, FileCode2, FileX, FileWarning, Sparkles, Hourglass } from 'lucide-react';
import { qk, useQuery } from '../../ipc/query';
import type { FileChange, FileDiff } from '../../ipc/types';
import type { RepoSource } from '../../source';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount } from '../../util/format';
import { Badge, Shimmer } from '../ui/bits';
import { Button } from '../ui/Button';
import { CopyButton } from '../ui/CopyButton';
import { canLoad, collapseReason, filePath, KIND_LABEL, pathLabel, type Collapse } from './diffs';
import { DiffTable } from './DiffTable';
import { MiniBar } from './MiniBar';

export interface FileDiffCardProps {
  f: FileChange;
  inline: FileDiff | undefined;
  deferred: boolean;
  src: RepoSource;
  owner: string;
  repo: string;
  oid: string;
  parent: string | null;
  expanded: boolean;
  onToggle: () => void;
  loadRequested: boolean;
  onLoad: () => void;
}

function collapsedText(c: Exclude<Collapse, null>, f: FileChange): { icon: typeof FileX; text: string } {
  switch (c) {
    case 'binary':
      return { icon: FileX, text: 'Binary file not shown.' };
    case 'large':
      return { icon: FileWarning, text: 'Large diff not rendered: the file is larger than 1 MB.' };
    case 'generated':
      return { icon: Sparkles, text: 'Generated file (lockfile, minified or build output). The diff is hidden.' };
    case 'deferred':
      return { icon: Hourglass, text: 'This commit is large, so its diffs load on demand.' };
    case 'lines':
      return {
        icon: FileWarning,
        text:
          f.additions + f.deletions > 0
            ? `Large diff: ${formatCount(f.additions + f.deletions)} lines changed.`
            : 'This diff was not loaded with the commit.',
      };
  }
}

function LoadedDiff({ src, oid, index, path }: { src: RepoSource; oid: string; index: number; path: string }) {
  const d = useQuery(qk('commit_file_diff', src.key, oid, index, true), () => src.commitFileDiff(oid, index, true), { stable: true });
  if (d.error) return <p className="dcard__note field__error">{d.error.message}</p>;
  if (!d.data) {
    return (
      <div className="dcard__note" aria-busy="true">
        <Shimmer width="70%" block />
      </div>
    );
  }
  return <DiffTable diff={d.data} path={path} />;
}

export function FileDiffCard(p: FileDiffCardProps) {
  const { f } = p;
  const path = filePath(f);
  const label = pathLabel(f);
  const reason = collapseReason(f, p.inline !== undefined, p.deferred);
  const kind = KIND_LABEL[f.kind];
  const viewAt = f.kind === 'deleted' ? p.parent : p.oid;
  const viewPath = f.kind === 'deleted' ? (f.oldPath ?? path) : path;
  const Chevron = p.expanded ? ChevronDown : ChevronRight;

  let body = null;
  if (p.expanded) {
    if (reason === null || (reason === 'generated' && p.loadRequested && p.inline)) {
      body = p.inline ? <DiffTable diff={p.inline} path={path} /> : null;
    } else if (p.loadRequested && canLoad(reason)) {
      body = <LoadedDiff src={p.src} oid={p.oid} index={f.index} path={path} />;
    } else {
      const t = collapsedText(reason, f);
      body = (
        <div className="dcard__collapsed" data-collapsed={reason}>
          <t.icon size={16} aria-hidden="true" />
          <span>{t.text}</span>
          {canLoad(reason) && (
            <Button size="sm" onClick={p.onLoad}>
              Load diff
            </Button>
          )}
        </div>
      );
    }
  }

  return (
    <div className={`box dcard${p.expanded ? '' : ' dcard--closed'}`} id={`diff-${f.index}`} data-index={f.index}>
      <div className="dcard__header">
        <button
          type="button"
          className="dcard__toggle"
          aria-expanded={p.expanded}
          aria-label={p.expanded ? `Collapse ${path}` : `Expand ${path}`}
          onClick={p.onToggle}
        >
          <Chevron size={16} aria-hidden="true" />
        </button>
        {!p.deferred && !f.binary && <MiniBar additions={f.additions} deletions={f.deletions} />}
        {f.binary && <Badge>binary</Badge>}
        <span className="dcard__path mono" title={label.from ? `${label.from} → ${label.to}` : label.to}>
          {label.from && (
            <>
              <span className="dcard__old">{label.from}</span>
              <span className="dcard__arrow" aria-label="renamed to">
                {' → '}
              </span>
            </>
          )}
          <span>{label.to}</span>
        </span>
        <CopyButton text={path} label="Copy file path" />
        {kind && <Badge tone={f.kind === 'added' ? 'success' : f.kind === 'deleted' ? 'danger' : 'indigo'}>{kind}</Badge>}
        <span className="spacer" />
        {viewAt && (
          <Link to={href('blob', { explore: p.src.explore, owner: p.owner, repo: p.repo, ref: viewAt, path: viewPath })} className="btn btn--sm btn--invisible dcard__view">
            <FileCode2 size={16} aria-hidden="true" />
            View file
          </Link>
        )}
      </div>
      {body}
    </div>
  );
}
