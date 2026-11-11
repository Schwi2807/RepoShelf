// Dashboard list (S1, FR-1..4): filtered + sorted once in TS (D-27); > 500 rows virtualised against the window
// (D-31); missing repos get the warning row.

import { SearchX } from 'lucide-react';
import { useEffect } from 'react';
import type { RepoSummary } from '../../ipc/types';
import { MARK_DASHBOARD_ROWS, markOnce } from '../../util/perfMark';
import { Empty } from '../ui/bits';
import { WindowList } from '../ui/WindowList';
import { ClearFilter } from './RepoToolbar';
import { MissingRow, RepoRow } from './RepoRow';

export function RepoList({ repos, query }: { repos: RepoSummary[]; query: string }) {
  const hasRows = repos.length > 0;
  // NFR-P1 timing for the real-app e2e: first time rows are on screen in this page load.
  useEffect(() => {
    if (hasRows) markOnce(MARK_DASHBOARD_ROWS);
  }, [hasRows]);
  if (!hasRows) {
    return (
      <div className="repo-list repo-list--empty">
        <Empty icon={SearchX} title="No repositories match" actions={<ClearFilter />}>
          {query ? (
            <>
              Nothing on the shelf matches <strong>“{query}”</strong> with the current type filter.
            </>
          ) : (
            'No repositories of this type on the shelf.'
          )}
        </Empty>
      </div>
    );
  }
  return (
    <WindowList
      className="repo-list"
      count={repos.length}
      estimate={124}
      rowKey={(i) => repos[i].id}
      row={(i) => {
        const r = repos[i];
        return r.status.type === 'missing' ? <MissingRow r={r} /> : <RepoRow r={r} />;
      }}
    />
  );
}
