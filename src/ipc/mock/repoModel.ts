// In-memory git model for one mock repo: history, refs, tree, blobs, diffs, releases — all derived from a RepoDef.
// Errors mirror rs-core: unknown ref → notFound "ref not found", unknown path → notFound "path not found", missing
// folder → repoMissing.

import { ipcError } from '../errors';
import type {
  AheadBehind,
  BlobKind,
  BlobMeta,
  BranchRow,
  ChangeKind,
  CommitDetail,
  CommitSummary,
  CommitsPage,
  DiffLine,
  EntryKind,
  FileChange,
  FileDiff,
  LastCommits,
  RefItem,
  RefKind,
  RefsList,
  Release,
  Rendered,
  ResolvedRev,
  TagRow,
  TreeEntry,
  TreeListing,
} from '../types';
import { languageFor } from '../../hl/languages';
import { fakeOid, genHistory, hash32, HOUR, sig, type GenCommit } from './fixtures/gen';
import type { BranchDef, FileSpec, RepoDef } from './fixtures/model';

export const PAGE_LIMIT = 35;
export const SCAN_CAP = 20_000;
const MAX_FILES = 300;
const FILE_LINES = 2_000;

const enc = new TextEncoder();

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function generatedText(lines: number, seed: string): string {
  const out: string[] = [`// @generated table "${seed}" — do not edit`];
  for (let i = 1; i < lines; i++) out.push(`pub const T_${i}: u32 = 0x${(hash32(seed + i) & 0xffff).toString(16)};`);
  return out.join('\n') + '\n';
}

function specText(spec: FileSpec): string | null {
  if (typeof spec === 'string') return spec;
  if ('exec' in spec) return spec.exec;
  if ('svg' in spec) return spec.svg;
  if ('generatedText' in spec) return generatedText(spec.generatedText.lines, spec.generatedText.seed);
  if ('lfs' in spec) return `version https://git-lfs.github.com/spec/v1\noid sha256:${fakeOid('lfs')}${fakeOid('lfs2').slice(0, 24)}\nsize ${spec.lfs}\n`;
  if ('symlink' in spec) return spec.symlink;
  return null;
}

const IMAGE_EXT: Record<string, string> = {
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  bmp: 'image/bmp',
  ico: 'image/x-icon',
  svg: 'image/svg+xml',
};

export function isVersionTag(name: string): boolean {
  return /^v?\d+(\.\d+)*([-+].*)?$/.test(name);
}

function isGenerated(path: string): boolean {
  const name = path.split('/').pop() ?? path;
  return (
    /(^|\/)(dist|generated)\//.test(path) ||
    name.endsWith('.lock') ||
    name === 'package-lock.json' ||
    name.includes('_generated.') ||
    name.startsWith('generated_') ||
    name.endsWith('.min.js')
  );
}

/** rs-core releases: first annotation line, unless empty or equal to the tag name. */
function titleOf(message: string | undefined, tag: string): string {
  const first = (message ?? '').split('\n', 1)[0].trim();
  return first.length === 0 || first === tag ? tag : first;
}

interface Resolved {
  rev: ResolvedRev;
  chain: GenCommit[];
}

export class MockRepo {
  readonly def: RepoDef;
  readonly mainline: GenCommit[];
  private readonly byOid = new Map<string, GenCommit>();
  private readonly sideChains = new Map<string, GenCommit[]>();
  private readonly dirs = new Map<string, TreeEntry[]>();
  private readonly fileList: string[];
  private readonly textCache = new Map<string, string | null>();

  constructor(def: RepoDef) {
    this.def = def;
    this.mainline = def.history ? genHistory(def.history) : [];
    for (const c of this.mainline) this.byOid.set(c.summary.oid, c);
    for (const b of [...def.branches, ...def.remoteBranches]) this.side(b);
    if (def.head.type === 'detached' && def.head.oid === '' && this.mainline.length > 0) {
      def.head = { type: 'detached', oid: this.mainline[0].summary.oid };
    }
    this.fileList = Object.keys(def.files).sort();
    this.buildTree();
  }

  get id(): number {
    return this.def.summary.id;
  }

  // -- history ------------------------------------------------------------------------------------------------

