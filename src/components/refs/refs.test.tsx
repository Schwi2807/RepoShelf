import { describe, expect, it } from 'vitest';
import { boot, click, go, installHarness, settle, typeInto } from '../../test/appHarness';
import { barWidths, countLabel } from './AheadBehindBar';

installHarness();

describe('ahead/behind bar widths', () => {
  it('scale to the page maximum, ahead right and behind left', () => {
    expect(barWidths({ ahead: 3, behind: 30, capped: false }, 30)).toEqual({ behind: 100, ahead: 10 });
    expect(barWidths({ ahead: 0, behind: 0, capped: false }, 30)).toEqual({ behind: 0, ahead: 0 });
    expect(barWidths({ ahead: 6, behind: 0, capped: false }, 12)).toEqual({ behind: 0, ahead: 50 });
  });

  it('keep tiny counts visible and never exceed the half', () => {
    expect(barWidths({ ahead: 1, behind: 0, capped: false }, 1000)).toEqual({ behind: 0, ahead: 3 });
    // a row larger than the max seen so far is clamped by its own size
    expect(barWidths({ ahead: 50, behind: 10, capped: false }, 20)).toEqual({ behind: 20, ahead: 100 });
  });

  it('capped counts fill the bar and read "1,000+"', () => {
    expect(barWidths({ ahead: 1, behind: 1000, capped: true }, 1000)).toEqual({ behind: 100, ahead: 3 });
    expect(countLabel(1000, true)).toBe('1,000+');
    expect(countLabel(12, false)).toBe('12');
  });
});

describe('Branches page', () => {
  it('sections, lazy bars against HEAD, filter via ?q=', async () => {
    const { host } = await boot('', '#/northwind/parsekit/branches');
    await settle(500);
    expect([...host.querySelectorAll('.refsec .box__title')].map((h) => h.textContent)).toEqual(['Default', 'Your branches', 'Remote branches']);
    const def = host.querySelector('[data-section="default"] .brow')!;
    expect(def.textContent).toContain('main');
    expect(def.textContent).toContain('Compared branch');
    const tut = host.querySelector('[data-branch="docs/tutorial"]')!;
    expect(tut.querySelector('.abbar__num--behind')?.textContent).toBe('30');
    expect(tut.querySelector('.abbar__num--ahead')?.textContent).toBe('1');
    expect(tut.querySelector<HTMLElement>('.abbar__fill--behind')!.style.width).toBe('100%');
    expect(tut.querySelector('.brow__meta')?.textContent).toMatch(/^Updated .+ by .+$/);
    await typeInto(host.querySelector('input[aria-label="Filter branches"]'), 'stream');
    await settle(100);
    expect(location.hash).toContain('q=stream');
    expect([...host.querySelectorAll('.brow__name')].map((a) => a.textContent)).toEqual(['feature/streaming', 'origin/feature/streaming']);
  });

  it('detached HEAD: no counts are requested (the backend refuses), rows show a dash', async () => {
    const { host, backend } = await boot('', '#/');
    const calls: string[] = [];
    const orig = backend.invoke.bind(backend);
    (backend as { invoke: typeof backend.invoke }).invoke = ((cmd: string, args: never) => {
      calls.push(cmd);
      return orig(cmd as never, args);
    }) as typeof backend.invoke;
    await go('#/local/beacon/branches', 400);
    expect(host.querySelector('[data-testid="detached-note"]')).not.toBeNull();
    expect(host.querySelector('.abbar')).toBeNull();
    expect(calls).not.toContain('branches_ahead_behind');
    expect(host.querySelector('.brow--error')).toBeNull();
  });

  it('?huge: a capped count reads "1,000+"', async () => {
    const { host } = await boot('?huge', '#/northwind/parsekit/branches');
    await settle(1500);
    const tut = host.querySelector('[data-branch="docs/tutorial"] .abbar:not(.abbar--loading)')!;
    expect(tut.getAttribute('data-capped')).toBe('true');
    expect(tut.querySelector('.abbar__num--behind')?.textContent).toBe('1,000+');
  });
});

