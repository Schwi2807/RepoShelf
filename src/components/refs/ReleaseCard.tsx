// Release card (FR-31, S9): left column date, tag (tree link), short oid (commit link); right title, Latest /
// Pre-release badge, "Open on GitHub" (cached GitHub answer), a tag chip + commit link + date line, sanitised notes
// through MarkdownBody, the GitHub Assets list (v0.2.0 F4, ReleaseAssets), and "Commits since <prev>" →
// `commits/refs/tags/<tag>?since=refs/tags/<prev>` (D-81: full tag refs so a same-named branch never wins).

import { ExternalLink, GitCommitHorizontal, GitCompareArrows, Tag } from 'lucide-react';
import type { ReactNode } from 'react';
import type { Release } from '../../ipc/types';
import type { RepoSource } from '../../source';
import { MarkdownBody } from '../../markdown/MarkdownBody';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatDay, shortOid } from '../../util/format';
import { commitsHref } from '../commits/paging';
import { Badge } from '../ui/bits';
import { confirmExternal } from '../ui/ConfirmExternalLink';
import { RelativeTime } from '../ui/RelativeTime';
import { ReleaseAssets } from './ReleaseAssets';

export interface ReleaseCardProps {
  rel: Release;
  src: RepoSource;
  owner: string;
  repo: string;
  /** github.com origin: Assets + Source code rows (F4). */
  github?: boolean;
  /** A GitHub release list is cached (some release has `htmlUrl`). */
  ghLoaded?: boolean;
}

export function ReleaseCard({ rel, src, owner, repo, github = false, ghLoaded = false }: ReleaseCardProps) {
  const ref = `refs/tags/${rel.tag}`;
  const explore = src.explore;
  // online the release's commit is only known when GitHub names one (a branch name there is dropped, D-124)
  const commitLink = (cls: string, children: ReactNode) =>
    rel.commit ? (
      <Link to={href('commit', { explore, owner, repo, oid: rel.commit })} className={cls} title={rel.commit}>
        {children}
      </Link>
    ) : null;
  const htmlUrl = rel.htmlUrl;
  return (
    <article className="rcard" data-tag={rel.tag} aria-label={`Release ${rel.title}`}>
      <div className="rcard__side">
        <div className="rcard__date">
          <RelativeTime secs={rel.date} />
        </div>
        <Link to={href('tree', { explore, owner, repo, ref, path: '' })} className="rcard__meta" title={`Browse the files at ${rel.tag}`}>
          <Tag size={14} aria-hidden="true" />
          <span className="mono truncate">{rel.tag}</span>
        </Link>
        {commitLink(
          'rcard__meta',
          <>
            <GitCommitHorizontal size={14} aria-hidden="true" />
            <span className="mono">{shortOid(rel.commit)}</span>
          </>,
        )}
      </div>
      <div className="box rcard__body">
        <div className="rcard__head">
          <h2 className="rcard__title">
            <Link to={href('tree', { explore, owner, repo, ref, path: '' })}>{rel.title}</Link>
          </h2>
          {rel.latest && <Badge tone="success">Latest</Badge>}
          {rel.prerelease && <Badge tone="attention">Pre-release</Badge>}
          {rel.ghNotes && <Badge title="Notes loaded from GitHub">GitHub notes</Badge>}
          {htmlUrl && (
            <a
              href={htmlUrl}
              className="rcard__gh small"
              title={`Open ${htmlUrl} in your browser`}
              onClick={(e) => {
                e.preventDefault();
                confirmExternal(htmlUrl);
              }}
            >
              <ExternalLink size={14} aria-hidden="true" />
              Open on GitHub
            </a>
          )}
        </div>
        <div className="rcard__sub muted small">
          <Link to={href('tree', { explore, owner, repo, ref, path: '' })} className="rcard__tagchip" title={`Browse the files at ${rel.tag}`}>
            <Tag size={12} aria-hidden="true" />
            <span className="mono">{rel.tag}</span>
          </Link>
          {commitLink('rcard__oid mono', shortOid(rel.commit))}
          <span>{formatDay(rel.date)}</span>
        </div>
        {rel.notesHtml ? (
          <MarkdownBody html={rel.notesHtml} src={src} owner={owner} repo={repo} rev={ref} commit={rel.commit || undefined} className="markdown rcard__notes" />
        ) : (
          <p className="rcard__none muted">No release notes. {rel.title === rel.tag ? 'The tag has no message.' : ''}</p>
        )}
        <ReleaseAssets rel={rel} repoId={typeof src.key === 'number' ? src.key : null} github={github} ghLoaded={ghLoaded} />
        {rel.previousTag && !explore && (
          <div className="rcard__foot">
            <Link to={commitsHref(owner, repo, ref, { since: `refs/tags/${rel.previousTag}` })} className="rcard__compare">
              <GitCompareArrows size={16} aria-hidden="true" />
              Commits since {rel.previousTag}
            </Link>
          </div>
        )}
      </div>
    </article>
  );
}
