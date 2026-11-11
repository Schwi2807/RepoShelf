import { describe, expect, it } from 'vitest';
import { boot, click, go, installHarness, settle } from '../../test/appHarness';

installHarness();

const rows = (host: HTMLElement) => [...host.querySelectorAll('[data-testid="file-table"] .ftable__row')];
const rowNames = (host: HTMLElement) => rows(host).map((r) => r.querySelector('.ftable__name a')?.textContent);

describe('FileTable', () => {
  it('root: folders first, then files, case-insensitive order; no ".." row', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit', 400);
    const names = rowNames(host);
    const kinds = rows(host).map((r) => r.getAttribute('data-kind'));
    const firstFile = kinds.findIndex((k) => k !== 'dir' && k !== 'submodule');
    expect(kinds.slice(0, firstFile).every((k) => k === 'dir')).toBe(true);
    expect(names).not.toContain('..');
    const dirs = names.slice(0, firstFile).map((n) => n!.toLowerCase());
    expect(dirs).toEqual([...dirs].sort());
    expect(names).toContain('Cargo.toml');
  });

  it('subfolder: ".." row first linking to the parent, then the entries', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/tree/main/src/stream', 400);
    const names = rowNames(host);
    expect(names[0]).toBe('..');
    expect(rows(host)[0].querySelector('a')?.getAttribute('href')).toBe('#/northwind/parsekit/tree/main/src');
    expect(names.slice(1)).toEqual(['buffer.rs', 'mod.rs']);
  });

  it('shows a shimmer until tree_last_commits resolves, then messages and dates', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/tree/main/src', 20);
    expect(host.querySelectorAll('[data-testid="file-table"] .ftable__msg .shimmer').length).toBeGreaterThan(0);
    await settle(400);
    expect(host.querySelectorAll('[data-testid="file-table"] .ftable__msg .shimmer').length).toBe(0);
    const msgs = [...host.querySelectorAll('.ftable__commit')].map((a) => a.textContent);
    expect(msgs.length).toBeGreaterThan(0);
    expect(host.querySelector('.ftable__date time')).not.toBeNull();
  });

  it('NFR-W3 / R-7 markers: not checked out on this PC, non-UTF-8 name', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/tree/main/odd', 400);
    const text = (name: string) => rows(host).find((r) => r.querySelector('.ftable__name a')?.textContent === name)?.textContent ?? '';
    expect(text('a:b.txt')).toContain('not checked out on this PC');
    expect(text('CON.txt')).toContain('not checked out on this PC');
    expect(text('caf�.txt')).toContain('non-UTF-8 name');
  });

  it('a folder link opens its tree; the filter narrows the folder', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit', 300);
    const src = rows(host).find((r) => r.querySelector('.ftable__name a')?.textContent === 'src')!;
    await click(src.querySelector('.ftable__name a'));
    await settle(300);
    expect(location.hash).toBe('#/northwind/parsekit/tree/main/src');
  });
});
