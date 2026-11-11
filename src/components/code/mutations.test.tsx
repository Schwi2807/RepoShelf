import { describe, expect, it } from 'vitest';
import { boot, click, go, installHarness, settle, typeInto } from '../../test/appHarness';

installHarness();

function button(root: ParentNode, label: string): HTMLButtonElement | undefined {
  return [...root.querySelectorAll<HTMLButtonElement>('button')].find(
    (b) => b.textContent?.trim().startsWith(label) || b.getAttribute('aria-label') === label,
  );
}

describe('repo mutations update the open screen and the dashboard (D-69)', () => {
  it('Edit About → Save: sidebar and dashboard row show the new values', async () => {
    const { host } = await boot();
    await go('#/lumen-ui/lumen', 300);
    await click(button(host, 'Edit repository details'));
    const dialog = document.querySelector('[role="dialog"]')!;
    await typeInto(dialog.querySelector('textarea'), 'A brand new description.');
    await typeInto(dialog.querySelector('.chip-input__field'), 'design-system');
    await click(button(dialog, 'Save changes'));
    await settle(100);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(host.querySelector('.about__desc')?.textContent).toBe('A brand new description.');
    expect([...host.querySelectorAll('.about .chip')].map((c) => c.textContent)).toContain('design-system');
    await go('#/', 100);
    const row = [...host.querySelectorAll('.repo-row')].find((r) => r.textContent?.includes('lumen-ui'));
    expect(row?.textContent).toContain('A brand new description.');
    expect(row?.textContent).toContain('design-system');
  });

  it('Edit About: an invalid topic is refused with the backend wording before saving', async () => {
    const { host } = await boot();
    await go('#/lumen-ui/lumen', 300);
    await click(button(host, 'Edit repository details'));
    const dialog = document.querySelector('[role="dialog"]')!;
    await typeInto(dialog.querySelector('.chip-input__field'), 'Bad_Topic');
    await click(button(dialog, 'Save changes'));
    await settle(50);
    expect(dialog.querySelector('[role="alert"]')?.textContent).toContain("use lowercase letters, digits and '-'");
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  });

  it('header "…" → Remove: lands on the dashboard without the repo, no "not found" screen', async () => {
    const { host } = await boot();
    await go('#/lumen-ui/lumen', 300);
    await click(button(host, 'More repository actions'));
    await click([...document.querySelectorAll<HTMLButtonElement>('.menu__item')].find((b) => b.textContent?.includes('Remove from shelf')));
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('Recycle Bin');
    await click(button(dialog, 'Remove and move to Recycle Bin'));
    await settle(100);
    expect(location.hash).toBe('#/');
    expect(host.textContent).not.toContain('Repository not found');
    expect([...host.querySelectorAll('.repo-row__name')].some((a) => a.textContent?.includes('lumen'))).toBe(false);
  });

  it('Fetch on an imported repo asks first, then shows "N new commits"', async () => {
    const { host } = await boot();
    await go('#/northwind/parsekit', 300);
    await click(button(host, 'Fetch'));
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.textContent).toContain('refs/remotes/origin/*');
    expect(dialog.textContent).toContain('C:\\Code\\parsekit\\.git');
    await click(button(dialog.querySelector('.dialog__footer')!, 'Fetch'));
    await settle(300);
    expect(host.querySelector('.rh__msg')?.textContent).toBe('3 new commits');
  });

  it('Fast-forward on a clone shows the result and the button goes away', async () => {
    const { host } = await boot();
    await go('#/lumen-ui/lumen', 300);
    await click(button(host, 'Fast-forward'));
    await settle(300);
    expect(host.querySelector('.rh__msg')?.textContent).toMatch(/^Fast-forwarded to [0-9a-f]{7}$/);
    expect(button(host, 'Fast-forward')).toBeUndefined();
  });
});
