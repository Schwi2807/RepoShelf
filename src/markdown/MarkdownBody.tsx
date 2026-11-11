// Mounts sanitised markdown HTML (README, .md preview, release notes) after `rewriteHtml`, then:
// repo images → `blob_meta` + `blob_bytes` → blob: URLs (revoked on unmount); fenced code → highlighter worker;
// every click is intercepted: in-app → router, `#fragment` → scroll inside the body, http(s)/mailto →
// ConfirmExternalLink, anything else → nothing (NFR-S3).

import { useEffect, useRef, type MouseEvent } from 'react';
import { confirmExternal } from '../components/ui/ConfirmExternalLink';
import { blobBytes, blobMeta } from '../ipc/api';
import { highlightBlock } from '../hl/highlighter';
import { languageForFence } from '../hl/languages';
import { navigate } from '../router/router';
import { rewriteHtml } from './linkRewrite';

export interface MarkdownBodyProps {
  html: string;
  repoId?: number;
  owner?: string;
  repo?: string;
  /** Ref name for in-app links (decoded). */
  rev?: string;
  /** Resolved commit oid used to load repo images (stable). Falls back to `rev`. */
  commit?: string;
  className?: string;
}

export function MarkdownBody({ html, repoId, owner = '', repo = '', rev = '', commit, className }: MarkdownBodyProps) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let alive = true;
    const urls: string[] = [];
    const { fragment, images, codeBlocks } = rewriteHtml(html, { owner, repo, rev });
    el.replaceChildren(fragment);

    if (repoId !== undefined) {
      const at = commit ?? rev;
      for (const { el: img, path } of images) {
        Promise.all([blobMeta(repoId, at, path), blobBytes(repoId, at, path)])
          .then(([meta, bytes]) => {
            if (!alive) return;
            if (meta.kind.type !== 'image') throw new Error('not an image');
            const url = URL.createObjectURL(new Blob([bytes], { type: meta.kind.mime }));
            urls.push(url);
            img.src = url;
          })
          .catch(() => {
            if (!alive) return;
            const s = document.createElement('span');
            s.className = 'md-image-missing';
            s.textContent = `${img.getAttribute('alt') || 'image'} (not found: ${path})`;
            img.replaceWith(s);
          });
      }
    }

    for (const { el: code, lang } of codeBlocks) {
      const id = languageForFence(lang);
      if (id === null) continue;
      const text = code.textContent ?? '';
      void highlightBlock(text.endsWith('\n') ? text.slice(0, -1) : text, id).then((lines) => {
        if (!alive || lines === null) return;
        // hljs output only: escaped text inside <span class="hljs-…">
        code.innerHTML = lines.join('\n');
        code.classList.add('hljs');
      });
    }

    return () => {
      alive = false;
      for (const u of urls) URL.revokeObjectURL(u);
    };
  }, [html, repoId, owner, repo, rev, commit]);

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const a = (e.target as Element).closest('a');
    if (!a || !host.current?.contains(a)) return;
    e.preventDefault();
    const target = a.getAttribute('href');
    if (target === null) return;
    if (a.dataset.route === '1') {
      navigate(target);
    } else if (a.dataset.fragment !== undefined) {
      const id = a.dataset.fragment;
      const dest = [...host.current.querySelectorAll<HTMLElement>('[id]')].find((x) => x.id === id);
      dest?.scrollIntoView({ block: 'start' });
    } else if (a.dataset.external === '1') {
      confirmExternal(target);
    }
  };

  return <div ref={host} className={`markdown ${className ?? ''}`} onClick={onClick} />;
}
