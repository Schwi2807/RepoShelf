// S7 Branches tab (FR-29): filter (`?q=`), sections Default / Your branches / Remote branches, rows with lazy
// ahead/behind bars against HEAD's branch (D-58).

import { GitBranch, Search, X } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import { EmptyRepo, ListSkeleton } from '../../components/layout/RefGate';
import { useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { AB_CHUNK, BranchChunk } from '../../components/refs/BranchRow';
import { Empty } from '../../components/ui/bits';
import { branchesList } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { BranchRow } from '../../ipc/types';
import { navigate, withQuery } from '../../router/router';
import { useRoute } from '../../router/useRoute';

function chunks<T>(xs: T[], n: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < xs.length; i += n) out.push(xs.slice(i, i + n));
  return out;
}

export function filterBranches(rows: BranchRow[], q: string): BranchRow[] {
  const s = q.trim().toLowerCase();
  return s ? rows.filter((r) => r.item.name.toLowerCase().includes(s)) : rows;
}

export default function Branches() {
  const r = useRepo();
  const route = useRoute();
  const q = route.query.q ?? '';
  const list = useQuery(qk('branches_list', r.repoId), () => branchesList(r.repoId));
  const [max, setMax] = useState(0);
  const onCounts = useCallback((m: number) => setMax((x) => Math.max(x, m)), []);

  const head = r.detail?.head;
  const headBranch = head?.type === 'branch' ? head.name : null;
  const base = headBranch ?? r.summary.defaultBranch ?? '';

  const sections = useMemo(() => {
    const rows = filterBranches(list.data ?? [], q);
    return [
      { id: 'default', title: 'Default', rows: rows.filter((x) => x.item.isDefault) },
      { id: 'yours', title: 'Your branches', rows: rows.filter((x) => x.item.kind === 'branch' && !x.item.isDefault) },
      { id: 'remote', title: 'Remote branches', rows: rows.filter((x) => x.item.kind === 'remoteBranch') },
    ];
  }, [list.data, q]);

  if (head?.type === 'unborn') return <EmptyRepo what="branches" />;

  const setQ = (v: string) => navigate(withQuery(route, { q: v }), { replace: true, keepScroll: true });
  const total = list.data?.length ?? 0;
  const shown = sections.reduce((n, s) => n + s.rows.length, 0);

  return (
    <div className="stack refs-page">
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
      {list.error ? (
        <Box>
          <Empty icon={GitBranch} title="Branches cannot be listed">
            {list.error.message}
          </Empty>
        </Box>
      ) : !list.data ? (
        <ListSkeleton />
      ) : total === 0 ? (
        <Box>
          <Empty icon={GitBranch} title="No branches">
            This repository has no local or remote-tracking branches.
          </Empty>
        </Box>
      ) : shown === 0 ? (
        <Box>
          <Empty icon={Search} title={`No branches match “${q}”`} />
        </Box>
      ) : (
        sections
          .filter((s) => s.rows.length > 0)
          .map((s) => (
            <section key={s.id} className="box refsec" aria-label={s.title} data-section={s.id}>
              <div className="box__header refsec__header">
                <h2 className="box__title">{s.title}</h2>
                {s.id !== 'default' && base && (
                  <span className="muted small">
                    Ahead / behind compared with <span className="mono">{base}</span>
                  </span>
                )}
              </div>
              {chunks(s.rows, AB_CHUNK).map((rows) => (
                <BranchChunk
                  key={rows[0].item.fullName}
                  rows={rows}
                  repoId={r.repoId}
                  owner={r.owner}
                  repo={r.repo}
                  base={base}
                  headBranch={headBranch}
                  max={max}
                  onCounts={onCounts}
                />
              ))}
            </section>
          ))
      )}
    </div>
  );
}
