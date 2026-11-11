// Minimal external store for `useSyncExternalStore` (no state library, ARCH §7.3).

import { useSyncExternalStore } from 'react';

export interface Store<T> {
  get(): T;
  set(next: T | ((prev: T) => T)): void;
  subscribe(l: () => void): () => void;
}

export function createStore<T>(initial: T): Store<T> {
  let value = initial;
  const ls = new Set<() => void>();
  return {
    get: () => value,
    set(next) {
      const v = typeof next === 'function' ? (next as (p: T) => T)(value) : next;
      if (Object.is(v, value)) return;
      value = v;
      for (const l of [...ls]) l();
    },
    subscribe(l) {
      ls.add(l);
      return () => {
        ls.delete(l);
      };
    },
  };
}

export function useStore<T>(s: Store<T>): T {
  return useSyncExternalStore(s.subscribe, s.get, s.get);
}
