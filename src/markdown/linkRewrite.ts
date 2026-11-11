// Second line of defence for rendered markdown (ARCH §6.6, NFR-S1..S3). The HTML is already ammonia-sanitised in
// Rust; here it is parsed into an INERT <template> (nothing loads, nothing runs) and rewritten before mounting:
// - `rsrepo:/path` anchors → in-app blob routes (the blob view redirects to the tree view for directories);
// - `rsrepo:/path` images → collected for `blob_bytes` → blob: URLs (src removed until then);
// - remote http(s) images → a link chip "Remote image: <host>" (NFR-S2: nothing loads from the network);
// - http(s)/mailto anchors → marked external (click → ConfirmExternalLink);
// - `#fragment` anchors kept (scrolled in-page by MarkdownBody);
// - any other href (javascript:, data:, //host, relative leftovers…) removed;
// - scripts, styles, frames, forms, inline handlers and style attributes stripped.

import { href } from '../router/router';

export interface RewriteContext {
  owner: string;
  repo: string;
  /** Ref name used in routes (decoded, e.g. "feature/x"). */
  rev: string;
}

export interface RepoImage {
  el: HTMLImageElement;
  /** Decoded git path. */
  path: string;
}

export interface CodeBlock {
  el: HTMLElement;
  lang: string;
}

export interface RewriteResult {
  fragment: DocumentFragment;
  images: RepoImage[];
  codeBlocks: CodeBlock[];
}

const DROP_ELEMENTS = 'script, style, iframe, frame, frameset, object, embed, form, link, meta, base, noscript, template';
const DATA_IMAGE = /^data:image\/(png|gif|jpeg|webp)[;,]/i;

/** `rsrepo:/docs/a%20b.md#part` → { path: 'docs/a b.md', fragment: 'part' }; null when malformed. */
export function parseRsrepo(url: string): { path: string; fragment: string | null } | null {
  if (!url.startsWith('rsrepo:/')) return null;
  const rest = url.slice('rsrepo:/'.length);
  const hashAt = rest.indexOf('#');
  const rawPath = hashAt >= 0 ? rest.slice(0, hashAt) : rest;
  const fragment = hashAt >= 0 ? rest.slice(hashAt + 1) : null;
  let path: string;
  try {
    path = rawPath
      .split('/')
      .map((s) => decodeURIComponent(s))
      .join('/');
  } catch {
    return null;
  }
  path = path.replace(/^\/+|\/+$/g, '');
  if (path.split('/').some((s) => s === '..' || s === '.')) return null;
  return { path, fragment };
}

function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return 'unknown host';
  }
}

function isWeb(url: string): boolean {
  return /^https?:\/\/[^/]/i.test(url);
}

export function rewriteHtml(html: string, ctx: RewriteContext, doc: Document = document): RewriteResult {
  const tpl = doc.createElement('template');
  tpl.innerHTML = html;
  const root = tpl.content;

  for (const el of [...root.querySelectorAll(DROP_ELEMENTS)]) el.remove();
  for (const el of [...root.querySelectorAll('*')]) {
    for (const attr of [...el.attributes]) {
      const n = attr.name.toLowerCase();
      if (n.startsWith('on') || n === 'style' || n === 'srcset' || n === 'target' || n === 'formaction' || n === 'background') {
        el.removeAttribute(attr.name);
      }
    }
  }

  // anchors
  for (const a of [...root.querySelectorAll('a')]) {
    const raw = (a.getAttribute('href') ?? '').trim();
    a.removeAttribute('data-route');
    a.removeAttribute('data-external');
    if (raw.length === 0) {
      a.removeAttribute('href');
      continue;
    }
    if (raw.startsWith('#')) {
      a.setAttribute('data-fragment', raw.slice(1));
      continue;
    }
    if (raw.startsWith('rsrepo:')) {
      const p = parseRsrepo(raw);
      if (p && p.path.length > 0) {
        a.setAttribute('href', href('blob', { owner: ctx.owner, repo: ctx.repo, ref: ctx.rev, path: p.path }));
        a.setAttribute('data-route', '1');
      } else if (p && p.path.length === 0) {
        a.setAttribute('href', href('tree', { owner: ctx.owner, repo: ctx.repo, ref: ctx.rev, path: '' }));
        a.setAttribute('data-route', '1');
      } else {
        a.removeAttribute('href');
        a.classList.add('md-dead-link');
      }
      continue;
    }
    if (isWeb(raw) || /^mailto:/i.test(raw)) {
      a.setAttribute('data-external', '1');
      a.setAttribute('rel', 'noopener noreferrer');
      continue;
    }
    a.removeAttribute('href');
    a.classList.add('md-dead-link');
  }

  // images
  const images: RepoImage[] = [];
  for (const img of [...root.querySelectorAll('img')]) {
    const src = (img.getAttribute('src') ?? '').trim();
    const alt = img.getAttribute('alt') ?? '';
    if (src.startsWith('rsrepo:')) {
      const p = parseRsrepo(src);
      img.removeAttribute('src');
      if (p && p.path.length > 0) {
        img.setAttribute('data-rsrepo-path', p.path);
        images.push({ el: img, path: p.path });
      } else {
        img.replaceWith(missingImage(doc, alt || 'image'));
      }
      continue;
    }
    if (isWeb(src)) {
      const chip = doc.createElement('a');
      chip.className = 'md-remote-image';
      chip.setAttribute('href', src);
      chip.setAttribute('data-external', '1');
      chip.setAttribute('rel', 'noopener noreferrer');
      chip.setAttribute('title', alt ? `${alt} — ${src}` : src);
      chip.textContent = `Remote image: ${hostOf(src)}`;
      // an image wrapped in a link: replace the whole link so there is one clear target
      const parent = img.parentElement;
      if (parent && parent.tagName === 'A' && parent.childNodes.length === 1) parent.replaceWith(chip);
      else img.replaceWith(chip);
      continue;
    }
    if (DATA_IMAGE.test(src)) continue;
    img.replaceWith(missingImage(doc, alt || 'image'));
  }

  // fenced code
  const codeBlocks: CodeBlock[] = [];
  for (const code of [...root.querySelectorAll('pre > code')]) {
    const cls = [...code.classList].find((c) => c.startsWith('language-'));
    if (cls) codeBlocks.push({ el: code as HTMLElement, lang: cls.slice('language-'.length) });
  }

  return { fragment: root, images, codeBlocks };
}

function missingImage(doc: Document, alt: string): HTMLElement {
  const s = doc.createElement('span');
  s.className = 'md-image-missing';
  s.textContent = alt;
  return s;
}
