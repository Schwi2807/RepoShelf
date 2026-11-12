// Server data cache + `useQuery` (ARCH §7.3). In-memory LRU (200 entries) keyed by command+args.
// - `stable: true` entries (keyed by a resolved oid: tree at a commit, blobs, commit detail) never go stale.
// - Other entries (ref-keyed: refs, repo detail, library…) go stale on window focus and when a job finishes
//   (`invalidate()`); mounted queries then refetch while still showing the old data.
// - Cancellable queries get a fresh requestId per attempt; when the last user unmounts (route change), an in-flight
//   request is cancelled with `request_cancel`. A `cancelled` rejection is never cached or shown.
// StrictMode's mount → unmount → mount is absorbed: the cancel decision is deferred one microtask.

import { useCallback, useEffect, useReducer, useRef } from 'react';
import { newRequestId, requestCancel } from './api';
import { toIpcError } from './errors';
import type { IpcError } from './types';

export const CACHE_LIMIT = 200;

export interface QueryOptions {
  /** Pass a requestId and cancel on unmount. */
  cancelable?: boolean;
  /** Oid-keyed result: never invalidated. */
  stable?: boolean;
}

type Fetcher<T> = (requestId: string) => Promise<T>;

interface Entry {
  data?: unknown;
  hasData: boolean;
  error?: IpcError;
  stale: boolean;
  stable: boolean;
  inflight?: { requestId: string; cancelable: boolean };
  /** Invalidated while `inflight` ran: fetch once more when it ends. */
  again?: boolean;
}

interface Active {
  count: number;
  fn: Fetcher<unknown>;
  opts: QueryOptions;
}

const cache = new Map<string, Entry>();
const active = new Map<string, Active>();
const keyListeners = new Map<string, Set<() => void>>();

/** Stable cache key for a command + args. */
export function qk(cmd: string, ...args: unknown[]): string {
  return cmd + ':' + JSON.stringify(args);
}

function touch(key: string, e: Entry): void {
  cache.delete(key);
  cache.set(key, e);
  if (cache.size <= CACHE_LIMIT) return;
  for (const k of cache.keys()) {
    if (cache.size <= CACHE_LIMIT) break;
    if (active.has(k)) continue;
    cache.delete(k);
  }
}

function entryFor(key: string, stable: boolean): Entry {
  let e = cache.get(key);
  if (!e) {
    e = { hasData: false, stale: true, stable };
    touch(key, e);
  }
  return e;
}

function notify(key: string): void {
  const ls = keyListeners.get(key);
  if (ls) for (const l of [...ls]) l();
}

/**
 * `again`: an invalidation (job finished, focus, mutation). If a request is already running it may have read the
 * old state (M13: a fast release download finished while the list refetch from the fetch job was in flight, and
 * the page kept "Download"), so one more request runs after it.
 */
function start(key: string, a: Active, again = false): void {
  const e = entryFor(key, a.opts.stable === true);
  if (e.inflight) {
    if (again) e.again = true;
    return;
  }
  const requestId = newRequestId();
  const cancelable = a.opts.cancelable === true;
  e.inflight = { requestId, cancelable };
  let p: Promise<unknown>;
  try {
    p = a.fn(requestId);
  } catch (err) {
    p = Promise.reject(err);
  }
  p.then(
    (data) => {
      if (e.inflight?.requestId !== requestId) return;
      e.inflight = undefined;
      e.data = data;
      e.hasData = true;
      e.error = undefined;
      e.stale = false;
      if (cache.get(key) === e) touch(key, e);
      notify(key);
      rerun(key, e);
    },
    (err: unknown) => {
      if (e.inflight?.requestId !== requestId) return;
      e.inflight = undefined;
      const ie = toIpcError(err);
      e.stale = true;
      if (ie.code !== 'cancelled') e.error = ie;
      notify(key);
      rerun(key, e);
    },
  );
  notify(key);
}

/** The pending "again" of `start`: refetch now if still mounted, else just leave the entry stale. */
function rerun(key: string, e: Entry): void {
  if (!e.again) return;
  e.again = false;
  if (e.stable) return;
  e.stale = true;
  const a = active.get(key);
  if (a && cache.get(key) === e) start(key, a);
}

