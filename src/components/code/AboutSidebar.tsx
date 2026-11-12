// About sidebar (FR-18, FR-19, FR-20): gear → Edit About, description, homepage (confirmed external link), topics,
// origin, license, "Imported from" / "Cloned … · last fetched …", counts; Releases box; Languages box (stacked bar +
// list, own palette). v0.2.0: values are the effective ones (the user's win over GitHub's, D-102); a line says
// when some come from GitHub.

import { Archive, Eye, GitBranch, Globe, HardDrive, Link2, Package, Scale, Settings, Tag } from 'lucide-react';
import { useState } from 'react';
import { languages, releasesList } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { LanguageStat, RepoDetail, RepoSummary } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount, formatDay } from '../../util/format';
import { langClass } from '../../util/lang-colors';
import { EditAboutDialog } from '../dialogs/EditAboutDialog';
import { LangDot, TopicChips } from '../dashboard/meta';
import { Sidebar, SidebarSection } from '../layout/Sidebar';
import { Button } from '../ui/Button';
import { Badge, Shimmer } from '../ui/bits';
import { confirmExternal, externalKind } from '../ui/ConfirmExternalLink';
import { CopyButton } from '../ui/CopyButton';
import { RelativeTime } from '../ui/RelativeTime';

/** A link that asks before opening the system browser (NFR-S3); non-http text (ssh origins, D-48) stays text. */
export function ExternalText({ url, className }: { url: string; className?: string }) {
  if (externalKind(url) !== 'web') {
    return (
      <span className={`row ${className ?? ''}`}>
        <span className="mono break-all">{url}</span>
        <CopyButton text={url} label="Copy URL" />
      </span>
    );
  }
  return (
    <a
      href={url}
      className={className}
      onClick={(e) => {
        e.preventDefault();
        confirmExternal(url);
      }}
    >
      {url.replace(/^https?:\/\//, '').replace(/\/$/, '')}
    </a>
  );
}

/** v0.2.0 (D-102): true when an effective About value shown comes from GitHub (the user's field is empty). */
export function ghShown(s: RepoSummary): boolean {
  const g = s.aboutGithub;
  if (!g) return false;
  const u = s.aboutUser;
  return (
    (!u.description?.trim() && !!g.description?.trim()) || (!u.homepage?.trim() && !!g.homepage?.trim()) || (u.topics.length === 0 && g.topics.length > 0)
  );
}

export function AboutSidebar({
  summary,
  detail,
  commit,
}: {
  summary: RepoSummary;
  detail: RepoDetail | null;
  /** Resolved commit (null for an empty repo). */
  commit: string | null;
}) {
  const [editing, setEditing] = useState(false);
  const { owner, name } = summary;
  const empty = !summary.description && !summary.homepage && summary.topics.length === 0;
  return (
    <Sidebar label="About this repository">
      <SidebarSection
        title="About"
        actions={<Button size="sm" variant="invisible" icon={Settings} label="Edit repository details" onClick={() => setEditing(true)} />}
      >
        <div className="about">
          {empty && <p className="muted about__none">No description, website, or topics provided.</p>}
          {summary.description && <p className="about__desc">{summary.description}</p>}
          {summary.homepage && (
            <p className="about__line about__home">
              <Link2 size={16} aria-hidden="true" />
              <ExternalText url={summary.homepage} className="truncate strong-link" />
            </p>
          )}
          <TopicChips topics={summary.topics} />
          {ghShown(summary) && summary.aboutGithub && (
            <p className="about__source muted small" data-testid="about-from-github">
              Some details from GitHub, fetched <RelativeTime secs={summary.aboutGithub.fetchedAt} />. Edit to use your own.
            </p>
          )}
          <ul className="about__facts">
            {summary.originUrl && (
              <li className="about__line">
                <Globe size={16} aria-hidden="true" />
                <ExternalText url={summary.originUrl} className="truncate" />
              </li>
            )}
            {detail?.license && (
              <li className="about__line">
                <Scale size={16} aria-hidden="true" />
                <span>{detail.license} license</span>
              </li>
            )}
            {summary.source === 'imported' ? (
              <li className="about__line">
                <HardDrive size={16} aria-hidden="true" />
                <span>
                  Imported from <span className="mono break-all">{summary.path}</span>
                </span>
              </li>
            ) : (
              <li className="about__line">
                <Archive size={16} aria-hidden="true" />
                <span>
                  Cloned {summary.clonedAt !== null ? formatDay(summary.clonedAt) : ''}
                  {summary.lastFetchedAt !== null ? (
                    <>
                      {' · '}last fetched <RelativeTime secs={summary.lastFetchedAt} />
                    </>
                  ) : (
                    ' · never fetched'
                  )}
                </span>
              </li>
            )}
            {summary.source === 'imported' && summary.lastFetchedAt !== null && (
              <li className="about__line">
                <Eye size={16} aria-hidden="true" />
                <span>
                  Last fetched <RelativeTime secs={summary.lastFetchedAt} />
                </span>
              </li>
            )}
            {detail && (
              <>
                <li className="about__line">
                  <GitBranch size={16} aria-hidden="true" />
                  <Link to={href('branches', { owner, repo: name })}>
                    <strong>{formatCount(detail.branchCount)}</strong> {detail.branchCount === 1 ? 'branch' : 'branches'}
                  </Link>
                </li>
                <li className="about__line">
                  <Tag size={16} aria-hidden="true" />
                  <Link to={href('tags', { owner, repo: name })}>
                    <strong>{formatCount(detail.tagCount)}</strong> {detail.tagCount === 1 ? 'tag' : 'tags'}
                  </Link>
                </li>
              </>
            )}
          </ul>
        </div>
      </SidebarSection>
      {detail && <ReleasesBox repoId={summary.id} owner={owner} repo={name} count={detail.releaseCount} />}
      {commit && <LanguagesBox repoId={summary.id} commit={commit} />}
      {editing && <EditAboutDialog summary={summary} onClose={() => setEditing(false)} />}
    </Sidebar>
  );
}

export function ReleasesBox({ repoId, owner, repo, count }: { repoId: number; owner: string; repo: string; count: number }) {
  const rel = useQuery(count > 0 ? qk('releases_list', repoId) : null, () => releasesList(repoId));
  const latest = rel.data?.find((r) => r.latest) ?? rel.data?.[0];
  return (
    <SidebarSection
      title={
        <Link to={href('releases', { owner, repo })} className="sidebar__title-link">
          Releases {count > 0 && <span className="counter">{count}</span>}
        </Link>
      }
    >
      {count === 0 ? (
        <p className="muted small">No releases (version tags) yet.</p>
      ) : latest ? (
        <Link to={href('releases', { owner, repo })} className="release-mini">
          <Package size={18} aria-hidden="true" className="release-mini__icon" />
          <span className="release-mini__body">
            <span className="row">
              <strong className="truncate">{latest.title}</strong>
              {latest.latest && <Badge tone="success">Latest</Badge>}
            </span>
            <span className="muted small">{formatDay(latest.date)}</span>
          </span>
        </Link>
      ) : (
        <Shimmer width="70%" block />
      )}
      {count > 1 && (
        <Link to={href('releases', { owner, repo })} className="small">
          + {count - 1} {count - 1 === 1 ? 'release' : 'releases'}
        </Link>
      )}
    </SidebarSection>
  );
}

export function LangBar({ items }: { items: LanguageStat[] }) {
  return (
    <div className="langbar" role="img" aria-label={items.map((i) => `${i.name} ${i.percent.toFixed(1)}%`).join(', ')}>
      {items.map((i) => (
        <span
          key={i.name}
          className={`langbar__seg ${langClass(i.name)}`}
          style={{ width: `${Math.max(i.percent, 0.6)}%` }}
          title={`${i.name} ${i.percent.toFixed(1)}%`}
        />
      ))}
    </div>
  );
}

export function LangList({ items }: { items: LanguageStat[] }) {
  return (
    <ul className="langlist">
      {items.map((i) => (
        <li key={i.name} className="langlist__item">
          <LangDot name={i.name} />
          <strong>{i.name}</strong>
          <span className="muted">{i.percent.toFixed(1)}%</span>
        </li>
      ))}
    </ul>
  );
}

export function LanguagesBox({ repoId, commit }: { repoId: number; commit: string }) {
  const langs = useQuery(qk('languages', repoId, commit), (requestId) => languages(repoId, commit, requestId), {
    cancelable: true,
    stable: true,
  });
  const items = langs.data?.items ?? [];
  if (langs.data && items.length === 0) return null;
  return (
    <SidebarSection title="Languages">
      {langs.error ? (
        <p className="field__error">{langs.error.message}</p>
      ) : !langs.data ? (
        <div className="stack stack--tight">
          <Shimmer width="100%" height={8} block />
          <Shimmer width="60%" />
        </div>
      ) : (
        <>
          <LangBar items={items} />
          <LangList items={items} />
        </>
      )}
    </SidebarSection>
  );
}
