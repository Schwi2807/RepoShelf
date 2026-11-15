// S8 Tags tab (FR-30): Releases | Tags toggle, tags newest first (order from `tags_list`), annotated / lightweight.

import { Tag } from 'lucide-react';
import { EmptyRepo, ListSkeleton } from '../../components/layout/RefGate';
import { OnlineTags } from '../../components/explore/OnlineRefs';
import { useLocal, useRepo } from '../../components/layout/repoContext';
import { Box } from '../../components/layout/Sidebar';
import { ReleasesTagsToggle } from '../../components/refs/ReleasesTagsToggle';
import { TagRow } from '../../components/refs/TagRow';
import { Empty } from '../../components/ui/bits';
import { tagsList } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';

export default function Tags() {
  return useRepo().explore ? <OnlineTags /> : <LocalTags />;
}

function LocalTags() {
  const ctx = useRepo();
  const loc = useLocal();
  const r = { repoId: loc.repoId, owner: ctx.owner, repo: ctx.repo, detail: loc.detail };
  const tags = useQuery(qk('tags_list', r.repoId), () => tagsList(r.repoId));
  if (r.detail?.head.type === 'unborn') return <EmptyRepo what="tags" />;
  return (
    <div className="stack refs-page">
      <div className="codebar">
        <ReleasesTagsToggle owner={r.owner} repo={r.repo} current="tags" releases={r.detail?.releaseCount ?? null} tags={tags.data?.length ?? r.detail?.tagCount ?? null} />
      </div>
      {tags.error ? (
        <Box>
          <Empty icon={Tag} title="Tags cannot be listed">
            {tags.error.message}
          </Empty>
        </Box>
      ) : !tags.data ? (
        <ListSkeleton />
      ) : tags.data.length === 0 ? (
        <Box>
          <Empty icon={Tag} title="No tags yet">
            Tags mark specific points in the history, such as versions.
          </Empty>
        </Box>
      ) : (
        <div className="box">
          <div className="box__header">
            <Tag size={16} aria-hidden="true" />
            <h2 className="box__title">Tags</h2>
          </div>
          <ul className="trows">
            {tags.data.map((t) => (
              <TagRow key={t.item.fullName} row={t} owner={r.owner} repo={r.repo} />
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
