// Shared M5 placeholder body for repo tabs: shows what RepoLayout resolved so the shell can be checked end to end.
// Replaced screen by screen in M6/M7.

import { useRepo } from '../components/layout/repoContext';
import { Box } from '../components/layout/Sidebar';
import { Shimmer } from '../components/ui/bits';
import { href } from '../router/router';
import { Link } from '../router/Link';
import { shortOid } from '../util/format';

export function RepoPlaceholder({ title, detail }: { title: string; detail?: string }) {
  const r = useRepo();
  let body;
  if (r.detail?.head.type === 'unborn') {
    body = <p className="muted">This repository is empty.</p>;
  } else if (r.revError) {
    const def = r.summary.defaultBranch;
    body = (
      <p>
        <span className="muted">{r.revError.code === 'notFound' ? `Ref “${r.refName ?? ''}” not found.` : r.revError.message}</span>{' '}
        {def && <Link to={href('tree', { owner: r.owner, repo: r.repo, ref: def, path: '' })}>Go to {def}</Link>}
      </p>
    );
  } else if (!r.resolved) {
    body = <Shimmer width={240} />;
  } else {
    body = (
      <p className="muted">
        {r.resolved.kind} <span className="mono">{r.refName}</span> at <span className="mono">{shortOid(r.resolved.commit)}</span>
        {detail ? ` · ${detail}` : ''}
      </p>
    );
  }
  return (
    <Box header={<span className="box__title">{title}</span>}>
      <div className="box__body">{body}</div>
    </Box>
  );
}
