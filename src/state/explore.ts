// Explore status store (v0.3.0 M10b): the quota header and the Settings section read it. The backend does not push
// it (M10a gotcha), so every Explore call asks `explore_status` again once it settled (debounced, successes and
// failures alike, so a rate-limit rejection shows up in the header at once). The status never contains the token.

import { exploreStatus } from '../ipc/api';
import type { ExploreStatus } from '../ipc/types';
import { createStore, useStore } from './store';

export const exploreStore = createStore<ExploreStatus | null>(null);

let inflight: Promise<void> | null = null;
let queued = false;
let timer: ReturnType<typeof setTimeout> | null = null;
export const STATUS_DEBOUNCE_MS = 120;

/** Read the status now (coalesced: calls during a read share the next one). */
export function refreshExploreStatus(): Promise<void> {
  if (inflight) {
    queued = true;
    return inflight;
  }
  inflight = exploreStatus()
    .then(
      (s) => exploreStore.set(s),
      () => undefined,
    )
    .finally(() => {
      inflight = null;
      if (queued) {
        queued = false;
        void refreshExploreStatus();
      }
    });
  return inflight;
}

/** An Explore call settled: refresh the status shortly (many calls of one page become one read). */
export function noteExploreCall(): void {
  if (timer !== null) return;
  timer = setTimeout(() => {
    timer = null;
    void refreshExploreStatus();
  }, STATUS_DEBOUNCE_MS);
}

/** A command that returns the status (token set / remove, clear cache) publishes it directly. */
export function setExploreStatus(s: ExploreStatus): void {
  exploreStore.set(s);
}

export function useExploreStatus(): ExploreStatus | null {
  return useStore(exploreStore);
}

/** Tests only. */
export function resetExploreForTests(): void {
  if (timer !== null) clearTimeout(timer);
  timer = null;
  inflight = null;
  queued = false;
  exploreStore.set(null);
}
