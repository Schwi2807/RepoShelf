// S2 row 2: Code, Commits, Branches, Tags, Releases with icons and counts; active tab underlined.

import { Code2, GitBranch, History, Package, Tag, type LucideIcon } from 'lucide-react';
import type { RouteName } from '../../router/routes';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { formatCount } from '../../util/format';
import { Counter } from '../ui/bits';

export interface TabCounts {
  commits: number | null;
  branches: number | null;
  tags: number | null;
  releases: number | null;
}

interface TabDef {
  key: 'code' | 'commits' | 'branches' | 'tags' | 'releases';
  label: string;
  icon: LucideIcon;
  routes: RouteName[];
}

const TABS: TabDef[] = [
  { key: 'code', label: 'Code', icon: Code2, routes: ['code', 'tree', 'blob'] },
  { key: 'commits', label: 'Commits', icon: History, routes: ['commits', 'commit'] },
  { key: 'branches', label: 'Branches', icon: GitBranch, routes: ['branches'] },
  { key: 'tags', label: 'Tags', icon: Tag, routes: ['tags'] },
  { key: 'releases', label: 'Releases', icon: Package, routes: ['releases'] },
];

export function TabBar({
  owner,
  repo,
  refName,
  refFromRoute,
  current,
  counts,
  explore = false,
}: {
  explore?: boolean;
  owner: string;
  repo: string;
  refName: string | null;
  refFromRoute: boolean;
  current: RouteName;
  counts: TabCounts;
}) {
  const to = (key: TabDef['key']): string => {
    switch (key) {
      case 'code':
        return refName && refFromRoute ? href('tree', { explore, owner, repo, ref: refName, path: '' }) : href('code', { explore, owner, repo });
      case 'commits':
        return refName ? href('commits', { explore, owner, repo, ref: refName }) : href('code', { explore, owner, repo });
      default:
        return href(key, { explore, owner, repo });
    }
  };
  return (
    <nav className="tabs" aria-label="Repository">
      {TABS.map((t) => {
        const n = t.key === 'code' ? null : counts[t.key];
        return (
          <Link key={t.key} to={to(t.key)} className="tabs__tab" aria-current={t.routes.includes(current) ? 'page' : undefined}>
            <t.icon size={16} aria-hidden="true" />
            {t.label}
            {n !== null && <Counter n={formatCount(n)} />}
          </Link>
        );
      })}
    </nav>
  );
}