  private side(b: BranchDef): GenCommit[] {
    const cached = this.sideChains.get(b.name);
    if (cached) return cached;
    const out: GenCommit[] = [];
    const base = this.mainline[b.at];
    if (base && b.ahead) {
      let parent = base.summary.oid;
      let t = base.summary.author.time.secs;
      b.ahead.forEach((s, j) => {
        t += 3 * HOUR;
        const author = base.summary.author;
        const c: GenCommit = {
          summary: {
            oid: fakeOid(this.def.summary.name, 'side', b.name, j),
            summary: s,
            author: { ...author, time: { secs: t, offsetMin: author.time.offsetMin } },
            committer: { ...author, time: { secs: t, offsetMin: author.time.offsetMin } },
            parents: [parent],
          },
          message: s,
        };
        parent = c.summary.oid;
        this.byOid.set(c.summary.oid, c);
        out.push(c);
      });
    }
    this.sideChains.set(b.name, out);
    return out;
  }

  private tipOf(b: BranchDef): GenCommit | undefined {
    const s = this.side(b);
    return s.length > 0 ? s[s.length - 1] : this.mainline[b.at];
  }

  /** Commits reachable from `oid`, newest first. */
  chainFrom(oid: string): GenCommit[] {
    const mi = this.mainline.findIndex((c) => c.summary.oid === oid);
    if (mi >= 0) return this.mainline.slice(mi);
    for (const [name, chain] of this.sideChains) {
      const si = chain.findIndex((c) => c.summary.oid === oid);
      if (si >= 0) {
        const b = [...this.def.branches, ...this.def.remoteBranches].find((x) => x.name === name);
        return [...chain.slice(0, si + 1).reverse(), ...this.mainline.slice(b?.at ?? 0)];
      }
    }
    return [];
  }

  commit(oid: string): GenCommit {
    const c = this.byOid.get(oid);
    if (!c) throw ipcError('notFound', 'commit not found');
    return c;
  }

  // -- refs ---------------------------------------------------------------------------------------------------

  defaultBranch(): string | null {
    return this.def.summary.defaultBranch;
  }

  refs(): RefsList {
    const d = this.defaultBranch();
    const item = (b: BranchDef, kind: RefKind, prefix: string): RefItem | null => {
      const tip = this.tipOf(b);
      if (!tip) return null;
      return { name: b.name, fullName: prefix + b.name, kind, target: tip.summary.oid, isDefault: kind === 'branch' && b.name === d };
    };
    const tags: RefItem[] = [];
    for (const t of this.def.tags) {
      const c = this.mainline[t.at];
      if (c) tags.push({ name: t.name, fullName: 'refs/tags/' + t.name, kind: 'tag', target: c.summary.oid, isDefault: false });
    }
    // rs-core refs::list: by name (byte order), local branches with the default first.
    const byName = (a: RefItem, b: RefItem) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
    const branches = this.def.branches
      .map((b) => item(b, 'branch', 'refs/heads/'))
      .filter((x): x is RefItem => x !== null)
      .sort(byName)
      .sort((a, b) => Number(b.isDefault) - Number(a.isDefault));
    return {
      defaultBranch: d,
      branches,
      remoteBranches: this.def.remoteBranches
        .map((b) => item(b, 'remoteBranch', 'refs/remotes/'))
        .filter((x): x is RefItem => x !== null)
        .sort(byName),
      tags: tags.sort(byName),
    };
  }

  resolve(input: string): Resolved {
    const notFound = () => ipcError('notFound', 'ref not found');
    if (this.mainline.length === 0) throw notFound();
    const r = this.refs();
    const mk = (kind: RefKind, fullName: string | null, oid: string): Resolved => ({
      rev: { input, kind, fullName, commit: oid, tree: fakeOid(this.def.summary.name, 'tree', oid) },
      chain: this.chainFrom(oid),
    });
    const all = [...r.branches, ...r.tags, ...r.remoteBranches];
    const full = all.find((x) => x.fullName === input);
    if (full) return mk(full.kind, full.fullName, full.target);
    for (const list of [r.branches, r.tags, r.remoteBranches]) {
      const hit = list.find((x) => x.name === input);
      if (hit) return mk(hit.kind, hit.fullName, hit.target);
    }
    if (input === 'HEAD') {
      const h = this.def.head;
      if (h.type === 'detached') return mk('commit', null, h.oid);
      if (h.type === 'branch') return this.resolve(h.name);
    }
    if (/^[0-9a-f]{4,40}$/.test(input)) {
      const hits = [...this.byOid.keys()].filter((o) => o.startsWith(input));
      if (hits.length === 1) return mk('commit', null, hits[0]);
      if (hits.length > 1) throw ipcError('invalidRev', `invalid ref: ambiguous commit prefix: ${input}`);
    }
    throw notFound();
  }

