// TS mirror of the IPC types (rs-core `types.rs`, `jobs.rs`, `settings.rs`; src-tauri `IpcError`, `RemoveResult`).
// Wire shape (D-32): camelCase fields; Option<T> is `T | null` (always present); unit-only enums are camelCase
// strings; enums with data are tagged unions on `type`. Checked against the Rust golden
// `src/ipc/__samples__/ipc-samples.json` by `types.test.ts` (D-24).

export type Source = 'imported' | 'cloned';

export type RepoStatus =
  | { type: 'ok' }
  | { type: 'missing' }
  | { type: 'corrupt'; message: string }
  | { type: 'empty' };

/** Dashboard row (FR-1), from the DB only. Times are epoch seconds. */
export interface RepoSummary {
  id: number;
  owner: string;
  name: string;
  source: Source;
  path: string;
  description: string | null;
  topics: string[];
  homepage: string | null;
  originUrl: string | null;
  originHost: string | null;
  defaultBranch: string | null;
  primaryLanguage: string | null;
  lastCommitAt: number | null;
  addedAt: number;
  clonedAt: number | null;
  lastOpenedAt: number | null;
  lastFetchedAt: number | null;
  status: RepoStatus;
}

export type HeadState =
  | { type: 'branch'; name: string }
  | { type: 'detached'; oid: string }
  | { type: 'unborn'; name: string };

export interface RepoDetail {
  summary: RepoSummary;
  isBare: boolean;
  head: HeadState;
  branchCount: number;
  tagCount: number;
  releaseCount: number;
  license: string | null;
  fetchable: boolean;
  canFastForward: boolean | null;
  dirty: boolean | null;
}

export type RefKind = 'branch' | 'remoteBranch' | 'tag' | 'commit';

export interface ResolvedRev {
  input: string;
  kind: RefKind;
  fullName: string | null;
  commit: string;
  tree: string;
}

export interface RefItem {
  name: string;
  fullName: string;
  kind: RefKind;
  target: string;
  isDefault: boolean;
}

export interface RefsList {
  defaultBranch: string | null;
  branches: RefItem[];
  remoteBranches: RefItem[];
  tags: RefItem[];
}

/** Epoch seconds + UTC offset in minutes. */
export interface Time {
  secs: number;
  offsetMin: number;
}

export interface Signature {
  name: string;
  email: string;
  time: Time;
}

export interface CommitSummary {
  oid: string;
  summary: string;
  author: Signature;
  committer: Signature;
  parents: string[];
}

export interface LatestCommit {
  commit: CommitSummary;
  /** null = still computing, call `commit_count`. */
  count: number | null;
}

export type EntryKind = 'dir' | 'file' | 'exec' | 'symlink' | 'submodule';

export interface TreeEntry {
  name: string;
  path: string;
  /** Raw bytes when the name is not valid UTF-8 (R-7). */
  pathRaw: number[] | null;
  kind: EntryKind;
  oid: string;
  size: number | null;
  storableHere: boolean;
}

export interface TreeListing {
  rev: ResolvedRev;
  path: string;
  entries: TreeEntry[];
  readme: string | null;
}

export interface EntryCommit {
  name: string;
  /** null = older than the 10,000-commit cap. */
  commit: CommitSummary | null;
}

export interface LastCommits {
  tip: string;
  path: string;
  entries: EntryCommit[];
  capped: boolean;
}

export type BlobKind =
  | { type: 'text' }
  | { type: 'image'; mime: string }
  | { type: 'binary' }
  | { type: 'lfsPointer'; size: number | null }
  | { type: 'submodule'; url: string | null }
  | { type: 'symlink'; target: string };

export interface BlobMeta {
  path: string;
  oid: string;
  size: number;
  kind: BlobKind;
  lines: number | null;
  sloc: number | null;
  lossy: boolean;
  /** size <= 1 MB */
  highlight: boolean;
  /** size <= 5 MB */
  displayable: boolean;
  langHint: string | null;
}

export interface BlobText {
  meta: BlobMeta;
  text: string | null;
}

/** Sanitised HTML (ammonia). */
export interface Rendered {
  html: string;
  headings: Heading[];
  sourcePath: string;
}

export interface Heading {
  level: number;
  text: string;
  anchor: string;
}

export interface LanguageStat {
  name: string;
  bytes: number;
  percent: number;
}

export interface Languages {
  tree: string;
  items: LanguageStat[];
}

export interface CommitsPage {
  tip: string;
  /** Where the next page starts (D-37). */
  skip: number;
  commits: CommitSummary[];
  hasMore: boolean;
  scanned: number;
  capped: boolean;
}

export type ChangeKind = 'added' | 'deleted' | 'modified' | 'renamed' | 'copied' | 'typeChange';

