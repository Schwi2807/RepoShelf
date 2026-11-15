// Full-page Explore states (v0.3.0 M10b): each error code of an Explore call gets its own wording and way out.
// `rateLimited` says when the limit resets (the message carries it, the status has the exact time), `notFound`
// hints at private repositories, `refused` is "Explore is off", `network` offers a retry.

import { Compass, Globe, SearchX, ShieldAlert, WifiOff, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import type { IpcError } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { useExploreStatus } from '../../state/explore';
import { useSettings } from '../../state/settings';
import { Button } from '../ui/Button';
import { Box } from '../layout/Sidebar';
import { Empty } from '../ui/bits';
import { clockTime } from './QuotaBar';

export function ExploreOff() {
  return (
    <main className="container page" data-testid="explore-off">
      <Empty
        icon={Compass}
        title="Explore is turned off"
        actions={<Link to={href('settings')}>Open Settings</Link>}
      >
        Explore reads repositories from GitHub. Turn it on in Settings → Explore to use it; nothing is requested while it is off.
      </Empty>
    </main>
  );
}

/** What to say for a failed Explore call. `what` names the thing that was being loaded ("The repository"). */
export function ExploreFailure({
  error,
  what,
  onRetry,
  inline = false,
}: {
  error: IpcError;
  what: string;
  onRetry?: () => void;
  /** inside a page that already has its own frame (a tab) instead of replacing the page */
  inline?: boolean;
}) {
  const s = useExploreStatus();
  const { settings } = useSettings();
  let icon: LucideIcon = ShieldAlert;
  let title: ReactNode = `${what} cannot be shown`;
  let body: ReactNode = error.message;
  let actions: ReactNode = onRetry ? (
    <Button size="sm" onClick={onRetry}>
      Try again
    </Button>
  ) : undefined;
  switch (error.code) {
    case 'notFound':
      icon = SearchX;
      title = `${what} was not found on GitHub`;
      body = settings.githubTokenSaved
        ? 'It does not exist, was renamed or deleted, or your token has no access to it.'
        : 'It does not exist, was renamed or deleted, or it is private. Private repositories need a GitHub token (Settings → Explore).';
      actions = <Link to={href('exploreSearch', {}, {})}>Search GitHub</Link>;
      break;
    case 'rateLimited':
      title = 'GitHub rate limit reached';
      body = (
        <>
          {(s?.limitedUntil ?? s?.searchLimitedUntil) != null ? (
            <>
              Resets at <strong>{clockTime((s?.limitedUntil ?? s?.searchLimitedUntil)!)}</strong>. Pages you opened before still open from the saved copy.
            </>
          ) : (
            error.message
          )}{' '}
          A GitHub token raises the hourly limit from 60 to 5,000 (Settings → Explore).
        </>
      );
      actions = (
        <>
          <Link to={href('settings')}>Open Settings</Link>
          {onRetry && (
            <Button size="sm" onClick={onRetry}>
              Try again
            </Button>
          )}
        </>
      );
      break;
    case 'network':
      icon = WifiOff;
      title = 'GitHub cannot be reached';
      body = 'Check the connection. Pages you opened before are kept for a while and still open.';
      break;
    case 'refused':
      icon = Compass;
      title = 'Explore refused this';
      actions = <Link to={href('settings')}>Open Settings</Link>;
      break;
    case 'invalidInput':
    case 'invalidPath':
    case 'invalidRev':
      icon = Globe;
      title = 'That address is not valid';
      break;
  }
  const empty = (
    <Empty icon={icon} title={title} actions={actions}>
      {body}
    </Empty>
  );
  return inline ? (
    <Box className="explore-error" >
      <div data-testid="explore-error" data-code={error.code}>
        {empty}
      </div>
    </Box>
  ) : (
    <main className="container page" data-testid="explore-error" data-code={error.code}>
      {empty}
    </main>
  );
}