  // -- tree / blobs -------------------------------------------------------------------------------------------

  private buildTree(): void {
    const dirs = this.dirs;
    const ensure = (d: string): TreeEntry[] => {
      let l = dirs.get(d);
      if (!l) {
        l = [];
        dirs.set(d, l);
      }
      return l;
    };
    ensure('');
    for (const path of this.fileList) {
      const parts = path.split('/');
      for (let i = 0; i < parts.length; i++) {
        const dir = parts.slice(0, i).join('/');
        const p = parts.slice(0, i + 1).join('/');
        const list = ensure(dir);
        if (list.some((e) => e.path === p)) continue;
        const isLeaf = i === parts.length - 1;
        const spec = this.def.files[path];
        let kind: EntryKind = 'dir';
        let size: number | null = null;
        if (isLeaf) {
          kind =
            typeof spec === 'string'
              ? 'file'
              : 'exec' in spec
                ? 'exec'
                : 'symlink' in spec
                  ? 'symlink'
                  : 'submodule' in spec
                    ? 'submodule'
                    : 'file';
          size = kind === 'submodule' ? null : this.size(path);
        } else {
          ensure(p);
        }
        list.push({
          name: parts[i],
          path: p,
          pathRaw: null,
          kind,
          oid: fakeOid(this.def.summary.name, kind === 'dir' ? 'tree' : 'blob', p),
          size,
          storableHere: true,
        });
      }
    }
    for (const l of dirs.values()) {
      l.sort((a, b) => {
        const ad = a.kind === 'dir' ? 0 : 1;
        const bd = b.kind === 'dir' ? 0 : 1;
        if (ad !== bd) return ad - bd;
        return a.name.toLowerCase() < b.name.toLowerCase() ? -1 : a.name.toLowerCase() > b.name.toLowerCase() ? 1 : 0;
      });
    }
  }

  private text(path: string): string | null {
    if (!this.textCache.has(path)) {
      const spec = this.def.files[path];
      this.textCache.set(path, spec === undefined ? null : specText(spec));
    }
    return this.textCache.get(path) ?? null;
  }

  private size(path: string): number {
    const spec = this.def.files[path];
    if (spec === undefined) return 0;
    if (typeof spec !== 'string' && 'binary' in spec) return spec.binary;
    if (typeof spec !== 'string' && 'pngBase64' in spec) return b64ToBytes(spec.pngBase64).length;
    const t = this.text(path);
    return t === null ? 0 : enc.encode(t).length;
  }

  isDir(path: string): boolean {
    return this.dirs.has(path);
  }

  tree(rev: string, path: string): TreeListing {
    const res = this.resolve(rev);
    const entries = this.dirs.get(path);
    if (!entries) throw ipcError('notFound', 'path not found');
    const readmeEntry = entries.find((e) => e.kind !== 'dir' && /^readme(\.|$)/i.test(e.name));
    // rs-core: `readme` is the entry NAME within the listed directory.
    return { rev: res.rev, path, entries, readme: readmeEntry ? readmeEntry.name : null };
  }

  lastCommits(rev: string, path: string): LastCommits {
    const res = this.resolve(rev);
    const entries = this.dirs.get(path);
    if (!entries) throw ipcError('notFound', 'path not found');
    const window = Math.min(res.chain.length, 25);
    return {
      tip: res.rev.commit,
      path,
      entries: entries.map((e) => ({
        name: e.name,
        commit: res.chain[hash32(e.path) % window]?.summary ?? null,
      })),
      capped: false,
    };
  }

