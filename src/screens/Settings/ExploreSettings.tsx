// Settings → Explore (v0.3.0 M10b): the on/off switch, the optional GitHub token and "Clear Explore cache".
// The token field is UNCONTROLLED on purpose: the typed text lives only in the input element, never in React state, a
// URL or `history.state`; submit reads it, blanks the field first, and hands it to `explore_token_set` (which stores it
// in Windows Credential Manager and never returns it). Afterwards the page only knows "saved ✓" (from the status the
// backend answers) and offers Remove — the token cannot be shown again.

import { CircleCheck, KeyRound, Trash2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Box } from '../../components/layout/Sidebar';
import { Button } from '../../components/ui/Button';
import { exploreCacheClear, exploreTokenRemove, exploreTokenSet } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { isExploreKey, purge } from '../../ipc/query';
import { refreshExploreStatus, setExploreStatus, useExploreStatus } from '../../state/explore';
import { refreshSettings, updateSettings, useSettings } from '../../state/settings';
import { formatBytes, plural } from '../../util/format';

export function ExploreSettings() {
  const { settings } = useSettings();
  const status = useExploreStatus();
  const input = useRef<HTMLInputElement>(null);
  const [hasText, setHasText] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ text: string; error: boolean } | null>(null);
  const [cacheMsg, setCacheMsg] = useState<string | null>(null);
  useEffect(() => {
    void refreshExploreStatus();
  }, []);

  const tokenSaved = status?.tokenSaved ?? settings.githubTokenSaved;

  const save = async () => {
    const el = input.current;
    if (!el || busy) return;
    const value = el.value.trim();
    // blank the field before anything is awaited: the token must not stay in the page on success or on failure
    el.value = '';
    setHasText(false);
    if (value.length === 0) return;
    setBusy(true);
    setMsg(null);
    try {
      const st = await exploreTokenSet(value);
      setExploreStatus(st);
      purge(isExploreKey);
      await refreshSettings();
      setMsg({ text: 'Token saved in Windows Credential Manager.', error: false });
    } catch (e) {
      setMsg({ text: errorText(e), error: true });
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setMsg(null);
    try {
      setExploreStatus(await exploreTokenRemove());
      purge(isExploreKey);
      await refreshSettings();
      setMsg({ text: 'Token removed.', error: false });
    } catch (e) {
      setMsg({ text: errorText(e), error: true });
    } finally {
      setBusy(false);
    }
  };

  const clear = async () => {
    setBusy(true);
    setCacheMsg(null);
    try {
      const before = status?.cacheEntries ?? 0;
      setExploreStatus(await exploreCacheClear());
      purge(isExploreKey);
      setCacheMsg(`Cleared ${plural(before, 'saved answer')}.`);
    } catch (e) {
      setCacheMsg(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Box header={<h2 className="box__title">Explore</h2>}>
      <div className="box__body stack" data-testid="explore-settings">
        <label className="check-row">
          <input
            type="checkbox"
            name="exploreEnabled"
            data-testid="explore-enabled"
            checked={settings.exploreEnabled}
            onChange={(e) => void updateSettings({ exploreEnabled: e.target.checked })}
          />
          <span>
            <strong>Enable Explore</strong>
            <span className="muted small">
              Browse and search GitHub repositories (code, commits, branches, tags, releases) without cloning them. RepoShelf asks
              api.github.com for what you open; the page itself never connects anywhere. Off: the Explore entry disappears and nothing is
              requested.
            </span>
          </span>
        </label>

        <div className="field" data-testid="explore-token">
          <span className="label" id="explore-token-label">
            GitHub token (optional)
          </span>
          {tokenSaved ? (
            <div className="row row--wrap" data-testid="explore-token-saved">
              <span className="token-saved">
                <CircleCheck size={16} aria-hidden="true" /> Token saved ✓
              </span>
              <Button size="sm" icon={Trash2} disabled={busy} data-testid="explore-token-remove" onClick={() => void remove()}>
                Remove
              </Button>
            </div>
          ) : (
            <form
              className="row row--wrap"
              onSubmit={(e) => {
                e.preventDefault();
                void save();
              }}
            >
              <input
                ref={input}
                className="input token-input"
                type="password"
                aria-labelledby="explore-token-label"
                data-testid="explore-token-input"
                autoComplete="off"
                autoCorrect="off"
                autoCapitalize="off"
                spellCheck={false}
                placeholder="ghp_… or github_pat_…"
                onInput={(e) => setHasText(e.currentTarget.value.trim().length > 0)}
              />
              <Button type="submit" size="sm" icon={KeyRound} disabled={busy || !hasText} data-testid="explore-token-save">
                Save token
              </Button>
            </form>
          )}
          <p className="field__hint">
            Raises GitHub's limit from 60 to 5,000 requests an hour and lets you open private repositories your token can read. It is kept in
            Windows Credential Manager, sent only to api.github.com and never shown again. A token with no permissions is enough for public
            repositories.
          </p>
          {msg && (
            <p className={msg.error ? 'field__error' : 'muted small'} role={msg.error ? 'alert' : 'status'} data-testid="explore-token-msg">
              {msg.text}
            </p>
          )}
        </div>

        <div className="row row--wrap">
          <Button icon={Trash2} disabled={busy || (status !== null && status.cacheEntries === 0)} data-testid="explore-cache-clear" onClick={() => void clear()}>
            Clear Explore cache
          </Button>
          <span className="muted small" data-testid="explore-cache-size">
            {status ? `${plural(status.cacheEntries, 'saved answer')}, ${formatBytes(status.cacheBytes)}. ` : ''}
            Answers from GitHub are kept for reuse (up to 100 MB). Your token and your shelf are not touched.
          </span>
        </div>
        {cacheMsg && (
          <p className="muted small" role="status" data-testid="explore-cache-msg">
            {cacheMsg}
          </p>
        )}
      </div>
    </Box>
  );
}
