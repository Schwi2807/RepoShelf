// Header search (S0, FR-32): "Search or jump to a repo…", dropdown of matches (Recent when empty), arrow keys +
// Enter jump to the repo, "/" focuses it from anywhere (NFR-A1).

import { BookMarked, Clock, Search } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from 'react';
import type { RepoSummary } from '../../ipc/types';
import { href, navigate } from '../../router/router';
import { filterRepos, sortRepos, useLibrary } from '../../state/library';
import { useRecent } from '../../state/recent';
import { useDismiss } from '../ui/Popover';

const MAX = 8;

function isTypingTarget(t: EventTarget | null): boolean {
  if (!(t instanceof HTMLElement)) return false;
  return t.isContentEditable || t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT';
}

export function RepoJump() {
  const { repos } = useLibrary();
  const recent = useRecent();
  const [q, setQ] = useState('');
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const listId = useId();
  useDismiss(open, () => setOpen(false), [wrap]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey || isTypingTarget(e.target)) return;
      if (document.querySelector('[aria-modal="true"]')) return;
      e.preventDefault();
      input.current?.focus();
      input.current?.select();
      setOpen(true);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const results: RepoSummary[] = useMemo(() => {
    if (q.trim().length === 0) return recent.slice(0, MAX);
    return sortRepos(filterRepos(repos ?? [], q), { key: 'updated', desc: true }).slice(0, MAX);
  }, [q, repos, recent]);

  const go = (r: RepoSummary) => {
    setOpen(false);
    setQ('');
    input.current?.blur();
    navigate(href('code', { owner: r.owner, repo: r.name }));
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setOpen(true);
      setActive((a) => Math.min(a + 1, Math.max(results.length - 1, 0)));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === 'Enter') {
      const r = results[active];
      if (r) {
        e.preventDefault();
        go(r);
      } else if (q.trim()) {
        setOpen(false);
        navigate(href('dashboard', {}, { q: q.trim() }));
      }
    } else if (e.key === 'Escape') {
      setOpen(false);
      input.current?.blur();
    }
  };

  const showList = open && (results.length > 0 || q.trim().length > 0);

  return (
    <div className="jump" ref={wrap}>
      <div className="jump__field">
        <Search size={16} aria-hidden="true" />
        <input
          ref={input}
          className="jump__input"
          type="search"
          role="combobox"
          aria-expanded={showList}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-label="Search or jump to a repository"
          placeholder="Search or jump to a repo…"
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setActive(0);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={onKeyDown}
          spellCheck={false}
          autoComplete="off"
        />
        {q.length === 0 && (
          <kbd className="kbd" aria-hidden="true">
            /
          </kbd>
        )}
      </div>
      {showList && (
        <div className="popover">
          {q.trim().length === 0 && <div className="popover__header">Recent</div>}
          <div id={listId} role="listbox" aria-label="Repositories">
            {results.map((r, i) => (
              <a
                key={r.id}
                role="option"
                aria-selected={i === active}
                className="jump__item"
                href={href('code', { owner: r.owner, repo: r.name })}
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={(e) => {
                  e.preventDefault();
                  go(r);
                }}
              >
                {q.trim().length === 0 ? <Clock size={16} aria-hidden="true" /> : <BookMarked size={16} aria-hidden="true" />}
                <span className="truncate">
                  {r.owner} / <strong>{r.name}</strong>
                </span>
              </a>
            ))}
            {results.length === 0 && <div className="menu__note">No repository matches “{q.trim()}”.</div>}
          </div>
        </div>
      )}
    </div>
  );
}