function acquire(key: string, fn: Fetcher<unknown>, opts: QueryOptions): void {
  let a = active.get(key);
  if (a) {
    a.count += 1;
    a.fn = fn;
  } else {
    a = { count: 1, fn, opts };
    active.set(key, a);
  }
  const e = entryFor(key, opts.stable === true);
  if (e.stale || (!e.hasData && !e.error)) start(key, a);
}

function release(key: string): void {
  const a = active.get(key);
  if (!a) return;
  a.count -= 1;
  if (a.count > 0) return;
  queueMicrotask(() => {
    const now = active.get(key);
    if (!now || now.count > 0) return;
    active.delete(key);
    const e = cache.get(key);
    if (e?.inflight && e.inflight.cancelable) {
      const id = e.inflight.requestId;
      e.inflight = undefined;
      e.stale = true;
      requestCancel(id).catch(() => undefined);
    }
  });
}

/** Mark every non-stable entry stale and refetch the mounted ones (window focus, job finished). */
export function invalidate(prefix?: string): void {
  for (const [key, e] of cache) {
    if (e.stable) continue;
    if (prefix !== undefined && !key.startsWith(prefix)) continue;
    e.stale = true;
    const a = active.get(key);
    if (a) start(key, a, true);
  }
}

/** Drop one entry (after a mutation such as Edit About). Mounted users refetch. */
export function evict(key: string): void {
  const e = cache.get(key);
  if (!e) return;
  e.stale = true;
  const a = active.get(key);
  if (a) start(key, a, true);
  else cache.delete(key);
}

/** Put a value into the cache (e.g. a mutation's response). */
export function prime<T>(key: string, data: T, stable = false): void {
  const e = entryFor(key, stable);
  e.data = data;
  e.hasData = true;
  e.error = undefined;
  e.stale = false;
  touch(key, e);
  notify(key);
}

/** Tests only. */
export function resetQueryCache(): void {
  cache.clear();
  active.clear();
  keyListeners.clear();
}

export function cacheSize(): number {
  return cache.size;
}

let focusInstalled = false;
/** Window focus → invalidate ref-keyed entries (installed once from main.tsx). */
export function startQueryInvalidation(): void {
  if (focusInstalled) return;
  focusInstalled = true;
  window.addEventListener('focus', () => invalidate());
}

export interface QueryState<T> {
  data: T | undefined;
  error: IpcError | undefined;
  /** No data yet and a request is (or will be) running. */
  loading: boolean;
  /** A request is running (also while showing older data). */
  fetching: boolean;
  refetch: () => void;
}

/**
 * Cached query. `key` null = disabled (e.g. waiting for a repoId). `fn` receives a requestId (use it only for the
 * cancellable commands).
 */
export function useQuery<T>(key: string | null, fn: Fetcher<T>, opts: QueryOptions = {}): QueryState<T> {
  const [, force] = useReducer((x: number) => x + 1, 0);
  const fnRef = useRef(fn);
  fnRef.current = fn;
  const cancelable = opts.cancelable === true;
  const stable = opts.stable === true;

  useEffect(() => {
    if (key === null) return;
    let ls = keyListeners.get(key);
    if (!ls) {
      ls = new Set();
      keyListeners.set(key, ls);
    }
    ls.add(force);
    acquire(key, (id) => fnRef.current(id), { cancelable, stable });
    force();
    return () => {
      ls.delete(force);
      if (ls.size === 0) keyListeners.delete(key);
      release(key);
    };
  }, [key, cancelable, stable]);

  const refetch = useCallback(() => {
    if (key === null) return;
    const a = active.get(key);
    const e = cache.get(key);
    if (e && !e.stable) e.stale = true;
    if (a) start(key, a, true);
  }, [key]);

  const e = key === null ? undefined : cache.get(key);
  return {
    data: e?.hasData ? (e.data as T) : undefined,
    error: e?.error,
    loading: key !== null && !e?.hasData && !e?.error,
    fetching: e?.inflight !== undefined,
    refetch,
  };
}
