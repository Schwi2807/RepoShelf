// D-24: every sample in the committed Rust golden (`__samples__/ipc-samples.json`, written by the src-tauri test
// `ipc_samples_match_the_golden`) must match the hand-written shape of its TS type: same keys, same primitive
// kinds. Shapes are typed against `types.ts` (Shape<T> needs exactly T's keys), so a renamed field fails either
// the typecheck (TS side) or this test (Rust side).
import { describe, expect, it } from 'vitest';
import samples from './__samples__/ipc-samples.json';
import type * as T from './types';

type Prim = 'string' | 'number' | 'boolean';
type Spec =
  | Prim
  | { nullable: Spec }
  | { array: Spec }
  | { ref: string }
  | { tuple: Spec[] };

interface ObjShape {
  kind: 'object';
  fields: Record<string, Spec>;
}
interface UnionShape {
  kind: 'union';
  variants: Record<string, Record<string, Spec>>;
}
interface EnumShape {
  kind: 'enum';
  values: string[];
}
interface TupleShape {
  kind: 'tuple';
  items: Spec[];
}
type AnyShape = ObjShape | UnionShape | EnumShape | TupleShape;

type Fields<X> = { [K in keyof X]-?: Spec };

const obj = <X>(fields: Fields<X>): ObjShape => ({ kind: 'object', fields: fields as Record<string, Spec> });
const union = <X extends { type: string }>(variants: {
  [V in X['type']]: Fields<Omit<Extract<X, { type: V }>, 'type'>>;
}): UnionShape => ({ kind: 'union', variants: variants as Record<string, Record<string, Spec>> });
const enumOf = <X extends string>(values: { [V in X]: true }): EnumShape => ({ kind: 'enum', values: Object.keys(values) });

const n = (s: Spec): Spec => ({ nullable: s });
const arr = (s: Spec): Spec => ({ array: s });
const ref = (r: string): Spec => ({ ref: r });

const fetchOutcome = obj<T.FetchOutcome>({
  newCommits: 'number',
  newTags: arr('string'),
  keptLocalTags: arr('string'),
  skippedRefs: arr('string'),
  updatedBranches: arr('string'),
});

