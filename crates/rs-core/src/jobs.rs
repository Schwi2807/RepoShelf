//! Background jobs (clone, fetch, fetch-all) and request-scoped cancellation (ARCH §3.2, §5.5; FR-7, NFR-P7, L-6).
//!
//! No Tauri here: the command layer supplies a `Worker` (runs a job from its kind) and a sink (pushes every
//! `JobInfo` change to the UI). This module is the only writer of the `jobs` table.

use crate::db::Db;
use crate::error::{Error, Result};
use crate::types::JobResult;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub type JobId = u64;

/// Pure data, so a job can be re-run from its row (Retry, also after a restart).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum JobKind {
    Clone { url: String },
    Fetch { repo_id: i64 },
    FetchAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Waiting,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn as_db(self) -> &'static str {
        match self {
            JobState::Waiting => "waiting",
            JobState::Running => "running",
            JobState::Done => "done",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
    pub fn from_db(s: &str) -> Option<JobState> {
        Some(match s {
            "waiting" => JobState::Waiting,
            "running" => JobState::Running,
            "done" => JobState::Done,
            "failed" => JobState::Failed,
            "cancelled" => JobState::Cancelled,
            _ => return None,
        })
    }
    pub fn is_terminal(self) -> bool {
        matches!(self, JobState::Done | JobState::Failed | JobState::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Progress {
    Connecting,
    Receiving { received: usize, total: usize, bytes: usize },
    Resolving { done: usize, total: usize },
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    pub progress: Option<Progress>,
    pub title: String,
    pub result: Option<JobResult>,
    pub error: Option<String>,
    pub started_at: i64,
}

/// Runs a job from its kind; supplied once by the owner (Shelf-aware closure calling clone::clone / fetch::fetch).
pub type Worker = Arc<dyn Fn(&JobKind, &JobCtx) -> Result<JobResult> + Send + Sync>;
/// Receives every `JobInfo` change (state transitions always, progress throttled).
pub type Sink = Arc<dyn Fn(&JobInfo) + Send + Sync>;

/// `jobs` row: id, kind_json, title, state, error, result_json, created_at.
type JobRow = (i64, String, String, String, Option<String>, Option<String>, i64);

pub const INTERRUPTED: &str = "Interrupted (app closed)";
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(80);
const GATE_SLICE: Duration = Duration::from_millis(100);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------------------------------------------
// Cancellation and gates

/// Cancel flag for request-scoped work (last commits, counts, languages, ahead/behind, library refresh).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    /// `Err(Cancelled)` once cancelled; call every 256 commits / 1024 entries.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() { Err(Error::Cancelled) } else { Ok(()) }
    }
    pub fn flag(&self) -> &Arc<AtomicBool> {
        &self.0
    }
}

struct GateInner {
    max: usize,
    used: Mutex<usize>,
    cv: Condvar,
}

/// Counting semaphore (Mutex + Condvar). `acquire` waits in 100 ms slices so a waiting caller can be cancelled.
#[derive(Clone)]
pub struct Gate {
    inner: Arc<GateInner>,
}

/// Releases its slot on drop.
pub struct GatePermit {
    inner: Arc<GateInner>,
}

impl Drop for GatePermit {
    fn drop(&mut self) {
        let mut used = lock(&self.inner.used);
        *used = used.saturating_sub(1);
        self.inner.cv.notify_all();
    }
}

impl Gate {
    pub fn new(max: usize) -> Gate {
        Gate { inner: Arc::new(GateInner { max: max.max(1), used: Mutex::new(0), cv: Condvar::new() }) }
    }
    /// `None` when `cancel` was set before a slot became free.
    pub fn acquire(&self, cancel: &AtomicBool) -> Option<GatePermit> {
        let mut used = lock(&self.inner.used);
        loop {
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            if *used < self.inner.max {
                *used += 1;
                return Some(GatePermit { inner: self.inner.clone() });
            }
            used = self.inner.cv.wait_timeout(used, GATE_SLICE).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
    pub fn in_use(&self) -> usize {
        *lock(&self.inner.used)
    }
    pub fn capacity(&self) -> usize {
        self.inner.max
    }
}

/// Gate of 2 for heavy request-scoped walks so they never starve clone threads (ARCH §5.5).
#[derive(Clone)]
pub struct ComputeGate(Gate);

pub const COMPUTE_SLOTS: usize = 2;

impl Default for ComputeGate {
    fn default() -> Self {
        ComputeGate::new()
    }
}

impl ComputeGate {
    pub fn new() -> ComputeGate {
        ComputeGate(Gate::new(COMPUTE_SLOTS))
    }
    pub fn acquire(&self, cancel: &CancelToken) -> Result<GatePermit> {
        self.0.acquire(cancel.flag()).ok_or(Error::Cancelled)
    }
    pub fn in_use(&self) -> usize {
        self.0.in_use()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// JobCtx

type Emit = Arc<dyn Fn(&Progress) + Send + Sync>;

/// Handed to the worker: progress (throttled) and cancellation. Public constructor so rs-core tests can call
/// `clone::clone` / `fetch::fetch` directly.
pub struct JobCtx {
    cancel: Arc<AtomicBool>,
    emit: Emit,
    last: Mutex<Option<(Instant, std::mem::Discriminant<Progress>)>>,
}

impl JobCtx {
    pub fn new(cancel: Arc<AtomicBool>, emit: impl Fn(&Progress) + Send + Sync + 'static) -> JobCtx {
        JobCtx { cancel, emit: Arc::new(emit), last: Mutex::new(None) }
    }
    /// No progress receiver, fresh cancel flag.
    pub fn noop() -> JobCtx {
        JobCtx::new(Arc::new(AtomicBool::new(false)), |_| {})
    }
    /// Emits when the phase changes, on `Finalizing`, or ≥ 80 ms after the last emitted event; drops the rest.
    pub fn progress(&self, p: Progress) {
        let now = Instant::now();
        let d = std::mem::discriminant(&p);
        {
            let mut last = lock(&self.last);
            let due = match *last {
                None => true,
                Some((at, phase)) => phase != d || matches!(p, Progress::Finalizing) || now.duration_since(at) >= PROGRESS_INTERVAL,
            };
            if !due {
                return;
            }
            *last = Some((now, d));
        }
        (self.emit)(&p);
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled() { Err(Error::Cancelled) } else { Ok(()) }
    }
    /// For git2 callbacks (`transfer_progress` returns `!cancelled`).
    pub fn cancel_flag(&self) -> &Arc<AtomicBool> {
        &self.cancel
    }
}

// ---------------------------------------------------------------------------------------------------------------
// JobRunner

struct Entry {
    info: JobInfo,
    cancel: Arc<AtomicBool>,
}

struct Inner {
    db: Db,
    gate: Gate,
    worker: Worker,
    sink: Sink,
    jobs: Mutex<Vec<Entry>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Inner {
    /// Mutates one entry under the lock, then sends the new snapshot to the sink outside the lock.
    fn update(&self, id: JobId, f: impl FnOnce(&mut JobInfo)) -> Option<JobInfo> {
        let snap = {
            let mut jobs = lock(&self.jobs);
            let e = jobs.iter_mut().find(|e| e.info.id == id)?;
            f(&mut e.info);
            e.info.clone()
        };
        (self.sink)(&snap);
        Some(snap)
    }

    fn persist_state(&self, info: &JobInfo) {
        let result_json = info.result.as_ref().and_then(|r| serde_json::to_string(r).ok());
        let finished = info.state.is_terminal().then(crate::now_secs);
        // A failed status write must not kill the job thread; the in-memory list stays authoritative.
        let _ = self.db.with(|c| {
            c.execute(
                "UPDATE jobs SET state = ?1, error = ?2, result_json = ?3, finished_at = ?4 WHERE id = ?5",
                rusqlite::params![info.state.as_db(), info.error, result_json, finished, info.id as i64],
            )?;
            Ok(())
        });
    }

    fn set_state(&self, id: JobId, state: JobState, result: Option<JobResult>, error: Option<String>) {
        if let Some(info) = self.update(id, |i| {
            i.state = state;
            if state.is_terminal() {
                i.progress = None;
            }
            i.result = result;
            i.error = error;
        }) {
            self.persist_state(&info);
        }
    }
}

fn run_job(inner: Arc<Inner>, id: JobId, kind: JobKind, cancel: Arc<AtomicBool>) {
    let Some(permit) = inner.gate.acquire(&cancel) else {
        inner.set_state(id, JobState::Cancelled, None, None);
        return;
    };
    inner.set_state(id, JobState::Running, None, None);
    let weak = Arc::downgrade(&inner);
    let ctx = JobCtx::new(cancel.clone(), move |p| {
        if let Some(inner) = weak.upgrade() {
            inner.update(id, |i| {
                if i.state == JobState::Running {
                    i.progress = Some(p.clone());
                }
            });
        }
    });
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (inner.worker)(&kind, &ctx)));
    drop(permit);
    let was_cancelled = cancel.load(Ordering::SeqCst);
    match outcome {
        Ok(Ok(r)) => inner.set_state(id, JobState::Done, Some(r), None),
        Ok(Err(Error::Cancelled)) => inner.set_state(id, JobState::Cancelled, None, None),
        Ok(Err(_)) if was_cancelled => inner.set_state(id, JobState::Cancelled, None, None),
        Ok(Err(e)) => inner.set_state(id, JobState::Failed, None, Some(e.to_string())),
        Err(_) => inner.set_state(id, JobState::Failed, None, Some("Internal error (the job crashed)".into())),
    }
}

/// Owns all jobs; at most `max_concurrent` run at once, the rest are `Waiting` (cancellable).
pub struct JobRunner {
    inner: Arc<Inner>,
}

impl JobRunner {
    pub fn new(db: Db, max_concurrent: usize, worker: Worker, sink: Sink) -> Self {
        JobRunner {
            inner: Arc::new(Inner {
                db,
                gate: Gate::new(max_concurrent),
                worker,
                sink,
                jobs: Mutex::new(Vec::new()),
                threads: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Startup: rows left `running`/`waiting` by a killed process become `Failed` "Interrupted (app closed)";
    /// every row is loaded into the list (Retry works after a restart).
    pub fn load_persisted(&self) -> Result<()> {
        let rows: Vec<JobRow> = self.inner.db.tx(|tx| {
            tx.execute(
                "UPDATE jobs SET state = 'failed', error = ?1, finished_at = ?2 WHERE state IN ('running', 'waiting')",
                rusqlite::params![INTERRUPTED, crate::now_secs()],
            )?;
            let mut st = tx.prepare("SELECT id, kind_json, title, state, error, result_json, created_at FROM jobs ORDER BY id")?;
            Ok(st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
                .collect::<std::result::Result<_, _>>()?)
        })?;
        let mut jobs = lock(&self.inner.jobs);
        for (id, kind_json, title, state, error, result_json, created_at) in rows {
            let id = id as JobId;
            if jobs.iter().any(|e| e.info.id == id) {
                continue;
            }
            let (Ok(kind), Some(state)) = (serde_json::from_str::<JobKind>(&kind_json), JobState::from_db(&state)) else {
                continue; // written by a newer build; leave the row alone
            };
            let result = result_json.and_then(|j| serde_json::from_str(&j).ok());
            jobs.push(Entry {
                info: JobInfo { id, kind, state, progress: None, title, result, error, started_at: created_at },
                cancel: Arc::new(AtomicBool::new(false)),
            });
        }
        jobs.sort_by_key(|e| e.info.id);
        Ok(())
    }

    /// INSERTs a `waiting` row (its rowid is the JobId) and starts a thread that waits for a gate slot.
    pub fn submit(&self, kind: JobKind, title: String) -> Result<JobId> {
        let kind_json = serde_json::to_string(&kind).map_err(|e| Error::InvalidInput(e.to_string()))?;
        let now = crate::now_secs();
        let id = self.inner.db.with(|c| {
            c.execute(
                "INSERT INTO jobs (kind_json, title, state, created_at) VALUES (?1, ?2, 'waiting', ?3)",
                rusqlite::params![kind_json, title, now],
            )?;
            Ok(c.last_insert_rowid() as JobId)
        })?;
        let cancel = Arc::new(AtomicBool::new(false));
        let info = JobInfo { id, kind: kind.clone(), state: JobState::Waiting, progress: None, title, result: None, error: None, started_at: now };
        lock(&self.inner.jobs).push(Entry { info: info.clone(), cancel: cancel.clone() });
        (self.inner.sink)(&info);
        let inner = self.inner.clone();
        let handle = std::thread::Builder::new()
            .name(format!("reposhelf-job-{id}"))
            .spawn(move || run_job(inner, id, kind, cancel))
            .map_err(Error::Io)?;
        let mut threads = lock(&self.inner.threads);
        threads.retain(|h| !h.is_finished());
        threads.push(handle);
        Ok(id)
    }

    /// Requests cancellation of a waiting or running job. `false` if unknown or already finished.
    pub fn cancel(&self, id: JobId) -> bool {
        let jobs = lock(&self.inner.jobs);
        match jobs.iter().find(|e| e.info.id == id) {
            Some(e) if !e.info.state.is_terminal() => {
                e.cancel.store(true, Ordering::SeqCst);
                true
            }
            _ => false,
        }
    }

    /// Re-submits a failed/cancelled job's kind as a NEW row; the old row is deleted afterwards (inserting first
    /// guarantees a fresh id: SQLite would reuse the rowid of a just-deleted max row).
    pub fn retry(&self, id: JobId) -> Result<JobId> {
        let (kind, title) = {
            let jobs = lock(&self.inner.jobs);
            let e = jobs.iter().find(|e| e.info.id == id).ok_or_else(|| Error::not_found("job"))?;
            if !matches!(e.info.state, JobState::Failed | JobState::Cancelled) {
                return Err(Error::Refused("Only failed or cancelled jobs can be retried.".into()));
            }
            (e.info.kind.clone(), e.info.title.clone())
        };
        let new = self.submit(kind, title)?;
        lock(&self.inner.jobs).retain(|e| e.info.id != id);
        self.inner.db.with(|c| Ok(c.execute("DELETE FROM jobs WHERE id = ?1", [id as i64])?))?;
        Ok(new)
    }

    pub fn list(&self) -> Vec<JobInfo> {
        lock(&self.inner.jobs).iter().map(|e| e.info.clone()).collect()
    }

    pub fn get(&self, id: JobId) -> Option<JobInfo> {
        lock(&self.inner.jobs).iter().find(|e| e.info.id == id).map(|e| e.info.clone())
    }

    /// Removes a finished job (row deleted). Waiting/running jobs are left alone (cancel them first).
    pub fn dismiss(&self, id: JobId) {
        let removed = {
            let mut jobs = lock(&self.inner.jobs);
            match jobs.iter().position(|e| e.info.id == id && e.info.state.is_terminal()) {
                Some(pos) => {
                    jobs.remove(pos);
                    true
                }
                None => false,
            }
        };
        if removed {
            let _ = self.inner.db.with(|c| Ok(c.execute("DELETE FROM jobs WHERE id = ?1", [id as i64])?));
        }
    }

    /// Waits for every job thread started so far (tests, app shutdown after cancelling).
    pub fn join(&self) {
        let handles: Vec<JoinHandle<()>> = std::mem::take(&mut *lock(&self.inner.threads));
        for h in handles {
            let _ = h.join();
        }
    }

    pub fn running(&self) -> usize {
        self.inner.gate.in_use()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FetchOutcome;
    use std::sync::atomic::AtomicUsize;

    fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
        let start = Instant::now();
        while !f() {
            assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn fetched() -> JobResult {
        JobResult::Fetched(FetchOutcome::default())
    }

    struct Probe {
        running: AtomicUsize,
        max: AtomicUsize,
        started: Mutex<Vec<JobKind>>,
        release: AtomicBool,
    }

    /// Worker that blocks until released (or cancelled); `Fetch { repo_id: -1 }` fails immediately.
    fn blocking_worker(p: Arc<Probe>) -> Worker {
        Arc::new(move |kind: &JobKind, ctx: &JobCtx| {
            lock(&p.started).push(kind.clone());
            if *kind == (JobKind::Fetch { repo_id: -1 }) {
                return Err(Error::Refused("boom".into()));
            }
            let now = p.running.fetch_add(1, Ordering::SeqCst) + 1;
            p.max.fetch_max(now, Ordering::SeqCst);
            let r = loop {
                if ctx.cancelled() {
                    break Err(Error::Cancelled);
                }
                if p.release.load(Ordering::SeqCst) {
                    break Ok(fetched());
                }
                std::thread::sleep(Duration::from_millis(2));
            };
            p.running.fetch_sub(1, Ordering::SeqCst);
            r
        })
    }

    fn probe() -> Arc<Probe> {
        Arc::new(Probe { running: AtomicUsize::new(0), max: AtomicUsize::new(0), started: Mutex::new(vec![]), release: AtomicBool::new(false) })
    }

    fn runner(db: Db, max: usize, p: Arc<Probe>) -> (JobRunner, Arc<Mutex<Vec<JobInfo>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let r = JobRunner::new(db, max, blocking_worker(p), Arc::new(move |i: &JobInfo| lock(&ev).push(i.clone())));
        (r, events)
    }

    fn state_of(r: &JobRunner, id: JobId) -> JobState {
        r.get(id).unwrap().state
    }

    fn db_state(db: &Db, id: JobId) -> Option<String> {
        db.with(|c| {
            let mut st = c.prepare("SELECT state FROM jobs WHERE id = ?1")?;
            let mut rows = st.query([id as i64])?;
            Ok(match rows.next()? {
                Some(row) => Some(row.get(0)?),
                None => None,
            })
        })
        .unwrap()
    }

    #[test]
    fn never_more_than_three_running_with_six_submitted() {
        let db = Db::open_in_memory().unwrap();
        let p = probe();
        let (r, _) = runner(db.clone(), 3, p.clone());
        let ids: Vec<JobId> = (0..6).map(|i| r.submit(JobKind::Fetch { repo_id: i }, format!("job {i}")).unwrap()).collect();
        wait_until("3 running", || p.running.load(Ordering::SeqCst) == 3);
        std::thread::sleep(Duration::from_millis(150)); // give extra threads a chance to (wrongly) start
        let states: Vec<JobState> = ids.iter().map(|&id| state_of(&r, id)).collect();
        assert_eq!(states.iter().filter(|s| **s == JobState::Running).count(), 3, "{states:?}");
        assert_eq!(states.iter().filter(|s| **s == JobState::Waiting).count(), 3, "{states:?}");
        assert_eq!(r.running(), 3);
        p.release.store(true, Ordering::SeqCst);
        wait_until("all done", || ids.iter().all(|&id| state_of(&r, id) == JobState::Done));
        r.join();
        assert_eq!(p.max.load(Ordering::SeqCst), 3);
        for id in ids {
            assert_eq!(db_state(&db, id).as_deref(), Some("done"));
        }
    }

    #[test]
    fn waiting_job_cancel_never_runs() {
        let db = Db::open_in_memory().unwrap();
        let p = probe();
        let (r, events) = runner(db.clone(), 1, p.clone());
        let a = r.submit(JobKind::Clone { url: "https://example.com/a/b.git".into() }, "a".into()).unwrap();
        wait_until("a running", || state_of(&r, a) == JobState::Running);
        let b = r.submit(JobKind::FetchAll, "b".into()).unwrap();
        assert_eq!(state_of(&r, b), JobState::Waiting);
        assert!(r.cancel(b));
        wait_until("b cancelled", || state_of(&r, b) == JobState::Cancelled);
        assert_eq!(db_state(&db, b).as_deref(), Some("cancelled"));
        // the running job is cancellable too
        assert!(r.cancel(a));
        wait_until("a cancelled", || state_of(&r, a) == JobState::Cancelled);
        r.join();
        assert_eq!(*lock(&p.started), vec![JobKind::Clone { url: "https://example.com/a/b.git".into() }], "b must never start");
        assert!(!r.cancel(b), "already finished");
        assert!(!r.cancel(999));
        // every transition reached the sink; b went Waiting -> Cancelled only
        let b_states: Vec<JobState> = lock(&events).iter().filter(|i| i.id == b).map(|i| i.state).collect();
        assert_eq!(b_states, vec![JobState::Waiting, JobState::Cancelled]);
    }

    #[test]
    fn throttle_drops_events_under_80ms() {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let ctx = JobCtx::new(Arc::new(AtomicBool::new(false)), move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        let start = Instant::now();
        for i in 0..2_000 {
            ctx.progress(Progress::Receiving { received: i, total: 2_000, bytes: i * 10 });
        }
        let elapsed = start.elapsed().as_millis() as usize;
        let n = count.load(Ordering::SeqCst);
        assert!(n >= 1, "first event must pass");
        assert!(n <= elapsed / 80 + 1, "{n} events in {elapsed} ms");
        // phase change and Finalizing always pass, even right away
        let before = count.load(Ordering::SeqCst);
        ctx.progress(Progress::Resolving { done: 0, total: 1 });
        ctx.progress(Progress::Finalizing);
        ctx.progress(Progress::Finalizing);
        assert_eq!(count.load(Ordering::SeqCst), before + 3);
        // same phase again within 80 ms is dropped, after 80 ms it passes
        ctx.progress(Progress::Resolving { done: 1, total: 1 });
        ctx.progress(Progress::Resolving { done: 1, total: 1 });
        let mid = count.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(90));
        ctx.progress(Progress::Resolving { done: 1, total: 1 });
        assert_eq!(count.load(Ordering::SeqCst), mid + 1);
    }

    #[test]
    fn progress_reaches_sink_while_running() {
        let db = Db::open_in_memory().unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let worker: Worker = Arc::new(|_: &JobKind, ctx: &JobCtx| {
            ctx.progress(Progress::Connecting);
            ctx.progress(Progress::Receiving { received: 1, total: 2, bytes: 3 });
            Ok(fetched())
        });
        let r = JobRunner::new(db, 3, worker, Arc::new(move |i: &JobInfo| lock(&ev).push(i.clone())));
        let id = r.submit(JobKind::FetchAll, "all".into()).unwrap();
        wait_until("done", || state_of(&r, id) == JobState::Done);
        r.join();
        let ev = lock(&events);
        assert!(ev.iter().any(|i| i.progress == Some(Progress::Receiving { received: 1, total: 2, bytes: 3 })));
        let last = ev.last().unwrap();
        assert_eq!(last.state, JobState::Done);
        assert_eq!(last.progress, None);
        assert_eq!(last.result, Some(fetched()));
    }

    #[test]
    fn retry_creates_new_row_and_deletes_old() {
        let db = Db::open_in_memory().unwrap();
        let p = probe();
        let (r, _) = runner(db.clone(), 3, p.clone());
        let old = r.submit(JobKind::Fetch { repo_id: -1 }, "fails".into()).unwrap();
        wait_until("failed", || state_of(&r, old) == JobState::Failed);
        assert_eq!(r.get(old).unwrap().error.as_deref(), Some("boom"));
        assert_eq!(db_state(&db, old).as_deref(), Some("failed"));
        let new = r.retry(old).unwrap();
        assert_ne!(new, old);
        assert!(r.get(old).is_none());
        assert_eq!(db_state(&db, old), None, "old row deleted");
        assert_eq!(r.get(new).unwrap().kind, JobKind::Fetch { repo_id: -1 });
        assert_eq!(r.get(new).unwrap().title, "fails");
        wait_until("failed again", || state_of(&r, new) == JobState::Failed);
        // a done job cannot be retried
        p.release.store(true, Ordering::SeqCst);
        let ok = r.submit(JobKind::FetchAll, "ok".into()).unwrap();
        wait_until("done", || state_of(&r, ok) == JobState::Done);
        assert!(matches!(r.retry(ok), Err(Error::Refused(_))));
        assert!(matches!(r.retry(12345), Err(Error::NotFound { .. })));
        // dismiss removes row and entry
        r.dismiss(ok);
        assert!(r.get(ok).is_none());
        assert_eq!(db_state(&db, ok), None);
        r.join();
    }

    #[test]
    fn restart_marks_interrupted_rows_failed() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        {
            let db = Db::open(&f).unwrap();
            db.with(|c| {
                c.execute_batch(
                    r#"INSERT INTO jobs (id, kind_json, title, state, created_at) VALUES
                         (1, '{"type":"clone","url":"https://github.com/o/r.git"}', 'Clone o/r', 'running', 10),
                         (2, '{"type":"fetch","repoId":7}', 'Fetch x', 'waiting', 11),
                         (3, '{"type":"fetchAll"}', 'Fetch all', 'done', 12),
                         (4, '{"type":"fromTheFuture"}', 'odd', 'failed', 13);"#,
                )?;
                Ok(())
            })
            .unwrap();
        }
        let db = Db::open(&f).unwrap();
        let p = probe();
        let (r, _) = runner(db.clone(), 3, p.clone());
        r.load_persisted().unwrap();
        let list = r.list();
        assert_eq!(list.iter().map(|i| i.id).collect::<Vec<_>>(), vec![1, 2, 3]);
        for id in [1, 2] {
            let i = r.get(id).unwrap();
            assert_eq!(i.state, JobState::Failed);
            assert_eq!(i.error.as_deref(), Some(INTERRUPTED));
            assert_eq!(db_state(&db, id).as_deref(), Some("failed"));
        }
        assert_eq!(r.get(1).unwrap().kind, JobKind::Clone { url: "https://github.com/o/r.git".into() });
        assert_eq!(r.get(2).unwrap().started_at, 11);
        assert_eq!(state_of(&r, 3), JobState::Done);
        // loading twice does not duplicate
        r.load_persisted().unwrap();
        assert_eq!(r.list().len(), 3);
        // Retry works after a restart
        p.release.store(true, Ordering::SeqCst);
        let new = r.retry(2).unwrap();
        wait_until("retried done", || state_of(&r, new) == JobState::Done);
        assert_eq!(*lock(&p.started), vec![JobKind::Fetch { repo_id: 7 }]);
        r.join();
    }

    #[test]
    fn panicking_worker_fails_the_job() {
        let db = Db::open_in_memory().unwrap();
        let worker: Worker = Arc::new(|_: &JobKind, _: &JobCtx| panic!("worker bug"));
        let r = JobRunner::new(db, 1, worker, Arc::new(|_: &JobInfo| {}));
        let id = r.submit(JobKind::FetchAll, "x".into()).unwrap();
        wait_until("failed", || state_of(&r, id) == JobState::Failed);
        r.join();
        // the slot was released
        assert_eq!(r.running(), 0);
    }

    #[test]
    fn compute_gate_caps_at_two() {
        let gate = ComputeGate::new();
        let max = Arc::new(AtomicUsize::new(0));
        let cur = Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..6)
            .map(|_| {
                let (gate, max, cur) = (gate.clone(), max.clone(), cur.clone());
                std::thread::spawn(move || {
                    let _permit = gate.acquire(&CancelToken::new()).unwrap();
                    let n = cur.fetch_add(1, Ordering::SeqCst) + 1;
                    max.fetch_max(n, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(30));
                    cur.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(max.load(Ordering::SeqCst), COMPUTE_SLOTS);
        assert_eq!(gate.in_use(), 0);
    }

    #[test]
    fn compute_gate_waiter_can_be_cancelled() {
        let gate = ComputeGate::new();
        let a = gate.acquire(&CancelToken::new()).unwrap();
        let b = gate.acquire(&CancelToken::new()).unwrap();
        let tok = CancelToken::new();
        let t2 = tok.clone();
        let g2 = gate.clone();
        let h = std::thread::spawn(move || g2.acquire(&t2).map(|_| ()));
        std::thread::sleep(Duration::from_millis(50));
        tok.cancel();
        assert!(matches!(h.join().unwrap(), Err(Error::Cancelled)));
        assert!(matches!(tok.check(), Err(Error::Cancelled)));
        drop((a, b));
        assert_eq!(gate.in_use(), 0);
    }

    #[test]
    fn job_types_wire_shape() {
        use serde_json::json;
        assert_eq!(serde_json::to_value(JobKind::Fetch { repo_id: 3 }).unwrap(), json!({"type": "fetch", "repoId": 3}));
        assert_eq!(serde_json::to_value(JobKind::FetchAll).unwrap(), json!({"type": "fetchAll"}));
        assert_eq!(serde_json::to_value(JobState::Cancelled).unwrap(), json!("cancelled"));
        assert_eq!(
            serde_json::to_value(Progress::Receiving { received: 1, total: 2, bytes: 3 }).unwrap(),
            json!({"type": "receiving", "received": 1, "total": 2, "bytes": 3})
        );
        let info = JobInfo {
            id: 1,
            kind: JobKind::Clone { url: "u".into() },
            state: JobState::Running,
            progress: Some(Progress::Finalizing),
            title: "t".into(),
            result: None,
            error: None,
            started_at: 5,
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["startedAt"], json!(5));
        assert_eq!(v["progress"], json!({"type": "finalizing"}));
    }
}
