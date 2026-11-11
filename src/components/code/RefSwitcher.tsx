// Ref switcher (FR-13): branch button → popover with Branches | Tags tabs and a filter; the default branch is marked;
// tags are linked as `refs/tags/<name>` so a tag never resolves to a same-named branch; detached HEAD shows the short
// oid. Selecting a ref only changes the route (no checkout).

import { Check, GitBranch, GitCommitHorizontal, Tag } from 'lucide-react';
import { useMemo, useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import { refsList } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { RefItem, RefsList, ResolvedRev } from '../../ipc/types';
import { navigate } from '../../router/router';
import { Link } from '../../router/Link';
import { shortOid } from '../../util/format';
import { Badge, Shimmer } from '../ui/bits';
import { useDismiss } from '../ui/Popover';

export type RefTab = 'branches' | 'tags';

/** Route ref for an item: tags as `refs/tags/<name>`, branches by name. */
export function refRouteName(item: RefItem): string {
  return item.kind === 'tag' ? `refs/tags/${item.name}` : item.name;
}

/** Display name of a route ref: `refs/tags/v1` → `v1`, a full oid → short oid. */
export function refDisplayName(refName: string, resolved: ResolvedRev | null): string {
  if (refName.startsWith('refs/tags/')) return refName.slice('refs/tags/'.length);
  if (refName.startsWith('refs/heads/')) return refName.slice('refs/heads/'.length);
  if (resolved?.kind === 'commit' || /^[0-9a-f]{40}$/i.test(refName)) return shortOid(refName);
  return refName;
}

export interface RefSwitcherProps {
  repoId: number;
  /** Decoded route ref (or the default when the route has none). */
  refName: string | null;
  resolved: ResolvedRev | null;
  /** Where a ref leads (keeps the current path / tab). */
  hrefFor: (routeRef: string) => string;
}

export function filterRefs(items: RefItem[], q: string): RefItem[] {
  const s = q.trim().toLowerCase();
  return s.length === 0 ? items : items.filter((i) => i.name.toLowerCase().includes(s));
}

function isCurrent(item: RefItem, refName: string | null, resolved: ResolvedRev | null): boolean {
  if (refName === null) return false;
  if (resolved?.fullName) return resolved.fullName === item.fullName;
  return refRouteName(item) === refName || item.name === refName;
}

export function RefSwitcher({ repoId, refName, resolved, hrefFor }: RefSwitcherProps) {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<RefTab>(resolved?.kind === 'tag' ? 'tags' : 'branches');
  const [q, setQ] = useState('');
  const wrap = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);
  useDismiss(open, () => setOpen(false), [wrap], trigger);
  const refs = useQuery(open ? qk('refs_list', repoId) : null, () => refsList(repoId));

  const kind = resolved?.kind ?? (refName?.startsWith('refs/tags/') ? 'tag' : 'branch');
  const Icon = kind === 'tag' ? Tag : kind === 'commit' ? GitCommitHorizontal : GitBranch;
  const label = refName === null ? '—' : refDisplayName(refName, resolved);

  const toggle = () => {
    setOpen((o) => !o);
    setQ('');
    setTab(resolved?.kind === 'tag' ? 'tags' : 'branches');
  };

  const focusItem = (dir: 1 | -1) => {
    const els = [...(list.current?.querySelectorAll<HTMLAnchorElement>('a.refs__item') ?? [])];
    if (els.length === 0) return;
    const i = els.indexOf(document.activeElement as HTMLAnchorElement);
    els[i < 0 ? (dir === 1 ? 0 : els.length - 1) : (i + dir + els.length) % els.length].focus();
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusItem(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusItem(-1);
    }
  };

  return (
    <div className="dropdown" ref={wrap}>
      <button
        ref={trigger}
        type="button"
        className="btn refswitch"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={kind === 'commit' ? `Detached at ${refName ?? ''}` : `Switch branches or tags (current: ${label})`}
        onClick={toggle}
      >
        <Icon size={16} aria-hidden="true" />
        <span className="refswitch__name truncate">{label}</span>
        <svg width="12" height="12" viewBox="0 0 16 16" aria-hidden="true" className="refswitch__caret">
          <path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.6" />
        </svg>
      </button>
      {open && (
        <div className="popover popover--left refs" role="dialog" aria-label="Switch branches or tags" onKeyDown={onKey}>
          <div className="popover__header">Switch branches/tags</div>
          <div className="refs__filter">
            <input
              className="input"
              autoFocus
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder={tab === 'branches' ? 'Find a branch…' : 'Find a tag…'}
              aria-label={tab === 'branches' ? 'Filter branches' : 'Filter tags'}
              spellCheck={false}
            />
          </div>
          <div className="refs__tabs" role="tablist">
            {(['branches', 'tags'] as const).map((t) => (
              <button
                key={t}
                type="button"
                role="tab"
                aria-selected={tab === t}
                className="refs__tab"
                onClick={() => setTab(t)}
              >
                {t === 'branches' ? 'Branches' : 'Tags'}
              </button>
            ))}
          </div>
          <RefList
            data={refs.data}
            loading={refs.loading}
            error={refs.error?.message}
            tab={tab}
            q={q}
            refName={refName}
            resolved={resolved}
            hrefFor={hrefFor}
            listRef={list}
            onPick={() => setOpen(false)}
          />
        </div>
      )}
    </div>
  );
}

function RefList({
  data,
  loading,
  error,
  tab,
  q,
  refName,
  resolved,
  hrefFor,
  listRef,
  onPick,
}: {
  data: RefsList | undefined;
  loading: boolean;
  error: string | undefined;
  tab: RefTab;
  q: string;
  refName: string | null;
  resolved: ResolvedRev | null;
  hrefFor: (r: string) => string;
  listRef: RefObject<HTMLUListElement | null>;
  onPick: () => void;
}) {
  const items = useMemo(() => {
    if (!data) return [];
    const all = tab === 'branches' ? [...data.branches, ...data.remoteBranches] : data.tags;
    return filterRefs(all, q);
  }, [data, tab, q]);
  if (error) return <p className="refs__empty field__error">{error}</p>;
  if (loading || !data) {
    return (
      <div className="refs__empty">
        <Shimmer width="70%" block />
      </div>
    );
  }
  if (items.length === 0) {
    return <p className="refs__empty muted">{q ? `Nothing to show for “${q}”.` : tab === 'branches' ? 'No branches.' : 'No tags.'}</p>;
  }
  return (
    <ul className="refs__list" ref={listRef} role="listbox" aria-label={tab === 'branches' ? 'Branches' : 'Tags'}>
      {items.map((it) => {
        const cur = isCurrent(it, refName, resolved);
        const to = hrefFor(refRouteName(it));
        return (
          <li key={it.fullName} role="option" aria-selected={cur}>
            <Link
              to={to}
              className="refs__item"
              onClick={() => onPick()}
              onKeyDown={(e) => {
                if (e.key === ' ') {
                  e.preventDefault();
                  onPick();
                  navigate(to);
                }
              }}
            >
              <span className="refs__check" aria-hidden="true">
                {cur && <Check size={14} />}
              </span>
              <span className="truncate refs__name">{it.name}</span>
              {it.isDefault && <Badge>default</Badge>}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
