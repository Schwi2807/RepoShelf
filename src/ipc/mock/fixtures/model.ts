// Shape of a mock repository. Everything the mock answers is derived from one `RepoDef`; IPC types come from
// `types.ts`, so a drift in the wire types fails typecheck here too.

import type { HeadState, LanguageStat, Rendered, RepoSummary } from '../../types';
import type { CommitGen, Person } from './gen';

export type FileSpec =
  | string
  | { exec: string }
  | { svg: string }
  | { pngBase64: string }
  | { binary: number }
  | { symlink: string }
  | { submodule: string | null }
  | { lfs: number }
  | { generatedText: { lines: number; seed: string } }
  /** > 1 MB text: declared size, text generated only when read (no highlighting). */
  | { bigText: { lines: number; seed: string; bytes: number } }
  /** > 5 MB text: declared size only, the text is never materialised (not displayable). */
  | { hugeText: number }
  /** Non-UTF-8 text: shown decoded lossily (`meta.lossy`). */
  | { lossy: string };

export interface BranchDef {
  name: string;
  /** Mainline commit index (0 = newest) the branch starts from. */
  at: number;
  /** Extra commits on top of `at` that only this branch has. */
  ahead?: string[];
}

export interface TagDef {
  name: string;
  at: number;
  annotated?: boolean;
  tagger?: Person;
  /** Annotation message (first line = release title). */
  message?: string;
  /** Rendered (ammonia-shaped) annotation body for the release card. */
  notesHtml?: string;
}

export interface RepoDef {
  summary: RepoSummary;
  isBare: boolean;
  head: HeadState;
  license: string | null;
  fetchable: boolean;
  canFastForward: boolean | null;
  dirty: boolean | null;
  files: Record<string, FileSpec>;
  /** null = empty repository (unborn HEAD). */
  history: CommitGen | null;
  branches: BranchDef[];
  remoteBranches: BranchDef[];
  tags: TagDef[];
  /** Directory ('' = root) → rendered README. */
  readmes: Record<string, Rendered>;
  /** Markdown file path → rendered preview. */
  markdown: Record<string, Rendered>;
  languages: LanguageStat[];
  /** Commit (mainline index) that touches every file (`?huge`: 400 files). */
  hugeCommitAt?: number;
  /** GitHub release notes served by `releases_fetch_github` (tag → html). */
  ghNotes?: Record<string, { title: string; html: string }>;
  /** Paths Windows cannot store (`storableHere = false`, NFR-W3). */
  unstorable?: string[];
  /** Paths whose last segment is not valid UTF-8: raw bytes of the full path (`pathRaw`, R-7). */
  rawNames?: Record<string, number[]>;
}