const shapes = {
  Source: enumOf<T.Source>({ imported: true, cloned: true }),
  RepoStatus: union<T.RepoStatus>({ ok: {}, missing: {}, corrupt: { message: 'string' }, empty: {} }),
  RepoSummary: obj<T.RepoSummary>({
    id: 'number',
    owner: 'string',
    name: 'string',
    source: ref('Source'),
    path: 'string',
    description: n('string'),
    topics: arr('string'),
    homepage: n('string'),
    originUrl: n('string'),
    originHost: n('string'),
    defaultBranch: n('string'),
    primaryLanguage: n('string'),
    lastCommitAt: n('number'),
    addedAt: 'number',
    clonedAt: n('number'),
    lastOpenedAt: n('number'),
    lastFetchedAt: n('number'),
    status: ref('RepoStatus'),
  }),
  HeadState: union<T.HeadState>({ branch: { name: 'string' }, detached: { oid: 'string' }, unborn: { name: 'string' } }),
  RepoDetail: obj<T.RepoDetail>({
    summary: ref('RepoSummary'),
    isBare: 'boolean',
    head: ref('HeadState'),
    branchCount: 'number',
    tagCount: 'number',
    releaseCount: 'number',
    license: n('string'),
    fetchable: 'boolean',
    canFastForward: n('boolean'),
    dirty: n('boolean'),
  }),
  RefKind: enumOf<T.RefKind>({ branch: true, remoteBranch: true, tag: true, commit: true }),
  ResolvedRev: obj<T.ResolvedRev>({ input: 'string', kind: ref('RefKind'), fullName: n('string'), commit: 'string', tree: 'string' }),
  RefItem: obj<T.RefItem>({ name: 'string', fullName: 'string', kind: ref('RefKind'), target: 'string', isDefault: 'boolean' }),
  RefsList: obj<T.RefsList>({
    defaultBranch: n('string'),
    branches: arr(ref('RefItem')),
    remoteBranches: arr(ref('RefItem')),
    tags: arr(ref('RefItem')),
  }),
  Time: obj<T.Time>({ secs: 'number', offsetMin: 'number' }),
  Signature: obj<T.Signature>({ name: 'string', email: 'string', time: ref('Time') }),
  CommitSummary: obj<T.CommitSummary>({
    oid: 'string',
    summary: 'string',
    author: ref('Signature'),
    committer: ref('Signature'),
    parents: arr('string'),
  }),
  LatestCommit: obj<T.LatestCommit>({ commit: ref('CommitSummary'), count: n('number') }),
  EntryKind: enumOf<T.EntryKind>({ dir: true, file: true, exec: true, symlink: true, submodule: true }),
  TreeEntry: obj<T.TreeEntry>({
    name: 'string',
    path: 'string',
    pathRaw: n(arr('number')),
    kind: ref('EntryKind'),
    oid: 'string',
    size: n('number'),
    storableHere: 'boolean',
  }),
  TreeListing: obj<T.TreeListing>({ rev: ref('ResolvedRev'), path: 'string', entries: arr(ref('TreeEntry')), readme: n('string') }),
  EntryCommit: obj<T.EntryCommit>({ name: 'string', commit: n(ref('CommitSummary')) }),
  LastCommits: obj<T.LastCommits>({ tip: 'string', path: 'string', entries: arr(ref('EntryCommit')), capped: 'boolean' }),
  BlobKind: union<T.BlobKind>({
    text: {},
    image: { mime: 'string' },
    binary: {},
    lfsPointer: { size: n('number') },
    submodule: { url: n('string') },
    symlink: { target: 'string' },
  }),
  BlobMeta: obj<T.BlobMeta>({
    path: 'string',
    oid: 'string',
    size: 'number',
    kind: ref('BlobKind'),
    lines: n('number'),
    sloc: n('number'),
    lossy: 'boolean',
    highlight: 'boolean',
    displayable: 'boolean',
    langHint: n('string'),
  }),
  BlobText: obj<T.BlobText>({ meta: ref('BlobMeta'), text: n('string') }),
  Heading: obj<T.Heading>({ level: 'number', text: 'string', anchor: 'string' }),
  Rendered: obj<T.Rendered>({ html: 'string', headings: arr(ref('Heading')), sourcePath: 'string' }),
  LanguageStat: obj<T.LanguageStat>({ name: 'string', bytes: 'number', percent: 'number' }),
  Languages: obj<T.Languages>({ tree: 'string', items: arr(ref('LanguageStat')) }),
  CommitsPage: obj<T.CommitsPage>({
    tip: 'string',
    skip: 'number',
    commits: arr(ref('CommitSummary')),
    hasMore: 'boolean',
    scanned: 'number',
    capped: 'boolean',
  }),
  ChangeKind: enumOf<T.ChangeKind>({ added: true, deleted: true, modified: true, renamed: true, copied: true, typeChange: true }),
  FileChange: obj<T.FileChange>({
    index: 'number',
    kind: ref('ChangeKind'),
    oldPath: n('string'),
    newPath: n('string'),
    additions: 'number',
    deletions: 'number',
    binary: 'boolean',
    tooLarge: 'boolean',
    generated: 'boolean',
  }),
  LineKind: enumOf<T.LineKind>({ ctx: true, add: true, del: true, noNewline: true }),
  DiffLine: obj<T.DiffLine>({ kind: ref('LineKind'), oldNo: n('number'), newNo: n('number'), text: 'string' }),
  Hunk: obj<T.Hunk>({ header: 'string', lines: arr(ref('DiffLine')) }),
  FileDiff: obj<T.FileDiff>({ index: 'number', hunks: arr(ref('Hunk')), truncated: 'boolean' }),
  CommitDetail: obj<T.CommitDetail>({
    commit: ref('CommitSummary'),
    message: 'string',
    parentCompared: n('string'),
    isMerge: 'boolean',
    files: arr(ref('FileChange')),
    additions: 'number',
    deletions: 'number',
    diffs: arr(ref('FileDiff')),
    diffsDeferred: 'boolean',
  }),
  AheadBehind: obj<T.AheadBehind>({ ahead: 'number', behind: 'number', capped: 'boolean' }),
  AheadBehindEntry: { kind: 'tuple', items: ['string', ref('AheadBehind')] } satisfies TupleShape,
  BranchRow: obj<T.BranchRow>({ item: ref('RefItem'), last: ref('CommitSummary'), aheadBehind: n(ref('AheadBehind')) }),
  TagRow: obj<T.TagRow>({
    item: ref('RefItem'),
    commit: ref('CommitSummary'),
    annotated: 'boolean',
    tagger: n(ref('Signature')),
    date: 'number',
  }),
  Release: obj<T.Release>({
    tag: 'string',
    commit: 'string',
    date: 'number',
    title: 'string',
    notesHtml: n('string'),
    prerelease: 'boolean',
    latest: 'boolean',
    previousTag: n('string'),
    ghNotes: 'boolean',
  }),
  ImportPreview: obj<T.ImportPreview>({
    path: 'string',
    name: 'string',
    isBare: 'boolean',
    originUrl: n('string'),
    duplicateOf: n('number'),
    token: 'string',
  }),
  ClonePreview: obj<T.ClonePreview>({ url: 'string', host: 'string', owner: 'string', name: 'string', targetDir: 'string' }),
  FetchOutcome: fetchOutcome,
  JobResult: union<T.JobResult>({
    cloned: { repoId: 'number', skippedRefs: arr('string') },
    fetched: fetchOutcome.fields as Fields<T.FetchOutcome>,
  }),
  Removed: obj<T.Removed>({ path: 'string', source: ref('Source') }),
  RemoveResult: obj<T.RemoveResult>({ trashed: 'boolean' }),
  Theme: enumOf<T.Theme>({ light: true, dark: true, system: true }),
  SortKey: enumOf<T.SortKey>({ updated: true, name: true, added: true }),
  SortSpec: obj<T.SortSpec>({ key: ref('SortKey'), desc: 'boolean' }),
  SourceFilter: enumOf<T.SourceFilter>({ all: true, imported: true, cloned: true }),
  Settings: obj<T.Settings>({ theme: ref('Theme'), sort: ref('SortSpec'), sourceFilter: ref('SourceFilter'), showDirtyNote: 'boolean' }),
  Version: obj<T.Version>({ major: 'number', minor: 'number', patch: 'number', extra: 'number', pre: n('string') }),
  AppInfo: obj<T.AppInfo>({
    version: 'string',
    dataDir: 'string',
    archiveDir: 'string',
    cacheBytes: 'number',
    repoCount: 'number',
    archiveBytes: n('number'),
  }),
  JobKind: union<T.JobKind>({ clone: { url: 'string' }, fetch: { repoId: 'number' }, fetchAll: {} }),
  JobState: enumOf<T.JobState>({ waiting: true, running: true, done: true, failed: true, cancelled: true }),
  Progress: union<T.Progress>({
    connecting: {},
    receiving: { received: 'number', total: 'number', bytes: 'number' },
    resolving: { done: 'number', total: 'number' },
    finalizing: {},
  }),
  JobInfo: obj<T.JobInfo>({
    id: 'number',
    kind: ref('JobKind'),
    state: ref('JobState'),
    progress: n(ref('Progress')),
    title: 'string',
    result: n(ref('JobResult')),
    error: n('string'),
    startedAt: 'number',
  }),
  IpcError: obj<T.IpcError>({ code: 'string', message: 'string' }),
} satisfies Record<string, AnyShape>;

