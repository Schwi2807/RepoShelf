// Hand-written hash router (D-6). History entries carry a key in `history.state`; a Map<key, scrollY> restores the
// scroll position on back/forward (FR-34). Mouse buttons 3/4 and Alt+Left/Right go back/forward.
// The page's `location.search` holds the mock switches (`/?slow#/…`) and is kept on every navigation; the route query
// lives inside the hash (`#/…?L=10-20`).

import { buildPath, matchSegments, type Query, type Route, type RouteName, type RouteParams } from './routes';

export type { Query, Route, RouteName, RouteParams } from './routes';

const NOT_FOUND: Route = { name: 'notFound', params: {}, query: {} };

/** Parse a location hash (`#/owner/repo/tree/main/src?L=1`) into a route. Unknown or malformed → notFound. */
export function parse(hash: string): Route {
  let h = hash.startsWith('#') ? hash.slice(1) : hash;
  if (h.length === 0) h = '/';
  if (!h.startsWith('/')) return NOT_FOUND;
  const qi = h.indexOf('?');
  const pathPart = qi >= 0 ? h.slice(0, qi) : h;
  const queryPart = qi >= 0 ? h.slice(qi + 1) : '';
  let segs: string[];
  try {
    segs = pathPart
      .split('/')
      .filter((s) => s.length > 0)
      .map((s) => decodeURIComponent(s));
  } catch {
    return NOT_FOUND;
  }
  const { name, params } = matchSegments(segs);
  if (name === 'notFound') return NOT_FOUND;
  const query: Query = {};
  for (const [k, v] of new URLSearchParams(queryPart)) query[k] = v;
  return { name, params, query };
}

export type QueryInput = Record<string, string | number | boolean | null | undefined>;

/** Build an in-app href (`#/…`). Empty / null / undefined / false query values are left out. */
export function href(name: Exclude<RouteName, 'notFound'>, params: RouteParams = {}, query: QueryInput = {}): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(query)) {
    if (v === undefined || v === null || v === false || v === '') continue;
    qs.set(k, v === true ? '1' : String(v));
  }
  const q = qs.toString();
  return '#' + buildPath(name, params) + (q.length > 0 ? '?' + q : '');
}

/** Same route, other query (used for `?L=`, `?page=`, filters). */
export function withQuery(route: Route, query: QueryInput): string {
  if (route.name === 'notFound') return '#/';
  return href(route.name, route.params, query);
}

// ---------------------------------------------------------------------------------------------------------------
// History, listeners, scroll memory

type Listener = () => void;
const listeners = new Set<Listener>();
const scrollMemory = new Map<string, number>();
let currentKey = '';
let pendingRestore: number | null = null;
let restoreGen = 0;
let started = false;
let keySeq = 0;

function newKey(): string {
  keySeq += 1;
  return `${Date.now().toString(36)}-${keySeq}`;
}

function stateKey(): string | null {
  const s: unknown = history.state;
  if (s !== null && typeof s === 'object' && typeof (s as { key?: unknown }).key === 'string') {
    return (s as { key: string }).key;
  }
  return null;
}

function ensureKey(): void {
  const k = stateKey();
  if (k !== null) {
    currentKey = k;
    return;
  }
  currentKey = newKey();
  history.replaceState({ key: currentKey }, '');
}

function emit(): void {
  for (const l of [...listeners]) l();
}

function onScroll(): void {
  if (currentKey) scrollMemory.set(currentKey, window.scrollY);
}

function onPopState(): void {
  ensureKey();
  pendingRestore = scrollMemory.get(currentKey) ?? 0;
  restoreGen += 1;
  emit();
}

function onHashChange(): void {
  // A plain `<a href="#/…">` or a typed URL: new entry without our key.
  if (stateKey() === null) {
    ensureKey();
    pendingRestore = 0;
    restoreGen += 1;
  }
  emit();
}

function onMouseUp(e: MouseEvent): void {
  if (e.button === 3) {
    e.preventDefault();
    history.back();
  } else if (e.button === 4) {
    e.preventDefault();
    history.forward();
  }
}

