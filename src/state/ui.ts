// UI-only global state: which app-level dialog is open (Import / Clone, opened from the "+" menu and empty states).

import { createStore, useStore } from './store';

export type AppDialog = 'import' | 'clone' | null;

export const dialogStore = createStore<AppDialog>(null);

export function openDialog(d: Exclude<AppDialog, null>): void {
  dialogStore.set(d);
}

export function closeDialog(): void {
  dialogStore.set(null);
}

export function useAppDialog(): AppDialog {
  return useStore(dialogStore);
}
