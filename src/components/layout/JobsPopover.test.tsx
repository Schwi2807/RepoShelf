import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it } from 'vitest';
import { cloneStart } from '../../ipc/api';
import type { JobInfo } from '../../ipc/types';
import { boot, click, go, installHarness, settle } from '../../test/appHarness';
import { JobsList, jobTarget } from './JobsPopover';

installHarness();

const base: JobInfo = { id: 1, kind: { type: 'clone', url: 'https://github.com/o/r.git' }, state: 'running', progress: null, title: 'Clone o/r', result: null, error: null, startedAt: 0 };

function render(jobs: JobInfo[]): HTMLElement {
  const host = document.createElement('div');
  document.body.appendChild(host);
  act(() => createRoot(host).render(<JobsList jobs={jobs} />));
  return host;
}

const buttons = (row: Element) => [...row.querySelectorAll('button')].map((b) => b.textContent);

describe('jobs popover rows', () => {
  it('one row per state with the right actions', () => {
    const host = render([
      { ...base, id: 1, state: 'waiting' },
      { ...base, id: 2, state: 'running', progress: { type: 'receiving', received: 50, total: 200, bytes: 2048 } },
      { ...base, id: 3, state: 'running', kind: { type: 'fetch', repoId: 2 }, title: 'Fetch lumen-ui/lumen', progress: { type: 'resolving', done: 3, total: 4 } },
      { ...base, id: 4, state: 'done', result: { type: 'cloned', repoId: 99, skippedRefs: ['refs/heads/aux', 'refs/tags/CON'] } },
      {
        ...base,
        id: 5,
        state: 'done',
        kind: { type: 'fetch', repoId: 2 },
        title: 'Fetch lumen-ui/lumen',
        result: { type: 'fetched', newCommits: 3, newTags: ['v2.0.0'], keptLocalTags: ['v1.1.0'], skippedRefs: [], updatedBranches: ['origin/main'] },
      },
      { ...base, id: 6, state: 'failed', error: 'Could not reach github.com: the connection timed out.' },
      { ...base, id: 7, state: 'cancelled' },
      { ...base, id: 8, state: 'running', kind: { type: 'fetchAll' }, title: 'Fetch all', progress: { type: 'connecting' } },
    ]);
    const rows = [...host.querySelectorAll<HTMLElement>('.job')];
    expect(rows.map((r) => r.dataset.state)).toEqual(['waiting', 'running', 'running', 'done', 'done', 'failed', 'cancelled', 'running']);
    // per-kind headings, target without the kind word
    expect(rows.map((r) => r.querySelector('.job__kind')?.textContent)).toEqual(['Clone', 'Clone', 'Fetch', 'Clone', 'Fetch', 'Clone', 'Clone', 'Fetch all clones']);
    expect(rows[2].querySelector('.job__target')?.textContent).toBe('lumen-ui/lumen');
    expect(rows[7].querySelector('.job__target')?.textContent).toBe('every clone on the shelf');
    // progress text per variant
    expect(rows[0].querySelector('.job__status')?.textContent).toBe('Waiting for a free slot…');
    expect(rows[1].querySelector('.job__status')?.textContent).toBe('Receiving objects: 25% (50/200), 2 KB');
    expect(rows[1].querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('25');
    expect(rows[2].querySelector('.job__status')?.textContent).toBe('Resolving deltas: 75% (3/4)');
    expect(rows[7].querySelector('.progress--indeterminate')).not.toBeNull();
    // results: FR-9 skipped refs, FR-10 fetch report
    expect(rows[3].querySelector('.job__status')?.textContent).toBe('Cloned.Skipped 2 refs Windows cannot store: refs/heads/aux, refs/tags/CON.');
    expect(rows[4].querySelector('.job__status')?.textContent).toBe(
      '3 new commits, new tag v2.0.0.Kept local tags v1.1.0 (the remote has different ones; nothing was overwritten).',
    );
    expect(rows[5].querySelector('.job__error')?.textContent).toBe('Could not reach github.com: the connection timed out.');
    // actions: Cancel while waiting/running, Retry only when failed, Dismiss when finished
    expect(buttons(rows[0])).toEqual(['Cancel']);
    expect(buttons(rows[1])).toEqual(['Cancel']);
    expect(buttons(rows[3])).toEqual(['Dismiss']);
    expect(buttons(rows[5])).toEqual(['Retry', 'Dismiss']);
    expect(buttons(rows[6])).toEqual(['Dismiss']);
  });

  it('jobTarget falls back to the URL', () => {
    expect(jobTarget({ ...base, title: '' })).toBe('https://github.com/o/r.git');
  });
});

describe('jobs popover with the mock', () => {
  it('?clonefail: the clone failure stays listed with Retry', async () => {
    const { host } = await boot('?clonefail', '#/');
    await act(async () => {
      await cloneStart('example-org/new-thing');
    });
    await settle(300);
    await click(host.querySelector('button[aria-label^="Background jobs"]'));
    const row = host.querySelector<HTMLElement>('.jobs .job')!;
    expect(row.dataset.state).toBe('failed');
    expect(row.querySelector('.job__kind')?.textContent).toBe('Clone');
    expect(row.querySelector('.job__error')?.textContent).toBe('Could not reach github.com: the connection timed out.');
    expect(buttons(row)).toEqual(['Retry', 'Dismiss']);
  });

  it('a finished clone links to the new repository; a fetch reports tags and skipped refs', async () => {
    const { host } = await boot('', '#/lumen-ui/lumen');
    await act(async () => {
      await cloneStart('example-org/new-thing');
    });
    await click([...host.querySelectorAll('button')].find((b) => b.textContent?.trim() === 'Fetch'));
    await settle(400);
    await click(host.querySelector('button[aria-label^="Background jobs"]'));
    const rows = [...host.querySelectorAll<HTMLElement>('.jobs .job')];
    const clone = rows.find((r) => r.dataset.kind === 'clone')!;
    expect(clone.dataset.state).toBe('done');
    expect(clone.querySelector('a')?.getAttribute('href')).toBe('#/example-org/new-thing');
    const fetch = rows.find((r) => r.dataset.kind === 'fetch')!;
    expect(fetch.textContent).toContain('new tag v2.0.0-beta.2');
    expect(fetch.textContent).toContain('Skipped 1 ref Windows cannot store: refs/remotes/origin/AUX.');
  });

  it('?fetchfail: the failure text and Retry', async () => {
    const { host } = await boot('?fetchfail', '#/lumen-ui/lumen');
    await settle(200);
    await click([...host.querySelectorAll('button')].find((b) => b.textContent?.trim() === 'Fetch'));
    await settle(300);
    await click(host.querySelector('button[aria-label^="Background jobs"]'));
    const row = host.querySelector<HTMLElement>('.jobs .job')!;
    expect(row.dataset.state).toBe('failed');
    expect(row.querySelector('.job__error')?.textContent).toMatch(/^Could not reach github\.com/);
    await click([...row.querySelectorAll('button')].find((b) => b.textContent === 'Retry'));
    await settle(30);
    const rows = [...host.querySelectorAll<HTMLElement>('.jobs .job')];
    expect(rows).toHaveLength(1);
    expect(rows[0].dataset.state === 'running' || rows[0].dataset.state === 'waiting' || rows[0].dataset.state === 'failed').toBe(true);
    await go('#/', 10);
  });
});
