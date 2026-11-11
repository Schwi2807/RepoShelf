import { describe, expect, it } from 'vitest';
import type { CommitSummary, CommitsPage } from '../../ipc/types';
import { boot, click, go, installHarness, settle } from '../../test/appHarness';
import { groupCommits } from './DayGroup';
import { resetQueryCache } from '../../ipc/query';
import { COMMITS_PER_PAGE, commitsHref, pagerHrefs, readCommitsQuery, resetCommitsPageCache } from './paging';

installHarness();

const local = (y: number, m: number, d: number, h: number, min = 0) => Math.floor(new Date(y, m - 1, d, h, min).getTime() / 1000);

function commit(oid: string, secs: number, offsetMin = 0): CommitSummary {
  const s = { name: 'Avery Lindqvist', email: 'avery@example.org', time: { secs, offsetMin } };
  return { oid: oid.padEnd(40, '0'), summary: `c ${oid}`, author: s, committer: s, parents: [] };
}

const PARSEKIT_TIP_RE = /tip=[0-9a-f]{40}/;

describe('day groups (local calendar day of the commit time)', () => {
  it('splits at local midnight', () => {
    const g = groupCommits([commit('a', local(2026, 11, 12, 0, 30)), commit('b', local(2026, 11, 11, 23, 59)), commit('c', local(2026, 11, 11, 0, 1))]);
    expect(g.map((x) => [x.label, x.items.map((c) => c.oid[0])])).toEqual([
      ['Nov 12, 2026', ['a']],
      ['Nov 11, 2026', ['b', 'c']],
    ]);
  });

  it("ignores the commit's own UTC offset (grouped by the viewer's day, not the author's)", () => {
    const t = local(2026, 11, 11, 12, 0);
    const g = groupCommits([commit('a', t, 540), commit('b', t - 3600, -480), commit('c', t - 7200, 0)]);
    expect(g).toHaveLength(1);
    expect(g[0].items).toHaveLength(3);
  });

  it('a day interrupted by another day gets one section per run (walk order kept)', () => {
    const g = groupCommits([commit('a', local(2026, 11, 11, 9)), commit('b', local(2026, 11, 10, 9)), commit('c', local(2026, 11, 11, 8))]);
    expect(g.map((x) => x.label)).toEqual(['Nov 11, 2026', 'Nov 10, 2026', 'Nov 11, 2026']);
  });
});

describe('pager hrefs', () => {
  const page = (o: Partial<CommitsPage>): CommitsPage => ({ tip: 'f'.repeat(40), skip: 35, commits: [], hasMore: true, scanned: 35, capped: false, ...o });

  it('Older and Newer keep the pinned tip, path and since; page 1 has no page param', () => {
    const q = readCommitsQuery({ page: '2', tip: 'f'.repeat(40), path: 'src/lexer.rs', since: 'refs/tags/v0.2.0' });
    const h = pagerHrefs('northwind', 'parsekit', 'main', q, page({}));
    expect(h.newer).toBe(`#/northwind/parsekit/commits/main?path=src%2Flexer.rs&since=refs%2Ftags%2Fv0.2.0&tip=${'f'.repeat(40)}`);
    expect(h.older).toBe(`#/northwind/parsekit/commits/main?path=src%2Flexer.rs&since=refs%2Ftags%2Fv0.2.0&tip=${'f'.repeat(40)}&page=3`);
  });

  it('page 1 has no Newer; the last page has no Older unless the scan was capped', () => {
    const q = readCommitsQuery({});
    expect(pagerHrefs('o', 'r', 'main', q, page({ hasMore: false })).newer).toBeNull();
    expect(pagerHrefs('o', 'r', 'main', q, page({ hasMore: false })).older).toBeNull();
    expect(pagerHrefs('o', 'r', 'main', q, page({ hasMore: false, capped: true })).older).toContain('page=2');
  });

  it('reads the query defensively', () => {
    expect(readCommitsQuery({ page: '-3', tip: 'nothex' })).toEqual({ path: '', since: '', page: 1, tip: null });
    expect(readCommitsQuery({ page: '4', path: '/src/' }).path).toBe('src');
    expect(commitsHref('o', 'r', 'feature/x', { page: 1 })).toBe('#/o/r/commits/feature%2Fx');
  });
});

