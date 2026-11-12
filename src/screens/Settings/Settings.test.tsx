import { describe, expect, it } from 'vitest';
import { startTheme } from '../../state/theme';
import { boot, click, go, installHarness, settle } from '../../test/appHarness';

installHarness();

describe('Settings page (S10)', () => {
  it('theme radio sets and persists the theme', async () => {
    const { host } = await boot('', '#/settings');
    startTheme();
    await settle(100);
    await click(host.querySelector('input[value="dark"]'));
    await settle(50);
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(JSON.parse(localStorage.getItem('reposhelf-mock-settings') ?? '{}').theme).toBe('dark');
    await click(host.querySelector('input[value="light"]'));
    await settle(50);
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('shows both folders with Open, the cache size, and clears the cache', async () => {
    const { host } = await boot('', '#/settings');
    await settle(100);
    expect([...host.querySelectorAll('.setting-folder .mono')].map((p) => p.textContent)).toEqual([
      'C:\\Users\\demo\\AppData\\Local\\dev.schwi.reposhelf',
      'C:\\Users\\demo\\Documents\\RepoShelf',
    ]);
    const open = [...host.querySelectorAll('.setting-folder button')].filter((b) => b.textContent === 'Open');
    expect(open).toHaveLength(2);
    await click(open[0]);
    await settle(30);
    expect(host.querySelector('.setting-folder .field__error')).toBeNull();
    expect(host.querySelector('[data-testid="cache-size"]')?.textContent).toBe('3.3 MB');
    await click([...host.querySelectorAll('button')].find((b) => b.textContent === 'Clear cache'));
    await settle(50);
    expect(host.querySelector('[data-testid="cache-size"]')?.textContent).toBe('0 Bytes');
    expect(host.querySelector('[role="status"]')?.textContent).toMatch(/^Cleared 3\.3 MB\./);
    expect(host.textContent).toContain('RepoShelf 0.2.0');
  });

  it('the uncommitted-changes toggle shows the note on an imported work tree (D-16)', async () => {
    const { host } = await boot('', '#/northwind/parsekit');
    await settle(200);
    expect(host.querySelector('[data-testid="dirty-note"]')).toBeNull();
    await go('#/settings', 100);
    await click(host.querySelector('.check-row input'));
    await settle(50);
    await go('#/northwind/parsekit', 300);
    expect(host.querySelector('[data-testid="dirty-note"]')?.textContent).toContain('uncommitted changes');
  });
});
