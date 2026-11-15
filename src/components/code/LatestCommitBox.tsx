// Latest-commit strip (FR-14) in the file table's box header: initials, author, summary (link), short oid, relative
// date, "N commits" (count from `commit_latest`, else the cancellable `commit_count` — same cache key as the tab
// counter in RepoLayout). In a subdirectory: the directory's last commit (`file_last_commit`) + History; online
// (D-122) a folder shows History only, and the count comes with the latest commit (no separate call).

import { History } from 'lucide-react';
import { qk, useQuery } from '../../ipc/query';
import type { CommitSummary } from '../../ipc/types';
import type { RepoSource } from '../../source';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount, shortOid } from '../../util/format';
import { Avatar, Shimmer, Spinner } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';

export interface LatestCommitProps {
  src: RepoSource;
  owner: string;
  repo: string;
  refName: string;
  commit: string;
  path: string;
}

export function CommitLine({ c, owner, repo, explore = false }: { c: CommitSummary; owner: string; repo: string; explore?: boolean }) {
  const to = href('commit', { explore, owner, repo, oid: c.oid });
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

function RootLatest({ src, owner, repo, refName, commit }: LatestCommitProps) {
  const latest = useQuery(qk('commit_latest', src.key, commit), () => src.commitLatest(commit), { stable: true });
  const countFn = src.commitCount;
  const needCount = latest.data !== undefined && latest.data.count === null && countFn !== null;
  const count = useQuery(
    needCount ? qk('commit_count', src.key, commit) : null,
    (requestId) => countFn!(commit, requestId),
    { cancelable: true, stable: true },
  );
  const n = latest.data?.count ?? count.data ?? null;
  const explore = src.explore;
  return (
    <div className="latest" data-testid="latest-commit">
      {latest.data ? (
        <CommitLine c={latest.data.commit} explore={explore} owner={owner} repo={repo} />
      ) : latest.error ? (
        <span className="field__error">{latest.error.message}</span>
      ) : (
        <>
          <Shimmer width={24} height={24} />
          <Shimmer width="40%" />
          <span className="spacer" />
        </>
      )}
      <Link to={href('commits', { explore, owner, repo, ref: refName })} className="latest__history">
        <History size={16} aria-hidden="true" />
        {n !== null ? (
          <span>
            <strong>{formatCount(n)}</strong> {n === 1 ? 'Commit' : 'Commits'}
          </span>
        ) : latest.data !== undefined && countFn === null ? (
          <span>Commits</span>
        ) : (
          <>
            <Spinner size={12} label="Counting commits" /> Commits
          </>
        )}
      </Link>
    </div>
  );
}

function DirLatest({ src, owner, repo, refName, commit, path }: LatestCommitProps) {
  const explore = src.explore;
  // online a folder's last commit would be one more request per folder: History only
  const lastFn = explore ? null : src.fileLastCommit;
  const last = useQuery(
    lastFn === null ? null : qk('file_last_commit', src.key, commit, path),
    (requestId) => lastFn!(commit, path, requestId),
    { cancelable: true, stable: true },
  );
  if (lastFn === null) {
    return (
      <div className="latest" data-testid="latest-commit">
        <span className="muted">{path.split('/').pop()}</span>
        <span className="spacer" />
        <Link to={href('commits', { explore, owner, repo, ref: refName }, { path })} className="latest__history">
          <History size={16} aria-hidden="true" />
          History
        </Link>
      </div>
    );
  }
  return (
    <div className="latest" data-testid="latest-commit">
      {last.data ? (
        <CommitLine c={last.data} explore={explore} owner={owner} repo={repo} />
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
      <Link to={href('commits', { explore, owner, repo, ref: refName }, { path })} className="latest__history">
        <History size={16} aria-hidden="true" />
        History
      </Link>
    </div>
  );
}
