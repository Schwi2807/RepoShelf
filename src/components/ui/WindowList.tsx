// Long lists (> 500 rows) virtualised against the WINDOW scroll (D-31: `useWindowVirtualizer`), so the router's
// scroll memory restores them on back/forward like any other page. Short lists render plainly.

import { useWindowVirtualizer } from '@tanstack/react-virtual';
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';

export const VIRTUAL_THRESHOLD = 500;

export interface WindowListProps {
  count: number;
  /** Row height estimate in px (rows are measured after render). */
  estimate: number;
  row: (index: number) => ReactNode;
  /** Stable key per row. */
  rowKey?: (index: number) => string | number;
  threshold?: number;
  className?: string;
}

export function WindowList({ count, estimate, row, rowKey, threshold = VIRTUAL_THRESHOLD, className }: WindowListProps) {
  if (count <= threshold) {
    return (
      <div className={className}>
        {Array.from({ length: count }, (_, i) => (
          <div key={rowKey ? rowKey(i) : i}>{row(i)}</div>
        ))}
      </div>
    );
  }
  return <Virtual count={count} estimate={estimate} row={row} rowKey={rowKey} className={className} />;
}

function Virtual({ count, estimate, row, rowKey, className }: Omit<WindowListProps, 'threshold'>) {
  const ref = useRef<HTMLDivElement>(null);
  const [margin, setMargin] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el) setMargin(el.getBoundingClientRect().top + window.scrollY);
  }, []);
  const v = useWindowVirtualizer({
    count,
    estimateSize: () => estimate,
    overscan: 12,
    scrollMargin: margin,
    getItemKey: rowKey,
  });
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
