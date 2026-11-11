// S2 frame for every repo route: resolves slug → repoId (`repo_by_slug`), loads `repo_detail`, resolves the ref
// (`rev_resolve`) and provides all of it via RepoContext; renders RepoHeader + TabBar + RepoBanner around the tab.

import { SearchX } from 'lucide-react';
import { useMemo, type ReactNode } from 'react';
import { commitCount, repoBySlug, repoDetail, revResolve } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { Route } from '../../router/routes';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { Empty, Shimmer } from '../ui/bits';
import { RepoBanner } from './RepoBanner';
import { RepoHeader } from './RepoHeader';
import { RepoContext, defaultRef, type RepoContextValue } from './repoContext';
import { TabBar } from './TabBar';

export function RepoLayout({ route, children }: { route: Route; children: ReactNode }) {
  const owner = route.params.owner ?? '';
  const name = route.params.repo ?? '';
  const slug = useQuery(qk('repo_by_slug', owner.toLowerCase(), name.toLowerCase()), () => repoBySlug(owner, name));
  const summary = slug.data;
  const repoId = summary?.id ?? null;
  const detail = useQuery(repoId === null ? null : qk('repo_detail', repoId), () => repoDetail(repoId!));

  const routeRef = route.params.ref ?? null;
  const refName = routeRef ?? (summary ? defaultRef(summary, detail.data ?? null) : null);
  const canResolve = repoId !== null && refName !== null && (routeRef !== null || detail.data !== undefined);
  const unborn = detail.data?.head.type === 'unborn';
  const rev = useQuery(
    canResolve && !unborn ? qk('rev_resolve', repoId, refName) : null,
    () => revResolve(repoId!, refName!),
  );
  const resolved = rev.data ?? null;
  const count = useQuery(
    repoId !== null && resolved ? qk('commit_count', repoId, resolved.commit) : null,
    (requestId) => commitCount(repoId!, resolved!.commit, requestId),
    { cancelable: true, stable: true },
  );

  const ctx: RepoContextValue | null = useMemo(
    () =>
      summary
        ? {
            summary,
            repoId: summary.id,
            owner: summary.owner,
            repo: summary.name,
            detail: detail.data ?? null,
            detailError: detail.error,
            refName,
            refFromRoute: routeRef !== null,
            resolved,
            revError: rev.error,
            revLoading: rev.loading,
          }
        : null,
    [summary, detail.data, detail.error, refName, routeRef, resolved, rev.error, rev.loading],
  );

  if (slug.error) {
    return (
      <main className="container page">
        <Empty icon={SearchX} title="Repository not found" actions={<Link to={href('dashboard')}>Back to all repositories</Link>}>
          <span className="mono">
            {owner}/{name}
          </span>{' '}
          is not on the shelf.
        </Empty>
      </main>
    );
  }
  if (!summary || !ctx) {
    return (
      <div className="rh">
        <div className="container">
          <div className="rh__row">
            <Shimmer width={260} height={20} />
          </div>
        </div>
      </div>
    );
  }

  const d = detail.data;
  return (
    <RepoContext.Provider value={ctx}>
      <div className="rh">
        <div className="container">
          <RepoHeader summary={summary} detail={d ?? null} />
          <TabBar
            owner={summary.owner}
            repo={summary.name}
            refName={refName}
            refFromRoute={routeRef !== null}
            current={route.name}
            counts={{
              commits: count.data ?? null,
              branches: d ? d.branchCount : null,
              tags: d ? d.tagCount : null,
              releases: d ? d.releaseCount : null,
            }}
          />
        </div>
      </div>
      <main className="container page">
        <div className="stack">
          <RepoBanner summary={summary} error={detail.error} />
          {!detail.error && children}
        </div>
      </main>
    </RepoContext.Provider>
  );
}
