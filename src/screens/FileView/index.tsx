// S4 File view (FR-21..25, NFR-P4, NFR-S7, NFR-W3, D-8): ref switcher + breadcrumbs, a box with the file's last commit,
// the toolbar and the body: highlighted code with line anchors, raw text (`?plain=1`), markdown preview / code tabs
// (`?view=code`), image preview, or a placeholder (binary, large, LFS, submodule, symlink). `blob_meta` comes first;
// text or bytes are only requested when the kind allows it. A directory on a blob route redirects to its tree.

import { FileQuestion, TriangleAlert } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Breadcrumbs } from '../../components/code/Breadcrumbs';
import { RefSwitcher } from '../../components/code/RefSwitcher';
import { ExternalText } from '../../components/code/AboutSidebar';
import { CodeViewer } from '../../components/file/CodeViewer';
import { FileCommitHeader } from '../../components/file/FileCommitHeader';
import { FileToolbar, type FileMode } from '../../components/file/FileToolbar';
import { ImagePreview } from '../../components/file/ImagePreview';
import { formatLines, parseLines, type LineRange } from '../../components/file/lines';
import { Placeholder, PLACEHOLDER_ICONS } from '../../components/file/Placeholder';
import { useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { Empty, Shimmer } from '../../components/ui/bits';
import { tooLargeToHighlight } from '../../hl/highlighter';
import { LANG_IDS, languageFor, type LangId } from '../../hl/languages';
import { blobMeta, blobText, markdownRenderFile, treeList } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import type { BlobMeta } from '../../ipc/types';
import { MarkdownBody } from '../../markdown/MarkdownBody';
import type { Route } from '../../router/routes';
import { href, navigate, pendingScroll, withQuery } from '../../router/router';
import { Link } from '../../router/Link';
import { formatBytes } from '../../util/format';
import { baseName, isMarkdownPath, normalizePath, parentDir } from '../../util/paths';

const LANGS: ReadonlySet<string> = new Set(LANG_IDS);

/** Resolve a symlink target relative to the link's directory; null when it leaves the repository. */
export function resolveLink(linkPath: string, target: string): string | null {
  if (target.startsWith('/') || /^[A-Za-z]:/.test(target)) return null;
  const out = parentDir(linkPath).split('/').filter(Boolean);
  for (const seg of target.split('/')) {
    if (seg === '' || seg === '.') continue;
    if (seg === '..') {
      if (out.length === 0) return null;
      out.pop();
    } else out.push(seg);
  }
  return out.length === 0 ? null : out.join('/');
}

function langOf(meta: BlobMeta, text: string): LangId | null {
  if (!meta.highlight || tooLargeToHighlight(text)) return null;
  if (meta.langHint && LANGS.has(meta.langHint)) return meta.langHint as LangId;
  return languageFor(meta.path, text.slice(0, 200).split('\n', 1)[0]);
}

export default function FileView({ route }: { route: Route }) {
  return <FileScreen key={`${route.params.ref ?? ''}\u0000${route.params.path ?? ''}`} route={route} />;
}

function FileScreen({ route }: { route: Route }) {
  const r = useRepo();
  const path = normalizePath(route.params.path ?? '');
  const name = baseName(path);
  const commit = r.resolved?.commit ?? null;
  const refName = r.refName;
  // Back/forward restores the old scroll position; only a fresh visit jumps to `?L=` (D-8, D-64).
  const [fresh] = useState(() => (pendingScroll() ?? 0) === 0);

  const meta = useQuery(commit ? qk('blob_meta', r.repoId, commit, path) : null, () => blobMeta(r.repoId, commit!, path), {
    stable: true,
  });
  const probeDir = meta.error?.code === 'notFound' && commit !== null;
  const dirProbe = useQuery(probeDir ? qk('tree_list', r.repoId, commit, path) : null, () => treeList(r.repoId, commit!, path), {
    stable: true,
  });
  useEffect(() => {
    if (dirProbe.data && refName) navigate(href('tree', { owner: r.owner, repo: r.repo, ref: refName, path }), { replace: true });
  }, [dirProbe.data, refName, r.owner, r.repo, path]);

  const workTree = r.summary.source === 'imported' && r.detail?.isBare === false;
  const parent = useQuery(
    commit && workTree && path ? qk('tree_list', r.repoId, commit, parentDir(path)) : null,
    () => treeList(r.repoId, commit!, parentDir(path)),
    { stable: true },
  );
  const entry = parent.data?.entries.find((e) => e.path === path);

  const m = meta.data;
  const kind = m?.kind.type;
  const md = kind === 'text' && isMarkdownPath(path);
  const mode: FileMode = route.query.plain === '1' ? 'raw' : md && route.query.view !== 'code' ? 'preview' : 'code';
  const wantText = m !== undefined && kind === 'text' && m.displayable;
  const text = useQuery(wantText ? qk('blob_text', r.repoId, commit, path) : null, () => blobText(r.repoId, commit!, path), {
    stable: true,
  });
  const preview = useQuery(
    wantText && mode === 'preview' ? qk('markdown_render_file', r.repoId, commit, path) : null,
    () => markdownRenderFile(r.repoId, commit!, path),
    { stable: true },
  );

  const selection = mode === 'code' ? parseLines(route.query.L) : null;
  const lineHref = (lr: LineRange) => withQuery(route, { view: route.query.view, L: formatLines(lr) });

  // ---- not ready / errors ---------------------------------------------------------------------------------
  if (r.revError) {
    return (
      <Box>
        <Empty
          icon={FileQuestion}
          title={`Ref “${refName ?? ''}” not found`}
          actions={
            r.summary.defaultBranch ? (
              <Link to={href('blob', { owner: r.owner, repo: r.repo, ref: r.summary.defaultBranch, path })}>Open on {r.summary.defaultBranch}</Link>
            ) : undefined
          }
        >
          {r.revError.message}
        </Empty>
      </Box>
    );
  }
  if (meta.error && !(probeDir && (dirProbe.loading || dirProbe.data))) {
    return (
      <Box>
        <Empty
          icon={FileQuestion}
          title="File not found"
          actions={refName ? <Link to={href('tree', { owner: r.owner, repo: r.repo, ref: refName, path: parentDir(path) })}>Go to the folder</Link> : undefined}
        >
          <span className="mono">{path}</span> does not exist at <span className="mono">{refName}</span>.
        </Empty>
      </Box>
    );
  }
  if (!commit || !refName || !m) {
    return (
      <div className="stack">
        <Shimmer width={320} height={20} />
        <Box header={<Shimmer width="40%" />}>
          <div className="box__body stack">
            <Shimmer width="90%" />
            <Shimmer width="75%" />
            <Shimmer width="82%" />
          </div>
        </Box>
      </div>
    );
  }

  // ---- body --------------------------------------------------------------------------------------------------
  const textValue = text.data?.text ?? null;
  let body;
  switch (m.kind.type) {
    case 'image':
      body = m.displayable ? (
        <ImagePreview repoId={r.repoId} commit={commit} path={path} mime={m.kind.mime} name={name} />
      ) : (
        <Placeholder kind="large" icon={PLACEHOLDER_ICONS.large} title={`Image too large to display (${formatBytes(m.size)})`}>
          Use Save a copy to open it in another program.
        </Placeholder>
      );
      break;
    case 'binary':
      body = (
        <Placeholder kind="binary" icon={PLACEHOLDER_ICONS.binary} title={`Binary file, ${formatBytes(m.size)}, not shown.`}>
          Use Save a copy to open it in another program.
        </Placeholder>
      );
      break;
    case 'lfsPointer':
      body = (
        <Placeholder kind="lfs" icon={PLACEHOLDER_ICONS.lfs} title="Stored with Git LFS, not in this archive.">
          {m.kind.size !== null ? `The real file is ${formatBytes(m.kind.size)}. ` : ''}RepoShelf does not download LFS objects; only the
          small pointer file is in the repository.
        </Placeholder>
      );
      break;
    case 'submodule':
      body = (
        <Placeholder kind="submodule" icon={PLACEHOLDER_ICONS.submodule} title="Submodule">
          <p>
            Points to commit <span className="mono">{m.oid}</span>
          </p>
          {m.kind.url ? (
            <p className="row row--center">
              <span className="muted">from</span> <ExternalText url={m.kind.url} />
            </p>
          ) : (
            <p className="muted">No URL in .gitmodules.</p>
          )}
          <p className="muted small">Submodules are not cloned; their files are not part of this repository.</p>
        </Placeholder>
      );
      break;
    case 'symlink': {
      const target = resolveLink(path, m.kind.target);
      body = (
        <Placeholder kind="symlink" icon={PLACEHOLDER_ICONS.symlink} title="Symbolic link">
          <p>
            Points to <span className="mono">{m.kind.target}</span>
          </p>
          {target && (
            <p>
              <Link to={href('blob', { owner: r.owner, repo: r.repo, ref: refName, path: target })}>Open {target}</Link>
            </p>
          )}
        </Placeholder>
      );
      break;
    }
    case 'text':
      if (!m.displayable) {
        body = (
          <Placeholder kind="large" icon={PLACEHOLDER_ICONS.large} title={`This file is ${formatBytes(m.size)}, too large to display.`}>
            Files over 5 MB are not shown. Use Save a copy to open it in another program.
          </Placeholder>
        );
      } else if (text.error) {
        body = <p className="box__body field__error">{text.error.message}</p>;
      } else if (textValue === null) {
        body = (
          <div className="box__body stack">
            <Shimmer width="85%" />
            <Shimmer width="70%" />
            <Shimmer width="78%" />
          </div>
        );
      } else if (mode === 'raw') {
        body = <pre className="raw code">{textValue}</pre>;
      } else if (mode === 'preview') {
        body = preview.data ? (
          <div className="box__body readme__body">
            <MarkdownBody html={preview.data.html} repoId={r.repoId} owner={r.owner} repo={r.repo} rev={refName} commit={commit} />
          </div>
        ) : preview.error ? (
          <p className="box__body field__error">{preview.error.message}</p>
        ) : (
          <div className="box__body stack">
            <Shimmer width="40%" height={22} />
            <Shimmer width="85%" />
          </div>
        );
      } else {
        body = (
          <CodeViewer
            text={textValue}
            langId={langOf(m, textValue)}
            selection={selection}
            hrefFor={lineHref}
            onSelect={(lr) => navigate(lineHref(lr), { replace: true, keepScroll: true })}
            scrollToSelection={fresh}
          />
        );
      }
      break;
  }

  const base = { view: undefined, plain: undefined, L: undefined };
  const modes: { mode: FileMode; label: string; to: string }[] =
    m.kind.type !== 'text' || !m.displayable
      ? []
      : md
        ? [
            { mode: 'preview', label: 'Preview', to: withQuery(route, base) },
            { mode: 'code', label: 'Code', to: withQuery(route, { ...base, view: 'code' }) },
            { mode: 'raw', label: 'Raw', to: withQuery(route, { ...base, plain: 1 }) },
          ]
        : [
            { mode: 'code', label: 'Code', to: withQuery(route, base) },
            { mode: 'raw', label: 'Raw', to: withQuery(route, { ...base, plain: 1 }) },
          ];

  const notices = [];
  if (workTree && entry && !entry.storableHere) {
    notices.push(
      <div key="w3" className="notice notice--attention filebox__notice" role="note">
        <TriangleAlert size={16} aria-hidden="true" />
        <span>
          <strong>Not checked out on this PC.</strong> Windows cannot store this name or path, so the file is missing from the folder;
          it is shown from the repository's history.
        </span>
      </div>,
    );
  }
  if (entry?.pathRaw) {
    notices.push(
      <div key="raw" className="notice filebox__notice" role="note">
        <TriangleAlert size={16} aria-hidden="true" />
        <span>This file name is not valid UTF-8; it is shown with replacement characters.</span>
      </div>,
    );
  }
  if (m.lossy && m.kind.type === 'text') {
    notices.push(
      <div key="lossy" className="notice notice--attention filebox__notice" role="note">
        <TriangleAlert size={16} aria-hidden="true" />
        <span>This file is not valid UTF-8. It is shown decoded lossily: invalid bytes appear as �. Save a copy for the exact bytes.</span>
      </div>,
    );
  }
  if (m.kind.type === 'text' && m.displayable && mode === 'code' && textValue !== null && langOf(m, textValue) === null && (!m.highlight || tooLargeToHighlight(textValue))) {
    notices.push(
      <div key="large" className="notice filebox__notice" role="note">
        <TriangleAlert size={16} aria-hidden="true" />
        <span>Large file: shown without syntax highlighting.</span>
      </div>,
    );
  }

  return (
    <div className="stack fileview">
      <div className="codebar">
        <RefSwitcher
          repoId={r.repoId}
          refName={refName}
          resolved={r.resolved}
          hrefFor={(ref) => href('blob', { owner: r.owner, repo: r.repo, ref, path })}
        />
        <Breadcrumbs owner={r.owner} repo={r.repo} refName={refName} path={path} />
      </div>
      <div className="box filebox">
        <FileCommitHeader repoId={r.repoId} owner={r.owner} repo={r.repo} refName={refName} commit={commit} path={path} />
        <FileToolbar
          repoId={r.repoId}
          commit={commit}
          meta={m}
          mode={mode}
          modes={modes}
          text={m.kind.type === 'text' ? textValue : null}
          hasSelection={selection !== null}
        />
        {notices}
        <div className="filebox__body">{body}</div>
      </div>
    </div>
  );
}
