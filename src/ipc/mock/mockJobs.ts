// Mock job runner: timer-driven progress (connecting → receiving → resolving → finalizing → done/failed),
// cancel, retry, dismiss, one subscriber channel per page (D-53). Jobs run one at a time like the real runner.

import type { ChannelLike } from '../commands';
import { ipcError } from '../errors';
import type { JobId, JobInfo, JobKind, JobResult } from '../types';

export interface JobOutcome {
  /** Throw (a message string) to fail the job. */
  run: () => JobResult;
}

export type JobPlanner = (kind: JobKind) => JobOutcome;

export interface MockJobsOptions {
  /** Milliseconds per progress tick. */
  tickMs: number;
  planner: JobPlanner;
  onFinished?: (job: JobInfo) => void;
}

const TICKS_RECEIVING = 8;

export class MockJobs {
  private jobs: JobInfo[] = [];
  private nextId = 1;
  private channel: ChannelLike<JobInfo> | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private step = 0;
  private readonly opts: MockJobsOptions;

  constructor(opts: MockJobsOptions) {
    this.opts = opts;
  }

  subscribe(channel: ChannelLike<JobInfo>): JobInfo[] {
    this.channel = channel;
    return this.jobs.map((j) => ({ ...j }));
  }

  list(): JobInfo[] {
    return this.jobs.map((j) => ({ ...j }));
  }

  private send(j: JobInfo): void {
    this.channel?.onmessage({ ...j });
  }

  start(kind: JobKind, title: string): JobId {
    const job: JobInfo = {
      id: this.nextId++,
      kind,
      state: 'waiting',
      progress: null,
      title,
      result: null,
      error: null,
      startedAt: Math.floor(Date.now() / 1000),
    };
    this.jobs.push(job);
    this.send(job);
    this.pump();
    return job.id;
  }

  private running(): JobInfo | undefined {
    return this.jobs.find((j) => j.state === 'running');
  }

  private pump(): void {
    if (this.running() || this.timer !== null) return;
    const next = this.jobs.find((j) => j.state === 'waiting');
    if (!next) return;
    next.state = 'running';
    next.progress = { type: 'connecting' };
    this.step = 0;
    this.send(next);
    this.timer = setTimeout(() => this.tick(next), this.opts.tickMs);
  }

  private tick(job: JobInfo): void {
    this.timer = null;
    if (job.state !== 'running') {
      this.pump();
      return;
    }
    this.step += 1;
    const total = 1840;
    if (this.step <= TICKS_RECEIVING) {
      const received = Math.round((total * this.step) / TICKS_RECEIVING);
      job.progress = { type: 'receiving', received, total, bytes: received * 2210 };
    } else if (this.step <= TICKS_RECEIVING + 2) {
      job.progress = { type: 'resolving', done: this.step === TICKS_RECEIVING + 1 ? 900 : 1200, total: 1200 };
    } else if (this.step === TICKS_RECEIVING + 3) {
      job.progress = { type: 'finalizing' };
    } else {
      try {
        job.result = this.opts.planner(job.kind).run();
        job.state = 'done';
      } catch (e) {
        job.state = 'failed';
        job.error = typeof e === 'string' ? e : e instanceof Error ? e.message : 'failed';
      }
      job.progress = null;
      this.send(job);
      this.opts.onFinished?.({ ...job });
      this.pump();
      return;
    }
    this.send(job);
    this.timer = setTimeout(() => this.tick(job), this.opts.tickMs);
  }

  cancel(id: JobId): boolean {
    // As rs-core JobRunner: unknown or finished → false; a cancelled job has no error text.
    const j = this.jobs.find((x) => x.id === id);
    if (!j || (j.state !== 'waiting' && j.state !== 'running')) return false;
    j.state = 'cancelled';
    j.progress = null;
    j.error = null;
    if (this.timer !== null && this.running() === undefined) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.send(j);
    this.opts.onFinished?.({ ...j });
    this.pump();
    return true;
  }

  retry(id: JobId): JobId {
    const j = this.jobs.find((x) => x.id === id);
    if (!j) throw ipcError('notFound', 'job not found');
    if (j.state !== 'failed' && j.state !== 'cancelled') throw ipcError('refused', 'Only failed or cancelled jobs can be retried.');
    // As rs-core: the retry is a NEW row and the old row is removed.
    const next = this.start(j.kind, j.title);
    this.jobs = this.jobs.filter((x) => x.id !== id);
    return next;
  }

  /** Removes a finished job; waiting/running jobs are left alone (no error), as rs-core. */
  dismiss(id: JobId): void {
    this.jobs = this.jobs.filter((x) => x.id !== id || x.state === 'waiting' || x.state === 'running');
  }
}
