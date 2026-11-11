import { useEffect, useState } from 'react';
import { formatDateTime, relativeTime } from '../../util/format';

const listeners = new Set<() => void>();
let timer: ReturnType<typeof setInterval> | null = null;

function subscribeMinute(l: () => void): () => void {
  listeners.add(l);
  timer ??= setInterval(() => {
    for (const x of listeners) x();
  }, 60_000);
  return () => {
    listeners.delete(l);
    if (listeners.size === 0 && timer) {
      clearInterval(timer);
      timer = null;
    }
  };
}

/** "3 hours ago" with the exact local date as tooltip; re-renders every minute (one shared timer). */
export function RelativeTime({ secs, prefix }: { secs: number; prefix?: string }) {
  const [, tick] = useState(0);
  useEffect(() => subscribeMinute(() => tick((n) => n + 1)), []);
  const iso = new Date(secs * 1000).toISOString();
  return (
    <time dateTime={iso} title={formatDateTime(secs)}>
      {prefix ? `${prefix} ` : ''}
      {relativeTime(secs)}
    </time>
  );
}
