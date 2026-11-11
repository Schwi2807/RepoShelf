// S1 Dashboard — M5 placeholder (repo names + count, `?q=` filter, empty state). Full screen: M6.2.

import { Download, FolderPlus, Library } from 'lucide-react';
import { SourceBadge } from '../../components/layout/RepoHeader';
import { Box } from '../../components/layout/Sidebar';
import { Button } from '../../components/ui/Button';
import { Empty, Shimmer } from '../../components/ui/bits';
import type { Route } from '../../router/routes';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { filterRepos, sortRepos, useLibrary } from '../../state/library';
import { useSettings } from '../../state/settings';
import { openDialog } from '../../state/ui';

export default function Dashboard({ route }: { route: Route }) {
  const { repos, error } = useLibrary();
  const { settings } = useSettings();
  const q = route.query.q ?? '';
  if (repos === null) {
    return (
      <main className="container page">
        <Shimmer width={200} height={20} />
      </main>
    );
  }
  if (repos.length === 0) {
    return (
      <main className="container page">
        <Box>
          <Empty
            icon={Library}
            title="Your shelf is empty"
            actions={
              <>
                <Button icon={FolderPlus} onClick={() => openDialog('import')}>
                  Import folder…
                </Button>
                <Button variant="primary" icon={Download} onClick={() => openDialog('clone')}>
                  Clone URL…
                </Button>
              </>
            }
          >
            Import a repository from this PC or clone a public one.
          </Empty>
        </Box>
        {error && <p className="field__error">{error}</p>}
      </main>
    );
  }
  const list = sortRepos(filterRepos(repos, q, settings.sourceFilter), settings.sort);
  return (
    <main className="container page">
      <Box
        header={
          <>
            <span className="box__title">Repositories</span>
            <span className="counter">{repos.length}</span>
            {q && <span className="muted">matching “{q}”</span>}
          </>
        }
      >
        {list.map((r) => (
          <div key={r.id} className="box__row">
            <Link to={href('code', { owner: r.owner, repo: r.name })}>
              {r.owner} / <strong>{r.name}</strong>
            </Link>
            <SourceBadge summary={r} />
            {r.status.type === 'missing' && <span className="badge badge--attention">missing</span>}
          </div>
        ))}
      </Box>
    </main>
  );
}
