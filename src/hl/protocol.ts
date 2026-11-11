// Messages between highlighter.ts and hl.worker.ts.

import type { LangId } from './languages';

/** Above these the file stays plain text (NFR-P4). */
export const MAX_BYTES = 1024 * 1024;
export const MAX_LINES = 20_000;
export const CHUNK_LINES = 2_000;

export type ToWorker = { type: 'highlight'; id: number; langId: LangId; text: string } | { type: 'cancel'; id: number };

export type FromWorker =
  /** Lines [start, start + lines.length) as self-contained HTML. A later chunk may replace earlier lines. */
  | { type: 'chunk'; id: number; start: number; lines: string[] }
  | { type: 'done'; id: number }
  | { type: 'plain'; id: number; reason: 'tooLarge' | 'unknownLanguage' }
  | { type: 'error'; id: number; message: string };
