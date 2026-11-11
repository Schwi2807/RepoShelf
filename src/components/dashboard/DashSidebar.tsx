// Dashboard sidebar (S1): Recent (top 5 by last opened, D-28), archive size + repo count (`app_info`), and the
// optional "Fetch all clones" (one sequential, cancellable job; progress in the jobs popover).

import { Archive, BookMarked, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import { appInfo, fetchAllStart } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { qk, useQuery } from '../../ipc/query';
import type { RepoSummary } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { useJobs } from '../../state/jobs';
import { useRecent } from '../../state/recent';
import { formatBytes, plural } from '../../util/format';
import { Sidebar, SidebarSection } from '../layout/Sidebar';
import { Button } from '../ui/Button';
import { Shimmer } from '../ui/bits';
import { RelativeTime } from '../ui/RelativeTime';

export function DashSidebar({ repos }: { repos: RepoSummary[] }) {
  const recent = useRecent();
  const info = useQuery(qk('app_info', repos.length), () => appInfo());
  const jobs = useJobs();
  const [msg, setMsg] = useState<string | null>(null);
  const clones = repos.filter((r) => r.source === 'cloned').length;
  const fetchAllRunning = jobs.some((j) => j.kind.type === 'fetchAll' && (j.state === 'waiting' || j.state === 'running'));

  return (
    <Sidebar label="Shelf">
      <SidebarSection title="Recent">
        {recent.length === 0 ? (
          <p className="muted small">Repositories you open show up here.</p>
        ) : (
          <ul className="link-list">
            {recent.map((r) => (
              <li key={r.id}>
                <Link to={href('code', { owner: r.owner, repo: r.name })} className="link-list__item">
                  <BookMarked size={16} aria-hidden="true" />
                  <span className="truncate">
                    {r.owner}/<strong>{r.name}</strong>
                  </span>
                </Link>
                {r.lastOpenedAt !== null && (
                  <span className="link-list__note">
                    <RelativeTime secs={r.lastOpenedAt} prefix="opened" />
                  </span>
                )}
              </li>
            ))}
          </ul>
        )}
      </SidebarSection>
      <SidebarSection title="Archive">
        {info.data ? (
          <dl className="stats">
            <div className="stats__item">
              <dt>Repositories</dt>
              <dd>{info.data.repoCount}</dd>
            </div>
            <div className="stats__item">
              <dt>Archive size</dt>
              <dd>{info.data.archiveBytes === null ? 'large' : formatBytes(info.data.archiveBytes)}</dd>
            </div>
          </dl>
        ) : (
          <Shimmer width="80%" block />
        )}
        {info.data && (
          <p className="small muted mt-2 row">
            <Archive size={14} aria-hidden="true" />
            <span className="mono break-all">{info.data.archiveDir}</span>
          </p>
        )}
        {clones > 0 && (
          <div className="mt-3">
            <Button
              size="sm"
              icon={RefreshCw}
              disabled={fetchAllRunning}
              onClick={() => {
                setMsg(null);
                fetchAllStart().catch((e: unknown) => setMsg(errorText(e)));
              }}
            >
              {fetchAllRunning ? 'Fetching all clones…' : `Fetch all ${plural(clones, 'clone')}`}
            </Button>
            {msg && (
              <p className="field__error" role="alert">
                {msg}
              </p>
            )}
          </div>
        )}
      </SidebarSection>
    </Sidebar>
  );
}
