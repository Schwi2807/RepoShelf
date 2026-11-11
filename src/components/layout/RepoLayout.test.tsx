// D-87 (found by the M8 real-app e2e): opening a repo stamps `lastOpenedAt` in `repo_detail`; the dashboard's Recent
// list must follow without a restart.
import { describe, expect, it } from 'vitest';
import { boot, go, installHarness, settle } from '../../test/appHarness';

installHarness();

const recentText = (host: HTMLElement) => {
  const h = [...host.querySelectorAll('h2, h3')].find((e) => e.textContent?.trim() === 'Recent');
  return h?.closest('section')?.textContent ?? '';
};

describe('Recent follows opened repos', () => {
  it('a repo opened for the first time shows up in Recent', async () => {
    const { host } = await boot('', '#/');
    await settle();
    expect(recentText(host)).not.toContain('beacon');
    await go('#/local/beacon');
    await go('#/');
    expect(recentText(host)).toContain('beacon');
  });
});
