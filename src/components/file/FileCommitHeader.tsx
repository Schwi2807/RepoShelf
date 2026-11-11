// File view header (S4): the last commit that touched this file (`file_last_commit`, cancellable, oid-keyed) +
// History (commits list filtered by path).

import { History } from 'lucide-react';
import { fileLastCommit } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { CommitLine } from '../code/LatestCommitBox';
import { Shimmer } from '../ui/bits';

export function FileCommitHeader({
  repoId,
  owner,
  repo,
  refName,
  commit,
  path,
}: {
  repoId: number;
  owner: string;
  repo: string;
  refName: string;
  commit: string;
  path: string;
}) {
  const last = useQuery(
    qk('file_last_commit', repoId, commit, path),
    (requestId) => fileLastCommit(repoId, commit, path, requestId),
    { cancelable: true, stable: true },
  );
  return (
    <div className="box__header filebox__header">
      <div className="latest">
        {last.data ? (
          <CommitLine c={last.data} owner={owner} repo={repo} />
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
        <Link to={href('commits', { owner, repo, ref: refName }, { path })} className="latest__history">
          <History size={16} aria-hidden="true" />
          History
        </Link>
      </div>
    </div>
  );
}
