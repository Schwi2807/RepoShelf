// One typed function per IPC command (ARCH §5.1). Screens import only this module (and `query.ts`).
// Cancellable commands take a `requestId` (1–64 chars of [A-Za-z0-9_-]); `newRequestId()` makes one and
// `requestCancel(id)` cancels it (the backend remembers an early cancel for 30 s).

import { call, createChannel } from './bridge';
import type { ChannelLike } from './commands';
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
  ExploreRefs,
  ExploreRepo,
  ExploreSearch,
  ExploreSort,
  ExploreStatus,
  FileDiff,
  ImportPreview,
  JobId,
  JobInfo,
  Languages,
  LastCommits,
  LatestCommit,
  RefsList,
  Release,
  RemoteImage,
  RemoveResult,
  Rendered,
  RepoDetail,
  RepoSummary,
  ResolvedRev,
  Settings,
  TagRow,
  TreeListing,
} from './types';

let reqSeq = 0;
const reqPrefix = Math.random().toString(36).slice(2, 8);

/** A fresh request id (valid per the backend's `[A-Za-z0-9_-]{1,64}` rule). */
export function newRequestId(): string {
  reqSeq += 1;
  return `q-${reqPrefix}-${reqSeq}`;
}

// App / settings
export const appInfo = (): Promise<AppInfo> => call('app_info', {});
export const settingsGet = (): Promise<Settings> => call('settings_get', {});
export const settingsSet = (settings: Settings): Promise<Settings> => call('settings_set', { settings });
export const cacheClear = (): Promise<AppInfo> => call('cache_clear', {});
export const appOpenDir = (which: AppDir): Promise<void> => call('app_open_dir', { which }).then(() => undefined);
export const requestCancel = (requestId: string): Promise<boolean> => call('request_cancel', { requestId });
export const openExternal = (url: string): Promise<void> => call('open_external', { url }).then(() => undefined);

// Library
export const libraryList = (): Promise<RepoSummary[]> => call('library_list', {});
export const libraryRefresh = (requestId: string): Promise<RepoSummary[]> => call('library_refresh', { requestId });
export const repoBySlug = (owner: string, name: string): Promise<RepoSummary> =>
  call('repo_by_slug', { owner, name });
export const importPick = (): Promise<ImportPreview | null> => call('import_pick', {});
/** `autoDownload` (v0.2.0): the dialog's checkbox; null = the setting. */
export const importConfirm = (token: string, autoDownload: boolean | null = null): Promise<RepoSummary> =>
  call('import_confirm', { token, autoDownload });
export const repoLocate = (repoId: number): Promise<RepoSummary | null> => call('repo_locate', { repoId });
export const repoRemove = (repoId: number): Promise<RemoveResult> => call('repo_remove', { repoId });
export const repoSetMetadata = (
  repoId: number,
  description: string | null,
  homepage: string | null,
  topics: string[],
): Promise<RepoSummary> => call('repo_set_metadata', { repoId, description, homepage, topics });
export const repoDetail = (repoId: number): Promise<RepoDetail> => call('repo_detail', { repoId });
export const repoOpenFolder = (repoId: number): Promise<void> =>
  call('repo_open_folder', { repoId }).then(() => undefined);

// Refs / tree / blobs
export const refsList = (repoId: number): Promise<RefsList> => call('refs_list', { repoId });
export const revResolve = (repoId: number, rev: string): Promise<ResolvedRev> => call('rev_resolve', { repoId, rev });
export const treeList = (repoId: number, rev: string, path: string): Promise<TreeListing> =>
  call('tree_list', { repoId, rev, path });
export const treeLastCommits = (repoId: number, rev: string, path: string, requestId: string): Promise<LastCommits> =>
  call('tree_last_commits', { repoId, rev, path, requestId });
export const commitLatest = (repoId: number, rev: string): Promise<LatestCommit> =>
  call('commit_latest', { repoId, rev });
export const commitCount = (repoId: number, rev: string, requestId: string): Promise<number> =>
  call('commit_count', { repoId, rev, requestId });
export const readmeRender = (repoId: number, rev: string, path: string): Promise<Rendered | null> =>
  call('readme_render', { repoId, rev, path });