describe('Tags and Releases', () => {
  it('tags newest first with annotated / lightweight badges and links', async () => {
    const { host } = await boot('', '#/northwind/parsekit/tags');
    await settle(300);
    const rows = [...host.querySelectorAll<HTMLElement>('.trow')];
    expect(rows.map((r) => r.dataset.tag)).toEqual(['v0.3.0', 'v0.2.0', 'v0.1.0', 'experiment-1']);
    expect(rows.map((r) => r.querySelector('.badge')?.textContent)).toEqual(['annotated', 'annotated', 'lightweight', 'lightweight']);
    expect(rows[0].querySelector('.trow__name')?.getAttribute('href')).toBe('#/northwind/parsekit/tree/refs%2Ftags%2Fv0.3.0');
    expect(rows[0].querySelector('.trow__oid')?.getAttribute('href')).toMatch(/^#\/northwind\/parsekit\/commit\/[0-9a-f]{40}$/);
    expect(host.querySelector('.segmented [aria-current="page"]')?.textContent).toBe('Tags 4');
  });

  it('release cards: Latest / Pre-release, notes, compare link with full tag refs', async () => {
    const { host } = await boot('', '#/lumen-ui/lumen/releases');
    await settle(300);
    const cards = [...host.querySelectorAll<HTMLElement>('.rcard')];
    expect(cards.map((c) => c.dataset.tag)).toEqual(['v2.0.0-beta.1', 'v1.1.0', 'v1.0.0', 'v0.9.0']);
    expect(cards[0].textContent).toContain('Pre-release');
    expect(cards[1].textContent).toContain('Latest');
    expect(cards[0].querySelector('.rcard__notes')?.textContent).toContain('First beta of Lumen 2');
    expect(cards[0].querySelector('.rcard__compare')?.getAttribute('href')).toBe(
      '#/lumen-ui/lumen/commits/refs%2Ftags%2Fv2.0.0-beta.1?since=refs%2Ftags%2Fv1.1.0',
    );
    expect(cards[3].querySelector('.rcard__compare')).toBeNull();
    expect(cards[3].textContent).toContain('No release notes');
  });

  it('GitHub notes: asks first, then shows the loaded notes and the date', async () => {
    const { host } = await boot('', '#/lumen-ui/lumen/releases');
    await settle(300);
    const btn = [...host.querySelectorAll('button')].find((b) => b.textContent === 'Load GitHub release notes')!;
    await click(btn);
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('one request to api.github.com');
    await click([...dialog.querySelectorAll('button')].find((b) => b.textContent === 'Load notes'));
    await settle(100);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(host.querySelector('[data-testid="gh-fetched"]')?.textContent).toMatch(/^GitHub notes loaded /);
    const card = host.querySelector('.rcard[data-tag="v1.1.0"]')!;
    expect(card.querySelector('.rcard__title')?.textContent).toBe('Lumen 1.1 — theming');
    expect(card.textContent).toContain('GitHub notes');
  });

  it('no GitHub button for other hosts; an unsupported build hides it', async () => {
    const { host, backend } = await boot('', '#/inkwell/quill/releases');
    await settle(300);
    expect([...host.querySelectorAll('button')].some((b) => b.textContent?.includes('GitHub'))).toBe(false);
    const orig = backend.invoke.bind(backend);
    (backend as { invoke: typeof backend.invoke }).invoke = (async (cmd: string, args: never) => {
      if (cmd === 'releases_fetch_github') throw { code: 'unsupported', message: 'not built in' };
      return orig(cmd as never, args);
    }) as typeof backend.invoke;
    await go('#/lumen-ui/lumen/releases', 300);
    await click([...host.querySelectorAll('button')].find((b) => b.textContent === 'Load GitHub release notes'));
    await click([...document.querySelectorAll('[role="dialog"] button')].find((b) => b.textContent === 'Load notes'));
    await settle(100);
    expect([...host.querySelectorAll('button')].some((b) => b.textContent?.includes('GitHub'))).toBe(false);
  });
});