  private kindOf(path: string): BlobKind {
    const spec = this.def.files[path];
    if (typeof spec === 'string') {
      const ext = path.split('.').pop()?.toLowerCase() ?? '';
      if (ext === 'svg') return { type: 'image', mime: IMAGE_EXT.svg };
      return { type: 'text' };
    }
    if ('svg' in spec) return { type: 'image', mime: 'image/svg+xml' };
    if ('pngBase64' in spec) return { type: 'image', mime: 'image/png' };
    if ('binary' in spec) return { type: 'binary' };
    if ('symlink' in spec) return { type: 'symlink', target: spec.symlink };
    if ('submodule' in spec) return { type: 'submodule', url: spec.submodule };
    if ('lfs' in spec) return { type: 'lfsPointer', size: spec.lfs };
    return { type: 'text' };
  }

  meta(rev: string, path: string): BlobMeta {
    this.resolve(rev);
    if (this.def.files[path] === undefined) {
      // rs-core blob: a directory → NotFound("file"), nothing at all → NotFound("path")
      if (this.isDir(path)) throw ipcError('notFound', 'file not found');
      throw ipcError('notFound', 'path not found');
    }
    const kind = this.kindOf(path);
    const size = this.size(path);
    const text = kind.type === 'text' || kind.type === 'lfsPointer' ? this.text(path) : null;
    const lines = text === null ? null : text.length === 0 ? 0 : text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
    const sloc = text === null ? null : text.split('\n').filter((l) => l.trim().length > 0).length;
    return {
      path,
      oid: fakeOid(this.def.summary.name, 'blob', path),
      size,
      kind,
      lines,
      sloc,
      lossy: false,
      highlight: size <= 1024 * 1024,
      displayable: size <= 5 * 1024 * 1024,
      langHint: kind.type === 'text' ? languageFor(path, (text ?? '').split('\n', 1)[0]) : null,
    };
  }

  blobText(rev: string, path: string): { meta: BlobMeta; text: string | null } {
    const meta = this.meta(rev, path);
    const text = meta.displayable && (meta.kind.type === 'text' || meta.kind.type === 'lfsPointer') ? this.text(path) : null;
    return { meta, text };
  }

  bytes(rev: string, path: string): ArrayBuffer {
    const meta = this.meta(rev, path);
    if (meta.size > 5 * 1024 * 1024) throw ipcError('refused', 'The file is larger than 5 MB. Use Save a copy.');
    const spec = this.def.files[path];
    let u8: Uint8Array;
    if (typeof spec !== 'string' && 'pngBase64' in spec) u8 = b64ToBytes(spec.pngBase64);
    else if (typeof spec !== 'string' && 'binary' in spec) u8 = new Uint8Array(spec.binary);
    else u8 = enc.encode(this.text(path) ?? '');
    const out = new ArrayBuffer(u8.byteLength);
    new Uint8Array(out).set(u8);
    return out;
  }

  readme(rev: string, dir: string): Rendered | null {
    this.tree(rev, dir);
    return this.def.readmes[dir] ?? null;
  }

  markdownFile(rev: string, path: string): Rendered {
    this.meta(rev, path);
    const r = this.def.markdown[path];
    if (r) return r;
    const t = this.text(path) ?? '';
    const esc = t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    return { html: `<pre class="readme-plain">${esc}</pre>`, headings: [], sourcePath: path };
  }

  fileLastCommit(rev: string, path: string): CommitSummary | null {
    const res = this.resolve(rev);
    if (this.def.files[path] === undefined && !this.isDir(path)) throw ipcError('notFound', 'path not found');
    const window = Math.min(res.chain.length, 25);
    return res.chain[hash32(path) % window]?.summary ?? null;
  }

  // -- commits ------------------------------------------------------------------------------------------------

  private touches(c: GenCommit, path: string, chain: GenCommit[]): boolean {
    if (c === chain[chain.length - 1]) return true;
    return hash32(c.summary.oid + path) % 4 === 0;
  }

