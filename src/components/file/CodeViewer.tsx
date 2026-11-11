// Code viewer (FR-22, NFR-P4, D-5, D-8): line-number gutter + lines. Raw lines render first (React text nodes); the
// highlighter worker's chunks swap in per line (the only HTML from file content). Click a number selects `?L=n`,
// shift-click extends the range; the first load scrolls the selection into view. > 500 lines are virtualised against
// the window (fixed 20 px rows, no wrapping; the width comes from the longest line).

import { useWindowVirtualizer } from '@tanstack/react-virtual';
import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type MouseEvent, type RefObject } from 'react';
import { highlight } from '../../hl/highlighter';
import type { LangId } from '../../hl/languages';
import { VIRTUAL_THRESHOLD } from '../ui/WindowList';
import { maxColumns, nextSelection, splitTextLines, type LineRange } from './lines';

export const LINE_H = 20;
/** Sticky global header (60 px) + some context above the selected line. */
const SCROLL_OFFSET = 60 + 96;

export interface CodeViewerProps {
  text: string;
  /** null = plain (unknown language, too large, or highlighting off). */
  langId: LangId | null;
  selection: LineRange | null;
  /** Href of a line selection (`?L=`), for the gutter anchors. */
  hrefFor: (r: LineRange) => string;
  onSelect: (r: LineRange) => void;
  /** Scroll the selection into view once, after the first render (skipped on back/forward scroll restore). */
  scrollToSelection: boolean;
}

export function CodeViewer({ text, langId, selection, hrefFor, onSelect, scrollToSelection }: CodeViewerProps) {
  const lines = useMemo(() => splitTextLines(text), [text]);
  const [hl, setHl] = useState<(string | undefined)[]>([]);
  const host = useRef<HTMLDivElement>(null);
  const virtual = lines.length > VIRTUAL_THRESHOLD;

  useEffect(() => {
    setHl([]);
    if (langId === null) return;
    const job = highlight(text, langId, (start, chunk) => {
      setHl((prev) => {
        const next = prev.slice();
        for (let i = 0; i < chunk.length; i++) next[start + i] = chunk[i];
        return next;
      });
    });
    return () => job.cancel();
  }, [text, langId]);

  const onGutter = (e: MouseEvent<HTMLAnchorElement>, n: number) => {
    e.preventDefault();
    onSelect(nextSelection(selection, n, e.shiftKey));
  };

  // First-load anchor scroll (D-8). Runs once; later selections never move the page.
  const scrolled = useRef(false);
  // Deferred two frames: with cached data this viewer commits together with the route, and the router's
  // scroll-to-top (markRendered, a parent layout effect) and the window virtualiser's first scroll run after it.
  useEffect(() => {
    if (scrolled.current || !scrollToSelection || !selection) return;
    const start = selection.start;
    let f2 = 0;
    const f1 = requestAnimationFrame(() => {
      f2 = requestAnimationFrame(() => {
        if (!host.current || scrolled.current) return;
        scrolled.current = true;
        const top = host.current.getBoundingClientRect().top + window.scrollY + (start - 1) * LINE_H;
        window.scrollTo(0, Math.max(0, top - SCROLL_OFFSET));
      });
    });
    return () => {
      cancelAnimationFrame(f1);
      cancelAnimationFrame(f2);
    };
  }, [scrollToSelection, selection]);

  const cols = useMemo(() => maxColumns(lines), [lines]);
  const digits = String(lines.length).length;
  const gutterCh = Math.max(digits, 2) + 2;

  return (
    <div className="cv" ref={host} data-testid="code-viewer">
      <div className="cv__scroll">
        <div className="cv__inner code" style={{ minWidth: `calc(${cols + gutterCh + 4}ch + 32px)`, ['--gutter-ch' as string]: `${gutterCh}ch` }}>
          {virtual ? (
            <VirtualLines lines={lines} hl={hl} selection={selection} hrefFor={hrefFor} onGutter={onGutter} host={host} />
          ) : (
            lines.map((l, i) => (
              <Line key={i} n={i + 1} text={l} html={hl[i]} sel={isSel(selection, i + 1)} edge={edgeOf(selection, i + 1)} href={hrefFor({ start: i + 1, end: i + 1 })} onGutter={onGutter} />
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function isSel(s: LineRange | null, n: number): boolean {
  return s !== null && n >= s.start && n <= s.end;
}

function edgeOf(s: LineRange | null, n: number): 'first' | 'last' | 'both' | undefined {
  if (!s) return undefined;
  if (n === s.start && n === s.end) return 'both';
  if (n === s.start) return 'first';
  if (n === s.end) return 'last';
  return undefined;
}

function VirtualLines({
  lines,
  hl,
  selection,
  hrefFor,
  onGutter,
  host,
}: {
  lines: string[];
  hl: (string | undefined)[];
  selection: LineRange | null;
  hrefFor: (r: LineRange) => string;
  onGutter: (e: MouseEvent<HTMLAnchorElement>, n: number) => void;
  host: RefObject<HTMLDivElement | null>;
}) {
  const [margin, setMargin] = useState(0);
  useLayoutEffect(() => {
    const el = host.current;
    if (!el) return;
    const measure = () => setMargin(el.getBoundingClientRect().top + window.scrollY);
    measure();
    // content above (commit header, notices) can arrive later: re-measure when the page layout changes
    const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    ro?.observe(document.body);
    return () => ro?.disconnect();
  }, [host]);
  const v = useWindowVirtualizer({ count: lines.length, estimateSize: () => LINE_H, overscan: 40, scrollMargin: margin });
  return (
    <div className="cv__virtual" style={{ height: v.getTotalSize() }}>
      {v.getVirtualItems().map((it) => {
        const n = it.index + 1;
        return (
          <div key={it.key} className="cv__vrow" style={{ transform: `translateY(${it.start - margin}px)` }}>
            <Line n={n} text={lines[it.index]} html={hl[it.index]} sel={isSel(selection, n)} edge={edgeOf(selection, n)} href={hrefFor({ start: n, end: n })} onGutter={onGutter} />
          </div>
        );
      })}
    </div>
  );
}

const Line = memo(function Line({
  n,
  text,
  html,
  sel,
  edge,
  href,
  onGutter,
}: {
  n: number;
  text: string;
  html: string | undefined;
  sel: boolean;
  edge: 'first' | 'last' | 'both' | undefined;
  href: string;
  onGutter: (e: MouseEvent<HTMLAnchorElement>, n: number) => void;
}) {
  return (
    <div className={`cv__line${sel ? ' cv__line--sel' : ''}${edge ? ` cv__line--${edge}` : ''}`} data-line={n} id={`L${n}`}>
      <a className="cv__num" href={href} onClick={(e) => onGutter(e, n)} aria-label={`Line ${n}`} tabIndex={-1}>
        {n}
      </a>
      {html !== undefined ? (
        // highlighter worker output only: escaped text inside <span class="hljs-…"> (D-5)
        <span className="cv__code" dangerouslySetInnerHTML={{ __html: html || ' ' }} />
      ) : (
        <span className="cv__code">{text.length > 0 ? text : ' '}</span>
      )}
    </div>
  );
});
