// One commit in the list (FR-26, S5): summary (link) with an expandable description, author + "committed …", and on
// the right the short oid (link + copy) and a browse-the-tree button. The description is not part of
// `CommitSummary`, so it is loaded on first expand from `commit_detail` (D-78; same cache key as the commit page).

import { Check, Copy, Ellipsis, FileCode2 } from 'lucide-react';
import { useState } from 'react';
import { commitDetail } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { CommitSummary } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { shortOid } from '../../util/format';
import { Avatar, Shimmer } from '../ui/bits';
import { copyText } from '../ui/CopyButton';
import { RelativeTime } from '../ui/RelativeTime';

/** Everything after the summary line, trimmed ("" when the message is one line). */
export function messageBody(message: string): string {
  const i = message.indexOf('\n');
  return i < 0 ? '' : message.slice(i + 1).replace(/^\s*\n/, '').trimEnd();
}

/** "committed" / "authored and <committer> committed" (GitHub wording). */
export function committedText(c: CommitSummary): string {
  const same = c.author.name === c.committer.name && c.author.email === c.committer.email;
  return same ? 'committed' : `authored and ${c.committer.name} committed`;
}

export function CopyOid({ oid, owner, repo }: { oid: string; owner: string; repo: string }) {
  const [ok, setOk] = useState(false);
  return (
    <span className="oidbtn">
      <Link to={href('commit', { owner, repo, oid })} className="oidbtn__link mono" title={`View commit ${oid}`}>
        {shortOid(oid)}
      </Link>
      <button
        type="button"
        className={`oidbtn__copy${ok ? ' copy-ok' : ''}`}
        aria-label={ok ? 'Copied' : `Copy full commit id ${shortOid(oid)}`}
        title={ok ? 'Copied' : 'Copy full commit id'}
        onClick={async () => {
          if (await copyText(oid)) {
            setOk(true);
            setTimeout(() => setOk(false), 1500);
          }
        }}
      >
        {ok ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
      </button>
    </span>
  );
}

function Body({ repoId, oid }: { repoId: number; oid: string }) {
  const d = useQuery(qk('commit_detail', repoId, oid), () => commitDetail(repoId, oid), { stable: true });
  if (d.error) return <p className="crow__body field__error">{d.error.message}</p>;
  if (!d.data) {
    return (
      <div className="crow__body">
        <Shimmer width="60%" block />
      </div>
    );
  }
  const body = messageBody(d.data.message);
  return body ? <pre className="crow__body">{body}</pre> : <p className="crow__body muted">No description.</p>;
}

export function CommitRow({ c, repoId, owner, repo }: { c: CommitSummary; repoId: number; owner: string; repo: string }) {
  const [open, setOpen] = useState(false);
  const to = href('commit', { owner, repo, oid: c.oid });
  return (
    <li className="crow" data-oid={c.oid}>
      <div className="crow__main">
        <div className="crow__title">
          <Link to={to} className="crow__summary" title={c.summary}>
            {c.summary}
          </Link>
          <button
            type="button"
            className="crow__more"
            aria-expanded={open}
            aria-label={open ? 'Hide description' : 'Show description'}
            title={open ? 'Hide description' : 'Show description'}
            onClick={() => setOpen((o) => !o)}
          >
            <Ellipsis size={14} aria-hidden="true" />
          </button>
        </div>
        {open && <Body repoId={repoId} oid={c.oid} />}
        <div className="crow__meta">
          <Avatar name={c.author.name} size="sm" />
          <strong className="crow__author" title={c.author.email}>
            {c.author.name}
          </strong>
          <span>
            {committedText(c)} <RelativeTime secs={c.committer.time.secs} />
          </span>
        </div>
      </div>
      <div className="crow__actions">
        <CopyOid oid={c.oid} owner={owner} repo={repo} />
        <Link
          to={href('tree', { owner, repo, ref: c.oid, path: '' })}
          className="btn btn--sm btn--icon btn--invisible"
          aria-label="Browse the repository at this point in the history"
          title="Browse the repository at this point in the history"
        >
          <FileCode2 size={16} aria-hidden="true" />
        </Link>
      </div>
    </li>
  );
}
