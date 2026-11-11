import { describe, expect, it } from 'vitest';
import { boot, click, go, installHarness, settle, typeInto } from '../../test/appHarness';
import { refDisplayName } from './RefSwitcher';

installHarness();

async function openSwitcher(host: HTMLElement) {
  await click(host.querySelector('.refswitch'));
  await settle(100);
  const pop = host.querySelector('.refs');
  expect(pop).not.toBeNull();
  return pop!;
}

const items = (pop: Element) => [...pop.querySelectorAll('.refs__item')];
const itemNames = (pop: Element) => items(pop).map((a) => a.querySelector('.refs__name')?.textContent);

describe('RefSwitcher', () => {
  it('lists branches with the default marked and the current one checked; the filter narrows the list', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit', 300);
    const pop = await openSwitcher(host);
    const main = items(pop).find((a) => a.querySelector('.refs__name')?.textContent === 'main')!;
    expect(main.textContent).toContain('default');
    expect(main.closest('[role="option"]')?.getAttribute('aria-selected')).toBe('true');
    await typeInto(pop.querySelector('input'), 'STREAM');
    expect(itemNames(pop)).toEqual(['feature/streaming', 'origin/feature/streaming']);
    await typeInto(pop.querySelector('input'), 'nothing-like-this');
    expect(pop.textContent).toContain('Nothing to show');
  });

  it('tags are linked as refs/tags/<name> (one encoded segment) and keep the path', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit/tree/main/src', 300);
    const pop = await openSwitcher(host);
    await click([...pop.querySelectorAll('.refs__tab')].find((b) => b.textContent === 'Tags'));
    const v3 = items(pop).find((a) => a.querySelector('.refs__name')?.textContent === 'v0.3.0')!;
    expect(v3.getAttribute('href')).toBe('#/northwind/parsekit/tree/refs%2Ftags%2Fv0.3.0/src');
    await click(v3);
    await settle(300);
    expect(host.querySelector('.refswitch')?.textContent).toContain('v0.3.0');
    expect(host.querySelector('.refs')).toBeNull();
  });

  it('display names: tags without the prefix, full oids shortened', () => {
    expect(refDisplayName('refs/tags/v1.2.0', null)).toBe('v1.2.0');
    expect(refDisplayName('feature/x', null)).toBe('feature/x');
    expect(refDisplayName('0123456789abcdef0123456789abcdef01234567', null)).toBe('0123456');
  });
});
