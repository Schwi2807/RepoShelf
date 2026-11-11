// Dashboard toolbar (S1, FR-2, FR-3): filter bound to `?q=` (replace + keep scroll, so Back does not step through
// keystrokes), Type (source) filter and Sort menus persisted through settings, "New" → Import / Clone.

import { ArrowDownWideNarrow, Check, Download, FolderPlus, ListFilter, Plus, Search, X } from 'lucide-react';
import type { SortKey, SortSpec, SourceFilter } from '../../ipc/types';
import { href, navigate } from '../../router/router';
import { updateSettings, useSettings } from '../../state/settings';
import { openDialog } from '../../state/ui';
import { Button } from '../ui/Button';
import { Dropdown, type MenuEntry } from '../ui/Dropdown';

export const SORT_LABEL: Record<SortKey, string> = { updated: 'Last updated', name: 'Name', added: 'Date added' };
export const SOURCE_LABEL: Record<SourceFilter, string> = { all: 'All', imported: 'Imported', cloned: 'Cloned' };

function dirLabels(key: SortKey): [string, string] {
  return key === 'name' ? ['A → Z', 'Z → A'] : ['Oldest first', 'Newest first'];
}

/** Menu label with a fixed-width check column, so checked and unchecked items line up. */
export function checked(on: boolean, text: string) {
  return (
    <>
      <span className="menu__check" aria-hidden="true">
        {on && <Check size={14} />}
      </span>
      <span>{text}</span>
      {on && <span className="visually-hidden"> (selected)</span>}
    </>
  );
}

export function sortItems(sort: SortSpec, set: (s: SortSpec) => void): MenuEntry[] {
  const [asc, desc] = dirLabels(sort.key);
  return [
    ...(Object.keys(SORT_LABEL) as SortKey[]).map((k) => ({
      key: k,
      label: checked(sort.key === k, SORT_LABEL[k]),
      // name sorts A→Z by default, dates newest first
      onSelect: () => set({ key: k, desc: k === sort.key ? sort.desc : k !== 'name' }),
    })),
    'separator' as const,
    { key: 'asc', label: checked(!sort.desc, asc), onSelect: () => set({ ...sort, desc: false }) },
    { key: 'desc', label: checked(sort.desc, desc), onSelect: () => set({ ...sort, desc: true }) },
  ];
}

export function RepoToolbar({ query }: { query: string }) {
  const { settings } = useSettings();
  const setQuery = (q: string) => navigate(href('dashboard', {}, { q }), { replace: true, keepScroll: true });
  return (
    <div className="toolbar" role="search">
      <div className="filter-input">
        <Search size={16} aria-hidden="true" className="filter-input__icon" />
        <input
          className="input filter-input__field"
          type="search"
          aria-label="Find a repository"
          placeholder="Find a repository…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape' && query) {
              e.stopPropagation();
              setQuery('');
            }
          }}
          spellCheck={false}
          autoComplete="off"
        />
        {query && (
          <button type="button" className="filter-input__clear" aria-label="Clear filter" onClick={() => setQuery('')}>
            <X size={14} aria-hidden="true" />
          </button>
        )}
      </div>
      <Dropdown
        text={
          <>
            <span className="muted">Type:</span> {SOURCE_LABEL[settings.sourceFilter]}
          </>
        }
        icon={ListFilter}
        align="right"
        header="Select type"
        items={(Object.keys(SOURCE_LABEL) as SourceFilter[]).map((k) => ({
          key: k,
          label: checked(settings.sourceFilter === k, SOURCE_LABEL[k]),
          onSelect: () => void updateSettings({ sourceFilter: k }),
        }))}
      />
      <Dropdown
        text={
          <>
            <span className="muted">Sort:</span> {SORT_LABEL[settings.sort.key]}
          </>
        }
        icon={ArrowDownWideNarrow}
        align="right"
        header="Select order"
        items={sortItems(settings.sort, (sort) => void updateSettings({ sort }))}
      />
      <Dropdown
        text="New"
        icon={Plus}
        variant="primary"
        align="right"
        items={[
          { key: 'import', label: 'Import folder…', icon: FolderPlus, onSelect: () => openDialog('import') },
          { key: 'clone', label: 'Clone URL…', icon: Download, onSelect: () => openDialog('clone') },
        ]}
      />
    </div>
  );
}

/** Clear-filter link shown when nothing matches. */
export function ClearFilter() {
  return (
    <Button size="sm" variant="invisible" icon={X} onClick={() => navigate(href('dashboard'), { replace: true })}>
      Clear filter
    </Button>
  );
}
