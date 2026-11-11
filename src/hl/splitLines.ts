// hljs output → one self-contained HTML string per line: spans still open at a `\n` are closed at the end of the
// line and reopened at the start of the next (ARCH §7.6). Input contains only text, entities and
// `<span class="…">` / `</span>` (hljs escapes the source).

export function splitLines(html: string): string[] {
  const lines: string[] = [];
  const open: string[] = [];
  let cur = '';
  let i = 0;
  while (i < html.length) {
    const c = html[i];
    if (c === '<') {
      const end = html.indexOf('>', i);
      if (end < 0) {
        cur += html.slice(i);
        break;
      }
      const tag = html.slice(i, end + 1);
      if (tag.startsWith('</')) open.pop();
      else open.push(tag);
      cur += tag;
      i = end + 1;
    } else if (c === '\n') {
      cur += '</span>'.repeat(open.length);
      lines.push(cur);
      cur = open.join('');
      i += 1;
    } else {
      const next = html.slice(i).search(/[<\n]/);
      const stop = next < 0 ? html.length : i + next;
      cur += html.slice(i, stop);
      i = stop;
    }
  }
  lines.push(cur);
  return lines;
}

/** Escape plain text for the raw (unhighlighted) first paint. */
export function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