describe('Commits screen', () => {
  it('lists day groups and pages Older → Newer over one pinned tip', async () => {
    const { host } = await boot('', '#/northwind/parsekit/commits/main');
    await settle(400);
    const first = [...host.querySelectorAll<HTMLElement>('.crow')].map((r) => r.dataset.oid);
    expect(first).toHaveLength(COMMITS_PER_PAGE);
    expect(host.querySelector('.cgroup__title')?.textContent).toMatch(/^Commits on [A-Z][a-z]{2} \d{1,2}, \d{4}$/);
    expect(host.querySelector('a[rel="prev"]')).toBeNull();

    await click(host.querySelector('a[rel="next"]'));
    await settle(300);
    expect(location.hash).toMatch(PARSEKIT_TIP_RE);
    expect(location.hash).toContain('page=2');
    const second = [...host.querySelectorAll<HTMLElement>('.crow')].map((r) => r.dataset.oid);
    expect(second).toHaveLength(COMMITS_PER_PAGE);
    expect(second.some((o) => first.includes(o))).toBe(false);
    const newer = host.querySelector('a[rel="prev"]')!.getAttribute('href')!;
    expect(newer).toMatch(PARSEKIT_TIP_RE);
    expect(newer).not.toContain('page=');

    await click(host.querySelector('a[rel="prev"]'));
    await settle(200);
    expect([...host.querySelectorAll<HTMLElement>('.crow')].map((r) => r.dataset.oid)).toEqual(first);
  });

  it('a path-filtered page 2 loads directly (pages walked through the returned skip)', async () => {
    const { host } = await boot('?huge', '#/northwind/parsekit/commits/main?path=src/lexer.rs');
    await settle(400);
    const first = [...host.querySelectorAll<HTMLElement>('.crow')].map((r) => r.dataset.oid);
    const older = host.querySelector('a[rel="next"]')!.getAttribute('href')!;
    await click(host.querySelector('a[rel="next"]'));
    await settle(300);
    const second = [...host.querySelectorAll<HTMLElement>('.crow')].map((r) => r.dataset.oid);
    // the same URL with cold caches (as after a reload): page 1 is walked first to find page 2's start
    await go('#/', 50);
    resetCommitsPageCache();
    resetQueryCache();
    await go(older, 600);
    expect([...host.querySelectorAll<HTMLElement>('.crow')].map((r) => r.dataset.oid)).toEqual(second);
    expect(second.some((o) => first.includes(o))).toBe(false);
    expect(host.querySelector('[data-testid="path-chip"]')?.textContent).toContain('src/lexer.rs');
  });

  it('a capped scan offers "Search older commits"', async () => {
    const { host, backend } = await boot('', '#/');
    const orig = backend.invoke.bind(backend);
    (backend as { invoke: typeof backend.invoke }).invoke = (async (cmd: string, args: never) => {
      if (cmd === 'commits_page') return { tip: 'a'.repeat(40), skip: 20_000, commits: [], hasMore: false, scanned: 20_000, capped: true } as CommitsPage;
      return orig(cmd as never, args);
    }) as typeof backend.invoke;
    await go('#/northwind/parsekit/commits/main?path=docs/never.md', 400);
    expect(host.querySelector('.empty__title')?.textContent).toBe('No matching commits yet');
    const older = host.querySelector('a[rel="next"]');
    expect(older?.textContent).toBe('Search older commits');
    expect(older?.getAttribute('href')).toContain('page=2');
  });

  it('expanding a row loads the description; the since banner and path chip show', async () => {
    const { host } = await boot('', '#/lumen-ui/lumen/commits/refs%2Ftags%2Fv2.0.0-beta.1?since=refs%2Ftags%2Fv1.1.0');
    await settle(400);
    expect(host.querySelector('[data-testid="since-banner"]')?.textContent).toContain('since v1.1.0');
    expect(host.querySelectorAll('.crow').length).toBe(3);
    const more = host.querySelector<HTMLButtonElement>('.crow__more')!;
    await click(more);
    await settle(100);
    expect(more.getAttribute('aria-expanded')).toBe('true');
    expect(host.querySelector('.crow__body')).not.toBeNull();
  });
});
