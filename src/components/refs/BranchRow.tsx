// Branch rows (FR-29, S7): name (tree link), "Updated … by X", ahead/behind bar, commit link. Rows come in fixed
// chunks of 20; a chunk asks `branches_ahead_behind` only once it scrolls into view (IntersectionObserver; without
// one every chunk counts as visible) and the request is cancelled when the page goes away (D-79).

import { GitBranch } from 'lucide-react';
import { useEffect, useRef, useState, type RefObject } from 'react';
import { branchesAheadBehind } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { AheadBehind, BranchRow as Row } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { shortOid } from '../../util/format';
import { Badge } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';
import { AheadBehindBar } from './AheadBehindBar';

export const AB_CHUNK = 20;

export interface BranchRowProps {
  row: Row;
  owner: string;
  repo: string;
  /** Branch the counts compare with (HEAD's branch, D-58). */
  base: string;
  head: boolean;
  ab: AheadBehind | null | undefined;
  max: number;
  /** The count request failed: show a dash instead of a bar. */
  failed?: boolean;
}

export function BranchRow({ row, owner, repo, base, head, ab, max, failed }: BranchRowProps) {
  const it = row.item;
  const isBase = it.name === base;
  return (
    <li className="brow" data-branch={it.name}>
      <div className="brow__main">
        <div className="brow__title">
          <GitBranch size={16} aria-hidden="true" className="brow__icon" />
          <Link to={href('tree', { owner, repo, ref: it.name, path: '' })} className="brow__name mono truncate" title={it.fullName}>
            {it.name}
          </Link>
          {it.isDefault && <Badge tone="accent">default</Badge>}
          {head && <Badge title="The branch checked out in this repository">HEAD</Badge>}
        </div>
        <div className="brow__meta">
          Updated <RelativeTime secs={row.last.committer.time.secs} /> by <strong>{row.last.author.name}</strong>
        </div>
      </div>
      <div className="brow__ab">{isBase ? (
          <span className="muted small">Compared branch</span>
        ) : failed ? (
          <span className="muted">—</span>
        ) : (
          <AheadBehindBar ab={ab} max={max} base={base} />
        )}</div>
      <Link to={href('commit', { owner, repo, oid: row.last.oid })} className="brow__oid mono" title={row.last.summary}>
        {shortOid(row.last.oid)}
      </Link>
    </li>
  );
}

function useVisible<T extends Element>(): [RefObject<T | null>, boolean] {
  const ref = useRef<T>(null);
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined');
  useEffect(() => {
    if (visible || !ref.current || typeof IntersectionObserver === 'undefined') return;
    // already on screen at mount (the usual case for the first chunks): no need to wait for the observer
    const top = ref.current.getBoundingClientRect().top;
    if (top < window.innerHeight + 200) {
      setVisible(true);
      return;
    }
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setVisible(true);
      },
      { rootMargin: '200px 0px' },
    );
    io.observe(ref.current);
    return () => io.disconnect();
  }, [visible]);
  return [ref, visible];
}

/** One chunk of rows; reports the counts it loaded so the page can scale every bar to the same maximum. */
export function BranchChunk({
  rows,
  repoId,
  owner,
  repo,
  base,
  headBranch,
  max,
  onCounts,
}: {
  rows: Row[];
  repoId: number;
  owner: string;
  repo: string;
  base: string;
  headBranch: string | null;
  max: number;
  onCounts: (m: number) => void;
}) {
  const [ref, visible] = useVisible<HTMLUListElement>();
  const names = rows.map((r) => r.item.name).filter((n) => n !== base);
  const q = useQuery(
    visible && names.length > 0 ? qk('branches_ahead_behind', repoId, base, names) : null,
    (requestId) => branchesAheadBehind(repoId, names, requestId),
    { cancelable: true },
  );
  const map = new Map(q.data ?? []);
  const localMax = Math.max(0, ...[...map.values()].map((a) => Math.max(a.ahead, a.behind)));
  useEffect(() => {
    if (localMax > 0) onCounts(localMax);
  }, [localMax, onCounts]);
  return (
    <ul className="brows" ref={ref}>
      {rows.map((r) => (
        <BranchRow
          key={r.item.fullName}
          row={r}
          owner={owner}
          repo={repo}
          base={base}
          head={r.item.kind === 'branch' && r.item.name === headBranch}
          ab={r.aheadBehind ?? map.get(r.item.name)}
          failed={q.error !== undefined}
          max={max}
        />
      ))}
      {q.error && <li className="brow brow--error field__error">Could not count commits: {q.error.message}</li>}
    </ul>
  );
}
