import { describe, expect, it } from 'vitest';
import { boot, click, installHarness, settle, typeInto } from '../../test/appHarness';

installHarness();

const names = (host: HTMLElement) => [...host.querySelectorAll('.repo-row__name')].map((a) => a.textContent?.replace(/\s+/g, ' ').trim());

function menuItem(label: string): HTMLButtonElement | undefined {
  return [...document.querySelectorAll<HTMLButtonElement>('.menu__item')].find((b) => b.textContent?.includes(label));
}

function button(root: ParentNode, label: string): HTMLButtonElement | undefined {
  return [...root.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.trim().startsWith(label));
}

describe('Dashboard RepoList', () => {
  it('filters by ?q= ("pars" matches name, description and topics, case-insensitive)', async () => {
    const { host } = await boot('', '#/?q=PARS');
    await settle(100);
    const shown = names(host);
    expect(shown).toContain('northwind / parsekit');
    expect(shown.some((n) => n?.includes('lumen'))).toBe(false);
    expect(host.querySelector<HTMLInputElement>('input[aria-label="Find a repository"]')?.value).toBe('PARS');
  });

  it('typing in the filter replaces the history entry (Back does not step through keystrokes)', async () => {
    const { host } = await boot();
    const before = history.length;
    const input = host.querySelector('input[aria-label="Find a repository"]');
    await typeInto(input, 'q');
    await typeInto(input, 'qu');
    await typeInto(input, 'quill');
    await settle(50);
    expect(history.length).toBe(before);
    expect(location.hash).toBe('#/?q=quill');
    expect(names(host)).toEqual(['inkwell / quill']);
  });

  it('sort order: last updated (newest first) by default, then Name A → Z, then Z → A', async () => {
    const { host } = await boot();
    await settle(100);
    const byUpdated = names(host);
    expect(byUpdated[0]).toBe('northwind / parsekit');
    await click(button(host, 'Sort:'));
    await click(menuItem('Name'));
    await settle(50);
    const byName = names(host).map((n) => n!.split(' / ')[1].toLowerCase());
    expect(byName).toEqual([...byName].sort());
    await click(button(host, 'Sort:'));
    await click(menuItem('Z → A'));
    await settle(50);
    const desc = names(host).map((n) => n!.split(' / ')[1].toLowerCase());
    expect(desc).toEqual([...byName].reverse());
  });

  it('missing repo row: Locate… re-points it, Remove from shelf asks first and unregisters', async () => {
    const { host } = await boot();
    await settle(100);
    const row = () => host.querySelector('.repo-row--missing');
    expect(row()?.textContent).toContain('telemetry');
    expect(row()?.textContent).toContain('Not found');
    await click(button(row()!, 'Locate'));
    await settle(100);
    expect(row()).toBeNull();
    expect(names(host)).toContain('orbit-labs / telemetry');
  });

  it('missing repo row: Remove from shelf → dialog (unregister only) → gone', async () => {
    const { host } = await boot();
    await settle(100);
    const row = host.querySelector('.repo-row--missing')!;
    await click(button(row, 'Remove from shelf'));
    await settle(20);
    const dialog = document.querySelector('[role="dialog"]');    expect(dialog?.textContent).toContain('unregister only');
    expect(dialog?.textContent).toContain('E:\\Archive\\telemetry');
    await click(button(dialog!, 'Remove from shelf'));
    await settle(50);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(names(host).some((n) => n?.includes('telemetry'))).toBe(false);
  });
});
