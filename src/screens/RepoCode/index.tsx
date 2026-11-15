// S3 Code tab (FR-13..19, FR-21): top bar (ref switcher, counts or breadcrumbs, file filter, Code menu), box with the
// latest-commit header and the file table, README card; About sidebar. Edge states: empty repo, ref not found, path
// not found. Every query below is keyed by the resolved commit oid (stable, D-4 / ARCH §7.3).

import { FileQuestion, GitBranch, Inbox, Search, X } from 'lucide-react';
import { useMemo, useState } from 'react';
import { AboutSidebar } from '../../components/code/AboutSidebar';
import { ExploreAbout } from '../../components/explore/ExploreAbout';
import { ExploreCodeMenu } from '../../components/explore/ExploreCodeMenu';
import { ExploreFailure } from '../../components/explore/ExploreErrors';
import { Breadcrumbs } from '../../components/code/Breadcrumbs';
import { CodeMenu } from '../../components/code/CodeMenu';
import { CountsLinks } from '../../components/code/CountsLinks';
import { FileTable } from '../../components/code/FileTable';
import { LatestCommitBox } from '../../components/code/LatestCommitBox';
import { ReadmeCard } from '../../components/code/ReadmeCard';
import { RefSwitcher } from '../../components/code/RefSwitcher';
import { useRepo } from '../../components/layout/repoContext';
import { Box, Columns } from '../../components/layout/Sidebar';
import { Empty, Shimmer } from '../../components/ui/bits';
import { qk, useQuery } from '../../ipc/query';
import type { Route } from '../../router/routes';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { normalizePath } from '../../util/paths';

function Skeleton() {
  return (
    <Box
      header={
        <>
          <Shimmer width={24} height={24} />
          <Shimmer width="45%" />
        </>
      }
    >
      {[70, 55, 62, 40, 66].map((w, i) => (
        <div key={i} className="ftable__row">
          <span className="ftable__name">
            <Shimmer width={`${w}%`} />
          </span>
          <span className="ftable__msg">
            <Shimmer width="50%" />
          </span>
          <span className="ftable__date" />
        </div>
      ))}
    </Box>
  );
}

/** Keyed by ref + path so per-folder state (the file filter) starts fresh in every folder. */
export default function RepoCode({ route }: { route: Route }) {
  return <CodeTab key={`${route.params.ref ?? ''}\u0000${route.params.path ?? ''}`} route={route} />;
}

