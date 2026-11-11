// Jobs store (FR-7, NFR-P7): `jobs_subscribe` once per page load (a reload subscribes again, D-53); updates merge
// by id. Done (and cancelled) rows hide after 5 s and are dismissed in the backend; failures stay until dismissed.
// A job that finishes invalidates ref-keyed queries and refreshes the library.

import { jobsCancel, jobsDismiss, jobsRetry, jobsSubscribe } from '../ipc/api';
import { invalidate } from '../ipc/query';
import type { JobId, JobInfo } from '../ipc/types';
import { refreshLibrary } from './library';
import { createStore, useStore } from './store';

export const AUTO_HIDE_MS = 5000;

export const jobsStore = createStore<JobInfo[]>([]);
const hideTimers = new Map<JobId, ReturnType<typeof setTimeout>>();
const finishedSeen = new Set<JobId>();

export function isActive(j: JobInfo): boolean {
  return j.state === 'waiting' || j.state === 'running';
}

function scheduleHide(j: JobInfo): void {
  if (hideTimers.has(j.id)) return;
  hideTimers.set(
    j.id,
    setTimeout(() => {
      hideTimers.delete(j.id);
      jobsStore.set((list) => list.filter((x) => x.id !== j.id));
      jobsDismiss(j.id).catch(() => undefined);
    }, AUTO_HIDE_MS),
  );
}

/** Merge one update (exported for tests). */
export function applyJobUpdate(j: JobInfo): void {
  jobsStore.set((list) => {
    const i = list.findIndex((x) => x.id === j.id);
    return i >= 0 ? list.map((x) => (x.id === j.id ? j : x)) : [...list, j];
  });
  if (j.state === 'done' || j.state === 'cancelled') scheduleHide(j);
  if (!isActive(j) && !finishedSeen.has(j.id)) {
    finishedSeen.add(j.id);
    if (j.state === 'done') {
      invalidate();
      void refreshLibrary();
    }
  }
}

let started: Promise<void> | null = null;

export function startJobs(): Promise<void> {
  started ??= jobsSubscribe(applyJobUpdate).then(
    (snapshot) => {
      // Channel updates can arrive before the snapshot resolves; they are newer, so the snapshot never overwrites them.
      const seen = new Set(jobsStore.get().map((j) => j.id));
      for (const j of snapshot) {
        if (seen.has(j.id)) continue;
        if (!isActive(j)) finishedSeen.add(j.id);
        applyJobUpdate(j);
      }
    },
    () => undefined,
  );
  return started;
}

export function cancelJob(id: JobId): Promise<boolean> {
  return jobsCancel(id);
}

export async function retryJob(id: JobId): Promise<JobId> {
  const next = await jobsRetry(id);
  dismissJob(id);
  return next;
}

export function dismissJob(id: JobId): void {
  const t = hideTimers.get(id);
  if (t) clearTimeout(t);
  hideTimers.delete(id);
  jobsStore.set((list) => list.filter((x) => x.id !== id));
  jobsDismiss(id).catch(() => undefined);
}

export function useJobs(): JobInfo[] {
  return useStore(jobsStore);
}

/** Tests only. */
export function resetJobsForTests(): void {
  for (const t of hideTimers.values()) clearTimeout(t);
  hideTimers.clear();
  finishedSeen.clear();
  started = null;
  jobsStore.set([]);
}
