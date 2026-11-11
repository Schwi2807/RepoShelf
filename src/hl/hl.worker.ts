// Highlighter worker (D-5). The only path besides the ammonia-sanitised markdown that produces HTML from file
// content; hljs escapes the text and emits only `<span class="hljs-…">`.

import { runHighlight } from './engine';
import type { FromWorker, ToWorker } from './protocol';

interface WorkerScope {
  postMessage(m: FromWorker): void;
  onmessage: ((e: MessageEvent<ToWorker>) => void) | null;
}

const scope = self as unknown as WorkerScope;
const cancelled = new Set<number>();

scope.onmessage = (e) => {
  const m = e.data;
  if (m.type === 'cancel') {
    cancelled.add(m.id);
    return;
  }
  runHighlight(m.id, m.langId, m.text, (out) => scope.postMessage(out), () => cancelled.has(m.id))
    .catch((err: unknown) =>
      scope.postMessage({ type: 'error', id: m.id, message: err instanceof Error ? err.message : String(err) }),
    )
    .finally(() => cancelled.delete(m.id));
};
