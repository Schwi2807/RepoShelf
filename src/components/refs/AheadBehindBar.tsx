// Ahead/behind bar (FR-29, S7): behind grows to the left of the centre line, ahead to the right, both scaled to the
// largest count shown on the page. A capped count (the walk stopped at the cap) reads "1,000+".

import type { AheadBehind } from '../../ipc/types';
import { formatCount } from '../../util/format';
import { Shimmer } from '../ui/bits';

/** Bar widths in percent of one half; non-zero counts get at least 3 % so they stay visible. */
export function barWidths(ab: AheadBehind, max: number): { behind: number; ahead: number } {
  const scale = Math.max(1, max, ab.ahead, ab.behind);
  const pct = (n: number) => (n <= 0 ? 0 : Math.max(3, Math.min(100, Math.round((n / scale) * 1000) / 10)));
  return { behind: pct(ab.behind), ahead: pct(ab.ahead) };
}

export function countLabel(n: number, capped: boolean): string {
  return `${formatCount(n)}${capped ? '+' : ''}`;
}

export function AheadBehindBar({ ab, max, base }: { ab: AheadBehind | null | undefined; max: number; base: string }) {
  if (!ab) {
    return (
      <span className="abbar abbar--loading" aria-label="Counting commits ahead and behind">
        <Shimmer width={96} height={8} />
      </span>
    );
  }
  const w = barWidths(ab, max);
  const behind = countLabel(ab.behind, ab.capped && ab.behind >= ab.ahead);
  const ahead = countLabel(ab.ahead, ab.capped && ab.ahead >= ab.behind);
  const title = `${behind} commit${ab.behind === 1 ? '' : 's'} behind, ${ahead} ahead of ${base}${ab.capped ? ' (counting stopped at the cap)' : ''}`;
  return (
    <span className="abbar" title={title} aria-label={title} data-capped={ab.capped || undefined}>
      <span className="abbar__num abbar__num--behind">{behind}</span>
      <span className="abbar__track">
        <span className="abbar__half abbar__half--behind">
          <span className="abbar__fill abbar__fill--behind" style={{ width: `${w.behind}%` }} />
        </span>
        <span className="abbar__mid" aria-hidden="true" />
        <span className="abbar__half">
          <span className="abbar__fill abbar__fill--ahead" style={{ width: `${w.ahead}%` }} />
        </span>
      </span>
      <span className="abbar__num abbar__num--ahead">{ahead}</span>
    </span>
  );
}
