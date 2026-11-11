import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';
import { setBackendForTests } from './ipc/bridge';
import { createMockBackend } from './ipc/mock';
import { resetQueryCache } from './ipc/query';
import { navigate, startRouter, stopRouter } from './router/router';
import { resetJobsForTests, startJobs } from './state/jobs';
import { resetLibraryForTests, startLibrary } from './state/library';
import { loadSettings, resetSettingsForTests } from './state/settings';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLElement;

const settle = async (ms = 400) => {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
};

async function boot(search = '') {
  setBackendForTests(createMockBackend(search, { tickMs: 1 }));
  history.replaceState(null, '', '/#/');
  startRouter();
  await loadSettings();
  await startLibrary();
  await startJobs();
  host = document.createElement('div');
  document.body.appendChild(host);
  root = createRoot(host);
  await act(async () => root.render(<App />));
  await settle(50);
}

beforeEach(() => {
  resetQueryCache();
  resetLibraryForTests();
  resetJobsForTests();
  resetSettingsForTests();
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  stopRouter();
  setBackendForTests(null);
  vi.restoreAllMocks();
});

describe('App shell (mock backend)', () => {
  it('renders the global header and the dashboard', async () => {
    await boot();
    expect(host.querySelector('header.gh')).not.toBeNull();
    expect(host.querySelector('input[aria-label="Search or jump to a repository"]')).not.toBeNull();
    expect(host.textContent).toContain('Repositories');
    expect(host.textContent).toContain('parsekit');
  });

  it('?empty shows the empty shelf', async () => {
    await boot('?empty');
    expect(host.textContent).toContain('Your shelf is empty');
  });

  it('repo route: header, tabs with counts, resolved ref', async () => {
    await boot();
    await act(async () => navigate('#/northwind/parsekit'));
    await settle();
    expect(host.querySelector('.rh__title')?.textContent).toContain('northwind/parsekit');
    const tabs = [...host.querySelectorAll('.tabs__tab')].map((t) => t.textContent);
    expect(tabs).toEqual(['Code', 'Commits148', 'Branches4', 'Tags4', 'Releases3']);
    expect(host.querySelector('.tabs__tab[aria-current="page"]')?.textContent).toBe('Code');
    expect(host.querySelector('.refswitch')?.textContent).toContain('main');
    expect(host.querySelector('[data-testid="latest-commit"]')?.textContent).toMatch(/[0-9a-f]{7}/);
    expect(host.querySelector('[data-testid="file-table"]')).not.toBeNull();
  });

  it('unknown repo and unknown route', async () => {
    await boot();
    await act(async () => navigate('#/nobody/nothing'));
    await settle(50);
    expect(host.textContent).toContain('Repository not found');
    await act(async () => navigate('#/a/b/what'));
    await settle(50);
    expect(host.textContent).toContain('Nothing here');
  });

  it('missing repo shows the banner; empty repo says so', async () => {
    await boot();
    await act(async () => navigate('#/orbit-labs/telemetry'));
    await settle();
    expect(host.textContent).toContain('The repository folder is missing.');
    await act(async () => navigate('#/local/scratchpad'));
    await settle();
    expect(host.textContent).toContain('This repository is empty.');
  });

  it('applies the remembered scroll position after the route rendered on back (D-31)', async () => {
    await boot();
    await act(async () => navigate('#/northwind/parsekit/tree/main/src'));
    await settle(50);
    window.scrollTo(0, 480);
    window.dispatchEvent(new Event('scroll'));
    await act(async () => navigate('#/northwind/parsekit/blob/main/src/lib.rs'));
    await settle(50);
    const spy = vi.spyOn(window, 'scrollTo');
    const popped = new Promise<void>((r) => window.addEventListener('popstate', () => r(), { once: true }));
    await act(async () => {
      history.back();
      await popped;
    });
    await settle(50);
    expect(spy).toHaveBeenCalledWith(0, 480);
  });

  it('"/" focuses the repo search', async () => {
    await boot();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '/' }));
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Search or jump to a repository');
  });
});
