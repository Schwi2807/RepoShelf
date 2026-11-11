import { describe, expect, it } from 'vitest';
import { boot, click, go, installHarness, settle } from '../../test/appHarness';
import { formatLines, maxColumns, nextSelection, parseLines, splitTextLines } from './lines';

installHarness();

const selected = (host: HTMLElement) => [...host.querySelectorAll('.cv__line--sel')].map((l) => Number(l.getAttribute('data-line')));

describe('line ranges', () => {
  it('parse / format / extend', () => {
    expect(parseLines('10-20')).toEqual({ start: 10, end: 20 });
    expect(parseLines('20-10')).toEqual({ start: 10, end: 20 });
    expect(parseLines('L7')).toEqual({ start: 7, end: 7 });
    expect(parseLines('x')).toBeNull();
    expect(parseLines('0')).toBeNull();
    expect(formatLines({ start: 3, end: 3 })).toBe('3');
    expect(nextSelection({ start: 10, end: 20 }, 4, true)).toEqual({ start: 4, end: 10 });
    expect(nextSelection({ start: 10, end: 20 }, 4, false)).toEqual({ start: 4, end: 4 });
    expect(splitTextLines('a\nb\n')).toEqual(['a', 'b']);
    expect(maxColumns(['\tab', 'abcdef'])).toBe(6);
  });
});

describe('CodeViewer (file view)', () => {
  it('?L=10-20 selects that range', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/blob/main/src/lexer.rs?L=10-20', 400);
    expect(host.querySelector('[data-testid="code-viewer"]')).not.toBeNull();
    expect(selected(host)).toEqual(Array.from({ length: 11 }, (_, i) => 10 + i));
  });

  it('click selects one line; shift-click extends the range (route keeps ?L=)', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/blob/main/src/lexer.rs', 400);
    const num = (n: number) => host.querySelector(`.cv__line[data-line="${n}"] .cv__num`);
    await click(num(5));
    await settle(50);
    expect(location.hash).toBe('#/northwind/parsekit/blob/main/src/lexer.rs?L=5');
    expect(selected(host)).toEqual([5]);
    await click(num(9), { shiftKey: true });
    await settle(50);
    expect(location.hash).toBe('#/northwind/parsekit/blob/main/src/lexer.rs?L=5-9');
    expect(selected(host)).toEqual([5, 6, 7, 8, 9]);
    expect([...host.querySelectorAll('button')].some((b) => b.textContent === 'Copy link')).toBe(true);
  });

  it('placeholders and redirects', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/blob/main/assets/sample.bin');
    expect(host.querySelector('[data-placeholder="binary"]')?.textContent).toContain('Binary file, 18 KB, not shown.');
    await go('#/northwind/parsekit/blob/main/data/corpus.json');
    expect(host.querySelector('[data-placeholder="lfs"]')?.textContent).toContain('Stored with Git LFS');
    await go('#/northwind/parsekit/blob/main/data/export.log');
    expect(host.querySelector('[data-placeholder="large"]')?.textContent).toContain('too large to display');
    await go('#/northwind/parsekit/blob/main/vendor/ucd');
    expect(host.querySelector('[data-placeholder="submodule"]')?.textContent).toContain('git.example.org');
    await go('#/northwind/parsekit/blob/main/latest');
    expect(host.querySelector('[data-placeholder="symlink"]')?.textContent).toContain('Open docs/architecture.md');
    await go('#/northwind/parsekit/blob/main/docs/legacy-notes.txt');
    expect(host.textContent).toContain('not valid UTF-8');
    await go('#/northwind/parsekit/blob/main/odd/a:b.txt');
    expect(host.textContent).toContain('Not checked out on this PC.');
    // a directory on a blob route → its tree
    await go('#/northwind/parsekit/blob/main/src', 600);
    expect(location.hash).toBe('#/northwind/parsekit/tree/main/src');
  });

  it('raw view and markdown preview / code tabs', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/blob/main/src/error.rs?plain=1');
    expect(host.querySelector('pre.raw')?.textContent).toContain('pub type Result<T>');
    await go('#/northwind/parsekit/blob/main/docs/architecture.md');
    expect(host.querySelector('.filebox .markdown')).not.toBeNull();
    expect(host.querySelector('.segmented__item[aria-selected="true"]')?.textContent).toBe('Preview');
    await go('#/northwind/parsekit/blob/main/docs/architecture.md?view=code');
    expect(host.querySelector('[data-testid="code-viewer"]')).not.toBeNull();
  });
});
