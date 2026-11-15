// Frame of every Explore repo route (v0.3.0 M10b, D-127): the twin of RepoLayout for a GitHub repository. It loads
// `explore_repo`, resolves the route ref with `explore_resolve`, builds the online `RepoSource` and provides the
// same RepoContext the repo screens already read (Code, File, Commits, Commit, Branches, Tags, Releases), with
// `local: null`. Header = name, badges, Save to shelf / Open in shelf, Open on GitHub; then tabs without counts
// (counts cost one request each, D-122) and the quota strip.

import { BookMarked, Compass, Ellipsis, ExternalLink, GitFork, Link2, Lock } from 'lucide-react';
import { useMemo, type ReactNode } from 'react';
import { qk, useQuery } from '../../ipc/query';
import type { ExploreRepo } from '../../ipc/types';
import { exploreRepoInfo, exploreSource } from '../../source';
import type { Route } from '../../router/routes';
import { copyLocation, href } from '../../router/router';
import { Link } from '../../router/Link';
import { useSettings } from '../../state/settings';
import { Badge, Shimmer } from '../ui/bits';
import { confirmExternal } from '../ui/ConfirmExternalLink';
import { Dropdown } from '../ui/Dropdown';
import { RepoContext, type RepoContextValue } from '../layout/repoContext';
import { TabBar } from '../layout/TabBar';
import { ExploreFailure, ExploreOff } from './ExploreErrors';
import { QuotaBar } from './QuotaBar';
import { SaveToShelfButton } from './SaveToShelf';

export function ExploreHeader({ repo }: { repo: ExploreRepo }) {
  return (
    <div className="rh__row" data-testid="explore-header">
      <h1 className="rh__title">
        <BookMarked size={18} aria-hidden="true" className="muted" />
        <Link to={href('exploreSearch', {}, { q: `user:${repo.owner}` })} title={`Search ${repo.owner}'s repositories`}>
          {repo.owner}
        </Link>
        <span className="rh__sep">/</span>
        <strong>
          <Link to={href('code', { explore: true, owner: repo.owner, repo: repo.name })}>{repo.name}</Link>
        </strong>
        <Badge tone="indigo" icon={Compass} title="Read from GitHub; nothing is saved on this PC unless you save it to the shelf">
          On GitHub
        </Badge>
        {repo.isPrivate && (
          <Badge icon={Lock} title="A private repository (your token can read it)">
            Private
          </Badge>
        )}
        {repo.archived && <Badge tone="attention">Archived</Badge>}
        {repo.fork && (
          <Badge icon={GitFork} title="A fork">
            Fork
          </Badge>
        )}
      </h1>
      <div className="rh__spacer" />
      <div className="rh__actions">
        <SaveToShelfButton repo={repo} />
        <Dropdown
          label="More repository actions"
          icon={Ellipsis}
          size="sm"
          align="right"
          caret={false}
          items={[
            { key: 'gh', label: 'Open on GitHub', icon: ExternalLink, onSelect: () => confirmExternal(repo.htmlUrl) },
            { key: 'copy', label: 'Copy location', icon: Link2, onSelect: () => void copyLocation() },
          ]}
        />
      </div>
    </div>
  );
}

/** `/explore/<owner>/<repo>/…`: off → the "Explore is turned off" page, else the frame below. */
export function ExploreLayout({ route, children }: { route: Route; children: ReactNode }) {
  const { settings, loaded } = useSettings();
  // a reload on an Explore URL must not ask GitHub before the switch is known
  if (!loaded) return <main className="container page" />;
  if (!settings.exploreEnabled) return <ExploreOff />;
  return <ExploreRepoLayout route={route}>{children}</ExploreRepoLayout>;
}

function ExploreRepoLayout({ route, children }: { route: Route; children: ReactNode }) {
  const owner = route.params.owner ?? '';
  const name = route.params.repo ?? '';
  const base = useMemo(() => exploreSource(owner, name, null), [owner, name]);
  // not stable: `onShelf` changes when a Save finishes (the jobs store invalidates, D-125)
  const repoQ = useQuery(qk('explore_repo', base.key), () => exploreRepoInfo(owner, name));
  const online = repoQ.data ?? null;

  const routeRef = route.params.ref ?? null;
  const refName = routeRef ?? online?.defaultBranch ?? null;
  const rev = useQuery(online !== null && refName !== null ? qk('rev_resolve', base.key, refName) : null, () => base.resolve(refName!));
  const resolved = rev.data ?? null;
  const src = useMemo(() => exploreSource(owner, name, resolved), [owner, name, resolved]);

  const ctx: RepoContextValue | null = useMemo(
    () =>
      online
        ? {
            src,
            explore: true,
            owner: online.owner,
            repo: online.name,
            defaultBranch: online.defaultBranch,
            unborn: false,
            refName,
            refFromRoute: routeRef !== null,
            resolved,
            revError: rev.error,
            revLoading: rev.loading,
            local: null,
            online,
          }
        : null,
    [src, online, refName, routeRef, resolved, rev.error, rev.loading],
  );

  if (repoQ.error && !online) {
    return <ExploreFailure error={repoQ.error} what={`${owner}/${name}`} onRetry={repoQ.refetch} />;
  }
  if (!online || !ctx) {
    return (
      <>
        <div className="rh">
          <div className="container">
            <div className="rh__row">
              <Shimmer width={260} height={20} />
            </div>
          </div>
        </div>
        <main className="container page">
          <QuotaBar />
        </main>
      </>
    );
  }
  return (
    <RepoContext.Provider value={ctx}>
      <div className="rh">
        <div className="container">
          <ExploreHeader repo={online} />
          <TabBar
            explore
            owner={online.owner}
            repo={online.name}
            refName={refName}
            refFromRoute={routeRef !== null}
            current={route.name}
            counts={{ commits: null, branches: null, tags: null, releases: null }}
          />
        </div>
      </div>
      <main className="container page">
        <div className="stack">
          <QuotaBar />
          {children}
        </div>
      </main>
    </RepoContext.Provider>
  );
}
