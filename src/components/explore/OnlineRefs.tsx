// Branches and Tags tabs of an Explore repository (v0.3.0 M10b). Both come from ONE `explore_refs` answer (name +
// commit id, at most 300 of each, with flags when GitHub had more). The shelf's rows also show the last commit and the
// ahead/behind bar; online those would be one request per row (D-122), so a row is name · default badge · commit id.

import { FileCode2, GitBranch, Search, Tag, X } from 'lucide-react';
import { useMemo } from 'react';
import { exploreRefsInfo } from '../../source';
import { qk, useQuery } from '../../ipc/query';
import type { RefItem } from '../../ipc/types';
import { href, navigate, withQuery } from '../../router/router';
import { Link } from '../../router/Link';
import { useRoute } from '../../router/useRoute';
import { shortOid } from '../../util/format';
import { Box } from '../layout/Sidebar';
import { ListSkeleton } from '../layout/RefGate';
import { useRepo } from '../layout/repoContext';
import { ReleasesTagsToggle } from '../refs/ReleasesTagsToggle';
import { Badge, Empty } from '../ui/bits';
import { ExploreFailure } from './ExploreErrors';

function useRefs() {
  const r = useRepo();
  return useQuery(qk('explore_refs', r.src.key), () => exploreRefsInfo(r.owner, r.repo));
}

function Truncated({ what }: { what: string }) {
  return (
    <p className="muted small" data-testid="explore-truncated">
      GitHub has more {what} than shown here: the first 300 are listed.
    </p>
  );
}

export function OnlineBranches() {
  const route = useRoute();
  const q = route.query.q ?? '';
  const refs = useRefs();
  const rows = useMemo(() => {
    const s = q.trim().toLowerCase();
    const all = refs.data?.refs.branches ?? [];
    return s ? all.filter((b) => b.name.toLowerCase().includes(s)) : all;
  }, [refs.data, q]);
  const setQ = (v: string) => navigate(withQuery(route, { q: v }), { replace: true, keepScroll: true });
  return (
    <div className="stack refs-page" data-testid="online-branches">
      <div className="codebar">
        <h1 className="commits__title">Branches</h1>
        <span className="spacer" />
        <div className="filter-input filter-input--compact">
          <Search size={14} aria-hidden="true" className="filter-input__icon" />
          <input
            className="input filter-input__field"
            type="search"
            placeholder="Search branches…"
            aria-label="Filter branches"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            spellCheck={false}
            autoComplete="off"
          />
          {q && (
            <button type="button" className="filter-input__clear" aria-label="Clear branch filter" onClick={() => setQ('')}>
              <X size={14} aria-hidden="true" />
            </button>
          )}
        </div>
      </div>
      {refs.error ? (
        <ExploreFailure inline error={refs.error} what="The branch list" onRetry={refs.refetch} />
      ) : !refs.data ? (
        <ListSkeleton />
      ) : refs.data.refs.branches.length === 0 ? (
        <Box>
          <Empty icon={GitBranch} title="No branches">
            This repository has no branches on GitHub.
          </Empty>
        </Box>
      ) : rows.length === 0 ? (
        <Box>
          <Empty icon={Search} title={`No branches match “${q}”`} />
        </Box>
      ) : (
        <section className="box refsec" aria-label="Branches">
          <div className="box__header refsec__header">
            <h2 className="box__title">Branches</h2>
            <span className="muted small">Last-commit details and ahead / behind are on the shelf version.</span>
          </div>
          <ul className="brows">
            {rows.map((b) => (
              <OnlineBranchRow key={b.fullName} b={b} />
            ))}
          </ul>
        </section>
      )}
      {refs.data?.branchesTruncated && <Truncated what="branches" />}
    </div>
  );
}

function OnlineBranchRow({ b }: { b: RefItem }) {
  const r = useRepo();
  return (
    <li className="brow" data-branch={b.name}>
      <div className="brow__main">
        <div className="brow__title">
          <GitBranch size={16} aria-hidden="true" className="brow__icon" />
          <Link to={href('tree', { explore: true, owner: r.owner, repo: r.repo, ref: b.name, path: '' })} className="brow__name mono truncate" title={b.fullName}>
            {b.name}
          </Link>
          {b.isDefault && <Badge tone="accent">default</Badge>}
        </div>
      </div>
      <Link to={href('commit', { explore: true, owner: r.owner, repo: r.repo, oid: b.target })} className="brow__oid mono" title={b.target}>
        {shortOid(b.target)}
      </Link>
    </li>
  );
}

export function OnlineTags() {
  const r = useRepo();
  const refs = useRefs();
  const tags = refs.data?.refs.tags ?? [];
  return (
    <div className="stack refs-page" data-testid="online-tags">
      <div className="codebar">
        <ReleasesTagsToggle owner={r.owner} repo={r.repo} current="tags" releases={null} tags={refs.data ? tags.length : null} />
      </div>
      {refs.error ? (
        <ExploreFailure inline error={refs.error} what="The tag list" onRetry={refs.refetch} />
      ) : !refs.data ? (
        <ListSkeleton />
      ) : tags.length === 0 ? (
        <Box>
          <Empty icon={Tag} title="No tags yet">
            Tags mark specific points in the history, such as versions.
          </Empty>
        </Box>
      ) : (
        <div className="box">
          <div className="box__header">
            <Tag size={16} aria-hidden="true" />
            <h2 className="box__title">Tags</h2>
          </div>
          <ul className="trows">
            {tags.map((t) => {
              const ref = `refs/tags/${t.name}`;
              return (
                <li className="trow" key={t.fullName} data-tag={t.name}>
                  <div className="trow__main">
                    <div className="trow__title">
                      <Tag size={16} aria-hidden="true" className="trow__icon" />
                      <Link to={href('tree', { explore: true, owner: r.owner, repo: r.repo, ref, path: '' })} className="trow__name truncate" title={t.fullName}>
                        {t.name}
                      </Link>
                    </div>
                    <div className="trow__meta">
                      <Link to={href('commit', { explore: true, owner: r.owner, repo: r.repo, oid: t.target })} className="mono trow__oid" title={t.target}>
                        {shortOid(t.target)}
                      </Link>
                    </div>
                  </div>
                  <Link to={href('tree', { explore: true, owner: r.owner, repo: r.repo, ref, path: '' })} className="btn btn--sm">
                    <FileCode2 size={16} aria-hidden="true" />
                    Browse files
                  </Link>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {refs.data?.tagsTruncated && <Truncated what="tags" />}
    </div>
  );
}
