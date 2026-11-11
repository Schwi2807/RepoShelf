import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it } from 'vitest';
import type { FileChange, FileDiff } from '../../ipc/types';
import { boot, click, go, installHarness, settle } from '../../test/appHarness';
import { collapseReason, miniBlocks, pathLabel } from './diffs';
import { DiffTable } from './DiffTable';

installHarness();

function renderTable(diff: FileDiff): HTMLElement {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const root = createRoot(host);
  act(() => root.render(<DiffTable diff={diff} path="a.txt" />));
  return host;
}

const change = (o: Partial<FileChange>): FileChange => ({
  index: 0,
  kind: 'modified',
  oldPath: 'a.txt',
  newPath: 'a.txt',
  additions: 1,
  deletions: 1,
  binary: false,
  tooLarge: false,
  generated: false,
  ...o,
});

describe('DiffTable', () => {
  it('renders a hunk: header row, numbers, markers, and the NoNewline marker without numbers', () => {
    const host = renderTable({
      index: 0,
      truncated: false,
      hunks: [
        {
          header: '@@ -1,2 +1,2 @@',
          lines: [
            { kind: 'ctx', oldNo: 1, newNo: 1, text: 'keep' },
            { kind: 'del', oldNo: 2, newNo: null, text: 'old <b>' },
            { kind: 'noNewline', oldNo: null, newNo: null, text: '\\ No newline at end of file' },
            { kind: 'add', oldNo: null, newNo: 2, text: 'new' },
            { kind: 'noNewline', oldNo: null, newNo: null, text: '\\ No newline at end of file' },
          ],
        },
      ],
    });
    const rows = [...host.querySelectorAll('tr')];
    expect(rows[0].className).toBe('dtable__hunk');
    expect(rows[0].textContent).toBe('@@ -1,2 +1,2 @@');
    const cells = (i: number) => [...rows[i].querySelectorAll('td')].map((td) => td.textContent);
    expect(cells(1)).toEqual(['1', '1', ' ', 'keep']);
    expect(cells(2)).toEqual(['2', '', '-', 'old <b>']);
    expect(rows[2].className).toContain('dtable__line--del');
    expect(host.querySelector('b')).toBeNull(); // text, never HTML
    expect(rows[3].className).toContain('dtable__line--nonl');
    expect(cells(3)).toEqual(['', '', '', 'No newline at end of file']);
    expect(cells(4)).toEqual(['', '2', '+', 'new']);
    expect(rows[4].className).toContain('dtable__line--add');
    expect(host.querySelectorAll('.dtable__line--nonl')).toHaveLength(2);
  });

  it('says so when a file has no content changes, and when the diff was cut off', () => {
    expect(renderTable({ index: 0, truncated: false, hunks: [] }).textContent).toContain('No content changes');
    const t = renderTable({ index: 0, truncated: true, hunks: [{ header: '@@ -1 +1 @@', lines: [{ kind: 'add', oldNo: null, newNo: 1, text: 'x' }] }] });
    expect(t.textContent).toContain('cut off');
  });
});

describe('collapse rules and labels', () => {
  it('binary / > 1 MB / generated / deferred / large', () => {
    expect(collapseReason(change({ binary: true }), false, false)).toBe('binary');
    expect(collapseReason(change({ tooLarge: true }), false, false)).toBe('large');
    expect(collapseReason(change({ generated: true }), true, false)).toBe('generated');
    expect(collapseReason(change({}), true, false)).toBeNull();
    expect(collapseReason(change({}), false, true)).toBe('deferred');
    expect(collapseReason(change({ additions: 2500 }), false, false)).toBe('lines');
  });

  it('renames read old → new; mini bar blocks follow the ratio', () => {
    expect(pathLabel(change({ kind: 'renamed', oldPath: 'a.md', newPath: 'b.md' }))).toEqual({ from: 'a.md', to: 'b.md' });
    expect(pathLabel(change({ kind: 'deleted', newPath: null }))).toEqual({ from: null, to: 'a.txt' });
    expect(miniBlocks(0, 0)).toEqual(['none', 'none', 'none', 'none', 'none']);
    expect(miniBlocks(1, 1)).toEqual(['add', 'del', 'none', 'none', 'none']);
    expect(miniBlocks(90, 10)).toEqual(['add', 'add', 'add', 'add', 'del']);
    expect(miniBlocks(0, 40)).toEqual(['del', 'del', 'del', 'del', 'del']);
  });
});

async function commitHref(host: HTMLElement, repo: string, n = 0): Promise<string> {
  await go(`#/${repo}/commits/main`, 400);
  return host.querySelectorAll<HTMLAnchorElement>('.crow__summary')[n].getAttribute('href')!;
}

