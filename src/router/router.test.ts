import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  current,
  href,
  locationText,
  markRendered,
  navigate,
  parse,
  pendingScroll,
  startRouter,
  stopRouter,
  type Route,
} from './router';
import type { RouteName, RouteParams } from './routes';

type Case = [Exclude<RouteName, 'notFound'>, RouteParams, Record<string, string>];

const CASES: Case[] = [
  ['dashboard', {}, {}],
  ['dashboard', {}, { q: 'pars' }],
  ['settings', {}, {}],
  ['code', { owner: 'acme', repo: 'widget' }, {}],
  ['tree', { owner: 'acme', repo: 'widget', ref: 'main', path: '' }, {}],
  ['tree', { owner: 'acme', repo: 'widget', ref: 'feature/x', path: 'src/deep/dir' }, {}],
  ['tree', { owner: 'acme', repo: 'widget', ref: 'refs/tags/v1.0', path: 'a b/c#d.txt' }, {}],
  ['blob', { owner: 'acme', repo: 'widget', ref: 'main', path: 'src/main.rs' }, { L: '10-20' }],
  ['blob', { owner: 'acme', repo: 'widget', ref: 'v2', path: 'README.md' }, { plain: '1', view: 'code' }],
  ['commits', { owner: 'local', repo: 'notes-2', ref: 'main' }, { path: 'src/a.rs', page: '2', tip: 'abc' }],
  ['commit', { owner: 'acme', repo: 'widget', oid: '0123456789abcdef0123456789abcdef01234567' }, { file: '3' }],
  ['branches', { owner: 'acme', repo: 'widget' }, {}],
  ['tags', { owner: 'acme', repo: 'widget' }, {}],
  ['releases', { owner: 'acme', repo: 'widget' }, {}],
];

describe('parse / href', () => {
  it.each(CASES)('round-trips %s %j %j', (name, params, query) => {
    const h = href(name, params, query);
    const r = parse(h);
    expect(r.name).toBe(name);
    const expected: RouteParams = { ...params };
    if ((name === 'tree' || name === 'blob') && expected.path === undefined) expected.path = '';
    expect(r.params).toEqual(expected);
    expect(r.query).toEqual(query);
    expect(href(name, r.params, r.query)).toBe(h);
  });

  it('encodes a ref with a slash as one segment (D-7)', () => {
    const h = href('tree', { owner: 'o', repo: 'r', ref: 'feature/x', path: 'src' });
    expect(h).toBe('#/o/r/tree/feature%2Fx/src');
    expect(parse('#/o/r/tree/feature%2Fx/src').params.ref).toBe('feature/x');
  });

  it('keeps ?L= in the query (D-8)', () => {
    const r = parse('#/o/r/blob/main/src/a.rs?L=10-20');
    expect(r.name).toBe('blob');
    expect(r.params.path).toBe('src/a.rs');
    expect(r.query.L).toBe('10-20');
  });

  it('tells #/settings from #/:owner/:repo', () => {
    expect(parse('#/settings').name).toBe('settings');
    expect(parse('#/settings/x').name).toBe('code');
    expect(parse('#/settings/x').params).toEqual({ owner: 'settings', repo: 'x' });
  });

  it('maps empty hash to the dashboard', () => {
    expect(parse('').name).toBe('dashboard');
    expect(parse('#').name).toBe('dashboard');
    expect(parse('#/').name).toBe('dashboard');
  });

  it.each([
    '#/o/r/unknown',
    '#/o/r/tree',
    '#/o/r/blob',
    '#/o/r/commits',
    '#/o/r/commit',
    '#/o/r/branches/x',
    '#/o/r/tree/%E0%A4%A/x',
    '#user-content-install',
    '#/only',
  ])('unknown or malformed %s → notFound', (h) => {
    expect(parse(h).name).toBe('notFound');
  });

  it('drops empty query values', () => {
    expect(href('dashboard', {}, { q: '', x: undefined, y: null, z: false })).toBe('#/');
    expect(href('blob', { owner: 'o', repo: 'r', ref: 'm', path: 'a' }, { plain: true })).toBe('#/o/r/blob/m/a?plain=1');
  });
});

describe('locationText (copy location)', () => {
  const r = (h: string): Route => parse(h);
  it('is decoded and GitHub-style', () => {
    expect(locationText(r('#/o/r/tree/feature%2Fx/src/a%20b'))).toBe('o/r/tree/feature/x/src/a b');
    expect(locationText(r('#/o/r/tree/main'))).toBe('o/r/tree/main');
    expect(locationText(r('#/o/r/blob/main/src/a.rs?L=10-20'))).toBe('o/r/blob/main/src/a.rs#L10-L20');
    expect(locationText(r('#/o/r/blob/main/src/a.rs?L=7'))).toBe('o/r/blob/main/src/a.rs#L7');
    expect(locationText(r('#/o/r'))).toBe('o/r');
  });
});

describe('history + scroll memory (FR-34)', () => {
  beforeEach(() => {
    history.replaceState(null, '', '/#/');
    startRouter();
  });
  afterEach(() => {
    stopRouter();
    vi.restoreAllMocks();
  });

  it('navigate pushes, keeps location.search, and updates current()', () => {
    history.replaceState(history.state, '', '/?slow#/');
    navigate('#/o/r');
    expect(location.search).toBe('?slow');
    expect(current().name).toBe('code');
    expect(typeof (history.state as { key: string }).key).toBe('string');
  });

  it('restores scroll on back only after the route rendered', async () => {
    navigate('#/o/r/tree/main/src');
    window.scrollTo(0, 640);
    window.dispatchEvent(new Event('scroll'));
    navigate('#/o/r/blob/main/src/a.rs');
    expect(pendingScroll()).toBe(0);
    markRendered();

    const spy = vi.spyOn(window, 'scrollTo');
    const popped = new Promise<void>((res) => window.addEventListener('popstate', () => res(), { once: true }));
    history.back();
    await popped;
    expect(current().name).toBe('tree');
    expect(pendingScroll()).toBe(640);
    expect(spy).not.toHaveBeenCalled();
    markRendered();
    expect(spy).toHaveBeenCalledWith(0, 640);
    expect(pendingScroll()).toBeNull();
  });

  it('Alt+Left goes back, mouse button 4 goes forward', async () => {
    navigate('#/o/r/branches');
    const backSpy = vi.spyOn(history, 'back');
    const fwdSpy = vi.spyOn(history, 'forward');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', altKey: true }));
    window.dispatchEvent(new MouseEvent('mouseup', { button: 4 }));
    window.dispatchEvent(new MouseEvent('mouseup', { button: 3 }));
    expect(backSpy).toHaveBeenCalledTimes(2);
    expect(fwdSpy).toHaveBeenCalledTimes(1);
  });
});
