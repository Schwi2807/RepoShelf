// App shell: global header + RouteView. Every route of ARCH §7.1 maps to a screen folder created in M5, so M6/M7
// never edit this file in parallel. Repo routes render inside RepoLayout (slug → repoId, rev → ResolvedRev).

import { useLayoutEffect, type ReactNode } from 'react';
import { DialogHost } from './components/dialogs/DialogHost';
import { ExploreLayout } from './components/explore/ExploreLayout';
import { GlobalHeader } from './components/layout/GlobalHeader';
import { RepoLayout } from './components/layout/RepoLayout';
import { ExternalLinkHost } from './components/ui/ConfirmExternalLink';
import { markRendered } from './router/router';
import { REPO_ROUTES, type Route } from './router/routes';
import { useRoute } from './router/useRoute';
import Branches from './screens/Branches';
import Commit from './screens/Commit';
import Commits from './screens/Commits';
import Dashboard from './screens/Dashboard';
import ExploreSearch from './screens/ExploreSearch';
import FileView from './screens/FileView';
import NotFound from './screens/NotFound';
import Releases from './screens/Releases';
import RepoCode from './screens/RepoCode';
import Settings from './screens/Settings';
import Tags from './screens/Tags';

function screenFor(route: Route): ReactNode {
  switch (route.name) {
    case 'dashboard':
      return <Dashboard route={route} />;
    case 'settings':
      return <Settings />;
    case 'code':
    case 'tree':
      return <RepoCode route={route} />;
    case 'blob':
      return <FileView route={route} />;
    case 'commits':
      return <Commits route={route} />;
    case 'commit':
      return <Commit route={route} />;
    case 'branches':
      return <Branches />;
    case 'tags':
      return <Tags />;
    case 'releases':
      return <Releases />;
    case 'exploreSearch':
      return <ExploreSearch route={route} />;
    case 'notFound':
      return <NotFound />;
  }
}

/** Renders the current route; applies the router's scroll restore after the route rendered (FR-34, D-31). */
export function RouteView() {
  const route = useRoute();
  useLayoutEffect(() => {
    markRendered();
  }, [route]);
  const screen = screenFor(route);
  if (REPO_ROUTES.has(route.name) && route.params.explore === true) {
    // GitHub repository seen online (D-127); the key differs from the shelf's, so "Open in shelf" remounts
    return (
      <ExploreLayout key={`x:${route.params.owner}/${route.params.repo}`.toLowerCase()} route={route}>
        {screen}
      </ExploreLayout>
    );
  }
  if (REPO_ROUTES.has(route.name)) {
    // key: a different repo remounts the layout (fresh queries); tab changes inside one repo keep it.
    return (
      <RepoLayout key={`${route.params.owner}/${route.params.repo}`.toLowerCase()} route={route}>
        {screen}
      </RepoLayout>
    );
  }
  return <>{screen}</>;
}

export default function App() {
  return (
    <>
      <GlobalHeader />
      <RouteView />
      <DialogHost />
      <ExternalLinkHost />
    </>
  );
}
