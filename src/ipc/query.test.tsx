import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setBackendForTests, type Backend } from './bridge';
import type { ArgsOf, CommandName, RetOf } from './commands';
import { CACHE_LIMIT, cacheSize, invalidate, prime, qk, resetQueryCache, useQuery, type QueryState } from './query';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

interface Pending {
  cmd: CommandName;
  args: unknown;
  resolve: (v: unknown) => void;
  reject: (e: unknown) => void;
}

function fakeBackend() {
  const pending: Pending[] = [];
  const calls: { cmd: CommandName; args: unknown }[] = [];
  const b: Backend = {
    invoke<K extends CommandName>(cmd: K, args: ArgsOf<K>): Promise<RetOf<K>> {
      calls.push({ cmd, args });
      if (cmd === 'request_cancel') {
        const id = (args as { requestId: string }).requestId;
        const p = pending.find((x) => (x.args as { requestId?: string }).requestId === id);
        p?.reject({ code: 'cancelled', message: 'cancelled' });
        return Promise.resolve(true as RetOf<K>);
      }
      return new Promise<RetOf<K>>((resolve, reject) => {
        pending.push({ cmd, args, resolve: resolve as (v: unknown) => void, reject });
      });
    },
    listen: () => () => undefined,
  };
  return { b, pending, calls };
}

const flush = () => act(async () => {
  await new Promise((r) => setTimeout(r, 0));
});

let root: Root;
let host: HTMLElement;
let seen: QueryState<number>[];

function Probe({ k, cancelable, stable }: { k: string | null; cancelable?: boolean; stable?: boolean }) {
  const q = useQuery<number>(
    k,
    (requestId) =>
      import('./api').then((api) => (cancelable ? api.commitCount(1, 'main', requestId) : api.appInfo().then((i) => i.repoCount))),
    { cancelable, stable },
  );
  seen.push(q);
  return <span>{q.data ?? '-'}</span>;
}

beforeEach(() => {
  resetQueryCache();
  seen = [];
  host = document.createElement('div');
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  setBackendForTests(null);
  vi.restoreAllMocks();
});

describe('useQuery', () => {
  it('serves a cache hit without a second call', async () => {
    const f = fakeBackend();
    setBackendForTests(f.b);
    await act(async () => root.render(<Probe k="count:a" cancelable />));
    await flush();
    expect(f.pending).toHaveLength(1);
    await act(async () => f.pending[0].resolve(42));
    await flush();
    expect(host.textContent).toBe('42');
    await act(async () => root.render(<Probe k={null} />));
    await act(async () => root.render(<Probe k="count:a" cancelable />));
    await flush();
    expect(host.textContent).toBe('42');
    expect(f.calls.filter((c) => c.cmd === 'commit_count')).toHaveLength(1);
  });

  it('cancels an in-flight request on unmount via request_cancel and never surfaces `cancelled`', async () => {
    const f = fakeBackend();
    setBackendForTests(f.b);
    await act(async () => root.render(<Probe k="count:b" cancelable />));
    await flush();
    const rid = (f.pending[0].args as { requestId: string }).requestId;
    expect(rid).toMatch(/^[A-Za-z0-9_-]{1,64}$/);
    await act(async () => root.render(<Probe k="count:other" cancelable />));
    await flush();
    const cancels = f.calls.filter((c) => c.cmd === 'request_cancel');
    expect(cancels).toEqual([{ cmd: 'request_cancel', args: { requestId: rid } }]);
    expect(seen.every((s) => s.error === undefined)).toBe(true);
    // Back to the first key: a fresh attempt with a new requestId.
    await act(async () => root.render(<Probe k="count:b" cancelable />));
    await flush();
    const again = f.calls.filter((c) => c.cmd === 'commit_count').map((c) => (c.args as { requestId: string }).requestId);
    expect(again).toHaveLength(3);
    expect(new Set(again).size).toBe(3);
  });

  it('does not cancel non-cancellable queries', async () => {
    const f = fakeBackend();
    setBackendForTests(f.b);
    await act(async () => root.render(<Probe k="info" />));
    await flush();
    await act(async () => root.render(<Probe k={null} />));
    await flush();
    expect(f.calls.some((c) => c.cmd === 'request_cancel')).toBe(false);
  });

  it('invalidate refetches ref-keyed entries but not oid-keyed ones', async () => {
    const f = fakeBackend();
    setBackendForTests(f.b);
    await act(async () =>
      root.render(
        <>
          <Probe k="ref-keyed" />
          <Probe k="oid-keyed" stable />
        </>,
      ),
    );
    await flush();
    expect(f.pending).toHaveLength(2);
    await act(async () => {
      f.pending[0].resolve({ repoCount: 1 });
      f.pending[1].resolve({ repoCount: 2 });
    });
    await flush();
    expect(host.textContent).toBe('12');
    await act(async () => invalidate());
    await flush();
    expect(f.pending).toHaveLength(3);
    // old data stays visible while refetching
    expect(host.textContent).toBe('12');
    await act(async () => f.pending[2].resolve({ repoCount: 7 }));
    await flush();
    expect(host.textContent).toBe('72');
  });

  it('an invalidate during an in-flight request runs one more request after it (M13 race)', async () => {
    const f = fakeBackend();
    setBackendForTests(f.b);
    await act(async () => root.render(<Probe k="raced" />));
    await flush();
    expect(f.pending).toHaveLength(1);
    // e.g. a release download finishes while the list refetch of the previous job is still running
    await act(async () => invalidate());
    await act(async () => invalidate());
    await flush();
    expect(f.pending).toHaveLength(1);
    await act(async () => f.pending[0].resolve({ repoCount: 1 }));
    await flush();
    expect(f.pending).toHaveLength(2);
    await act(async () => f.pending[1].resolve({ repoCount: 2 }));
    await flush();
    expect(host.textContent).toBe('2');
    expect(f.pending).toHaveLength(2);
  });

  it('keeps errors (non-cancelled) visible', async () => {
    const f = fakeBackend();
    setBackendForTests(f.b);
    await act(async () => root.render(<Probe k="err" />));
    await flush();
    await act(async () => f.pending[0].reject({ code: 'notFound', message: 'ref not found' }));
    await flush();
    expect(seen[seen.length - 1].error).toEqual({ code: 'notFound', message: 'ref not found' });
  });

  it('evicts beyond the LRU limit', () => {
    for (let i = 0; i < CACHE_LIMIT + 25; i++) prime(qk('x', i), i);
    expect(cacheSize()).toBe(CACHE_LIMIT);
  });
});
