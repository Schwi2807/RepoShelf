// Git path helpers (always '/'-separated, never filesystem paths — D-39). '' is the repository root.

export interface PathSegment {
  /** Segment name ("src"). */
  name: string;
  /** Path up to and including this segment ("crates/core/src"). */
  path: string;
}

/** Split a git path into cumulative breadcrumb segments; empty segments are dropped. */
export function pathSegments(path: string): PathSegment[] {
  const out: PathSegment[] = [];
  let acc = '';
  for (const name of path.split('/')) {
    if (name.length === 0) continue;
    acc = acc.length === 0 ? name : `${acc}/${name}`;
    out.push({ name, path: acc });
  }
  return out;
}

/** Normalise: no leading/trailing or doubled slashes. */
export function normalizePath(path: string): string {
  return pathSegments(path)
    .map((s) => s.name)
    .join('/');
}

/** Parent directory ('' for top-level entries and for the root itself). */
export function parentDir(path: string): string {
  const segs = pathSegments(path);
  return segs.length <= 1 ? '' : segs[segs.length - 2].path;
}

/** Last segment ('' for the root). */
export function baseName(path: string): string {
  const segs = pathSegments(path);
  return segs.length === 0 ? '' : segs[segs.length - 1].name;
}

/** Join a directory and a child name with one '/'. */
export function joinPath(dir: string, name: string): string {
  const d = normalizePath(dir);
  const n = normalizePath(name);
  if (d.length === 0) return n;
  if (n.length === 0) return d;
  return `${d}/${n}`;
}

/** File extension in lower case without the dot ('' when none; dotfiles like `.gitignore` have none). */
export function extension(path: string): string {
  const b = baseName(path);
  const i = b.lastIndexOf('.');
  return i <= 0 ? '' : b.slice(i + 1).toLowerCase();
}

/** True for paths the markdown preview applies to. */
export function isMarkdownPath(path: string): boolean {
  return ['md', 'markdown', 'mdown', 'mkd'].includes(extension(path));
}