function onKeyDown(e: KeyboardEvent): void {
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
  if (e.key === 'ArrowLeft') {
    e.preventDefault();
    history.back();
  } else if (e.key === 'ArrowRight') {
    e.preventDefault();
    history.forward();
  }
}

/** Install the global listeners once (main.tsx; tests call it too). */
export function startRouter(): void {
  if (started) return;
  started = true;
  if ('scrollRestoration' in history) history.scrollRestoration = 'manual';
  ensureKey();
  window.addEventListener('popstate', onPopState);
  window.addEventListener('hashchange', onHashChange);
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('mouseup', onMouseUp);
  window.addEventListener('keydown', onKeyDown);
}

/** Test helper: remove listeners and forget all state. */
export function stopRouter(): void {
  if (!started) return;
  started = false;
  window.removeEventListener('popstate', onPopState);
  window.removeEventListener('hashchange', onHashChange);
  window.removeEventListener('scroll', onScroll);
  window.removeEventListener('mouseup', onMouseUp);
  window.removeEventListener('keydown', onKeyDown);
  scrollMemory.clear();
  pendingRestore = null;
  currentKey = '';
}

export function subscribe(l: Listener): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

/** Navigate to an in-app hash (`#/…`). Keeps `location.search` (mock switches). */
export function navigate(to: string, opts: { replace?: boolean } = {}): void {
  const hash = to.startsWith('#') ? to : '#' + to;
  if (hash === location.hash && !opts.replace) return;
  scrollMemory.set(currentKey, window.scrollY);
  const key = newKey();
  const url = location.pathname + location.search + hash;
  if (opts.replace) history.replaceState({ key }, '', url);
  else history.pushState({ key }, '', url);
  currentKey = key;
  pendingRestore = 0;
  restoreGen += 1;
  emit();
}

export function back(): void {
  history.back();
}

let cached: { hash: string; route: Route } | null = null;

/** Current route (stable object per hash, for useSyncExternalStore). */
export function current(): Route {
  const h = location.hash;
  if (cached === null || cached.hash !== h) cached = { hash: h, route: parse(h) };
  return cached.route;
}

/** Scroll position waiting to be applied after the next render (null = none). Exposed for tests. */
export function pendingScroll(): number | null {
  return pendingRestore;
}

/**
 * Called by RouteView after a route rendered (layout effect). Applies the pending scroll position; content that
 * arrives asynchronously may still be too short, so it retries each frame for up to 2 s until the position sticks.
 */
export function markRendered(): void {
  if (pendingRestore === null) return;
  const target = pendingRestore;
  pendingRestore = null;
  const gen = restoreGen;
  const t0 = Date.now();
  const attempt = (): void => {
    if (gen !== restoreGen) return;
    window.scrollTo(0, target);
    if (Math.abs(window.scrollY - target) > 1 && Date.now() - t0 < 2000) requestAnimationFrame(attempt);
  };
  attempt();
}

// ---------------------------------------------------------------------------------------------------------------
// "Copy location" (FR-34): decoded GitHub-style text.

export function locationText(route: Route): string {
  const { owner = '', repo = '', ref = '', path = '', oid = '' } = route.params;
  const base = `${owner}/${repo}`;
  const withPath = (kind: string): string => `${base}/${kind}/${ref}${path ? '/' + path : ''}`;
  switch (route.name) {
    case 'dashboard':
      return '/';
    case 'settings':
      return 'settings';
    case 'notFound':
      return '';
    case 'code':
      return base;
    case 'tree':
      return withPath('tree');
    case 'blob': {
      const l = route.query.L;
      let frag = '';
      if (l) {
        const [a, b] = l.split('-');
        frag = b && b !== a ? `#L${a}-L${b}` : `#L${a}`;
      }
      return withPath('blob') + frag;
    }
    case 'commits':
      return `${base}/commits/${ref}${route.query.path ? '/' + route.query.path : ''}`;
    case 'commit':
      return `${base}/commit/${oid}`;
    case 'branches':
    case 'tags':
    case 'releases':
      return `${base}/${route.name}`;
  }
}

export async function copyLocation(route: Route = current()): Promise<string> {
  const text = locationText(route);
  await navigator.clipboard.writeText(text);
  return text;
}
