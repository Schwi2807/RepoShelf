// Latest-commit strip (FR-14) in the file table's box header: initials, author, summary (link), short oid, relative
// date, "N commits" (count from `commit_latest`, else the cancellable `commit_count` — same cache key as the tab
// counter in RepoLayout). In a subdirectory: the directory's last commit (`file_last_commit`) + History.

import { History } from 'lucide-react';
import { commitCount, commitLatest, fileLastCommit } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { CommitSummary } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount, shortOid } from '../../util/format';
import { Avatar, Shimmer, Spinner } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';

export interface LatestCommitProps {
  repoId: number;
  owner: string;
  repo: string;
  refName: string;
  commit: string;
  path: string;
}

export function CommitLine({ c, owner, repo }: { c: CommitSummary; owner: string; repo: string }) {
  const to = href('commit', { owner, repo, oid: c.oid });
  return (
    <>
      <Avatar name={c.author.name} />
      <strong className="latest__author truncate" title={c.author.email}>
        {c.author.name}
      </strong>
      <Link to={to} className="latest__summary truncate" title={c.summary}>
        {c.summary}
      </Link>
      <span className="spacer" />
      <Link to={to} className="latest__oid mono" title={c.oid}>
        {shortOid(c.oid)}
      </Link>
      <span className="latest__dot" aria-hidden="true">
        ·
      </span>
      <span className="latest__date">
        <RelativeTime secs={c.committer.time.secs} />
      </span>
    </>
  );
}

export function LatestCommitBox(p: LatestCommitProps) {
  return p.path === '' ? <RootLatest {...p} /> : <DirLatest {...p} />;
}

function RootLatest({ repoId, owner, repo, refName, commit }: LatestCommitProps) {
  const latest = useQuery(qk('commit_latest', repoId, commit), () => commitLatest(repoId, commit), { stable: true });
  const needCount = latest.data !== undefined && latest.data.count === null;
  const count = useQuery(
    needCount ? qk('commit_count', repoId, commit) : null,
    (requestId) => commitCount(repoId, commit, requestId),
    { cancelable: true, stable: true },
  );
  const n = latest.data?.count ?? count.data ?? null;
  return (
    <div className="latest" data-testid="latest-commit">
      {latest.data ? (
        <CommitLine c={latest.data.commit} owner={owner} repo={repo} />
      ) : latest.error ? (
        <span className="field__error">{latest.error.message}</span>
      ) : (
        <>
          <Shimmer width={24} height={24} />
          <Shimmer width="40%" />
          <span className="spacer" />
        </>
      )}
      <Link to={href('commits', { owner, repo, ref: refName })} className="latest__history">
        <History size={16} aria-hidden="true" />
        {n !== null ? (
          <span>
            <strong>{formatCount(n)}</strong> {n === 1 ? 'Commit' : 'Commits'}
          </span>
        ) : (
          <>
            <Spinner size={12} label="Counting commits" /> Commits
          </>
        )}
      </Link>
    </div>
  );
}

function DirLatest({ repoId, owner, repo, refName, commit, path }: LatestCommitProps) {
  const last = useQuery(
    qk('file_last_commit', repoId, commit, path),
    (requestId) => fileLastCommit(repoId, commit, path, requestId),
    { cancelable: true, stable: true },
  );
  return (
    <div className="latest" data-testid="latest-commit">
      {last.data ? (
        <CommitLine c={last.data} owner={owner} repo={repo} />
      ) : last.data === null ? (
        <span className="muted">Last change is older than 10,000 commits.</span>
      ) : last.error ? (
        <span className="field__error">{last.error.message}</span>
      ) : (
        <>
          <Shimmer width={24} height={24} />
          <Shimmer width="40%" />
          <span className="spacer" />
        </>
      )}
      {last.data === null && <span className="spacer" />}
      <Link to={href('commits', { owner, repo, ref: refName }, { path })} className="latest__history">
        <History size={16} aria-hidden="true" />
        History
      </Link>
    </div>
  );
}
