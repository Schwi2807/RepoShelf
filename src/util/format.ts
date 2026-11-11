// Formatting helpers: sizes, relative / absolute times, day groups (local dates), counts.

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** 1024-based: "512 Bytes", "12.3 KB", "4.1 MB". */
export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return '—';
  if (n < 1024) return `${n} ${n === 1 ? 'Byte' : 'Bytes'}`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 ? Math.round(v) : v.toFixed(1).replace(/\.0$/, '')} ${units[i]}`;
}

/** "1,234". */
export function formatCount(n: number): string {
  return Math.round(n)
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

export function plural(n: number, one: string, many = one + 's'): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}

/** "Nov 11, 2026" in the viewer's local time zone. */
export function formatDay(secs: number): string {
  const d = new Date(secs * 1000);
  return `${MONTHS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}

/** "Nov 11, 2026, 14:03" (local). */
export function formatDateTime(secs: number): string {
  const d = new Date(secs * 1000);
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${formatDay(secs)}, ${hh}:${mm}`;
}

/**
 * GitHub-style relative time: "just now", "5 minutes ago", "3 hours ago", "yesterday", "4 days ago",
 * "last month"… past 30 days: "on Nov 11" (same year) / "on Nov 11, 2025".
 */
export function relativeTime(secs: number, nowSecs: number = Date.now() / 1000): string {
  const diff = Math.floor(nowSecs - secs);
  if (diff < 0) {
    return diff > -60 ? 'just now' : `on ${formatDay(secs)}`;
  }
  if (diff < 45) return 'just now';
  if (diff < 90) return '1 minute ago';
  const min = Math.round(diff / 60);
  if (min < 45) return `${min} minutes ago`;
  if (min < 90) return '1 hour ago';
  const hours = Math.round(diff / 3600);
  if (hours < 24) return `${hours} hours ago`;
  const days = Math.floor(diff / 86400);
  if (days <= 1) return 'yesterday';
  if (days < 30) return `${days} days ago`;
  const d = new Date(secs * 1000);
  const n = new Date(nowSecs * 1000);
  return d.getFullYear() === n.getFullYear()
    ? `on ${MONTHS[d.getMonth()]} ${d.getDate()}`
    : `on ${formatDay(secs)}`;
}

/** Local calendar day key "2026-11-11". */
export function dayKey(secs: number): string {
  const d = new Date(secs * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export interface DayGroup<T> {
  key: string;
  /** "Nov 11, 2026" */
  label: string;
  items: T[];
}

/** Group items (already sorted) by the viewer's local day, keeping order; a day split by other days gets one group
 * per run (as GitHub does for out-of-order commit dates). */
export function dayGroups<T>(items: T[], secsOf: (t: T) => number): DayGroup<T>[] {
  const out: DayGroup<T>[] = [];
  for (const it of items) {
    const s = secsOf(it);
    const key = dayKey(s);
    const last = out[out.length - 1];
    if (last && last.key === key) last.items.push(it);
    else out.push({ key, label: formatDay(s), items: [it] });
  }
  return out;
}

export function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** Initials for the avatar: "Avery Lindqvist" → "AL", "jun" → "J". */
export function initials(name: string): string {
  const parts = name
    .trim()
    .split(/[\s._-]+/)
    .filter((p) => p.length > 0);
  if (parts.length === 0) return '?';
  const first = [...parts[0]][0] ?? '?';
  const last = parts.length > 1 ? ([...parts[parts.length - 1]][0] ?? '') : '';
  return (first + last).toUpperCase();
}
