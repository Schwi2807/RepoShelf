// Command table (ARCH §5.1 + D-58): snake_case command → camelCase args and result. `bridge.call` and the mock's
// handler table are both typed from this map, so a missing or drifted command fails `npm run typecheck`.

import type {
  AheadBehindEntry,
  AppDir,
  AppInfo,
  BlobMeta,
  BlobText,
  BranchRow,
  ClonePreview,
  CommitDetail,
  CommitSummary,
  CommitsPage,
  FileDiff,
  ImportPreview,
  JobId,
  JobInfo,
  Languages,
  LastCommits,
  LatestCommit,
  RefsList,
  Release,
  RemoveResult,
  Rendered,
  RepoDetail,
  RepoSummary,
  ResolvedRev,
  Settings,
  TagRow,
  TreeListing,
} from './types';

/** What `jobs_subscribe` needs from a channel (Tauri `Channel` or the mock's plain object). */
export interface ChannelLike<T> {
  onmessage: (message: T) => void;
}

type NoArgs = Record<string, never>;
interface Repo {
  repoId: number;
}
interface RepoRev extends Repo {
  rev: string;
}
interface RepoRevPath extends RepoRev {
  path: string;
}
interface Req {
  requestId: string;
}

export interface Commands {
  app_info: { args: NoArgs; ret: AppInfo };
  settings_get: { args: NoArgs; ret: Settings };
  settings_set: { args: { settings: Settings }; ret: Settings };
  cache_clear: { args: NoArgs; ret: AppInfo };
  app_open_dir: { args: { which: AppDir }; ret: null };
  library_list: { args: NoArgs; ret: RepoSummary[] };
  library_refresh: { args: Req; ret: RepoSummary[] };
  repo_by_slug: { args: { owner: string; name: string }; ret: RepoSummary };
  import_pick: { args: NoArgs; ret: ImportPreview | null };
  import_confirm: { args: { token: string }; ret: RepoSummary };
  repo_locate: { args: Repo; ret: RepoSummary | null };
  repo_remove: { args: Repo; ret: RemoveResult };
  repo_set_metadata: {
    args: Repo & { description: string | null; homepage: string | null; topics: string[] };
    ret: RepoSummary;
  };
  repo_detail: { args: Repo; ret: RepoDetail };
  repo_open_folder: { args: Repo; ret: null };
  refs_list: { args: Repo; ret: RefsList };
  rev_resolve: { args: RepoRev; ret: ResolvedRev };
  tree_list: { args: RepoRevPath; ret: TreeListing };
  tree_last_commits: { args: RepoRevPath & Req; ret: LastCommits };
  commit_latest: { args: RepoRev; ret: LatestCommit };
  commit_count: { args: RepoRev & Req; ret: number };
  readme_render: { args: RepoRevPath; ret: Rendered | null };
  markdown_render_file: { args: RepoRevPath; ret: Rendered };
  languages: { args: RepoRev & Req; ret: Languages };
  blob_meta: { args: RepoRevPath; ret: BlobMeta };
  blob_text: { args: RepoRevPath; ret: BlobText };
  blob_bytes: { args: RepoRevPath; ret: ArrayBuffer };
  blob_save_as: { args: RepoRevPath; ret: string | null };
  file_last_commit: { args: RepoRevPath & Req; ret: CommitSummary | null };
  commits_page: {
    args: RepoRev & { path: string | null; since: string | null; skip: number; tip: string | null };
    ret: CommitsPage;
  };
  commit_detail: { args: Repo & { oid: string }; ret: CommitDetail };
  commit_file_diff: { args: Repo & { oid: string; index: number; force: boolean }; ret: FileDiff };
  branches_list: { args: Repo; ret: BranchRow[] };
  branches_ahead_behind: { args: Repo & Req & { names: string[] }; ret: AheadBehindEntry[] };
  tags_list: { args: Repo; ret: TagRow[] };
  releases_list: { args: Repo; ret: Release[] };
  releases_fetch_github: { args: Repo; ret: Release[] };
  clone_preview: { args: { input: string }; ret: ClonePreview };
  clone_start: { args: { input: string }; ret: JobId };
  fetch_start: { args: Repo; ret: JobId };
  fetch_all_start: { args: NoArgs; ret: JobId };
  fast_forward: { args: Repo; ret: string };
  jobs_subscribe: { args: { channel: ChannelLike<JobInfo> }; ret: JobInfo[] };
  jobs_cancel: { args: { jobId: JobId }; ret: boolean };
  jobs_retry: { args: { jobId: JobId }; ret: JobId };
  jobs_dismiss: { args: { jobId: JobId }; ret: null };
  request_cancel: { args: Req; ret: boolean };
  open_external: { args: { url: string }; ret: null };
}

export type CommandName = keyof Commands;
export type ArgsOf<K extends CommandName> = Commands[K]['args'];
export type RetOf<K extends CommandName> = Commands[K]['ret'];

/** Every command name, in ARCH §5.1 order (48). */
export const COMMAND_NAMES = [
  'app_info',
  'settings_get',
  'settings_set',
  'cache_clear',
  'app_open_dir',
  'library_list',
  'library_refresh',
  'repo_by_slug',
  'import_pick',
  'import_confirm',
  'repo_locate',
  'repo_remove',
  'repo_set_metadata',
  'repo_detail',
  'repo_open_folder',
  'refs_list',
  'rev_resolve',
  'tree_list',
  'tree_last_commits',
  'commit_latest',
  'commit_count',
  'readme_render',
  'markdown_render_file',
  'languages',
  'blob_meta',
  'blob_text',
  'blob_bytes',
  'blob_save_as',
  'file_last_commit',
  'commits_page',
  'commit_detail',
  'commit_file_diff',
  'branches_list',
  'branches_ahead_behind',
  'tags_list',
  'releases_list',
  'releases_fetch_github',
  'clone_preview',
  'clone_start',
  'fetch_start',
  'fetch_all_start',
  'fast_forward',
  'jobs_subscribe',
  'jobs_cancel',
  'jobs_retry',
  'jobs_dismiss',
  'request_cancel',
  'open_external',
] as const satisfies readonly CommandName[];

/** Compile-time check that COMMAND_NAMES lists every command. */
export const COMMAND_NAMES_COMPLETE: [Exclude<CommandName, (typeof COMMAND_NAMES)[number]>] extends [never]
  ? true
  : false = true;

/** Event names the backend emits (payload type per event). */
export interface Events {
  'library-refreshed': RepoSummary[];
}
