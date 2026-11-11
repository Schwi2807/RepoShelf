// "History for <path> ×" (S5): the path filter from the file view's History link. The path links to the file (or
// folder) at the listed ref; × drops the filter (and the pinned tip / page).

import { FileClock, X } from 'lucide-react';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { commitsHref } from './paging';

export function PathChip({ owner, repo, refName, path, since }: { owner: string; repo: string; refName: string; path: string; since: string }) {
  return (
    <span className="pathchip" data-testid="path-chip">
      <FileClock size={14} aria-hidden="true" />
      <span>History for</span>
      <Link to={href('blob', { owner, repo, ref: refName, path })} className="pathchip__path mono truncate" title={path}>
        {path}
      </Link>
      <Link
        to={commitsHref(owner, repo, refName, { since })}
        className="pathchip__clear"
        aria-label={`Show all commits, not only those touching ${path}`}
        title="Clear the path filter"
      >
        <X size={14} aria-hidden="true" />
      </Link>
    </span>
  );
}
