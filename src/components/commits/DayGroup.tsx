// "Commits on Nov 11, 2026" section (S5): the viewer's local calendar day of the commit time (committer time, the
// walk's sort key), a timeline dot, then a box of rows. `groupCommits` keeps the walk order; a day interrupted by
// other days gets one section per run.

import { GitCommitHorizontal } from 'lucide-react';
import type { CommitSummary } from '../../ipc/types';
import type { RepoSource } from '../../source';
import { dayGroups, type DayGroup as Group } from '../../util/format';
import { CommitRow } from './CommitRow';

export function groupCommits(commits: CommitSummary[]): Group<CommitSummary>[] {
  return dayGroups(commits, (c) => c.committer.time.secs);
}

export function DayGroup({ group, src, owner, repo }: { group: Group<CommitSummary>; src: RepoSource; owner: string; repo: string }) {
  return (
    <section className="cgroup" aria-label={`Commits on ${group.label}`}>
      <h2 className="cgroup__title">
        <GitCommitHorizontal size={16} aria-hidden="true" className="cgroup__icon" />
        Commits on {group.label}
      </h2>
      <ol className="box cgroup__list">
        {group.items.map((c) => (
          <CommitRow key={c.oid} c={c} src={src} owner={owner} repo={repo} />
        ))}
      </ol>
    </section>
  );
}
