// File table (FR-15, FR-16, NFR-W3, R-7): folders first then files (backend order), `..` row in subfolders, icon per
// kind, last commit per entry loaded lazily (`tree_last_commits`, oid-keyed + cancellable on navigation) with a
// shimmer until it arrives; > 500 rows virtualised against the window (D-31).

import {
  CornerLeftUp,
  File,
  FileSymlink,
  FileTerminal,
  Folder,
  FolderGit2,
  type LucideIcon,
} from 'lucide-react';
import { useMemo } from 'react';
import { qk, useQuery } from '../../ipc/query';
import type { RepoSource } from '../../source';
import type { CommitSummary, EntryKind, TreeEntry } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { parentDir } from '../../util/paths';
import { Badge, Shimmer } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';
import { WindowList } from '../ui/WindowList';

const ICONS: Record<EntryKind, LucideIcon> = {
  dir: Folder,
  file: File,
  exec: FileTerminal,
  symlink: FileSymlink,
  submodule: FolderGit2,
};

const KIND_LABEL: Record<EntryKind, string> = {
  dir: 'Directory',
  file: 'File',
  exec: 'Executable file',
  symlink: 'Symbolic link',
  submodule: 'Submodule',
};

export function EntryIcon({ kind }: { kind: EntryKind }) {
  const Icon = ICONS[kind];
  return <Icon size={16} className={`entry-icon entry-icon--${kind}`} aria-label={KIND_LABEL[kind]} role="img" />;
}

export interface FileTableProps {
  src: RepoSource;
  owner: string;
  repo: string;
  refName: string;
  commit: string;
  path: string;
  entries: TreeEntry[];
  /** Imported work-tree repo: entries that cannot exist on Windows get the NFR-W3 badge. */
  workTree: boolean;
}

export type LastState = { loading: true } | { loading: false; byName: Map<string, CommitSummary | null>; error?: string };

export function FileTable({ src, owner, repo, refName, commit, path, entries, workTree }: FileTableProps) {
  // Online there is no last commit per entry (one request per file, D-122): the columns stay empty.
  const lastFn = src.treeLastCommits;
  const last = useQuery(
    lastFn === null ? null : qk('tree_last_commits', src.key, commit, path),
    (requestId) => lastFn!(commit, path, requestId),
    { cancelable: true, stable: true },
  );
  const explore = src.explore;
  const state: LastState = useMemo(() => {
    if (lastFn === null) return { loading: false, byName: new Map() };
    if (last.data) return { loading: false, byName: new Map(last.data.entries.map((e) => [e.name, e.commit])) };
    if (last.error) return { loading: false, byName: new Map(), error: last.error.message };
    return { loading: true };
  }, [lastFn, last.data, last.error]);

  const up = path !== '';
  const count = entries.length + (up ? 1 : 0);
  return (
    <div className="ftable" role="table" aria-label="Files" data-testid="file-table">
      <div className="visually-hidden" role="row">
        <span role="columnheader">Name</span>
        <span role="columnheader">Last commit message</span>
        <span role="columnheader">Last commit date</span>
      </div>
      <WindowList
        count={count}
        estimate={41}
        rowKey={(i) => (up && i === 0 ? '..' : entries[i - (up ? 1 : 0)].path)}
        row={(i) => {
          if (up && i === 0) return <UpRow explore={explore} owner={owner} repo={repo} refName={refName} path={path} />;
          const e = entries[i - (up ? 1 : 0)];
          return <EntryRow e={e} explore={explore} owner={owner} repo={repo} refName={refName} state={state} workTree={workTree} />;
        }}
      />
    </div>
  );
}

function UpRow({ explore, owner, repo, refName, path }: { explore: boolean; owner: string; repo: string; refName: string; path: string }) {
  return (
    <div className="ftable__row" role="row">
      <span className="ftable__name" role="cell">
        <CornerLeftUp size={16} className="entry-icon" aria-hidden="true" />
        <Link to={href('tree', { explore, owner, repo, ref: refName, path: parentDir(path) })} title="Parent directory" className="ftable__link">
          ..
        </Link>
      </span>
      <span role="cell" />
      <span role="cell" />
    </div>
  );
}

function EntryRow({
  e,
  explore,
  owner,
  repo,
  refName,
  state,
  workTree,
}: {
  e: TreeEntry;
  explore: boolean;
  owner: string;
  repo: string;
  refName: string;
  state: LastState;
  workTree: boolean;
}) {
  const to = href(e.kind === 'dir' ? 'tree' : 'blob', { explore, owner, repo, ref: refName, path: e.path });
  return (
    <div className="ftable__row" role="row" data-kind={e.kind}>
      <span className="ftable__name" role="cell">
        <EntryIcon kind={e.kind} />
        <Link to={to} className="ftable__link truncate" title={e.name}>
          {e.name}
        </Link>
        {e.kind === 'submodule' && <span className="muted small">submodule</span>}
        {e.pathRaw !== null && (
          <Badge tone="attention" title="This name is not valid UTF-8; it is shown with replacement characters.">
            non-UTF-8 name
          </Badge>
        )}
        {workTree && !e.storableHere && (
          <Badge tone="attention" title="Windows cannot store this name or path, so git could not check it out here.">
            not checked out on this PC
          </Badge>
        )}
      </span>
      <LastCommitCell name={e.name} explore={explore} owner={owner} repo={repo} state={state} />
    </div>
  );
}

export function LastCommitCell({ name, explore = false, owner, repo, state }: { name: string; explore?: boolean; owner: string; repo: string; state: LastState }) {
  if (state.loading) {
    return (
      <>
        <span className="ftable__msg" role="cell">
          <Shimmer width="60%" />
        </span>
        <span className="ftable__date" role="cell">
          <Shimmer width={64} />
        </span>
      </>
    );
  }
  const c = state.byName.get(name);
  if (c === undefined || c === null) {
    return (
      <>
        <span className="ftable__msg subtle" role="cell">
          {state.error ? '' : c === null ? 'Older than 10,000 commits' : ''}
        </span>
        <span className="ftable__date" role="cell" />
      </>
    );
  }
  return (
    <>
      <span className="ftable__msg" role="cell">
        <Link to={href('commit', { explore, owner, repo, oid: c.oid })} className="ftable__commit truncate" title={c.summary}>
          {c.summary}
        </Link>
      </span>
      <span className="ftable__date" role="cell">
        <RelativeTime secs={c.committer.time.secs} />
      </span>
    </>
  );
}
