// Edge states shared by the repo tabs that need a resolved ref (Commits) or any history (Branches, Tags, Releases):
// empty repository (unborn HEAD), ref not found (link to the default branch), and loading.

import { GitBranch, Inbox } from 'lucide-react';
import type { ReactNode } from 'react';
import { Link } from '../../router/Link';
import { Empty, Shimmer } from '../ui/bits';
import { Box } from './Sidebar';
import { useRepo, type RepoContextValue } from './repoContext';

export function EmptyRepo({ what }: { what: string }) {
  const r = useRepo();
  return (
    <Box>
      <Empty icon={Inbox} title="This repository is empty.">
        {what === 'commits' ? (
          <>
            Nothing has been committed to <span className="mono">{r.refName ?? r.summary.defaultBranch ?? 'the default branch'}</span> yet.
          </>
        ) : (
          <>
            There are no commits yet, so there are no {what}.
          </>
        )}
      </Empty>
    </Box>
  );
}

export function ListSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div className="box" aria-busy="true">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="box__row skel-row">
          <Shimmer width={20} height={20} />
          <Shimmer width={`${[48, 36, 55, 30, 42][i % 5]}%`} />
          <span className="spacer" />
          <Shimmer width={64} />
        </div>
      ))}
    </div>
  );
}

/**
 * For tabs that show the route ref: null when `r.resolved` is ready, otherwise the state to render instead.
 * `routeFor(defaultBranch)` builds the "go to the default branch" link for this tab.
 */
export function refGate(r: RepoContextValue, what: string, routeFor: (ref: string) => string): ReactNode | null {
  if (r.detail?.head.type === 'unborn') return <EmptyRepo what={what} />;
  if (r.revError) {
    const def = r.summary.defaultBranch;
    const nf = r.revError.code === 'notFound';
    return (
      <Box>
        <Empty
          icon={GitBranch}
          title={nf ? `Ref “${r.refName ?? ''}” not found` : 'This ref cannot be shown'}
          actions={def ? <Link to={routeFor(def)}>Go to the default branch ({def})</Link> : undefined}
        >
          {nf ? 'No branch, tag or commit by that name exists in this repository.' : r.revError.message}
        </Empty>
      </Box>
    );
  }
  if (!r.resolved || !r.refName) return <ListSkeleton />;
  return null;
}
