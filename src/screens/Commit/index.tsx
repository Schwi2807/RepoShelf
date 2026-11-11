// S6 Commit page (FR-27, FR-28): header card, diff summary + notes + file list, then one card per changed file
// (virtualised past 40 cards). `?file=N` scrolls to card N two frames after a fresh visit (as D-72's `?L=`), and
// the file list sets it with a replace. A short oid in the route is resolved first (commit_detail needs 40 hex).

import { GitCommitHorizontal } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CommitHeader } from '../../components/diff/CommitHeader';
import { DiffSummary } from '../../components/diff/DiffSummary';
import { FileDiffCard } from '../../components/diff/FileDiffCard';
import { ListSkeleton } from '../../components/layout/RefGate';
import { useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { Empty, Shimmer } from '../../components/ui/bits';
import { WindowList } from '../../components/ui/WindowList';
import { commitDetail, revResolve } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { FileDiff } from '../../ipc/types';
import type { Route } from '../../router/routes';
import { href, navigate, pendingScroll, withQuery } from '../../router/router';
import { Link } from '../../router/Link';

export const CARD_VIRTUAL_THRESHOLD = 40;

export default function Commit({ route }: { route: Route }) {
  return <CommitPage key={route.params.oid ?? ''} route={route} />;
}

function CommitPage({ route }: { route: Route }) {
  const r = useRepo();
  const raw = route.params.oid ?? '';
  const full = /^[0-9a-f]{40}$/i.test(raw) ? raw.toLowerCase() : null;
  const res = useQuery(full ? null : qk('rev_resolve', r.repoId, raw), () => revResolve(r.repoId, raw));
  const oid = full ?? res.data?.commit ?? null;
  const d = useQuery(oid ? qk('commit_detail', r.repoId, oid) : null, () => commitDetail(r.repoId, oid!), { stable: true });

  const [closed, setClosed] = useState<ReadonlySet<number>>(new Set());
  const [loads, setLoads] = useState<ReadonlySet<number>>(new Set());
  const [scrollTo, setScrollTo] = useState<{ index: number; seq: number } | null>(null);
  const seq = useRef(0);
  // as FileView (D-72): a back/forward visit has a remembered position pending; anything else is a fresh visit
  const fresh = useRef((pendingScroll() ?? 0) === 0);

  const inline = useMemo(() => {
    const m = new Map<number, FileDiff>();
    for (const x of d.data?.diffs ?? []) m.set(x.index, x);
    return m;
  }, [d.data]);

  const jump = useCallback((index: number) => {
    seq.current += 1;
    setScrollTo({ index, seq: seq.current });
  }, []);

  // `?file=N` on a fresh visit: two frames after the cards exist (the router's top-scroll runs first).
  const fileParam = route.query.file;
  const ready = d.data !== undefined;
  useEffect(() => {
    if (!ready || !fresh.current) return;
    fresh.current = false;
    const n = Number.parseInt(fileParam ?? '', 10);
    if (!Number.isFinite(n) || n < 0) return;
    let f2 = 0;
    const f1 = requestAnimationFrame(() => {
      f2 = requestAnimationFrame(() => jump(n));
    });
    return () => {
      cancelAnimationFrame(f1);
      cancelAnimationFrame(f2);
    };
  }, [ready, fileParam, jump]);

  if (res.error || d.error) {
    const e = res.error ?? d.error!;
    return (
      <Box>
        <Empty
          icon={GitCommitHorizontal}
          title={e.code === 'notFound' || e.code === 'invalidRev' ? 'Commit not found' : 'This commit cannot be shown'}
          actions={<Link to={href('commits', { owner: r.owner, repo: r.repo, ref: r.summary.defaultBranch ?? 'HEAD' })}>Browse the commits</Link>}
        >
          {e.code === 'notFound' || e.code === 'invalidRev' ? (
            <>
              No commit <span className="mono">{raw}</span> exists in this repository.
            </>
          ) : (
            e.message
          )}
        </Empty>
      </Box>
    );
  }
  if (!d.data || !oid) {
    return (
      <div className="stack">
        <div className="box chead" aria-busy="true">
          <Shimmer width="55%" height={24} />
          <Shimmer width="30%" />
        </div>
        <ListSkeleton rows={3} />
      </div>
    );
  }

  const det = d.data;
  const pick = (index: number) => {
    navigate(withQuery(route, { file: index }), { replace: true, keepScroll: true });
    setClosed((s) => {
      if (!s.has(index)) return s;
      const n = new Set(s);
      n.delete(index);
      return n;
    });
    jump(index);
  };

  return (
    <div className="stack commit-page">
      <CommitHeader d={det} owner={r.owner} repo={r.repo} />
      <DiffSummary d={det} owner={r.owner} repo={r.repo} onPick={pick} />
      {det.files.length === 0 ? (
        <Box>
          <Empty icon={GitCommitHorizontal} title="No file changes">
            This commit changes no files compared with {det.commit.parents.length === 0 ? 'an empty tree' : 'its first parent'}.
          </Empty>
        </Box>
      ) : (
        <WindowList
          className="dcards"
          count={det.files.length}
          estimate={det.diffsDeferred ? 96 : 220}
          threshold={CARD_VIRTUAL_THRESHOLD}
          rowKey={(i) => det.files[i].index}
          scrollTo={scrollTo}
          row={(i) => {
            const f = det.files[i];
            return (
              <FileDiffCard
                f={f}
                inline={inline.get(f.index)}
                deferred={det.diffsDeferred}
                repoId={r.repoId}
                owner={r.owner}
                repo={r.repo}
                oid={oid}
                parent={det.parentCompared}
                expanded={!closed.has(f.index)}
                onToggle={() =>
                  setClosed((s) => {
                    const n = new Set(s);
                    if (n.has(f.index)) n.delete(f.index);
                    else n.add(f.index);
                    return n;
                  })
                }
                loadRequested={loads.has(f.index)}
                onLoad={() => setLoads((s) => new Set(s).add(f.index))}
              />
            );
          }}
        />
      )}
    </div>
  );
}
