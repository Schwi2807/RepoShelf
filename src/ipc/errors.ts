// Every IPC call rejects with a plain `IpcError { code, message }` (M4 handoff). `toIpcError` normalises anything
// else (a string from Tauri internals, a thrown Error in the mock) so screens only ever see that shape.

import type { IpcError, IpcErrorCode } from './types';

const CODES: ReadonlySet<string> = new Set<IpcErrorCode>([
  'notFound',
  'invalidPath',
  'invalidRev',
  'invalidUrl',
  'invalidInput',
  'duplicate',
  'notARepo',
  'repoMissing',
  'repoCorrupt',
  'cancelled',
  'refused',
  'network',
  'unsupported',
  'git',
  'db',
  'io',
  'internal',
]);

export function isIpcError(e: unknown): e is IpcError {
  if (e === null || typeof e !== 'object') return false;
  const o = e as { code?: unknown; message?: unknown };
  return typeof o.code === 'string' && CODES.has(o.code) && typeof o.message === 'string';
}

export function toIpcError(e: unknown): IpcError {
  if (isIpcError(e)) return { code: e.code, message: e.message };
  if (typeof e === 'string') return { code: 'internal', message: e };
  if (e instanceof Error) return { code: 'internal', message: e.message };
  return { code: 'internal', message: 'Unexpected error' };
}

export function ipcError(code: IpcErrorCode, message: string): IpcError {
  return { code, message };
}

export function isCancelled(e: unknown): boolean {
  return isIpcError(e) && e.code === 'cancelled';
}

/** Text for an error line in the UI. */
export function errorText(e: unknown): string {
  const ie = toIpcError(e);
  return ie.message.length > 0 ? ie.message : ie.code;
}
