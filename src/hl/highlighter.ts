// Worker client (D-5). One shared module worker (`worker-src 'self'`), request ids, cancel. Without Worker support
// (tests) every request ends as plain text.

import type { LangId } from './languages';
import { MAX_BYTES, MAX_LINES, type FromWorker, type ToWorker } from './protocol';

export type HighlightEnd = 'done' | 'plain' | 'cancelled' | 'error';

export interface HighlightJob {
  cancel(): void;
  /** Resolves when the worker finished (or the job ended otherwise). */
  done: Promise<HighlightEnd>;
}

interface Pending {
  onChunk: (start: number, lines: string[]) => void;
  resolve: (e: HighlightEnd) => void;
}

let worker: Worker | null = null;
let workerBroken = false;
let seq = 0;
const pending = new Map<number, Pending>();

function getWorker(): Worker | null {
  if (worker || workerBroken) return worker;
  if (typeof Worker === 'undefined') {
    workerBroken = true;
    return null;
  }
  try {
    worker = new Worker(new URL('./hl.worker.ts', import.meta.url), { type: 'module', name: 'highlighter' });
  } catch {
    workerBroken = true;
    return null;
  }
  worker.onmessage = (e: MessageEvent<FromWorker>) => {
    const m = e.data;
    const p = pending.get(m.id);
    if (!p) return;
    if (m.type === 'chunk') {
      p.onChunk(m.start, m.lines);
      return;
    }
    pending.delete(m.id);
    p.resolve(m.type === 'done' ? 'done' : m.type === 'plain' ? 'plain' : 'error');
  };
  worker.onerror = () => {
    for (const p of pending.values()) p.resolve('error');
    pending.clear();
    worker?.terminate();
    worker = null;
    workerBroken = true;
  };
  return worker;
}

/** Quick client-side check so huge files are never posted to the worker. */
export function tooLargeToHighlight(text: string): boolean {
  if (text.length > MAX_BYTES) return true;
  let n = 1;
  for (let i = 0; i < text.length; i++) if (text.charCodeAt(i) === 10 && ++n > MAX_LINES) return true;
  return false;
}

/**
 * Highlight `text`; `onChunk(start, lines)` receives self-contained HTML lines as they arrive (a later chunk may
 * replace lines of an earlier one). Lines never arrive after `cancel()`.
 */
export function highlight(text: string, langId: LangId | null, onChunk: (start: number, lines: string[]) => void): HighlightJob {
  if (langId === null || tooLargeToHighlight(text)) {
    return { cancel: () => undefined, done: Promise.resolve('plain') };
  }
  const w = getWorker();
  if (!w) return { cancel: () => undefined, done: Promise.resolve('plain') };
  seq += 1;
  const id = seq;
  let resolve!: (e: HighlightEnd) => void;
  const done = new Promise<HighlightEnd>((r) => {
    resolve = r;
  });
  pending.set(id, { onChunk, resolve });
  const msg: ToWorker = { type: 'highlight', id, langId, text };
  w.postMessage(msg);
  return {
    cancel() {
      if (!pending.has(id)) return;
      pending.delete(id);
      const c: ToWorker = { type: 'cancel', id };
      w.postMessage(c);
      resolve('cancelled');
    },
    done,
  };
}

/** Whole block at once (markdown code fences). Null = keep the plain text. */
export async function highlightBlock(text: string, langId: LangId | null): Promise<string[] | null> {
  const lines: string[] = [];
  const job = highlight(text, langId, (start, ls) => {
    for (let i = 0; i < ls.length; i++) lines[start + i] = ls[i];
  });
  const end = await job.done;
  return end === 'done' ? lines : null;
}