function CodeTab({ route }: { route: Route }) {
  const path = normalizePath(route.params.path ?? '');
  const r = useRepo();
  const { src, explore, local } = r;
  const commit = r.resolved?.commit ?? null;
  const [filter, setFilter] = useState('');
  const tree = useQuery(commit ? qk('tree_list', src.key, commit, path) : null, () => src.tree(commit!, path), {
    stable: true,
  });
  const entries = useMemo(() => {
    const all = tree.data?.entries ?? [];
    const q = filter.trim().toLowerCase();
    return q ? all.filter((e) => e.name.toLowerCase().includes(q)) : all;
  }, [tree.data, filter]);

  const unborn = r.unborn;
  const refName = r.refName;
  const def = r.defaultBranch;

  let main;
  if (unborn) {
    main = (
      <Box>
        <Empty icon={Inbox} title="This repository is empty.">
          There are no commits on <span className="mono">{refName ?? def ?? 'the default branch'}</span> yet, so there is nothing to browse.
        </Empty>
      </Box>
    );
  } else if (r.revError && explore && r.revError.code !== 'notFound' && r.revError.code !== 'invalidRev') {
    main = <ExploreFailure inline error={r.revError} what="This ref" />;
  } else if (r.revError) {
    main = (
      <Box>
        <Empty
          icon={GitBranch}
          title={r.revError.code === 'notFound' ? `Ref “${refName ?? ''}” not found` : 'This ref cannot be shown'}
          actions={def ? <Link to={href('tree', { explore, owner: r.owner, repo: r.repo, ref: def, path: '' })}>Go to the default branch ({def})</Link> : undefined}
        >
          {r.revError.code === 'notFound' ? 'No branch, tag or commit by that name exists in this repository.' : r.revError.message}
        </Empty>
      </Box>
    );
  } else if (!r.resolved || !refName || commit === null) {
    main = <Skeleton />;
  } else {
    const workTree = local?.summary.source === 'imported' && local.detail?.isBare === false;
    main = (
      <>
        <div className="codebar">
          <RefSwitcher
            src={src}
            refName={refName}
            resolved={r.resolved}
            hrefFor={(ref) => href('tree', { explore, owner: r.owner, repo: r.repo, ref, path })}
          />
          {path ? (
            <Breadcrumbs owner={r.owner} repo={r.repo} refName={refName} path={path} />
          ) : (
            (explore || local?.detail) && (
              <CountsLinks owner={r.owner} repo={r.repo} branches={local?.detail?.branchCount ?? null} tags={local?.detail?.tagCount ?? null} />
            )
          )}
          <span className="spacer" />
          <div className="filter-input filter-input--compact">
            <Search size={14} aria-hidden="true" className="filter-input__icon" />
            <input
              className="input filter-input__field"
              type="search"
              placeholder="Filter files…"
              aria-label="Filter files in this folder"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
            {filter && (
              <button type="button" className="filter-input__clear" aria-label="Clear file filter" onClick={() => setFilter('')}>
                <X size={14} aria-hidden="true" />
              </button>
            )}
          </div>
          {local ? <CodeMenu summary={local.summary} detail={local.detail} /> : r.online && <ExploreCodeMenu repo={r.online} />}
        </div>
        {tree.error && explore && tree.error.code !== 'notFound' && tree.error.code !== 'invalidPath' ? (
          <ExploreFailure inline error={tree.error} what="This folder" onRetry={tree.refetch} />
        ) : tree.error ? (
          <Box>
            <Empty
              icon={FileQuestion}
              title="Path not found"
              actions={<Link to={href('tree', { explore, owner: r.owner, repo: r.repo, ref: refName, path: '' })}>Go to the repository root</Link>}
            >
              <span className="mono">{path}</span> does not exist at <span className="mono">{refName}</span>.
            </Empty>
          </Box>
        ) : (
          <div className="box filebox">
            <div className="box__header filebox__header">
              <LatestCommitBox src={src} owner={r.owner} repo={r.repo} refName={refName} commit={commit} path={path} />
            </div>
            {tree.data ? (
              entries.length === 0 && filter ? (
                <p className="filebox__empty muted">No files in this folder match “{filter}”.</p>
              ) : (
                <FileTable
                  src={src}
                  owner={r.owner}
                  repo={r.repo}
                  refName={refName}
                  commit={commit}
                  path={path}
                  entries={entries}
                  workTree={workTree}
                />
              )
            ) : (
              [60, 45, 52, 38].map((w, i) => (
                <div key={i} className="ftable__row">
                  <span className="ftable__name">
                    <Shimmer width={`${w}%`} />
                  </span>
                  <span className="ftable__msg">
                    <Shimmer width="50%" />
                  </span>
                  <span className="ftable__date" />
                </div>
              ))
            )}
          </div>
        )}
        {explore && tree.data && (
          <p className="muted small filebox__hint" data-testid="explore-files-hint">
            Online view: the last commit of each file is not loaded (one request per file). Save to shelf for the full picture.
          </p>
        )}
        {tree.data?.readme && (
          <ReadmeCard src={src} owner={r.owner} repo={r.repo} refName={refName} commit={commit} dir={path} />
        )}
      </>
    );
  }

  return (
    <Columns>
      <div className="stack code-main">{main}</div>
      {local ? (
        <AboutSidebar summary={local.summary} detail={local.detail} commit={unborn ? null : commit} />
      ) : (
        r.online && <ExploreAbout repo={r.online} src={src} commit={commit} />
      )}
    </Columns>
  );
}
