// S5 Commits tab (FR-26, FR-31 compare list, NFR-P5, D-9): ref switcher + optional path chip / since banner, commits
// grouped by local day, Newer / Older over a pinned tip. Pages are oid-keyed (route ref's resolved commit or the
// pinned `?tip=`), so they never go stale and Back is instant.

import { GitCompareArrows, History, Pin } from 'lucide-react';
import { useMemo } from 'react';
import { DayGroup, groupCommits } from '../../components/commits/DayGroup';
import { Pager } from '../../components/commits/Pager';
import { commitsHref, loadCommitsPage, pagerHrefs, readCommitsQuery } from '../../components/commits/paging';
import { PathChip } from '../../components/commits/PathChip';
import { RefSwitcher, refDisplayName } from '../../components/code/RefSwitcher';
import { ListSkeleton, refGate } from '../../components/layout/RefGate';
import { useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { Empty } from '../../components/ui/bits';
import { qk, useQuery } from '../../ipc/query';
import type { Route } from '../../router/routes';
import { Link } from '../../router/Link';
import { shortOid } from '../../util/format';

export default function Commits({ route }: { route: Route }) {
  const r = useRepo();
  const q = useMemo(() => readCommitsQuery(route.query), [route.query]);
  const commit = r.resolved?.commit ?? null;
  const page = useQuery(
    commit ? qk('commits_page', r.repoId, q.tip ?? commit, q.path, q.since, q.page) : null,
    () => loadCommitsPage(r.repoId, commit!, q),
    { stable: true },
  );
  const groups = useMemo(() => groupCommits(page.data?.commits ?? []), [page.data]);

  const gate = refGate(r, 'commits', (ref) => commitsHref(r.owner, r.repo, ref, { path: q.path }));
  if (gate) return <>{gate}</>;
  const refName = r.refName!;
  const label = refDisplayName(refName, r.resolved);
  const pager = page.data ? pagerHrefs(r.owner, r.repo, refName, q, page.data) : { newer: null, older: null };
  const pinnedOld = q.tip !== null && commit !== null && q.tip !== commit;

  let body;
  if (page.error) {
    body = (
      <Box>
        <Empty icon={History} title="This history cannot be shown">
          {page.error.code === 'notFound' && q.since ? (
            <>
              <span className="mono">{refDisplayName(q.since, null)}</span> was not found in this repository.
            </>
          ) : (
            page.error.message
          )}
        </Empty>
      </Box>
    );
  } else if (!page.data) {
    body = <ListSkeleton rows={6} />;
  } else if (page.data.commits.length === 0 && page.data.capped) {
    body = (
      <Box>
        <Empty icon={History} title="No matching commits yet">
          {page.data.scanned.toLocaleString('en-US')} commits were searched without one touching <span className="mono">{q.path}</span>. Search
          further back with the button below.
        </Empty>
      </Box>
    );
  } else if (page.data.commits.length === 0) {
    body = (
      <Box>
        <Empty
          icon={History}
          title={q.page > 1 ? 'No more commits' : q.path ? 'No commits touch this path' : 'No commits'}
          actions={q.page > 1 ? <Link to={commitsHref(r.owner, r.repo, refName, { path: q.path, since: q.since })}>Back to the newest commits</Link> : undefined}
        >
          {q.page > 1
            ? 'This page is past the end of the history.'
            : q.path
              ? (
                <>
                  Nothing in the history of <span className="mono">{label}</span> changed <span className="mono">{q.path}</span>.
                </>
              )
              : 'There is nothing to list.'}
        </Empty>
      </Box>
    );
  } else {
    body = (
      <div className="cgroups" data-testid="commit-groups">
        {groups.map((g) => (
          <DayGroup key={`${g.key}-${g.items[0].oid}`} group={g} repoId={r.repoId} owner={r.owner} repo={r.repo} />
        ))}
      </div>
    );
  }

  return (
    <div className="stack commits">
      <div className="codebar">
        <RefSwitcher
          repoId={r.repoId}
          refName={refName}
          resolved={r.resolved}
          hrefFor={(ref) => commitsHref(r.owner, r.repo, ref, { path: q.path })}
        />
        {q.path ? (
          <PathChip owner={r.owner} repo={r.repo} refName={refName} path={q.path} since={q.since} />
        ) : (
          <h1 className="commits__title">Commits</h1>
        )}
      </div>
      {q.since && (
        <div className="notice notice--compare" data-testid="since-banner">
          <GitCompareArrows size={16} aria-hidden="true" />
          <span>
            Commits on <strong className="mono">{label}</strong> since <strong className="mono">{refDisplayName(q.since, null)}</strong>
            {' · '}
            <Link to={commitsHref(r.owner, r.repo, refName, { path: q.path })}>Show the full history</Link>
          </span>
        </div>
      )}
      {pinnedOld && (
        <div className="notice" data-testid="pinned-banner">
          <Pin size={16} aria-hidden="true" />
          <span>
            Listing the history as of <span className="mono">{shortOid(q.tip!)}</span>; <span className="mono">{label}</span> has moved
            since. <Link to={commitsHref(r.owner, r.repo, refName, { path: q.path, since: q.since })}>Show the newest commits</Link>
          </span>
        </div>
      )}
      {body}
      {page.data && (page.data.commits.length > 0 || page.data.capped) && <Pager newer={pager.newer} older={pager.older} capped={page.data.capped} />}
    </div>
  );
}