  page(rev: string, a: { path: string | null; since: string | null; skip: number; tip: string | null }): CommitsPage {
    const res = this.resolve(rev);
    const tip = a.tip ?? res.rev.commit;
    let chain = this.chainFrom(tip);
    if (chain.length === 0) throw ipcError('notFound', 'commit not found');
    if (a.since) {
      const hide = new Set(this.chainFrom(this.resolve(a.since).rev.commit).map((c) => c.summary.oid));
      chain = chain.filter((c) => !hide.has(c.summary.oid));
    }
    const out: CommitSummary[] = [];
    let pos = a.skip;
    let scanned = 0;
    let capped = false;
    while (pos < chain.length && out.length < PAGE_LIMIT) {
      if (a.path && scanned >= SCAN_CAP) {
        capped = true;
        break;
      }
      const c = chain[pos];
      pos += 1;
      scanned += 1;
      if (!a.path || this.touches(c, a.path, chain)) out.push(c.summary);
    }
    let hasMore = false;
    if (!capped) {
      for (let p = pos; p < chain.length; p++) {
        if (!a.path || this.touches(chain[p], a.path, chain)) {
          hasMore = true;
          break;
        }
      }
    }
    return { tip, skip: pos, commits: out, hasMore, scanned, capped };
  }

  count(rev: string): number {
    return this.resolve(rev).chain.length;
  }

  private changedFiles(c: GenCommit): string[] {
    const idx = this.mainline.indexOf(c);
    if (this.def.hugeCommitAt !== undefined && idx === this.def.hugeCommitAt) {
      return this.fileList.filter((p) => p.startsWith('generated/')).slice(0, 400);
    }
    if (c.summary.parents.length === 0) return this.fileList.slice(0, 12);
    const texts = this.fileList.filter((p) => typeof this.def.files[p] === 'string');
    if (texts.length === 0) return [];
    const n = 1 + (hash32(c.summary.oid) % Math.min(4, texts.length));
    const out = new Set<string>();
    for (let i = 0; out.size < n && i < 20; i++) out.add(texts[hash32(c.summary.oid + i) % texts.length]);
    return [...out].sort();
  }

  private diffFor(c: GenCommit, path: string, index: number, added: boolean): { change: FileChange; diff: FileDiff } {
    const spec = this.def.files[path];
    const binary = spec !== undefined && typeof spec !== 'string' && ('pngBase64' in spec || 'binary' in spec);
    const text = this.text(path) ?? '';
    const lines = text.split('\n');
    if (lines[lines.length - 1] === '') lines.pop();
    const hunkLines: DiffLine[] = [];
    let additions = 0;
    let deletions = 0;
    let header = '';
    if (!binary) {
      if (added) {
        const shown = lines.slice(0, FILE_LINES);
        shown.forEach((t, i) => hunkLines.push({ kind: 'add', oldNo: null, newNo: i + 1, text: t }));
        additions = lines.length;
        header = `@@ -0,0 +1,${lines.length} @@`;
      } else {
        const at = lines.length > 0 ? hash32(c.summary.oid + path) % lines.length : 0;
        const from = Math.max(0, at - 3);
        const to = Math.min(lines.length, at + 4);
        for (let i = from; i < to; i++) {
          if (i === at) {
            hunkLines.push({ kind: 'del', oldNo: i + 1, newNo: null, text: lines[i].replace(/\S+\s*$/, 'old();') });
            hunkLines.push({ kind: 'add', oldNo: null, newNo: i + 1, text: lines[i] });
            additions += 1;
            deletions += 1;
          } else {
            hunkLines.push({ kind: 'ctx', oldNo: i + 1, newNo: i + 1, text: lines[i] });
          }
        }
        header = `@@ -${from + 1},${to - from} +${from + 1},${to - from} @@`;
      }
    }
    const kind: ChangeKind = added ? 'added' : 'modified';
    return {
      change: {
        index,
        kind,
        oldPath: added ? null : path,
        newPath: path,
        additions,
        deletions,
        binary,
        tooLarge: false,
        generated: isGenerated(path),
      },
      diff: { index, hunks: hunkLines.length > 0 ? [{ header, lines: hunkLines }] : [], truncated: false },
    };
  }

