// Mounts sanitised markdown HTML (README, .md preview, release notes) after `rewriteHtml`, then:
// repo images → `blob_meta` + `blob_bytes` → blob: URLs (revoked on unmount); remote images (setting on) →
// `image_fetch` + `image_bytes` → blob: URLs, lazily and capped (F2, else the chip); fenced code → highlighter worker;
// every click is intercepted: in-app → router, `#fragment` → scroll inside the body, http(s)/mailto →
// ConfirmExternalLink, anything else → nothing (NFR-S3).

import { useEffect, useRef, type MouseEvent } from 'react';
import { confirmExternal } from '../components/ui/ConfirmExternalLink';
import { blobBytes, blobMeta } from '../ipc/api';
import { highlightBlock } from '../hl/highlighter';
import { languageForFence } from '../hl/languages';
import { navigate } from '../router/router';
import { useSettings } from '../state/settings';
import { rewriteHtml, type RemoteImg } from './linkRewrite';
import { loadRemoteImage } from './remoteImages';

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
  const loadRemote = useSettings().settings.loadRemoteImages;

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let alive = true;
    const urls: string[] = [];
    const { fragment, images, remoteImages, codeBlocks } = rewriteHtml(html, { owner, repo, rev });
    el.replaceChildren(fragment);

    // F2: remote images through the backend, lazily (only chips near the viewport), at most MAX_PARALLEL at once
    // (remoteImages.ts). Any failure — or the setting off — keeps the "Remote image: host" chip.
    let observer: IntersectionObserver | null = null;
    if (loadRemote && remoteImages.length > 0) {
      const byChip = new Map<Element, RemoteImg>(remoteImages.map((r) => [r.chip, r]));
      const start = (r: RemoteImg) => {
        loadRemoteImage(r.url, () => alive)
          .then((blob) => {
            if (!alive || !r.chip.isConnected) return;
            const url = URL.createObjectURL(blob);
            urls.push(url);
            r.chip.replaceWith(r.restore);
            r.img.src = url;
          })
          .catch(() => {
            if (alive) r.chip.classList.add('md-remote-image--failed');
          });
      };
      if (typeof IntersectionObserver === 'function') {
        observer = new IntersectionObserver(
          (entries) => {
            for (const e of entries) {
              if (!e.isIntersecting) continue;
              observer?.unobserve(e.target);
              const r = byChip.get(e.target);
              if (r) start(r);
            }
          },
          { rootMargin: '300px 0px' },
        );
        for (const r of remoteImages) observer.observe(r.chip);
      } else {
        for (const r of remoteImages) start(r);
      }
    }

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
      observer?.disconnect();
      for (const u of urls) URL.revokeObjectURL(u);
    };
  }, [html, repoId, owner, repo, rev, commit, loadRemote]);

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
