// Releases | Tags sub-toggle (S8, S9), as on GitHub: two links styled as a segmented control.

import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { Counter } from '../ui/bits';

export function ReleasesTagsToggle({
  owner,
  repo,
  current,
  releases,
  tags,
}: {
  owner: string;
  repo: string;
  current: 'releases' | 'tags';
  releases: number | null;
  tags: number | null;
}) {
  return (
    <nav className="segmented" aria-label="Releases or tags">
      <Link
        to={href('releases', { owner, repo })}
        className="segmented__item"
        aria-selected={current === 'releases'}
        aria-current={current === 'releases' ? 'page' : undefined}
      >
        Releases <Counter n={releases} />
      </Link>
      <Link
        to={href('tags', { owner, repo })}
        className="segmented__item"
        aria-selected={current === 'tags'}
        aria-current={current === 'tags' ? 'page' : undefined}
      >
        Tags <Counter n={tags} />
      </Link>
    </nav>
  );
}
