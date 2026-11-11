// The only module that imports `@tauri-apps/api` (ARCH §7.4). In a plain browser (no `__TAURI_INTERNALS__`) every
// call goes to the mock backend, which is loaded with a dynamic import so its fixtures stay out of the initial JS.
// Only `core` (invoke, Channel) and `event` (listen) are used: the capability is `core:event:default` (D-54), so
// no window/webview/app APIs.

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ArgsOf, ChannelLike, CommandName, Events, RetOf } from './commands';
import { toIpcError } from './errors';

export const isTauri: boolean = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** What a non-Tauri backend (the mock, or a test fake) implements. */
export interface Backend {
  invoke<K extends CommandName>(cmd: K, args: ArgsOf<K>): Promise<RetOf<K>>;
  listen<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): () => void;
}

let backendOverride: Backend | null = null;
let mockBackend: Promise<Backend> | null = null;

function mock(): Promise<Backend> {
  if (backendOverride) return Promise.resolve(backendOverride);
  mockBackend ??= import('./mock').then((m) => m.createMockBackend(location.search));
  return mockBackend;
}

/** Tests: route every call to a fake (null restores the default). */
export function setBackendForTests(b: Backend | null): void {
  backendOverride = b;
  mockBackend = null;
}

export async function call<K extends CommandName>(cmd: K, args: ArgsOf<K>): Promise<RetOf<K>> {
  try {
    if (isTauri && !backendOverride) {
      return await invoke<RetOf<K>>(cmd, args as Record<string, unknown>);
    }
    const b = await mock();
    return await b.invoke(cmd, args);
  } catch (e) {
    throw toIpcError(e);
  }
}

/** A channel for `jobs_subscribe`: Tauri `Channel` in the app, a plain object for the mock. */
export function createChannel<T>(onmessage: (m: T) => void): ChannelLike<T> {
  if (isTauri && !backendOverride) return new Channel<T>(onmessage);
  return { onmessage };
}

/** Listen to a backend event. Returns an unsubscribe function. */
export async function onEvent<E extends keyof Events>(
  event: E,
  handler: (payload: Events[E]) => void,
): Promise<() => void> {
  if (isTauri && !backendOverride) {
    return listen<Events[E]>(event, (e) => handler(e.payload));
  }
  const b = await mock();
  return b.listen(event, handler);
}
