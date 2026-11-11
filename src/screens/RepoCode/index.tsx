// S3 Code tab — M5 placeholder: resolved ref + the directory README through the real markdown pipeline
// (MarkdownBody: link rewrite, repo images, highlighter worker). Full screen (FileTable, ReadmeCard, About…): M6.5.

import { BookOpen } from 'lucide-react';
import { useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { readmeRender } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';
import { MarkdownBody } from '../../markdown/MarkdownBody';
import type { Route } from '../../router/routes';
import { RepoPlaceholder } from '../placeholder';

export default function RepoCode({ route }: { route: Route }) {
  const path = route.params.path ?? '';
  const r = useRepo();
  const commit = r.resolved?.commit ?? null;
  const readme = useQuery(commit ? qk('readme_render', r.repoId, commit, path) : null, () => readmeRender(r.repoId, commit!, path), {
    stable: true,
  });
  return (
    <div className="stack">
      <RepoPlaceholder title="Code" detail={path ? `/${path}` : 'root'} />
      {readme.data && (
        <Box
          header={
            <>
              <BookOpen size={16} aria-hidden="true" className="muted" />
              <span className="box__title">{readme.data.sourcePath.split('/').pop()}</span>
            </>
          }
        >
          <div className="box__body">
            <MarkdownBody html={readme.data.html} repoId={r.repoId} owner={r.owner} repo={r.repo} rev={r.refName ?? ''} commit={commit ?? undefined} />
          </div>
        </Box>
      )}
    </div>
  );
}