type TypeName = keyof typeof shapes;

function checkSpec(spec: Spec, v: unknown, at: string, errs: string[]): void {
  if (typeof spec === 'string') {
    if (typeof v !== spec) errs.push(`${at}: expected ${spec}, got ${JSON.stringify(v)}`);
    return;
  }
  if ('nullable' in spec) {
    if (v !== null) checkSpec(spec.nullable, v, at, errs);
    return;
  }
  if ('array' in spec) {
    if (!Array.isArray(v)) {
      errs.push(`${at}: expected array`);
      return;
    }
    v.forEach((x, i) => checkSpec(spec.array, x, `${at}[${i}]`, errs));
    return;
  }
  if ('tuple' in spec) {
    checkShape({ kind: 'tuple', items: spec.tuple }, v, at, errs);
    return;
  }
  const target = (shapes as Record<string, AnyShape>)[spec.ref];
  if (!target) {
    errs.push(`${at}: unknown shape ${spec.ref}`);
    return;
  }
  checkShape(target, v, `${at}<${spec.ref}>`, errs);
}

function checkFields(fields: Record<string, Spec>, v: Record<string, unknown>, at: string, errs: string[], skip: string[] = []) {
  const want = Object.keys(fields).sort();
  const got = Object.keys(v)
    .filter((k) => !skip.includes(k))
    .sort();
  if (want.join() !== got.join()) errs.push(`${at}: keys ${JSON.stringify(got)} != ${JSON.stringify(want)}`);
  for (const k of want) if (k in v) checkSpec(fields[k], v[k], `${at}.${k}`, errs);
}