export interface FileChange {
  index: number;
  kind: ChangeKind;
  oldPath: string | null;
  newPath: string | null;
  additions: number;
  deletions: number;
  binary: boolean;
  tooLarge: boolean;
  generated: boolean;
}

export type LineKind = 'ctx' | 'add' | 'del' | 'noNewline';

export interface DiffLine {
  kind: LineKind;
  oldNo: number | null;
  newNo: number | null;
  text: string;
}

export interface Hunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  index: number;
  hunks: Hunk[];
  truncated: boolean;
}

export interface CommitDetail {
  commit: CommitSummary;
  message: string;
  parentCompared: string | null;
  isMerge: boolean;
  files: FileChange[];
  additions: number;
  deletions: number;
  diffs: FileDiff[];
  diffsDeferred: boolean;
}

export interface AheadBehind {
  ahead: number;
  behind: number;
  capped: boolean;
}

/** `branches_ahead_behind` returns `[name, AheadBehind][]`. */
export type AheadBehindEntry = [string, AheadBehind];

export interface BranchRow {
  item: RefItem;
  last: CommitSummary;
  aheadBehind: AheadBehind | null;
}

export interface TagRow {
  item: RefItem;
  commit: CommitSummary;
  annotated: boolean;
  tagger: Signature | null;
  date: number;
}

export interface Release {
  tag: string;
  commit: string;
  date: number;
  title: string;
  notesHtml: string | null;
  prerelease: boolean;
  latest: boolean;
  previousTag: string | null;
  ghNotes: boolean;
}

export interface ImportPreview {
  path: string;
  name: string;
  isBare: boolean;
  originUrl: string | null;
  duplicateOf: number | null;
  /** D-18: pass to `import_confirm`; valid 10 minutes, one use. */
  token: string;
}

export interface ClonePreview {
  url: string;
  host: string;
  owner: string;
  name: string;
  targetDir: string;
}

export interface FetchOutcome {
  newCommits: number;
  newTags: string[];
  keptLocalTags: string[];
  skippedRefs: string[];
  updatedBranches: string[];
}

export type JobResult =
  | { type: 'cloned'; repoId: number; skippedRefs: string[] }
  | ({ type: 'fetched' } & FetchOutcome);

export interface Removed {
  path: string;
  source: Source;
}

/** `repo_remove` result. */
export interface RemoveResult {
  trashed: boolean;
}

export type Theme = 'light' | 'dark' | 'system';
export type SortKey = 'updated' | 'name' | 'added';

export interface SortSpec {
  key: SortKey;
  desc: boolean;
}

export type SourceFilter = 'all' | 'imported' | 'cloned';

export interface Settings {
  theme: Theme;
  sort: SortSpec;
  sourceFilter: SourceFilter;
  showDirtyNote: boolean;
}

export interface Version {
  major: number;
  minor: number;
  patch: number;
  extra: number;
  pre: string | null;
}

/** `app_open_dir` argument (D-76): which app folder to open; never a path. */
export type AppDir = 'data' | 'archive';

export interface AppInfo {
  version: string;
  dataDir: string;
  archiveDir: string;
  cacheBytes: number;
  repoCount: number;
  /** null when the archive is too large to measure quickly. */
  archiveBytes: number | null;
}

// Jobs (rs-core jobs.rs)

export type JobId = number;

export type JobKind = { type: 'clone'; url: string } | { type: 'fetch'; repoId: number } | { type: 'fetchAll' };

export type JobState = 'waiting' | 'running' | 'done' | 'failed' | 'cancelled';

export type Progress =
  | { type: 'connecting' }
  | { type: 'receiving'; received: number; total: number; bytes: number }
  | { type: 'resolving'; done: number; total: number }
  | { type: 'finalizing' };

export interface JobInfo {
  id: JobId;
  kind: JobKind;
  state: JobState;
  progress: Progress | null;
  title: string;
  result: JobResult | null;
  error: string | null;
  startedAt: number;
}

/** Every command rejects with this (`code` = rs-core `Error::code()`, plus `internal`). */
export interface IpcError {
  code: IpcErrorCode;
  message: string;
}

export type IpcErrorCode =
  | 'notFound'
  | 'invalidPath'
  | 'invalidRev'
  | 'invalidUrl'
  | 'invalidInput'
  | 'duplicate'
  | 'notARepo'
  | 'repoMissing'
  | 'repoCorrupt'
  | 'cancelled'
  | 'refused'
  | 'network'
  | 'unsupported'
  | 'git'
  | 'db'
  | 'io'
  | 'internal';

/** Event emitted once after the startup summary refresh; payload `RepoSummary[]`. */
export const EVENT_LIBRARY_REFRESHED = 'library-refreshed';