export const markdownRenderFile = (repoId: number, rev: string, path: string): Promise<Rendered> =>
  call('markdown_render_file', { repoId, rev, path });
export const languages = (repoId: number, rev: string, requestId: string): Promise<Languages> =>
  call('languages', { repoId, rev, requestId });
export const blobMeta = (repoId: number, rev: string, path: string): Promise<BlobMeta> =>
  call('blob_meta', { repoId, rev, path });
export const blobText = (repoId: number, rev: string, path: string): Promise<BlobText> =>
  call('blob_text', { repoId, rev, path });
export const blobBytes = (repoId: number, rev: string, path: string): Promise<ArrayBuffer> =>
  call('blob_bytes', { repoId, rev, path });
export const blobSaveAs = (repoId: number, rev: string, path: string): Promise<string | null> =>
  call('blob_save_as', { repoId, rev, path });
export const fileLastCommit = (
  repoId: number,
  rev: string,
  path: string,
  requestId: string,
): Promise<CommitSummary | null> => call('file_last_commit', { repoId, rev, path, requestId });

// Commits
export interface CommitsPageArgs {
  path?: string | null;
  since?: string | null;
  skip?: number;
  tip?: string | null;
}
/** First page: no `tip`, `skip` 0. "Older": the first page's `tip` and the returned `skip` (D-9, D-37). */
export const commitsPage = (repoId: number, rev: string, a: CommitsPageArgs = {}): Promise<CommitsPage> =>
  call('commits_page', {
    repoId,
    rev,
    path: a.path ? a.path : null,
    since: a.since ?? null,
    skip: a.skip ?? 0,
    tip: a.tip ?? null,
  });
export const commitDetail = (repoId: number, oid: string): Promise<CommitDetail> =>
  call('commit_detail', { repoId, oid });
export const commitFileDiff = (repoId: number, oid: string, index: number, force: boolean): Promise<FileDiff> =>
  call('commit_file_diff', { repoId, oid, index, force });

// Branches / tags / releases
export const branchesList = (repoId: number): Promise<BranchRow[]> => call('branches_list', { repoId });
export const branchesAheadBehind = (repoId: number, names: string[], requestId: string): Promise<AheadBehindEntry[]> =>
  call('branches_ahead_behind', { repoId, names, requestId });
export const tagsList = (repoId: number): Promise<TagRow[]> => call('tags_list', { repoId });
export const releasesList = (repoId: number): Promise<Release[]> => call('releases_list', { repoId });
export const releasesFetchGithub = (repoId: number): Promise<Release[]> => call('releases_fetch_github', { repoId });
/** v0.2.0 (F4): download one GitHub release file into the release store → job `releaseDownload`. */
export const releaseDownload = (repoId: number, tag: string, asset: string): Promise<JobId> =>
  call('release_download', { repoId, tag, asset });
/** Show a downloaded release file in Explorer (`notFound` when it is not downloaded). */
export const releaseReveal = (repoId: number, tag: string, asset: string): Promise<void> =>
  call('release_reveal', { repoId, tag, asset }).then(() => undefined);

// Remote images (v0.2.0, F2): fetched + cached in Rust; the webview only gets bytes → blob:.
export const imageFetch = (url: string): Promise<RemoteImage> => call('image_fetch', { url });
export const imageBytes = (key: string): Promise<ArrayBuffer> => call('image_bytes', { key });

// Clone / fetch / jobs
export const clonePreview = (input: string): Promise<ClonePreview> => call('clone_preview', { input });
/** `autoDownload`: the action's option; null = the setting (v0.2.0). */
export const cloneStart = (input: string, autoDownload: boolean | null = null): Promise<JobId> =>
  call('clone_start', { input, autoDownload });
export const fetchStart = (repoId: number, autoDownload: boolean | null = null): Promise<JobId> =>
  call('fetch_start', { repoId, autoDownload });
export const fetchAllStart = (): Promise<JobId> => call('fetch_all_start', {});
export const fastForward = (repoId: number): Promise<string> => call('fast_forward', { repoId });
export const jobsCancel = (jobId: JobId): Promise<boolean> => call('jobs_cancel', { jobId });
export const jobsRetry = (jobId: JobId): Promise<JobId> => call('jobs_retry', { jobId });
export const jobsDismiss = (jobId: JobId): Promise<void> => call('jobs_dismiss', { jobId }).then(() => undefined);

