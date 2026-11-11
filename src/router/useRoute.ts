import { useSyncExternalStore } from 'react';
import { current, subscribe, type Route } from './router';

export function useRoute(): Route {
  return useSyncExternalStore(subscribe, current, current);
}
