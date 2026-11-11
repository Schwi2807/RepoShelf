// Unified diff table (FR-27, S6): old no | new no | marker | text. Hunk header rows are tinted; added / removed lines
// use the diff tokens. Plain text only (D-17: no highlighting) — lines are React text, never HTML.

import type { DiffLine, FileDiff } from '../../ipc/types';

const MARK: Record<DiffLine['kind'], string> = { ctx: ' ', add: '+', del: '-', noNewline: '' };

function Line({ l }: { l: DiffLine }) {
  if (l.kind === 'noNewline') {
    return (
      <tr className="dtable__line dtable__line--nonl">
        <td className="dtable__num" />
        <td className="dtable__num" />
        <td className="dtable__mark" />
        <td className="dtable__code">
          <span className="dtable__nonl" title={l.text}>
            No newline at end of file
          </span>
        </td>
      </tr>
    );
  }
  return (
    <tr className={`dtable__line dtable__line--${l.kind}`}>
      <td className="dtable__num" data-n={l.oldNo ?? undefined}>
        {l.oldNo ?? ''}
      </td>
      <td className="dtable__num" data-n={l.newNo ?? undefined}>
        {l.newNo ?? ''}
      </td>
      <td className="dtable__mark" aria-hidden="true">
        {MARK[l.kind]}
      </td>
      <td className="dtable__code">{l.text.length > 0 ? l.text : '​'}</td>
    </tr>
  );
}

export function DiffTable({ diff, path }: { diff: FileDiff; path: string }) {
  if (diff.hunks.length === 0) {
    return <p className="dcard__note muted">No content changes (only the file name or mode changed).</p>;
  }
  return (
    <div className="dtable__scroll">
      <table className="dtable" aria-label={`Changes in ${path}`}>
        <colgroup>
          <col className="dtable__col-num" />
          <col className="dtable__col-num" />
          <col className="dtable__col-mark" />
          <col />
        </colgroup>
        <tbody>
          {diff.hunks.map((h, hi) => [
            <tr key={`h${hi}`} className="dtable__hunk">
              <td className="dtable__num dtable__num--hunk" colSpan={2} />
              <td className="dtable__code dtable__hunktext" colSpan={2}>
                {h.header}
              </td>
            </tr>,
            ...h.lines.map((l, li) => <Line key={`${hi}-${li}`} l={l} />),
          ])}
        </tbody>
      </table>
      {diff.truncated && <p className="dcard__note muted">The diff is cut off here: the file has more changed lines than are shown.</p>}
    </div>
  );
}
