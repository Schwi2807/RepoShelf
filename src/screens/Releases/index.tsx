// S9 Releases tab (FR-31, NFR-O1): Releases | Tags toggle, optional GitHub notes button (github.com origins only),
// one card per version-like tag, newest first (order, Latest and Pre-release come from `releases_list`). v0.2.0:
// github.com repos get Assets + Source code rows per card (F4); files exist once the GitHub list is cached.

import { Package } from 'lucide-react';
import { EmptyRepo, ListSkeleton } from '../../components/layout/RefGate';
import { useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { GhNotesButton, isGithub } from '../../components/refs/GhNotesButton';
import { ReleaseCard } from '../../components/refs/ReleaseCard';
import { ReleasesTagsToggle } from '../../components/refs/ReleasesTagsToggle';
import { Empty } from '../../components/ui/bits';
import { qk, useQuery } from '../../ipc/query';
import { href } from '../../router/router';
import { Link } from '../../router/Link';

export default function Releases() {
  const r = useRepo();
  const { src, explore, local } = r;
  const rel = useQuery(qk('releases_list', src.key), () => src.releases());
  if (r.unborn) return <EmptyRepo what="releases" />;
  const list = rel.data ?? [];
  // online every release came from GitHub's answer, so its files are known and its links are GitHub's
  const github = explore || (local !== null && isGithub(local.summary));
  const ghLoaded = explore || list.some((x) => x.htmlUrl !== null);
  return (
    <div className="stack refs-page">
      <div className="codebar">
        <ReleasesTagsToggle owner={r.owner} repo={r.repo} current="releases" releases={rel.data?.length ?? local?.detail?.releaseCount ?? null} tags={local?.detail?.tagCount ?? null} />
        <span className="spacer" />
        {local && <GhNotesButton summary={local.summary} anyGhNotes={list.some((x) => x.ghNotes) || ghLoaded} />}
      </div>
      {rel.error ? (
        <Box>
          <Empty icon={Package} title="Releases cannot be listed">
            {rel.error.message}
          </Empty>
        </Box>
      ) : !rel.data ? (
        <ListSkeleton rows={3} />
      ) : list.length === 0 ? (
        <Box>
          <Empty icon={Package} title="There aren’t any releases here" actions={<Link to={href('tags', { explore, owner: r.owner, repo: r.repo })}>See all tags</Link>}>
            Releases are tags whose name looks like a version (<span className="mono">v1.2.0</span>, <span className="mono">2.0-rc1</span>).
          </Empty>
        </Box>
      ) : (
        <div className="rcards">
          {list.map((x) => (
            <ReleaseCard key={x.tag} rel={x} src={src} owner={r.owner} repo={r.repo} github={github} ghLoaded={ghLoaded} />
          ))}
        </div>
      )}
    </div>
  );
}
