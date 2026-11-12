// S0 global header (FR-32): logo + name (home) · RepoJump · "+" menu · jobs · theme toggle · settings.

import { ArrowLeft, ArrowRight, Download, FolderPlus, Monitor, Moon, Plus, Settings, Sun } from 'lucide-react';
import { useSyncExternalStore } from 'react';
import { back, forward, historyPos, href, subscribe } from '../../router/router';
import { Link } from '../../router/Link';
import { useSettings } from '../../state/settings';
import { nextTheme, setTheme } from '../../state/theme';
import { openDialog } from '../../state/ui';
import { Dropdown } from '../ui/Dropdown';
import { AppMark } from './AppMark';
import { JobsButton } from './JobsPopover';
import { RepoJump } from './RepoJump';

const THEME_LABEL = { system: 'System', light: 'Light', dark: 'Dark' } as const;

export function ThemeToggle() {
  const { settings } = useSettings();
  const t = settings.theme;
  const Icon = t === 'system' ? Monitor : t === 'light' ? Sun : Moon;
  const label = `Theme: ${THEME_LABEL[t]} (switch to ${THEME_LABEL[nextTheme(t)]})`;
  return (
    <button type="button" className="gh__btn" aria-label={label} title={label} onClick={() => void setTheme(nextTheme(t))}>
      <Icon size={16} aria-hidden="true" />
    </button>
  );
}

export function AddMenu() {
  return (
    <Dropdown
      align="right"
      items={[
        { key: 'import', label: 'Import folder…', icon: FolderPlus, onSelect: () => openDialog('import') },
        { key: 'clone', label: 'Clone URL…', icon: Download, onSelect: () => openDialog('clone') },
      ]}
      renderTrigger={({ open, toggle, ref, onKeyDown }) => (
        <button
          ref={ref}
          type="button"
          className="gh__btn"
          aria-label="Add a repository"
          title="Add a repository"
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={toggle}
          onKeyDown={onKeyDown}
        >
          <Plus size={16} aria-hidden="true" />
        </button>
      )}
    />
  );
}

/** F1: on-screen Back / Forward (Alt+Left/Right and mouse buttons 4/5 keep working, router.ts). Disabled when the
 * app's own history has no entry in that direction. */
export function HistoryButtons() {
  const pos = useSyncExternalStore(subscribe, historyPos, historyPos);
  return (
    <nav className="gh__hist" aria-label="History">
      <button type="button" className="gh__btn" aria-label="Back (Alt+Left)" title="Back (Alt+Left)" disabled={!pos.canBack} onClick={back}>
        <ArrowLeft size={16} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="gh__btn"
        aria-label="Forward (Alt+Right)"
        title="Forward (Alt+Right)"
        disabled={!pos.canForward}
        onClick={forward}
      >
        <ArrowRight size={16} aria-hidden="true" />
      </button>
    </nav>
  );
}

export function GlobalHeader() {
  return (
    <header className="gh">
      <div className="gh__inner">
        <Link to={href('dashboard')} className="gh__brand" aria-label="RepoShelf home">
          <AppMark className="gh__mark" />
          <span className="gh__brand-name">RepoShelf</span>
        </Link>
        <div className="gh__mid">
          <HistoryButtons />
          <div className="gh__search">
            <RepoJump />
          </div>
        </div>
        <nav className="gh__actions" aria-label="App">
          <AddMenu />
          <JobsButton />
          <ThemeToggle />
          <Link to={href('settings')} className="gh__btn" aria-label="Settings" title="Settings">
            <Settings size={16} aria-hidden="true" />
          </Link>
        </nav>
      </div>
    </header>
  );
}
