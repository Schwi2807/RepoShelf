// Breadcrumbs (FR-21): repo / dir / … / name, each segment a tree link, the last one plain; "Copy path".

import { Fragment } from 'react';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { pathSegments } from '../../util/paths';
import { CopyButton } from '../ui/CopyButton';

export interface BreadcrumbsProps {
  owner: string;
  repo: string;
  refName: string;
  path: string;
  /** Show the Copy path button. */
  copy?: boolean;
}

export function Breadcrumbs({ owner, repo, refName, path, copy = true }: BreadcrumbsProps) {
  const segs = pathSegments(path);
  return (
    <nav className="crumbs" aria-label="Breadcrumbs">
      <Link to={href('tree', { owner, repo, ref: refName, path: '' })} className="crumbs__root">
        {repo}
      </Link>
      {segs.map((s, i) => (
        <Fragment key={s.path}>
          <span className="crumbs__sep" aria-hidden="true">
            /
          </span>
          {i === segs.length - 1 ? (
            <strong className="crumbs__current" aria-current="page">
              {s.name}
            </strong>
          ) : (
            <Link to={href('tree', { owner, repo, ref: refName, path: s.path })}>{s.name}</Link>
          )}
        </Fragment>
      ))}
      {copy && segs.length > 0 && <CopyButton text={path} label="Copy path" />}
    </nav>
  );
}
