// Line selection in the route (D-8): `?L=10` or `?L=10-20` (1-based, inclusive).

export interface LineRange {
  start: number;
  end: number;
}

/** Parse `10`, `10-20`, also tolerant of `L10-L20`; reversed ranges are ordered; junk → null. */
export function parseLines(v: string | undefined | null): LineRange | null {
  if (!v) return null;
  const m = /^L?(\d{1,7})(?:-L?(\d{1,7}))?$/i.exec(v.trim());
  if (!m) return null;
  const a = Number(m[1]);
  const b = m[2] === undefined ? a : Number(m[2]);
  if (a < 1 || b < 1) return null;
  return { start: Math.min(a, b), end: Math.max(a, b) };
}

export function formatLines(r: LineRange): string {
  return r.start === r.end ? String(r.start) : `${r.start}-${r.end}`;
}

/** Click on line `n`: plain click selects it; shift-click extends from the current selection's start. */
export function nextSelection(current: LineRange | null, n: number, shift: boolean): LineRange {
  if (!shift || current === null) return { start: n, end: n };
  return { start: Math.min(current.start, n), end: Math.max(current.start, n) };
}

/** Split file text into lines (a final newline does not make an extra empty line). */
export function splitTextLines(text: string): string[] {
  if (text.length === 0) return [''];
  const lines = text.split('\n');
  if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
  return lines.map((l) => (l.endsWith('\r') ? l.slice(0, -1) : l));
}

/** Longest line in columns (tabs = 4), for the horizontal scroll width of the virtualised viewer. */
export function maxColumns(lines: string[]): number {
  let max = 0;
  for (const l of lines) {
    let n = 0;
    for (let i = 0; i < l.length; i++) n += l.charCodeAt(i) === 9 ? 4 - (n % 4) : 1;
    if (n > max) max = n;
  }
  return max;
}
