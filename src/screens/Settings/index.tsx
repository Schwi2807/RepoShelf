// S10 Settings page (FR-33, FR-35, D-16): theme (light / dark / system), data + archive folders (read-only, Copy and
// Open via the backend, D-76), cache size + Clear cache, the optional "uncommitted changes" note, the v0.2.0
// "Network" section (five switches + the download size cap, D-100), app version.

import { FolderOpen, Monitor, Moon, Sun, Trash2, type LucideIcon } from 'lucide-react';
import { useState } from 'react';
import { Box } from '../../components/layout/Sidebar';
import { Shimmer } from '../../components/ui/bits';
import { ExploreSettings } from './ExploreSettings';
import { Button } from '../../components/ui/Button';
import { CopyButton } from '../../components/ui/CopyButton';
import { appInfo, appOpenDir, cacheClear } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { invalidate, isExploreKey, prime, purge, qk, useQuery } from '../../ipc/query';
import type { AppDir, Settings as SettingsT, Theme } from '../../ipc/types';
import { resetRemoteImages } from '../../markdown/remoteImages';
import { refreshExploreStatus } from '../../state/explore';
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

type NetToggle = 'loadRemoteImages' | 'fetchReleaseNotes' | 'fetchRepoInfo' | 'autoDownloadLatestRelease' | 'autoDownloadOnFetchAll';

const NET_TOGGLES: { key: NetToggle; title: string; hint: string }[] = [
  {
    key: 'loadRemoteImages',
    title: 'Load remote images',
    hint: 'Pictures and badges in READMEs and release notes are downloaded by RepoShelf (https, public hosts, up to 5 MB, images only) and kept in the cache, so they also show offline later. Off: a “Remote image” link instead.',
  },
  {
    key: 'fetchReleaseNotes',
    title: 'Fetch GitHub release notes',
    hint: 'On Clone, Import and Fetch of a github.com repository, ask api.github.com for its releases (notes and the list of files).',
  },
  {
    key: 'fetchRepoInfo',
    title: 'Fetch GitHub repository info',
    hint: 'Same moments: the description, website and topics from GitHub. Your own Edit About values always win.',
  },
  {
    key: 'autoDownloadLatestRelease',
    title: 'Download the latest release for this PC',
    hint: 'After Clone, Import and Fetch, download the file of the newest GitHub release made for Windows on this PC into the archive folder (_releases). Never run or unpacked. Each dialog and the Fetch menu can turn it off once.',
  },
  {
    key: 'autoDownloadOnFetchAll',
    title: 'Also download on “Fetch all”',
    hint: 'Off by default: Fetch all only fetches.',
  },
];

export const SIZE_CAP_MIN = 1;
export const SIZE_CAP_MAX = 16384;

/** Parse the size-cap field: whole MB, clamped like the backend; null when not a number. */
export function parseSizeCap(text: string): number | null {
  const t = text.trim();
  if (!/^\d+$/.test(t)) return null;
  return Math.min(SIZE_CAP_MAX, Math.max(SIZE_CAP_MIN, Number(t)));
}

/** S10 "Network" (v0.2.0, D-100): every network use besides Clone / Fetch is a switch here; Rust enforces each. */
function NetworkSettings() {
  const { settings } = useSettings();
  const [cap, setCap] = useState<string | null>(null);
  const shown = cap ?? String(settings.downloadSizeCapMb);
  const invalid = cap !== null && parseSizeCap(cap) === null;
  const commitCap = () => {
    if (cap === null) return;
    const v = parseSizeCap(cap);
    if (v === null) return;
    setCap(null);
    if (v !== settings.downloadSizeCapMb) void updateSettings({ downloadSizeCapMb: v });
  };
  return (
    <Box header={<h2 className="box__title">Network</h2>}>
      <div className="box__body stack" data-testid="network-settings">
        <p className="muted small">
          RepoShelf uses the network only for what is switched on here, besides Clone and Fetch themselves. Everything goes through the
          app (https only, no cookies or sign-in); the page you are looking at never connects anywhere.
        </p>
        {NET_TOGGLES.map((t) => (
          <label key={t.key} className="check-row">
            <input
              type="checkbox"
              name={t.key}
              checked={settings[t.key]}
              onChange={(e) => void updateSettings({ [t.key]: e.target.checked } as Partial<SettingsT>)}
            />
            <span>
              <strong>{t.title}</strong>
              <span className="muted small">{t.hint}</span>
            </span>
          </label>
        ))}
        <div className="field setting-cap">
          <label className="label" htmlFor="setting-cap">
            Largest release download
          </label>
          <div className="row">
            <input
              id="setting-cap"
              className="input setting-cap__input"
              inputMode="numeric"
              value={shown}
              aria-invalid={invalid}
              onChange={(e) => setCap(e.target.value)}
              onBlur={commitCap}
              onKeyDown={(e) => {
                if (e.key === 'Enter') commitCap();
                if (e.key === 'Escape') setCap(null);
              }}
            />
            <span className="muted">MB</span>
          </div>
          <p className={invalid ? 'field__error' : 'field__hint'}>
            {invalid
              ? `A whole number from ${SIZE_CAP_MIN} to ${formatCount(SIZE_CAP_MAX)}.`
              : 'Bigger files are not downloaded (checked before and while downloading). 1,024 MB by default.'}
          </p>
        </div>
      </div>
    </Box>
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
      resetRemoteImages();
      // the general cache includes the saved GitHub answers (D-118)
      purge(isExploreKey);
      void refreshExploreStatus();
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
            <span className="muted small">Removes stored commit counts, last-commit columns, language stats, GitHub notes and remote images. Repositories,
              GitHub repository info and downloaded release files stay.</span>
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

      <NetworkSettings />

      <ExploreSettings />

      <p className="muted small settings__version">
        RepoShelf {info.data ? <span className="mono">{info.data.version}</span> : '…'} · read-only GitHub-style browser for your local
        repositories
      </p>
    </main>
  );
}
