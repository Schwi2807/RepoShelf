// Mock switches (FR-37, ARCH §7.4), read from `location.search` at start: `/?slow&huge#/…`.

export interface Switches {
  /** No repos at all (dashboard empty state). */
  empty: boolean;
  /** 600 ms delay on every call (shimmers visible). */
  slow: boolean;
  /** Every clone job fails. */
  clonefail: boolean;
  /** Every fetch job fails. */
  fetchfail: boolean;
  /** 1,000-entry dir, 10k-commit history, 400-file commit. */
  huge: boolean;
  /** v0.2.0: GitHub release lists already cached for github.com repos (assets visible without "Load"). */
  gh: boolean;
  /** v0.3.0: Explore starts with 2 requests left (rate-limit state without 60 clicks). */
  ratelimit: boolean;
}

export const SLOW_MS = 600;

export function readSwitches(search: string): Switches {
  const p = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);
  const on = (k: string): boolean => p.has(k) && p.get(k) !== '0' && p.get(k) !== 'false';
  return {
    empty: on('empty'),
    slow: on('slow'),
    clonefail: on('clonefail'),
    fetchfail: on('fetchfail'),
    huge: on('huge'),
    gh: on('gh'),
    ratelimit: on('ratelimit'),
  };
}
