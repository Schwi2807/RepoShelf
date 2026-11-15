// Route patterns (ARCH §7.1). Hash routes; `:ref` is ONE percent-encoded segment (D-7), `*path` is the rest of the
// segments (each encoded on its own), line anchors live in the query (`?L=10-20`, D-8).

export type RouteName =
  | 'dashboard'
  | 'settings'
  | 'code'
  | 'tree'
  | 'blob'
  | 'commits'
  | 'commit'
  | 'branches'
  | 'tags'
  | 'releases'
  | 'exploreSearch'
  | 'notFound';

export interface RouteParams {
  /** v0.3.0 (D-127): the route is the same screen served from GitHub (`#/explore/<owner>/<repo>/…`). */
  explore?: boolean;
  owner?: string;
  repo?: string;
  ref?: string;
  path?: string;
  oid?: string;
}

export type Query = Record<string, string>;

export interface Route {
  name: RouteName;
  params: RouteParams;
  query: Query;
}

/** Pattern table. Order matters: the first match wins (`settings` before `:owner/:repo`). */
export const PATTERNS: ReadonlyArray<readonly [Exclude<RouteName, 'notFound'>, string]> = [
  ['dashboard', '/'],
  ['settings', '/settings'],
  ['code', '/:owner/:repo'],
  ['tree', '/:owner/:repo/tree/:ref/*path'],
  ['blob', '/:owner/:repo/blob/:ref/*path'],
  ['commits', '/:owner/:repo/commits/:ref'],
  ['commit', '/:owner/:repo/commit/:oid'],
  ['branches', '/:owner/:repo/branches'],
  ['tags', '/:owner/:repo/tags'],
  ['releases', '/:owner/:repo/releases'],
];

/** The Explore search page (`#/explore`, `#/explore/search?q=`); matched before the repo patterns (D-127). */
export const EXPLORE_SEARCH_PATH = '/explore/search';

export const REPO_ROUTES: ReadonlySet<RouteName> = new Set([
  'code',
  'tree',
  'blob',
  'commits',
  'commit',
  'branches',
  'tags',
  'releases',
]);

function splitPattern(p: string): string[] {
  return p.split('/').filter((s) => s.length > 0);
}

const COMPILED = PATTERNS.map(([name, p]) => [name, splitPattern(p)] as const);

/** Match decoded segments against the pattern table. `explore/…` is the same table served from GitHub (D-127). */
export function matchSegments(segs: string[]): { name: RouteName; params: RouteParams } {
  if (segs[0] === 'explore') {
    // `explore` and `explore/search` are the search page; `explore/search/x` is a repository called x of the owner
    // "search". GitHub reserves `explore` as a user name, so no GitHub repository is shadowed.
    if (segs.length === 1 || (segs.length === 2 && segs[1] === 'search')) return { name: 'exploreSearch', params: { explore: true } };
    const m = matchRepo(segs.slice(1), false);
    return m.name === 'notFound' ? m : { name: m.name, params: { ...m.params, explore: true } };
  }
  return matchRepo(segs, true);
}

function matchRepo(segs: string[], withApp: boolean): { name: RouteName; params: RouteParams } {
  for (const [name, pat] of COMPILED) {
    if (!withApp && (name === 'dashboard' || name === 'settings')) continue;
    const params: Record<string, string> = {};
    let ok = true;
    let i = 0;
    for (; i < pat.length; i++) {
      const p = pat[i];
      if (p.startsWith('*')) {
        params[p.slice(1)] = segs.slice(i).join('/');
        i = segs.length;
        break;
      }
      const s = segs[i];
      if (s === undefined || s.length === 0) {
        ok = false;
        break;
      }
      if (p.startsWith(':')) params[p.slice(1)] = s;
      else if (p !== s) {
        ok = false;
        break;
      }
    }
    if (ok && i === segs.length) return { name, params };
  }
  return { name: 'notFound', params: {} };
}

/** Build the hash path for a route (without the leading `#`). Throws on a missing param. */
export function buildPath(name: Exclude<RouteName, 'notFound'>, params: RouteParams): string {
  if (name === 'exploreSearch') return EXPLORE_SEARCH_PATH;
  const pat = PATTERNS.find(([n]) => n === name)?.[1];
  if (pat === undefined) throw new Error(`unknown route ${name}`);
  const out: string[] = params.explore === true ? ['explore'] : [];
  for (const p of splitPattern(pat)) {
    if (p.startsWith('*')) {
      const v = (params[p.slice(1) as keyof RouteParams] as string | undefined) ?? '';
      for (const s of v.split('/')) if (s.length > 0) out.push(encodeURIComponent(s));
    } else if (p.startsWith(':')) {
      const v = params[p.slice(1) as keyof RouteParams] as string | undefined;
      if (v === undefined || v.length === 0) throw new Error(`route ${name}: missing :${p.slice(1)}`);
      out.push(encodeURIComponent(v));
    } else {
      out.push(p);
    }
  }
  return '/' + out.join('/');
}
