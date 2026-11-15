// Explore quota header (v0.3.0 M10b, D-129): the strip at the top of every Explore page. It shows what GitHub's
// rate-limit headers said last (requests left of the hour and when it resets; the search bucket on the search page),
// says so plainly when the limit is used up ("rate limit reached, resets at HH:MM"; saved pages keep opening), and
// marks saved data (a stale answer after a rate limit or without a network). The numbers come from `explore_status`
// (state/explore.ts), which is read again after every Explore call.

import { CircleAlert, Clock, Gauge, KeyRound, WifiOff } from 'lucide-react';
import { useEffect } from 'react';
import type { ExploreStatus, Quota } from '../../ipc/types';
import { href } from '../../router/router';
import { Link } from '../../router/Link';
import { refreshExploreStatus, useExploreStatus } from '../../state/explore';
import { formatCount } from '../../util/format';

/** Local clock time of a Unix time: "14:32". */
export function clockTime(secs: number): string {
  return new Date(secs * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

/** Seconds from now to a Unix time, as "in 12 min" (rounded up, at least 1). */
export function resetsIn(secs: number, nowSecs = Date.now() / 1000): string {
  const m = Math.max(1, Math.ceil((secs - nowSecs) / 60));
  return m >= 90 ? `in ${Math.round(m / 60)} h` : `in ${m} min`;
}

export interface QuotaView {
  /** "57 of 60 requests left" or null when GitHub has not been asked yet. */
  text: string | null;
  resetAt: number | null;
  /** nothing left in this bucket right now */
  limited: boolean;
  /** limit reset time when limited */
  limitedUntil: number | null;
}

export function quotaView(s: ExploreStatus | null, bucket: 'core' | 'search'): QuotaView {
  const q: Quota | null = s === null ? null : bucket === 'core' ? s.core : s.search;
  const until = s === null ? null : bucket === 'core' ? s.limitedUntil : s.searchLimitedUntil;
  const unit = bucket === 'search' ? 'searches' : 'requests';
  return {
    text: q === null ? null : `${formatCount(q.remaining)} of ${formatCount(q.limit)} ${unit} left`,
    resetAt: q?.resetAt ?? null,
    limited: until !== null,
    limitedUntil: until,
  };
}

/** `bucket`: which limit this page spends (`search` on the search page, `core` everywhere else). */
export function QuotaBar({ bucket = 'core' }: { bucket?: 'core' | 'search' }) {
  const s = useExploreStatus();
  useEffect(() => {
    void refreshExploreStatus();
  }, []);
  const v = quotaView(s, bucket);
  // a core limit also blocks everything but saved pages; the search limit only blocks searching
  const coreLimited = s?.limitedUntil != null;
  const limitedUntil = v.limitedUntil ?? (bucket === 'search' ? null : s?.limitedUntil ?? null);
  return (
    <div className="xbar" data-testid="explore-quota" role="region" aria-label="GitHub request quota">
      <span className="xbar__item">
        <Gauge size={14} aria-hidden="true" />
        <span data-testid="explore-quota-remaining">{v.text ?? (s?.tokenSaved ? 'Signed requests, 5,000 per hour' : 'Anonymous requests, 60 per hour')}</span>
        {v.resetAt !== null && !v.limited && (
          <span className="muted" title={`The hour resets at ${clockTime(v.resetAt)}`}>
            · resets {clockTime(v.resetAt)}
          </span>
        )}
      </span>
      {bucket === 'search' && s?.core && !coreLimited && (
        <span className="xbar__item muted">
          Browsing: {formatCount(s.core.remaining)} of {formatCount(s.core.limit)} left
        </span>
      )}
      {s?.tokenSaved ? (
        <span className="xbar__item muted" title="A GitHub token is saved in Windows Credential Manager">
          <KeyRound size={14} aria-hidden="true" />
          token in use
        </span>
      ) : (
        <span className="xbar__item muted">
          <Link to={href('settings')}>Add a token</Link> for 5,000 per hour
        </span>
      )}
      <span className="spacer" />
      {s?.stale && (
        <span className="xbar__item xbar__stale" data-testid="explore-stale" title="This page shows data saved earlier because GitHub could not be asked">
          {s.stale.reason === 'offline' ? <WifiOff size={14} aria-hidden="true" /> : <Clock size={14} aria-hidden="true" />}
          Saved data from {clockTime(s.stale.fetchedAt)} ({s.stale.reason === 'offline' ? 'no connection' : 'rate limit reached'})
        </span>
      )}
      {limitedUntil !== null && (
        <div className="xbar__limit" role="alert" data-testid="explore-rate-limit">
          <CircleAlert size={16} aria-hidden="true" />
          <span>
            <strong>Rate limit reached</strong>
            {bucket === 'search' && !coreLimited ? ' for searching' : ''}, resets at <strong>{clockTime(limitedUntil)}</strong> ({resetsIn(limitedUntil)}). Pages
            you opened before still open from the saved copy.
          </span>
        </div>
      )}
    </div>
  );
}
