// One-shot `performance.mark` per page load (M8.5): the real-app e2e reads these to time NFR-P1 (start → dashboard
// rows). No data leaves the page; marks are only visible to the page's own Performance API.

const done = new Set<string>();

export const MARK_DASHBOARD_ROWS = 'rs:dashboard-rows';

/** Records `name` the first time it is called in this page load; later calls are ignored. */
export function markOnce(name: string): void {
  if (done.has(name)) return;
  done.add(name);
  try {
    performance.mark(name);
  } catch {
    // Performance API unavailable: timing is best-effort.
  }
}

/** Tests only. */
export function resetMarks(): void {
  done.clear();
}
