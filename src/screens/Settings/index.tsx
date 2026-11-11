// S10 Settings page (FR-33, FR-35, D-16): theme (light / dark / system), data + archive folders (read-only, Copy and
// Open via the backend, D-76), cache size + Clear cache, the optional "uncommitted changes" note, app version.

import { FolderOpen, Monitor, Moon, Sun, Trash2, type LucideIcon } from 'lucide-react';
import { useState } from 'react';
import { Box } from '../../components/layout/Sidebar';
import { Shimmer } from '../../components/ui/bits';
import { Button } from '../../components/ui/Button';
import { CopyButton } from '../../components/ui/CopyButton';
import { appInfo, appOpenDir, cacheClear } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { invalidate, prime, qk, useQuery } from '../../ipc/query';
import type { AppDir, Theme } from '../../ipc/types';
import { updateSettings, useSettings } from '../../state/settings';
import { setTheme } from '../../state/theme';
import { formatBytes, formatCount } from '../../util/format';

const THEMES: { id: Theme; label: string; icon: LucideIcon; hint: string }[] = [
  { id: 'light', label: 'Light', icon: Sun, hint: 'Always light' },
  { id: 'dark', label: 'Dark', icon: Moon, hint: 'Always dark' },
  { id: 'system', label: 'System', icon: Monitor, hint: 'Follow the Windows setting' },
];

function Folder({ which, label, path, hint }: { which: AppDir; label: string; path: string; hint: string }) {
  const [err, setErr] = useState<string | null>(null);
  return (
    <div className="setting-folder">
      <div className="setting-folder__head">
        <span className="label">{label}</span>
        <span className="muted small">{hint}</span>
      </div>
      <div className="path-box setting-folder__path">
        <span className="mono break-all">{path}</span>
        <span className="spacer" />
        <CopyButton text={path} label={`Copy the ${label.toLowerCase()} path`} />
        <Button
          size="sm"
          icon={FolderOpen}
          onClick={async () => {
            setErr(null);
            try {
              await appOpenDir(which);
            } catch (e) {
              setErr(errorText(e));
            }
          }}
        >
          Open
        </Button>
      </div>
      {err && <p className="field__error">{err}</p>}
    </div>
  );
}

export default function Settings() {
  const info = useQuery(qk('app_info'), () => appInfo());
  const { settings, error } = useSettings();
  const [clearing, setClearing] = useState(false);
  const [clearMsg, setClearMsg] = useState<string | null>(null);

  const clear = async () => {
    setClearing(true);
    setClearMsg(null);
    try {
      const before = info.data?.cacheBytes ?? 0;
      const next = await cacheClear();
      prime(qk('app_info'), next);
      setClearMsg(`Cleared ${formatBytes(before)}. Counts and last-commit columns are computed again when needed.`);
    } catch (e) {
      setClearMsg(errorText(e));
    } finally {
      setClearing(false);
    }
  };

  return (
    <main className="container page settings">
      <h1 className="page-title">Settings</h1>
      {error && (
        <p className="field__error" role="alert">
          {error}
        </p>
      )}

      <Box header={<h2 className="box__title">Appearance</h2>}>
        <div className="box__body">
          <fieldset className="theme-pick">
            <legend className="label">Theme</legend>
            {THEMES.map((t) => (
              <label key={t.id} className={`theme-pick__item${settings.theme === t.id ? ' theme-pick__item--on' : ''}`}>
                <input type="radio" name="theme" value={t.id} checked={settings.theme === t.id} onChange={() => void setTheme(t.id)} />
                <t.icon size={18} aria-hidden="true" />
                <span className="theme-pick__text">
                  <strong>{t.label}</strong>
                  <span className="muted small">{t.hint}</span>
                </span>
              </label>
            ))}
          </fieldset>
        </div>
      </Box>

      <Box header={<h2 className="box__title">Folders</h2>}>
        <div className="box__body stack">
          {info.data ? (
            <>
              <Folder which="data" label="Data folder" path={info.data.dataDir} hint="Library database, settings and caches" />
              <Folder which="archive" label="Archive folder" path={info.data.archiveDir} hint="Where cloned repositories are kept (bare)" />
              <p className="muted small">
                Set by the app id <span className="mono">dev.schwi.reposhelf</span>; the <span className="mono">RS_DATA_DIR</span> and{' '}
                <span className="mono">RS_ARCHIVE_DIR</span> environment variables move them.
              </p>
            </>
          ) : info.error ? (
            <p className="field__error">{info.error.message}</p>
          ) : (
            <Shimmer width={360} block />
          )}
        </div>
      </Box>

      <Box header={<h2 className="box__title">Storage</h2>}>
        <div className="box__body stack">
          <dl className="stats settings-stats">
            <div className="stats__item">
              <dt>Repositories</dt>
              <dd>{info.data ? formatCount(info.data.repoCount) : '—'}</dd>
            </div>
            <div className="stats__item">
              <dt>Archive size</dt>
              <dd>{info.data ? (info.data.archiveBytes === null ? 'too large to measure' : formatBytes(info.data.archiveBytes)) : '—'}</dd>
            </div>
            <div className="stats__item">
              <dt>Cache</dt>
              <dd data-testid="cache-size">{info.data ? formatBytes(info.data.cacheBytes) : '—'}</dd>
            </div>
          </dl>
          <div className="row row--wrap">
            <Button icon={Trash2} onClick={() => void clear()} disabled={clearing || !info.data || info.data.cacheBytes === 0}>
              {clearing ? 'Clearing…' : 'Clear cache'}
            </Button>
            <span className="muted small">Removes stored commit counts, last-commit columns, language stats and GitHub notes. Repositories stay.</span>
          </div>
          {clearMsg && (
            <p className="muted small" role="status">
              {clearMsg}
            </p>
          )}
        </div>
      </Box>

      <Box header={<h2 className="box__title">Imported repositories</h2>}>
        <div className="box__body">
          <label className="check-row">
            <input
              type="checkbox"
              checked={settings.showDirtyNote}
              onChange={async (e) => {
                await updateSettings({ showDirtyNote: e.target.checked });
                invalidate('repo_detail');
              }}
            />
            <span>
              <strong>Note uncommitted changes</strong>
              <span className="muted small">
                For imported repositories with a working folder, say when files there differ from the last commit. RepoShelf only
                reads the folder: it never writes the index or any file. Off by default.
              </span>
            </span>
          </label>
        </div>
      </Box>

      <p className="muted small settings__version">
        RepoShelf {info.data ? <span className="mono">{info.data.version}</span> : '…'} · read-only GitHub-style browser for your local
        repositories
      </p>
    </main>
  );
}
