// File view header (S4): the last commit that touched this file (`file_last_commit`, cancellable, oid-keyed) +
// History (commits list filtered by path).

import { History } from 'lucide-react';
import { qk, useQuery } from '../../ipc/query';
import type { RepoSource } from '../../source';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { CommitLine } from '../code/LatestCommitBox';
import { Shimmer } from '../ui/bits';

export function FileCommitHeader({
  src,
  owner,
  repo,
  refName,
  commit,
  path,
}: {
  src: RepoSource;
  owner: string;
  repo: string;
  refName: string;
  commit: string;
  path: string;
}) {
  const lastFn = src.fileLastCommit;
  const explore = src.explore;
  const last = useQuery(
    lastFn === null ? null : qk('file_last_commit', src.key, commit, path),
    (requestId) => lastFn!(commit, path, requestId),
    { cancelable: true, stable: true },
  );
  return (
    <div className="box__header filebox__header">
      <div className="latest">
        {lastFn === null ? (
          <span className="spacer" />
        ) : last.data ? (
          <CommitLine c={last.data} explore={explore} owner={owner} repo={repo} />
        ) : last.data === null ? (
          <>
            <span className="muted">Last change is older than 10,000 commits.</span>
            <span className="spacer" />
          </>
        ) : last.error ? (
          <>
            <span className="field__error">{last.error.message}</span>
            <span className="spacer" />
          </>
        ) : (
          <>
            <Shimmer width={24} height={24} />
            <Shimmer width="40%" />
            <span className="spacer" />
          </>
        )}
        <Link to={href('commits', { explore, owner, repo, ref: refName }, { path })} className="latest__history">
          <History size={16} aria-hidden="true" />
          History
        </Link>
      </div>
    </div>
  );
}