describe('Commit page', () => {
  it('?huge 400-file commit: file list first, cards collapsed, "Load diff" loads one on click', async () => {
    const { host } = await boot('?huge', '#/');
    await go(await commitHref(host, 'northwind/parsekit'), 500);
    expect(host.querySelector('[data-testid="diff-summary"]')?.textContent).toBe('Showing 400 changed files');
    expect(host.querySelector('[data-testid="deferred-note"]')).not.toBeNull();
    const cards = host.querySelectorAll('.dcard');
    expect(cards.length).toBeGreaterThan(0);
    expect(cards.length).toBeLessThan(400); // virtualised
    expect(host.querySelector('.dtable')).toBeNull();
    const card = cards[0] as HTMLElement;
    expect(card.querySelector('[data-collapsed]')?.getAttribute('data-collapsed')).toBe('generated');
    expect(card.querySelector('.minibar')).toBeNull(); // no stats when deferred
    const load = [...card.querySelectorAll('button')].find((b) => b.textContent === 'Load diff')!;
    const i = Number(card.dataset.index);
    await click(load);
    await settle(200);
    const table = host.querySelector(`#diff-${i} .dtable`);
    expect(table).not.toBeNull();
    expect(table!.textContent).toContain(`generated case ${String(i).padStart(4, '0')}`);
  });

  it('a commit with a rename, a deletion and a no-newline change; header with parent link and full oid', async () => {
    const { host } = await boot();
    await go(await commitHref(host, 'northwind/parsekit', 3), 500);
    expect(host.querySelector('.chead__summary')?.textContent).toBe('Document the pull parser');
    expect(host.querySelector('[data-testid="commit-parents"]')?.textContent).toMatch(/^1 parent[0-9a-f]{7}$/);
    expect(host.querySelector('.chead__fulloid')?.textContent).toMatch(/^[0-9a-f]{40}$/);
    expect(host.querySelector('[data-testid="diff-summary"]')?.textContent).toMatch(/^Showing 5 changed files with \d+ additions? and \d+ deletions?$/);
    const paths = [...host.querySelectorAll('.dcard__path')].map((p) => p.textContent);
    expect(paths).toContain('docs/design.md → docs/architecture.md');
    expect([...host.querySelectorAll('.dcard .badge')].map((b) => b.textContent)).toEqual(expect.arrayContaining(['Renamed', 'Deleted']));
    expect(host.querySelectorAll('.dtable__line--nonl').length).toBe(2);
    // the deleted file's View file points at the parent commit
    const del = [...host.querySelectorAll<HTMLElement>('.dcard')].find((c) => c.textContent?.includes('docs/old-notes.md'))!;
    const parent = host.querySelector('.chead__oid')!.getAttribute('title')!;
    expect(del.querySelector('.dcard__view')?.getAttribute('href')).toBe(`#/northwind/parsekit/blob/${parent}/docs/old-notes.md`);
    // chevron hides a card body
    const toggle = del.querySelector<HTMLButtonElement>('.dcard__toggle')!;
    await click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(del.querySelector('.dtable')).toBeNull();
  });

  it('merge commit note (first parent) and root commit note; short oid in the route resolves', async () => {
    const { host } = await boot();
    await go('#/lumen-ui/lumen/commits/main?page=1', 400);
    const merge = [...host.querySelectorAll<HTMLAnchorElement>('.crow__summary')].find((a) => a.textContent === "Merge branch 'next-dialog'")!;
    await go(merge.getAttribute('href')!, 400);
    expect(host.querySelector('[data-testid="merge-note"]')?.textContent).toContain('first parent');
    expect(host.querySelector('[data-testid="commit-parents"]')?.textContent).toMatch(/^2 parents[0-9a-f]{7} \+ [0-9a-f]{7}$/);
    // the second parent (a side-branch commit) opens
    const second = host.querySelectorAll<HTMLAnchorElement>('.chead__oid')[1];
    await go(second.getAttribute('href')!.replace(/([0-9a-f]{7})[0-9a-f]{33}$/, '$1'), 400);
    expect(host.querySelector('.chead__summary')?.textContent).toBe('Dialog: fix Escape in nested menus');

    // root commit: last page of quill (64 commits → page 2)
    await go('#/inkwell/quill/commits/trunk?page=2', 400);
    const rows = host.querySelectorAll<HTMLAnchorElement>('.crow__summary');
    await go(rows[rows.length - 1].getAttribute('href')!, 400);
    expect(host.querySelector('[data-testid="root-note"]')).not.toBeNull();
    expect(host.querySelector('[data-testid="commit-parents"]')?.textContent).toBe('0 parents (root commit)');
    expect([...host.querySelectorAll('.dcard .badge')].every((b) => b.textContent === 'Added')).toBe(true);
  });
});
