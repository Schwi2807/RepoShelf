// Remote README / release-note images (v0.2.0 F2, D-105, D-109). The webview never loads them itself (CSP and
// NFR-S2 unchanged): Rust fetches and caches (`image_fetch` → key, mime), the bytes come over IPC (`image_bytes`)
// and become a `blob:` URL. One loader for the whole page, because a Releases page mounts many MarkdownBody
// instances: at most MAX_PARALLEL requests in flight (the backend has no limit), in-flight requests for the same
// URL are shared, and a small LRU of Blobs makes a second view instant. Each mount makes (and revokes) its own
// object URL.

import { imageBytes, imageFetch } from '../ipc/api';

export const MAX_PARALLEL = 4;
const BLOB_LRU = 64;

type Task = () => Promise<void>;
let active = 0;
const queue: Task[] = [];
const inflight = new Map<string, { promise: Promise<Blob>; wanted: (() => boolean)[] }>();
const done = new Map<string, Blob>();

function pump(): void {
  while (active < MAX_PARALLEL && queue.length > 0) {
    const task = queue.shift()!;
    active += 1;
    void task().finally(() => {
      active -= 1;
      pump();
    });
  }
}

function remember(url: string, blob: Blob): void {
  done.delete(url);
  done.set(url, blob);
  while (done.size > BLOB_LRU) {
    const oldest = done.keys().next().value;
    if (oldest === undefined) break;
    done.delete(oldest);
  }
}

/**
 * The image at `url` as a Blob (mime from the backend's sniffed allow-list, never the server header). Rejects with
 * the IPC error (`refused`, `network`, `invalidUrl`, …); the caller keeps the chip then. `isWanted` is asked when
 * the request reaches the front of the queue, so images of an unmounted view are skipped without a request.
 */
export function loadRemoteImage(url: string, isWanted: () => boolean = () => true): Promise<Blob> {
  const hit = done.get(url);
  if (hit) {
    remember(url, hit);
    return Promise.resolve(hit);
  }
  const running = inflight.get(url);
  if (running) {
    running.wanted.push(isWanted);
    return running.promise;
  }
  const wanted = [isWanted];
  const p = new Promise<Blob>((resolve, reject) => {
    queue.push(async () => {
      if (!wanted.some((w) => w())) {
        reject(new Error('not needed any more'));
        return;
      }
      try {
        const img = await imageFetch(url);
        const bytes = await imageBytes(img.key);
        const blob = new Blob([bytes], { type: img.mime });
        remember(url, blob);
        resolve(blob);
      } catch (e) {
        reject(e);
      }
    });
    pump();
  }).finally(() => inflight.delete(url));
  inflight.set(url, { promise: p, wanted });
  return p;
}

/** Tests: requests running right now (never above MAX_PARALLEL). */
export function activeRequests(): number {
  return active;
}

/** Tests / "Clear cache": forget the in-memory Blobs. */
export function resetRemoteImages(): void {
  done.clear();
}
