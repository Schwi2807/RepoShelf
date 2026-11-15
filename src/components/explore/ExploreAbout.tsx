// About sidebar of an Explore repository (v0.3.0 M10b): what GitHub says (description, website, topics, licence, stars,
// forks, open issues, language, dates) plus the Languages box. Topics link to an Explore search, not to the shelf.
// Counts of releases / branches / tags are not shown (each would cost a request, D-122); the links lead to the tabs.

import { CircleDot, Eye, GitBranch, GitFork, Globe, Link2, Package, Scale, Star, Tag } from 'lucide-react';
import type { ExploreRepo } from '../../ipc/types';
import type { RepoSource } from '../../source';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatBytes, formatCount } from '../../util/format';
import { LanguagesBox, ExternalText } from '../code/AboutSidebar';
import { LangDot } from '../dashboard/meta';
import { Sidebar, SidebarSection } from '../layout/Sidebar';
import { RelativeTime } from '../ui/RelativeTime';

export function ExploreTopicChips({ topics }: { topics: string[] }) {
  if (topics.length === 0) return null;
  return (
    <div className="chips">
      {topics.map((t) => (
        <Link key={t} to={href('exploreSearch', {}, { q: `topic:${t}` })} className="chip" title={`Search GitHub for the topic “${t}”`}>
          {t}
        </Link>
      ))}
    </div>
  );
}

export function ExploreAbout({ repo, src, commit }: { repo: ExploreRepo; src: RepoSource; commit: string | null }) {
  const { owner, name } = repo;
  const empty = !repo.description && !repo.homepage && repo.topics.length === 0;
  return (
    <Sidebar label="About this repository">
      <SidebarSection title="About">
        <div className="about" data-testid="explore-about">
          {empty && <p className="muted about__none">No description, website, or topics provided.</p>}
          {repo.description && <p className="about__desc">{repo.description}</p>}
          {repo.homepage && (
            <p className="about__line about__home">
              <Link2 size={16} aria-hidden="true" />
              <ExternalText url={repo.homepage} className="truncate strong-link" />
            </p>
          )}
          <ExploreTopicChips topics={repo.topics} />
          <ul className="about__facts">
            <li className="about__line">
              <Globe size={16} aria-hidden="true" />
              <ExternalText url={repo.htmlUrl} className="truncate" />
            </li>
            {repo.license && (
              <li className="about__line">
                <Scale size={16} aria-hidden="true" />
                <span>{repo.license} license</span>
              </li>
            )}
            <li className="about__line">
              <Star size={16} aria-hidden="true" />
              <span>
                <strong>{formatCount(repo.stars)}</strong> {repo.stars === 1 ? 'star' : 'stars'}
              </span>
            </li>
            <li className="about__line">
              <GitFork size={16} aria-hidden="true" />
              <span>
                <strong>{formatCount(repo.forks)}</strong> {repo.forks === 1 ? 'fork' : 'forks'}
              </span>
            </li>
            <li className="about__line">
              <CircleDot size={16} aria-hidden="true" />
              <span>
                <strong>{formatCount(repo.openIssues)}</strong> open {repo.openIssues === 1 ? 'issue' : 'issues'} and pull requests
              </span>
            </li>
            {repo.pushedAt !== null && (
              <li className="about__line">
                <Eye size={16} aria-hidden="true" />
                <span>
                  Last pushed <RelativeTime secs={repo.pushedAt} />
                </span>
              </li>
            )}
            <li className="about__line">
              <GitBranch size={16} aria-hidden="true" />
              <Link to={href('branches', { explore: true, owner, repo: name })}>Branches</Link>
            </li>
            <li className="about__line">
              <Tag size={16} aria-hidden="true" />
              <Link to={href('tags', { explore: true, owner, repo: name })}>Tags</Link>
            </li>
          </ul>
          <p className="about__source muted small">
            About {formatBytes(repo.sizeKb * 1024)} on GitHub{repo.language ? ' · mainly ' : ''}
            {repo.language && (
              <span className="lang-label">
                <LangDot name={repo.language} />
                {repo.language}
              </span>
            )}
          </p>
        </div>
      </SidebarSection>
      <SidebarSection
        title={
          <Link to={href('releases', { explore: true, owner, repo: name })} className="sidebar__title-link">
            <Package size={14} aria-hidden="true" /> Releases
          </Link>
        }
      >
        <Link to={href('releases', { explore: true, owner, repo: name })} className="small">
          See the releases of this repository
        </Link>
      </SidebarSection>
      {commit && <LanguagesBox src={src} commit={commit} />}
    </Sidebar>
  );
}
