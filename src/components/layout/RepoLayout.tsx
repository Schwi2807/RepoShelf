// S2 frame for every shelf repo route: resolves slug → repoId (`repo_by_slug`), loads `repo_detail`, resolves the ref
// (`rev_resolve`) and provides all of it via RepoContext; renders RepoHeader + TabBar + RepoBanner around the tab.
// The Explore twin (`explore/ExploreLayout`) provides the same context from GitHub.

import { SearchX } from 'lucide-react';
import { useEffect, useMemo, type ReactNode } from 'react';
import { commitCount, repoBySlug, repoDetail } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import { localSource } from '../../source';
import { noteSummary } from '../../state/library';
import type { Route } from '../../router/routes';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { Empty, Shimmer } from '../ui/bits';
import { DirtyNote, RepoBanner } from './RepoBanner';
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
  // `repo_detail` stamps `lastOpenedAt` in the DB; hand the fresher summary to the library store so the dashboard's
  // Recent list follows (D-87). The mock mutated its shared summary object, which hid this in M6.
  const detailSummary = detail.data?.summary;
  useEffect(() => {
    if (detailSummary) noteSummary(detailSummary);
  }, [detailSummary]);

  const src = useMemo(() => localSource(repoId ?? 0), [repoId]);
  const routeRef = route.params.ref ?? null;
  const refName = routeRef ?? (summary ? defaultRef(summary, detail.data ?? null) : null);
  const canResolve = repoId !== null && refName !== null && (routeRef !== null || detail.data !== undefined);
  const unborn = detail.data?.head.type === 'unborn';
  const rev = useQuery(canResolve && !unborn ? qk('rev_resolve', repoId, refName) : null, () => src.resolve(refName!));
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
            src,
            explore: false,
            owner: summary.owner,
            repo: summary.name,
            defaultBranch: summary.defaultBranch,
            unborn,
            refName,
            refFromRoute: routeRef !== null,
            resolved,
            revError: rev.error,
            revLoading: rev.loading,
            local: { summary, repoId: summary.id, detail: detail.data ?? null, detailError: detail.error },
            online: null,
          }
        : null,
    [src, summary, unborn, detail.data, detail.error, refName, routeRef, resolved, rev.error, rev.loading],
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
          {d?.dirty === true && <DirtyNote />}
          {!detail.error && children}
        </div>
      </main>
    </RepoContext.Provider>
  );
}
