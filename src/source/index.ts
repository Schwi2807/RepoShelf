// Data source seam (v0.3.0 M10b, D-127): the repo screens (code tab, file view, README, commits, commit page, ref
// lists, releases) read through a `RepoSource` instead of importing `api.ts`. `localSource` is the shelf (the 52
// local commands, addressed by repo id); `exploreSource` is a GitHub repository (the `explore_*` commands, addressed
// by owner + name — never a path or an id). A method that is `null` does not exist online (D-122: it would cost one
// request per file); the screens branch on that explicitly.
//
// `key` is the part of every query key that names the repository: the repo id (number) locally — local keys are
// byte-identical to M7's, so `prime` / `evict` / `invalidate('repo_detail')` keep working — and `x:owner/name`
// (lower-case) online. The two never collide.

import {
  blobBytes,
  blobMeta,
  blobText,
  commitCount,
  commitDetail,
  commitFileDiff,
  commitLatest,
  commitsPage,
  exploreBlobBytes,
  exploreBlobMeta,
  exploreBlobText,
  exploreCommit,
  exploreCommitFileDiff,
  exploreCommitLatest,
  exploreCommits,
  exploreLanguages,
  exploreRepo,
  exploreSaveToShelf,
  exploreReadme,
  exploreReleases,
  exploreRefs,
  exploreRenderFile,
  exploreResolve,
  exploreTree,
  fileLastCommit,
  languages,
  markdownRenderFile,
  readmeRender,
  refsList,
  releasesList,
  revResolve,
  treeLastCommits,
  treeList,
  type CommitsPageArgs,
} from '../ipc/api';
import type {
  BlobMeta,
  BlobText,
  ExploreRefs,
  ExploreRepo,
  JobId,
  CommitDetail,
  CommitSummary,
  CommitsPage,
  FileDiff,
  Languages,
  LastCommits,
  LatestCommit,
  RefsList,
  Release,
  Rendered,
  ResolvedRev,
  TreeListing,
} from '../ipc/types';
import { noteExploreCall } from '../state/explore';

export interface RepoSource {
  /** `true` for a GitHub repository seen online (links get the `explore` flag, local-only actions disappear). */
  readonly explore: boolean;
  /** Repository part of every query key (see the file comment). */
  readonly key: number | string;
  resolve(rev: string): Promise<ResolvedRev>;
  tree(commit: string, path: string): Promise<TreeListing>;
  /** Last commit of each entry. Not online. */
  readonly treeLastCommits: ((commit: string, path: string, requestId: string) => Promise<LastCommits>) | null;
  commitLatest(commit: string): Promise<LatestCommit>;
  /** Exact commit count. Online it comes with `commitLatest`, so there is no separate call. */
  readonly commitCount: ((commit: string, requestId: string) => Promise<number>) | null;
  /** Newest commit that touched a file (null = older than the walk cap). Not online for folders. */
  readonly fileLastCommit: ((commit: string, path: string, requestId: string) => Promise<CommitSummary | null>) | null;
  readme(commit: string, dir: string): Promise<Rendered | null>;
  renderFile(commit: string, path: string): Promise<Rendered>;
  languages(commit: string, requestId: string): Promise<Languages>;
  blobMeta(commit: string, path: string): Promise<BlobMeta>;
  blobText(commit: string, path: string): Promise<BlobText>;
  blobBytes(commit: string, path: string): Promise<ArrayBuffer>;
  refs(): Promise<RefsList>;
  commits(commit: string, args: CommitsPageArgs): Promise<CommitsPage>;
  commitDetail(oid: string): Promise<CommitDetail>;
  commitFileDiff(oid: string, index: number, force: boolean): Promise<FileDiff>;
  releases(): Promise<Release[]>;
}

const locals = new Map<number, RepoSource>();

