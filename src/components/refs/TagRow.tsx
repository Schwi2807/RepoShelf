// Tag row (FR-30, S8): name (tree link), date, short oid (commit link), annotated / lightweight badge, tagger, and
// "Browse files" in place of GitHub's zip icons.

import { FileCode2, Tag } from 'lucide-react';
import type { TagRow as Row } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { shortOid } from '../../util/format';
import { Badge } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';

export function TagRow({ row, owner, repo }: { row: Row; owner: string; repo: string }) {
  const ref = `refs/tags/${row.item.name}`;
  return (
    <li className="trow" data-tag={row.item.name}>
      <div className="trow__main">
        <div className="trow__title">
          <Tag size={16} aria-hidden="true" className="trow__icon" />
          <Link to={href('tree', { owner, repo, ref, path: '' })} className="trow__name truncate" title={row.item.fullName}>
            {row.item.name}
          </Link>
          {row.annotated ? (
            <Badge tone="indigo" title="Annotated tag: has its own message, tagger and date">
              annotated
            </Badge>
          ) : (
            <Badge title="Lightweight tag: a plain name for a commit">lightweight</Badge>
          )}
        </div>
        <div className="trow__meta">
          <RelativeTime secs={row.date} />
          {row.tagger && (
            <>
              {' · tagged by '}
              <strong>{row.tagger.name}</strong>
            </>
          )}
          {' · '}
          <Link to={href('commit', { owner, repo, oid: row.commit.oid })} className="mono trow__oid" title={row.commit.summary}>
            {shortOid(row.commit.oid)}
          </Link>
          <span className="trow__summary truncate"> {row.commit.summary}</span>
        </div>
      </div>
      <Link to={href('tree', { owner, repo, ref, path: '' })} className="btn btn--sm">
        <FileCode2 size={16} aria-hidden="true" />
        Browse files
      </Link>
    </li>
  );
}