  detail(oid: string): CommitDetail {
    const c = this.commit(oid);
    const files = this.changedFiles(c);
    const root = c.summary.parents.length === 0;
    const parts = files.map((p, i) => this.diffFor(c, p, i, root));
    const deferred = files.length > MAX_FILES;
    const changes = parts.map((p) => (deferred ? { ...p.change, additions: 0, deletions: 0 } : p.change));
    const shown = parts.filter((p) => !deferred && !p.change.binary && !p.change.generated && p.change.additions + p.change.deletions <= FILE_LINES);
    return {
      commit: c.summary,
      message: c.message,
      parentCompared: c.summary.parents[0] ?? null,
      isMerge: c.summary.parents.length > 1,
      files: changes,
      additions: changes.reduce((s, f) => s + f.additions, 0),
      deletions: changes.reduce((s, f) => s + f.deletions, 0),
      diffs: shown.map((p) => p.diff),
      diffsDeferred: deferred,
    };
  }

  fileDiff(oid: string, index: number): FileDiff {
    const c = this.commit(oid);
    const files = this.changedFiles(c);
    const p = files[index];
    if (p === undefined) throw ipcError('notFound', 'file index out of range');
    return this.diffFor(c, p, index, c.summary.parents.length === 0).diff;
  }

  // -- branches / tags / releases -----------------------------------------------------------------------------

  branches(): BranchRow[] {
    // rs-core refs::branches: local + origin/*, default first, then newest commit, then name.
    const r = this.refs();
    return [...r.branches, ...r.remoteBranches]
      .map((item) => ({ item, last: this.commit(item.target).summary, aheadBehind: null }))
      .sort(
        (a, b) =>
          Number(b.item.isDefault) - Number(a.item.isDefault) ||
          b.last.committer.time.secs - a.last.committer.time.secs ||
          (a.item.name < b.item.name ? -1 : a.item.name > b.item.name ? 1 : 0),
      );
  }

  aheadBehind(names: string[]): [string, AheadBehind][] {
    const h = this.def.head;
    const base = h.type === 'branch' ? h.name : this.defaultBranch();
    if (base === null) return [];
    const baseSet = new Set(this.resolve(base).chain.map((c) => c.summary.oid));
    return names.map((n) => {
      const chain = this.resolve(n).chain;
      const set = new Set(chain.map((c) => c.summary.oid));
      let ahead = 0;
      for (const o of set) if (!baseSet.has(o)) ahead += 1;
      let behind = 0;
      for (const o of baseSet) if (!set.has(o)) behind += 1;
      return [n, { ahead, behind, capped: false }];
    });
  }

  tags(): TagRow[] {
    const rows: TagRow[] = [];
    for (const t of this.def.tags) {
      const c = this.mainline[t.at];
      if (!c) continue;
      const tagger = t.annotated && t.tagger ? sig(t.tagger, c.summary.committer.time.secs + HOUR) : null;
      rows.push({
        item: { name: t.name, fullName: 'refs/tags/' + t.name, kind: 'tag', target: c.summary.oid, isDefault: false },
        commit: c.summary,
        annotated: t.annotated === true,
        tagger,
        date: tagger ? tagger.time.secs : c.summary.committer.time.secs,
      });
    }
    return rows.sort((a, b) => b.date - a.date);
  }

  releases(withGh: boolean): Release[] {
    const tags = this.tags().filter((t) => isVersionTag(t.item.name));
    const defs = new Map(this.def.tags.map((t) => [t.name, t]));
    let latestGiven = false;
    return tags.map((t, i) => {
      const d = defs.get(t.item.name);
      // rs-core Version::is_prerelease: the suffix after the numbers starts with '-'
      const pre = /^v?\d+(\.\d+)*-/.test(t.item.name);
      const latest = !pre && !latestGiven;
      if (latest) latestGiven = true;
      const gh = withGh ? this.def.ghNotes?.[t.item.name] : undefined;
      return {
        tag: t.item.name,
        commit: t.commit.oid,
        date: t.date,
        title: gh?.title ?? (t.annotated ? titleOf(d?.message, t.item.name) : t.item.name),
        notesHtml: gh?.html ?? d?.notesHtml ?? null,
        prerelease: pre,
        latest,
        previousTag: tags[i + 1]?.item.name ?? null,
        ghNotes: gh !== undefined,
      };
    });
  }
}
