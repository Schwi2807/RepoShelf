// Long lists (> 500 rows) virtualised against the WINDOW scroll (D-31: `useWindowVirtualizer`), so the router's
// scroll memory restores them on back/forward like any other page. Short lists render plainly.
// `scrollTo` (M7, `?file=N` on the commit page) brings row N to the top, below the sticky global header.

import { useWindowVirtualizer } from '@tanstack/react-virtual';
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

export const VIRTUAL_THRESHOLD = 500;
/** Space kept above a row scrolled to the top: the sticky global header plus a little air. */
export const SCROLL_TOP_GAP = 76;

export interface WindowListProps {
  count: number;
  /** Row height estimate in px (rows are measured after render). */
  estimate: number;
  row: (index: number) => ReactNode;
  /** Stable key per row. */
  rowKey?: (index: number) => string | number;
  threshold?: number;
  className?: string;
  /** Scroll row `index` to the top whenever `seq` changes. */
  scrollTo?: { index: number; seq: number } | null;
}

export function WindowList({ count, estimate, row, rowKey, threshold = VIRTUAL_THRESHOLD, className, scrollTo }: WindowListProps) {
  if (count <= threshold) return <Plain count={count} row={row} rowKey={rowKey} className={className} scrollTo={scrollTo} />;
  return <Virtual count={count} estimate={estimate} row={row} rowKey={rowKey} className={className} scrollTo={scrollTo} />;
}

function Plain({ count, row, rowKey, className, scrollTo }: Omit<WindowListProps, 'threshold' | 'estimate'>) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!scrollTo) return;
    const el = ref.current?.querySelector<HTMLElement>(`:scope > [data-index="${scrollTo.index}"]`);
    if (el) window.scrollTo(0, el.getBoundingClientRect().top + window.scrollY - SCROLL_TOP_GAP);
  }, [scrollTo]);
  return (
    <div ref={ref} className={className}>
      {Array.from({ length: count }, (_, i) => (
        <div key={rowKey ? rowKey(i) : i} data-index={i}>
          {row(i)}
        </div>
      ))}
    </div>
  );
}

function Virtual({ count, estimate, row, rowKey, className, scrollTo }: Omit<WindowListProps, 'threshold'>) {
  const ref = useRef<HTMLDivElement>(null);
  const [margin, setMargin] = useState(0);
  const pendingIndex = useRef<number | null>(null);
  const measure = (): number => {
    const el = ref.current;
    return el ? Math.round(el.getBoundingClientRect().top + window.scrollY) : 0;
  };
  useLayoutEffect(() => {
    setMargin(measure());
    // content above the list can grow later (e.g. the commit page's file list): keep the offset current
    if (typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(() => setMargin((m) => {
      const n = measure();
      return Math.abs(n - m) > 1 ? n : m;
    }));
    ro.observe(document.body);
    return () => ro.disconnect();
  }, []);
  const v = useWindowVirtualizer({
    count,
    estimateSize: () => estimate,
    overscan: 12,
    scrollMargin: margin,
    scrollPaddingStart: SCROLL_TOP_GAP,
    getItemKey: rowKey,
  });
  // A new request: if the list moved since the last measure, re-measure first and scroll on the next render.
  useEffect(() => {
    if (!scrollTo || scrollTo.index >= count) return;
    const m = measure();
    if (Math.abs(m - margin) > 1) {
      pendingIndex.current = scrollTo.index;
      setMargin(m);
    } else {
      v.scrollToIndex(scrollTo.index, { align: 'start' });
    }
    // only when a new request arrives (not when the virtualiser or count change)
  }, [scrollTo]);
  useEffect(() => {
    if (pendingIndex.current === null) return;
    const i = pendingIndex.current;
    pendingIndex.current = null;
    v.scrollToIndex(i, { align: 'start' });
  }, [margin]);
  return (
    <div ref={ref} className={className} style={{ height: v.getTotalSize(), position: 'relative' }}>
      {v.getVirtualItems().map((it) => (
        <div
          key={it.key}
          data-index={it.index}
          ref={v.measureElement}
          style={{ position: 'absolute', top: 0, left: 0, right: 0, transform: `translateY(${it.start - margin}px)` }}
        >
          {row(it.index)}
        </div>
      ))}
    </div>
  );
}