function checkShape(shape: AnyShape, v: unknown, at: string, errs: string[]): void {
  switch (shape.kind) {
    case 'enum':
      if (typeof v !== 'string' || !shape.values.includes(v)) errs.push(`${at}: ${JSON.stringify(v)} not in ${shape.values.join('|')}`);
      return;
    case 'tuple':
      if (!Array.isArray(v) || v.length !== shape.items.length) {
        errs.push(`${at}: expected a ${shape.items.length}-tuple`);
        return;
      }
      shape.items.forEach((s, i) => checkSpec(s, v[i], `${at}[${i}]`, errs));
      return;
    case 'object':
    case 'union': {
      if (typeof v !== 'object' || v === null || Array.isArray(v)) {
        errs.push(`${at}: expected object, got ${JSON.stringify(v)}`);
        return;
      }
      const o = v as Record<string, unknown>;
      if (shape.kind === 'object') {
        checkFields(shape.fields, o, at, errs);
        return;
      }
      const t = o.type;
      if (typeof t !== 'string' || !(t in shape.variants)) {
        errs.push(`${at}: unknown variant ${JSON.stringify(t)}`);
        return;
      }
      checkFields(shape.variants[t], o, `${at}{${t}}`, errs, ['type']);
    }
  }
}

const golden = samples as Record<string, unknown>;

describe('IPC types match the Rust golden samples (D-24)', () => {
  it('has a sample for every top-level shape and a shape for every sample', () => {
    const typeOf = (key: string) => key.split('.')[0];
    const sampled = new Set(Object.keys(golden).map(typeOf));
    const unknownSamples = [...sampled].filter((k) => !(k in shapes));
    expect(unknownSamples).toEqual([]);
    // shapes only ever nested (never sampled on their own) are still exercised through their parents
    const nestedOnly = ['RefItem', 'Signature', 'CommitSummary', 'EntryCommit', 'Heading', 'LanguageStat', 'Hunk', 'AheadBehind', 'FetchOutcome'];
    const missing = Object.keys(shapes).filter((k) => !sampled.has(k) && !nestedOnly.includes(k));
    expect(missing).toEqual([]);
  });

  for (const [key, value] of Object.entries(golden)) {
    it(`${key} matches its TS shape`, () => {
      const name = key.split('.')[0] as TypeName;
      const errs: string[] = [];
      checkShape(shapes[name], value, key, errs);
      expect(errs).toEqual([]);
      const variant = key.split('.')[1];
      if (variant && shapes[name].kind === 'union') expect((value as { type: string }).type).toBe(variant);
      if (variant && shapes[name].kind === 'enum') expect(value).toBe(variant);
    });
  }

  it('every enum value and union variant appears in the golden', () => {
    const seen = new Set(Object.keys(golden));
    for (const [name, shape] of Object.entries(shapes) as [string, AnyShape][]) {
      if (shape.kind === 'enum') for (const val of shape.values) expect(seen.has(`${name}.${val}`), `${name}.${val}`).toBe(true);
      if (shape.kind === 'union') for (const val of Object.keys(shape.variants)) expect(seen.has(`${name}.${val}`), `${name}.${val}`).toBe(true);
    }
  });

  it('rejects a camelCase drift', () => {
    const errs: string[] = [];
    const drifted = { ...(golden.AheadBehindEntry as [string, object])[1], ahead_count: 1 };
    checkShape(shapes.AheadBehind, drifted, 'drift', errs);
    expect(errs.length).toBeGreaterThan(0);
  });
});
