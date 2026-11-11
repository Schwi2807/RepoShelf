// Test harness: the whole app on the mock backend (happy-dom). Component tests drive real screens through routes.

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, vi } from 'vitest';
import App from '../App';
import { setBackendForTests } from '../ipc/bridge';
import { createMockBackend, type MockBackend } from '../ipc/mock';
import { resetQueryCache } from '../ipc/query';
import { navigate, startRouter, stopRouter } from '../router/router';
import { resetJobsForTests, startJobs } from '../state/jobs';
import { resetLibraryForTests, startLibrary } from '../state/library';
import { loadSettings, resetSettingsForTests } from '../state/settings';
import { closeDialog } from '../state/ui';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

export interface Harness {
  host: HTMLElement;
  backend: MockBackend;
}

let root: Root | null = null;
let current: Harness | null = null;

export const settle = async (ms = 400): Promise<void> => {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
};

export async function go(hash: string, ms = 400): Promise<void> {
  await act(async () => navigate(hash));
  await settle(ms);
}

export async function boot(search = '', hash = '#/'): Promise<Harness> {
  globalThis.localStorage?.removeItem('reposhelf-mock-settings');
  const backend = createMockBackend(search, { tickMs: 1 });
  setBackendForTests(backend);
  history.replaceState(null, '', '/' + hash);
  startRouter();
  await loadSettings();
  await startLibrary();
  await startJobs();
  const host = document.createElement('div');
  document.body.appendChild(host);
  root = createRoot(host);
  await act(async () => root!.render(<App />));
  await settle(50);
  current = { host, backend };
  return current;
}

export async function click(el: Element | null | undefined, init: MouseEventInit = {}): Promise<void> {
  if (!el) throw new Error('click: element not found');
  await act(async () => {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, ...init }));
  });
}

/** Set an input's value the way React notices. */
export async function typeInto(el: Element | null | undefined, value: string): Promise<void> {
  if (!(el instanceof HTMLInputElement) && !(el instanceof HTMLTextAreaElement)) throw new Error('typeInto: not an input');
  const proto = el instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype;
  await act(async () => {
    Object.getOwnPropertyDescriptor(proto, 'value')!.set!.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

export function installHarness(): void {
  beforeEach(() => {
    resetQueryCache();
    resetLibraryForTests();
    resetJobsForTests();
    resetSettingsForTests();
  });
  afterEach(() => {
    act(() => closeDialog());
    if (root) act(() => root!.unmount());
    root = null;
    current?.host.remove();
    current = null;
    document.body.innerHTML = '';
    stopRouter();
    setBackendForTests(null);
    vi.restoreAllMocks();
  });
}
