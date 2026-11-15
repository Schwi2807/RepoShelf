// Commit header card (FR-27, S6): summary, pre-wrapped description, author / committer with dates, parent links,
// full oid with copy, Browse files.

import { FileCode2, GitCommitHorizontal, GitMerge } from 'lucide-react';
import type { CommitDetail } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatDateTime, shortOid } from '../../util/format';
import { messageBody } from '../commits/CommitRow';
import { useExplore } from '../layout/repoContext';
import { Avatar } from '../ui/bits';
import { CopyButton } from '../ui/CopyButton';
import { RelativeTime } from '../ui/RelativeTime';

function tz(offsetMin: number): string {
  const s = offsetMin < 0 ? '-' : '+';
  const a = Math.abs(offsetMin);
  return `UTC${s}${String(Math.floor(a / 60)).padStart(2, '0')}:${String(a % 60).padStart(2, '0')}`;
}

export function CommitHeader({ d, owner, repo }: { d: CommitDetail; owner: string; repo: string }) {
  const c = d.commit;
  const explore = useExplore();
  const body = messageBody(d.message);
  const sameCommitter = c.author.name === c.committer.name && c.author.email === c.committer.email;
  return (
    <div className="box chead">
      <div className="chead__top">
        <h1 className="chead__summary">{c.summary}</h1>
        <Link to={href('tree', { explore, owner, repo, ref: c.oid, path: '' })} className="btn btn--sm chead__browse">
          <FileCode2 size={16} aria-hidden="true" />
          Browse files
        </Link>
      </div>
      {body && <pre className="chead__body">{body}</pre>}
      <div className="chead__meta">
        <div className="chead__people">
          <Avatar name={c.author.name} />
          <span>
            <strong title={c.author.email}>{c.author.name}</strong> authored{' '}
            <span title={`${formatDateTime(c.author.time.secs)} (${tz(c.author.time.offsetMin)} in the commit)`}>
              <RelativeTime secs={c.author.time.secs} />
            </span>
            {!sameCommitter && (
              <>
                {' · '}
                <strong title={c.committer.email}>{c.committer.name}</strong> committed <RelativeTime secs={c.committer.time.secs} />
              </>
            )}
          </span>
        </div>
        <div className="chead__ids">
          <span className="chead__parents" data-testid="commit-parents">
            {c.parents.length === 0 ? (
              <span className="muted">0 parents (root commit)</span>
            ) : (
              <>
                {c.parents.length > 1 ? <GitMerge size={14} aria-hidden="true" /> : <GitCommitHorizontal size={14} aria-hidden="true" />}
                <span className="muted">{c.parents.length === 1 ? '1 parent' : `${c.parents.length} parents`}</span>
                {c.parents.map((p, i) => (
                  <span key={p}>
                    {i > 0 && <span className="muted"> + </span>}
                    <Link to={href('commit', { explore, owner, repo, oid: p })} className="mono chead__oid" title={p}>
                      {shortOid(p)}
                    </Link>
                  </span>
                ))}
              </>
            )}
          </span>
          <span className="chead__full">
            <span className="muted">commit</span>
            <span className="mono chead__fulloid">{c.oid}</span>
            <CopyButton text={c.oid} label="Copy full commit id" />
          </span>
        </div>
      </div>
    </div>
  );
}
