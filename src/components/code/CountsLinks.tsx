// "N branches · N tags" links beside the ref switcher (S3).

import { GitBranch, Tag } from 'lucide-react';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount } from '../../util/format';

export function CountsLinks({ owner, repo, branches, tags }: { owner: string; repo: string; branches: number; tags: number }) {
  return (
    <div className="counts-links">
      <Link to={href('branches', { owner, repo })} className="counts-links__item">
        <GitBranch size={16} aria-hidden="true" />
        <strong>{formatCount(branches)}</strong> {branches === 1 ? 'Branch' : 'Branches'}
      </Link>
      <Link to={href('tags', { owner, repo })} className="counts-links__item">
        <Tag size={16} aria-hidden="true" />
        <strong>{formatCount(tags)}</strong> {tags === 1 ? 'Tag' : 'Tags'}
      </Link>
    </div>
  );
}
