// The highlighting work itself (runs inside hl.worker.ts; separate so it can be unit-tested). highlight.js core +
// one lazily imported language chunk per id. For long files the first 2,000 lines are highlighted on their own
// first (fast first paint), then the whole file in one pass (correct state across chunk borders; hljs 11 has no
// continuation API) and every chunk is posted, replacing the provisional first one.

import hljs from 'highlight.js/lib/core';
import type { LangId } from './languages';
import { LOADERS } from './loaders';
import { CHUNK_LINES, MAX_BYTES, MAX_LINES, type FromWorker } from './protocol';
import { splitLines } from './splitLines';

const loaded = new Map<LangId, Promise<void>>();

export function ensureLanguage(id: LangId): Promise<void> {
  let p = loaded.get(id);
  if (!p) {
    p = LOADERS[id]().then((m) => {
      hljs.registerLanguage(id, m.default);
    });
    loaded.set(id, p);
  }
  return p;
}

export function highlightLines(text: string, langId: LangId): string[] {
  const html = hljs.highlight(text, { language: langId, ignoreIllegals: true }).value;
  const lines = splitLines(html);
  if (text.endsWith('\n') && lines.length > 0 && lines[lines.length - 1] === '') lines.pop();
  return lines;
}

const yieldTask = () => new Promise<void>((r) => setTimeout(r, 0));

export async function runHighlight(
  id: number,
  langId: LangId,
  text: string,
  post: (m: FromWorker) => void,
  isCancelled: () => boolean,
): Promise<void> {
  let count = 1;
  for (let i = 0; i < text.length && count <= MAX_LINES; i++) if (text.charCodeAt(i) === 10) count++;
  if (text.length > MAX_BYTES || count > MAX_LINES) {
    post({ type: 'plain', id, reason: 'tooLarge' });
    return;
  }
  if (!(langId in LOADERS)) {
    post({ type: 'plain', id, reason: 'unknownLanguage' });
    return;
  }
  await ensureLanguage(langId);
  if (isCancelled()) return;
  if (count > CHUNK_LINES) {
    let cut = 0;
    for (let n = 0; n < CHUNK_LINES; n++) cut = text.indexOf('\n', cut) + 1;
    post({ type: 'chunk', id, start: 0, lines: highlightLines(text.slice(0, cut), langId) });
    await yieldTask();
    if (isCancelled()) return;
  }
  const all = highlightLines(text, langId);
  for (let start = 0; start < all.length; start += CHUNK_LINES) {
    if (isCancelled()) return;
    post({ type: 'chunk', id, start, lines: all.slice(start, start + CHUNK_LINES) });
    if (start + CHUNK_LINES < all.length) await yieldTask();
  }
  post({ type: 'done', id });
}
