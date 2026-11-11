import { act } from 'react';
import { describe, expect, it } from 'vitest';
import { mockClonePreview } from '../../ipc/mock/cloneUrl';
import { openDialog } from '../../state/ui';
import { boot, installHarness, settle, typeInto } from '../../test/appHarness';
import { PREVIEW_DEBOUNCE_MS } from './CloneDialog';

installHarness();

const ARCHIVE = 'C:\\Users\\demo\\Documents\\RepoShelf';

function backendMessage(input: string): string {
  try {
    mockClonePreview(input, ARCHIVE);
  } catch (e) {
    return (e as { message: string }).message;
  }
  throw new Error(`expected ${input} to be rejected`);
}

async function openClone() {
  await boot();
  await act(async () => openDialog('clone'));
  const input = document.querySelector<HTMLInputElement>('#clone-url');
  expect(input).not.toBeNull();
  return input!;
}

const cloneButton = () =>
  [...document.querySelectorAll<HTMLButtonElement>('.dialog__footer button')].find((b) => b.textContent?.trim() === 'Clone')!;

describe('CloneDialog', () => {
  it.each([
    'git@github.com:owner/repo.git',
    'ssh://git@github.com/owner/repo',
    'http://github.com/owner/repo',
    'file:///C:/repo',
    'C:\\Code\\repo',
    'https://user:pw@github.com/owner/repo',
    'git://github.com/owner/repo',
    'justaname',
  ])('rejected form %s shows the backend message and keeps Clone disabled', async (bad) => {
    const input = await openClone();
    await typeInto(input, bad);
    await settle(PREVIEW_DEBOUNCE_MS + 80);
    const err = document.querySelector('.clone-status [role="alert"]');
    expect(err?.textContent).toBe(backendMessage(bad));
    expect(document.querySelector('[data-testid="clone-preview"]')).toBeNull();
    expect(cloneButton().disabled).toBe(true);
  });

  it('a valid URL shows the host for confirmation and the target folder', async () => {
    const input = await openClone();
    await typeInto(input, 'codeberg.org/someone/tool');
    await settle(PREVIEW_DEBOUNCE_MS + 80);
    const prev = document.querySelector('[data-testid="clone-preview"]');
    expect(prev?.querySelector('.clone-preview__host')?.textContent).toBe('codeberg.org');
    expect(prev?.querySelector('.clone-preview__target')?.textContent).toBe(`${ARCHIVE}\\codeberg.org\\someone\\tool.git`);
    expect(cloneButton().disabled).toBe(false);
  });

  it('only the latest input counts (debounced, stale answers dropped)', async () => {
    const input = await openClone();
    await typeInto(input, 'http://x.org/a/b');
    await typeInto(input, 'owner/repo');
    await settle(PREVIEW_DEBOUNCE_MS + 80);
    expect(document.querySelector('.clone-status [role="alert"]')).toBeNull();
    expect(document.querySelector('.clone-preview__host')?.textContent).toBe('github.com');
  });
});
