// +A −D with five diffstat blocks (S6 card header, file list).

import { formatCount } from '../../util/format';
import { miniBlocks } from './diffs';

export function MiniBar({ additions, deletions, numbers = true }: { additions: number; deletions: number; numbers?: boolean }) {
  return (
    <span className="minibar" title={`${formatCount(additions)} additions, ${formatCount(deletions)} deletions`}>
      {numbers && (
        <>
          <span className="minibar__add">+{formatCount(additions)}</span>
          <span className="minibar__del">−{formatCount(deletions)}</span>
        </>
      )}
      <span className="minibar__blocks" aria-hidden="true">
        {miniBlocks(additions, deletions).map((b, i) => (
          <span key={i} className={`minibar__b minibar__b--${b}`} />
        ))}
      </span>
    </span>
  );
}