/** Subscribe this webview to job updates; resolves with the snapshot. One channel per webview (D-53). */
export function jobsSubscribe(onUpdate: (job: JobInfo) => void): Promise<JobInfo[]> {
  const channel: ChannelLike<JobInfo> = createChannel<JobInfo>(onUpdate);
  return call('jobs_subscribe', { channel });
}

// Explore (v0.3.0 M10a): GitHub repositories online, read-only. Every call is addressed by owner + name; the
// webview stays offline (Rust makes the requests). Answers reuse the local wire types (`TreeListing`, `BlobText`,
// `Rendered`, `CommitsPage`, `CommitDetail`, `Release`).
export const exploreStatus = (): Promise<ExploreStatus> => call('explore_status', {});
/** Saves the token in Windows Credential Manager (never stored elsewhere, never returned). */
export const exploreTokenSet = (token: string): Promise<ExploreStatus> => call('explore_token_set', { token });
export const exploreTokenRemove = (): Promise<ExploreStatus> => call('explore_token_remove', {});
export const exploreCacheClear = (): Promise<ExploreStatus> => call('explore_cache_clear', {});
export const exploreRepo = (owner: string, name: string): Promise<ExploreRepo> => call('explore_repo', { owner, name });
export const exploreRefs = (owner: string, name: string): Promise<ExploreRefs> => call('explore_refs', { owner, name });
export const exploreLanguages = (owner: string, name: string): Promise<Languages> =>
  call('explore_languages', { owner, name });
export const exploreResolve = (owner: string, name: string, rev: string): Promise<ResolvedRev> =>
  call('explore_resolve', { owner, name, rev });
export const exploreCommitLatest = (owner: string, name: string, rev: string): Promise<LatestCommit> =>
  call('explore_commit_latest', { owner, name, rev });
export const exploreTree = (owner: string, name: string, rev: string, path: string): Promise<TreeListing> =>
  call('explore_tree', { owner, name, rev, path });
export const exploreReadme = (owner: string, name: string, rev: string, path: string): Promise<Rendered | null> =>
  call('explore_readme', { owner, name, rev, path });
export const exploreRenderFile = (owner: string, name: string, rev: string, path: string): Promise<Rendered> =>
  call('explore_render_file', { owner, name, rev, path });
export const exploreBlobMeta = (owner: string, name: string, rev: string, path: string): Promise<BlobMeta> =>
  call('explore_blob_meta', { owner, name, rev, path });
export const exploreBlobText = (owner: string, name: string, rev: string, path: string): Promise<BlobText> =>
  call('explore_blob_text', { owner, name, rev, path });
export const exploreBlobBytes = (owner: string, name: string, rev: string, path: string): Promise<ArrayBuffer> =>
  call('explore_blob_bytes', { owner, name, rev, path });
/** First page: no `tip`, `skip` 0. "Older": the first page's `tip` and the returned `skip` (a multiple of 35 online). */
export const exploreCommits = (owner: string, name: string, rev: string, a: CommitsPageArgs = {}): Promise<CommitsPage> =>
  call('explore_commits', { owner, name, rev, path: a.path ? a.path : null, skip: a.skip ?? 0, tip: a.tip ?? null });
export const exploreCommit = (owner: string, name: string, oid: string): Promise<CommitDetail> =>
  call('explore_commit', { owner, name, oid });
export const exploreCommitFileDiff = (
  owner: string,
  name: string,
  oid: string,
  index: number,
  force: boolean,
): Promise<FileDiff> => call('explore_commit_file_diff', { owner, name, oid, index, force });
export const exploreReleases = (owner: string, name: string): Promise<Release[]> =>
  call('explore_releases', { owner, name });
export const exploreSearch = (query: string, sort: ExploreSort | null = null, page = 1): Promise<ExploreSearch> =>
  call('explore_search', { query, sort, page });
/** Save to shelf = the existing clone job; `autoDownload` as in `cloneStart`. */
export const exploreSaveToShelf = (owner: string, name: string, autoDownload: boolean | null = null): Promise<JobId> =>
  call('explore_save_to_shelf', { owner, name, autoDownload });
