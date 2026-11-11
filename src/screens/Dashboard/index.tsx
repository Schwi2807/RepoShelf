// S1 Dashboard (FR-1..4): "Repositories" + count, toolbar (filter `?q=`, Type, Sort, New), rows, missing rows,
// sidebar (Recent, Archive, Fetch all), empty state.

import { DashSidebar } from '../../components/dashboard/DashSidebar';
import { EmptyState } from '../../components/dashboard/EmptyState';
import { RepoList } from '../../components/dashboard/RepoList';
import { RepoToolbar } from '../../components/dashboard/RepoToolbar';
import { Columns } from '../../components/layout/Sidebar';
import { Counter, Shimmer } from '../../components/ui/bits';
import type { Route } from '../../router/routes';
import { filterRepos, sortRepos, useLibrary } from '../../state/library';
import { useSettings } from '../../state/settings';

export default function Dashboard({ route }: { route: Route }) {
  const { repos, error } = useLibrary();
  const { settings } = useSettings();
  const q = route.query.q ?? '';

  if (repos === null) {
    return (
      <main className="container page">
        <div className="stack">
          <Shimmer width={200} height={24} />
          <Shimmer width="100%" height={32} />
          <Shimmer width="70%" />
          <Shimmer width="60%" />
        </div>
      </main>
    );
  }

  if (repos.length === 0) {
    return (
      <main className="container page">
        <h1 className="page-title">Repositories</h1>
        <EmptyState />
        {error && <p className="field__error">{error}</p>}
      </main>
    );
  }

  const list = sortRepos(filterRepos(repos, q, settings.sourceFilter), settings.sort);
  const filtered = list.length !== repos.length;
  return (
    <main className="container page">
      <Columns>
        <section className="dash" aria-labelledby="dash-title">
          <div className="dash__head">
            <h1 id="dash-title" className="dash__title">
              Repositories
            </h1>
            <Counter n={repos.length} />
            {filtered && (
              <span className="muted small" role="status">
                {list.length} shown
              </span>
            )}
          </div>
          <RepoToolbar query={q} />
          {error && (
            <p className="field__error" role="alert">
              {error}
            </p>
          )}
          <RepoList repos={list} query={q} />
        </section>
        <DashSidebar repos={repos} />
      </Columns>
    </main>
  );
}
