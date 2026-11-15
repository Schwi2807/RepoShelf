// Settings store (FR-3 sort/filter, FR-33 theme). Loaded once; every change goes through `settings_set`.

import { settingsGet, settingsSet } from '../ipc/api';
import { errorText } from '../ipc/errors';
import type { Settings } from '../ipc/types';
import { createStore, useStore } from './store';

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  sort: { key: 'updated', desc: true },
  sourceFilter: 'all',
  showDirtyNote: false,
  // v0.2.0 "Network" defaults (same as rs-core settings.rs)
  loadRemoteImages: true,
  fetchReleaseNotes: true,
  fetchRepoInfo: true,
  autoDownloadLatestRelease: true,
  autoDownloadOnFetchAll: false,
  downloadSizeCapMb: 1024,
  exploreEnabled: true,
  githubTokenSaved: false,
};

export interface SettingsState {
  settings: Settings;
  loaded: boolean;
  error: string | null;
}

export const settingsStore = createStore<SettingsState>({ settings: DEFAULT_SETTINGS, loaded: false, error: null });

let loading: Promise<void> | null = null;

export function loadSettings(): Promise<void> {
  loading ??= settingsGet().then(
    (s) => settingsStore.set({ settings: s, loaded: true, error: null }),
    (e: unknown) => settingsStore.set((p) => ({ ...p, loaded: true, error: errorText(e) })),
  );
  return loading;
}

/** Read the settings again (the output-only `githubTokenSaved` changes with the token commands, D-120). */
export async function refreshSettings(): Promise<void> {
  try {
    const s = await settingsGet();
    settingsStore.set((p) => ({ ...p, settings: s, error: null }));
  } catch (e) {
    settingsStore.set((p) => ({ ...p, error: errorText(e) }));
  }
}

/** Optimistic update, then persisted; on failure the old value comes back. */
export async function updateSettings(patch: Partial<Settings>): Promise<void> {
  const prev = settingsStore.get().settings;
  const next = { ...prev, ...patch };
  settingsStore.set((p) => ({ ...p, settings: next }));
  try {
    const saved = await settingsSet(next);
    settingsStore.set((p) => ({ ...p, settings: saved, error: null }));
  } catch (e) {
    settingsStore.set((p) => ({ ...p, settings: prev, error: errorText(e) }));
  }
}

export function useSettings(): SettingsState {
  return useStore(settingsStore);
}

/** Tests only. */
export function resetSettingsForTests(): void {
  loading = null;
  settingsStore.set({ settings: DEFAULT_SETTINGS, loaded: false, error: null });
}