/** The shelf's repository `repoId` (memoised: one object per id). */
export function localSource(repoId: number): RepoSource {
  let s = locals.get(repoId);
  if (!s) {
    s = {
      explore: false,
      key: repoId,
      resolve: (rev) => revResolve(repoId, rev),
      tree: (c, p) => treeList(repoId, c, p),
      treeLastCommits: (c, p, id) => treeLastCommits(repoId, c, p, id),
      commitLatest: (c) => commitLatest(repoId, c),
      commitCount: (c, id) => commitCount(repoId, c, id),
      fileLastCommit: (c, p, id) => fileLastCommit(repoId, c, p, id),
      readme: (c, d) => readmeRender(repoId, c, d),
      renderFile: (c, p) => markdownRenderFile(repoId, c, p),
      languages: (c, id) => languages(repoId, c, id),
      blobMeta: (c, p) => blobMeta(repoId, c, p),
      blobText: (c, p) => blobText(repoId, c, p),
      blobBytes: (c, p) => blobBytes(repoId, c, p),
      refs: () => refsList(repoId),
      commits: (c, a) => commitsPage(repoId, c, a),
      commitDetail: (o) => commitDetail(repoId, o),
      commitFileDiff: (o, i, f) => commitFileDiff(repoId, o, i, f),
      releases: () => releasesList(repoId),
    };
    locals.set(repoId, s);
  }
  return s;
}

/** Every Explore call refreshes the quota header once it settled, whether it worked or not. */
export function track<T>(p: Promise<T>): Promise<T> {
  const done = () => noteExploreCall();
  p.then(done, done);
  return p;
}

/** Repository-level Explore calls (not part of a `RepoSource`: they exist before a ref is resolved). */
export const exploreRepoInfo = (owner: string, name: string): Promise<ExploreRepo> => track(exploreRepo(owner, name));
export const exploreRefsInfo = (owner: string, name: string): Promise<ExploreRefs> => track(exploreRefs(owner, name));
export const exploreSave = (owner: string, name: string, autoDownload: boolean | null): Promise<JobId> =>
  track(exploreSaveToShelf(owner, name, autoDownload));

/**
 * A GitHub repository. `resolved` is the route ref as resolved by the layout: asking the backend for the ref
 * NAME again (not the 40-hex commit) reuses the saved answer of that resolve, so a repository page stays at the
 * 5 requests of D-124; a sha would be one more request per commit. Any other commit (a commit page, a history
 * link) goes through as given.
 */
export function exploreSource(owner: string, name: string, resolved: ResolvedRev | null): RepoSource {
  // GitHub (and the backend written for it) knows short names: the shelf's routes carry tags as `refs/tags/<name>`
  // (so a tag never loses to a same-named branch), online that prefix is dropped before any request.
  const short = (rev: string): string => rev.replace(/^refs\/(?:tags|heads)\//, '');
  const at = (rev: string): string =>
    short(resolved !== null && rev === resolved.commit && resolved.input !== 'HEAD' ? resolved.input : rev);
  return {
    explore: true,
    key: `x:${owner}/${name}`.toLowerCase(),
    resolve: (rev) =>
      track(
        exploreResolve(owner, name, short(rev)).then((r) =>
          // the backend only sees the short name; the route said which kind it is
          rev.startsWith('refs/tags/')
            ? { ...r, input: rev, kind: 'tag' as const, fullName: rev }
            : rev.startsWith('refs/heads/')
              ? { ...r, input: rev, kind: 'branch' as const, fullName: rev }
              : r,
        ),
      ),
    tree: (c, p) => track(exploreTree(owner, name, at(c), p)),
    treeLastCommits: null,
    commitLatest: (c) => track(exploreCommitLatest(owner, name, at(c))),
    commitCount: null,
    fileLastCommit: (c, p) =>
      track(exploreCommits(owner, name, at(c), { path: p }).then((page) => page.commits[0] ?? null)),
    readme: (c, d) => track(exploreReadme(owner, name, at(c), d)),
    renderFile: (c, p) => track(exploreRenderFile(owner, name, at(c), p)),
    languages: () => track(exploreLanguages(owner, name)),
    blobMeta: (c, p) => track(exploreBlobMeta(owner, name, at(c), p)),
    blobText: (c, p) => track(exploreBlobText(owner, name, at(c), p)),
    blobBytes: (c, p) => track(exploreBlobBytes(owner, name, at(c), p)),
    refs: () => track(exploreRefs(owner, name).then((r) => r.refs)),
    commits: (c, a) => track(exploreCommits(owner, name, at(c), a)),
    commitDetail: (o) => track(exploreCommit(owner, name, o)),
    commitFileDiff: (o, i, f) => track(exploreCommitFileDiff(owner, name, o, i, f)),
    releases: () => track(exploreReleases(owner, name)),
  };
}
