// Theme (FR-33): light / dark / system. `data-theme` on <html> always holds the resolved value ('light' | 'dark');
// "system" follows `prefers-color-scheme` live. The choice is persisted through `settings_set`; a localStorage hint
// only avoids a flash of the wrong theme before the settings arrive.

import type { Theme } from '../ipc/types';
import { settingsStore, updateSettings } from './settings';
import { createStore, useStore } from './store';

export type ResolvedTheme = 'light' | 'dark';
const HINT_KEY = 'reposhelf-theme-hint';

export function resolveTheme(t: Theme, prefersDark: boolean): ResolvedTheme {
  if (t === 'system') return prefersDark ? 'dark' : 'light';
  return t;
}

function media(): MediaQueryList | null {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-color-scheme: dark)')
    : null;
}

export const resolvedThemeStore = createStore<ResolvedTheme>('light');

function apply(t: Theme): void {
  const r = resolveTheme(t, media()?.matches ?? false);
  document.documentElement.dataset.theme = r;
  resolvedThemeStore.set(r);
}

function readHint(): Theme | null {
  try {
    const h = localStorage.getItem(HINT_KEY);
    return h === 'light' || h === 'dark' || h === 'system' ? h : null;
  } catch {
    return null;
  }
}

let started = false;
/** Call once before the first render (main.tsx). */
export function startTheme(): void {
  if (started) return;
  started = true;
  apply(readHint() ?? settingsStore.get().settings.theme);
  settingsStore.subscribe(() => {
    const s = settingsStore.get();
    if (!s.loaded) return;
    apply(s.settings.theme);
    try {
      localStorage.setItem(HINT_KEY, s.settings.theme);
    } catch {
      // ignore
    }
  });
  media()?.addEventListener('change', () => apply(settingsStore.get().settings.theme));
}

export function setTheme(t: Theme): Promise<void> {
  return updateSettings({ theme: t });
}

/** Cycle used by the header toggle: system → light → dark → system. */
export function nextTheme(t: Theme): Theme {
  return t === 'system' ? 'light' : t === 'light' ? 'dark' : 'system';
}

export function useResolvedTheme(): ResolvedTheme {
  return useStore(resolvedThemeStore);
}
