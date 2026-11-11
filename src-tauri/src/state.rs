//! App state held by every command (M4.1, ARCH §5.5): the shelf, the job runner and its channel subscribers,
//! request-scoped cancellation, the compute gate, import tokens (D-18) and the OS seams (trash D-30, dialogs,
//! opener) behind traits so command tests never touch the Recycle Bin, open a dialog or launch a browser.

use crate::ipc_error::friendly;
use rs_core::Shelf;
use rs_core::error::{Error, Result};
use rs_core::jobs::{CancelToken, ComputeGate, JobCtx, JobInfo, JobKind, JobRunner, Worker};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

/// Clone/fetch jobs running at once (ARCH §5.5).
pub const MAX_JOBS: usize = 3;
/// D-18: an import preview token is valid this long.
pub const IMPORT_TOKEN_TTL: Duration = Duration::from_secs(10 * 60);
/// A `request_cancel` that arrives before its request registered is remembered this long.
const EARLY_CANCEL_TTL: Duration = Duration::from_secs(30);
const MAX_EARLY_CANCELS: usize = 256;
const MAX_IMPORT_TOKENS: usize = 32;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------------------------------------------
// OS seams

/// Recycle Bin (D-30). `RealTrash` in the app, a recording fake in tests.
pub trait Trasher: Send + Sync {
    fn delete(&self, path: &Path) -> std::result::Result<(), String>;
}

pub struct RealTrash;

impl Trasher for RealTrash {
    fn delete(&self, path: &Path) -> std::result::Result<(), String> {
        trash::delete(path).map_err(|e| e.to_string())
    }
}

/// Native dialogs, opened from Rust (D-10). `None` = the user cancelled.
pub trait Picker: Send + Sync {
    fn pick_folder(&self, title: &str) -> Option<PathBuf>;
    fn save_file(&self, title: &str, file_name: &str) -> Option<PathBuf>;
}

/// System browser / Explorer, from Rust (D-10, NFR-S3).
pub trait Opener: Send + Sync {
    fn open_url(&self, url: &str) -> std::result::Result<(), String>;
    fn open_folder(&self, path: &Path) -> std::result::Result<(), String>;
    /// Selects `path` in its parent folder (bare clones: reveal `<name>.git`).
    fn reveal(&self, path: &Path) -> std::result::Result<(), String>;
}

/// Tauri-backed picker and opener (only constructed inside the running app).
pub struct TauriShell<R: tauri::Runtime> {
    pub app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> Picker for TauriShell<R> {
    fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        use tauri::Manager;
        use tauri_plugin_dialog::DialogExt;
        let mut d = self.app.dialog().file().set_title(title);
        if let Some(w) = self.app.get_webview_window(crate::MAIN_WINDOW) {
            d = d.set_parent(&w);
        }
        d.blocking_pick_folder().and_then(|p| p.into_path().ok())
    }
    fn save_file(&self, title: &str, file_name: &str) -> Option<PathBuf> {
        use tauri::Manager;
        use tauri_plugin_dialog::DialogExt;
        let mut d = self.app.dialog().file().set_title(title).set_file_name(file_name);
        if let Some(w) = self.app.get_webview_window(crate::MAIN_WINDOW) {
            d = d.set_parent(&w);
        }
        d.blocking_save_file().and_then(|p| p.into_path().ok())
    }
}

impl<R: tauri::Runtime> Opener for TauriShell<R> {
    fn open_url(&self, url: &str) -> std::result::Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        self.app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }
    fn open_folder(&self, path: &Path) -> std::result::Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        self.app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
    }
    fn reveal(&self, path: &Path) -> std::result::Result<(), String> {
        use tauri_plugin_opener::OpenerExt;
        self.app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Request-scoped cancellation

#[derive(Default)]
struct Requests {
    live: HashMap<String, CancelToken>,
    /// Cancels that arrived before the request registered (unmount races the invoke).
    early: Vec<(String, Instant)>,
}

/// `requestId` → `CancelToken` for the duration of one command (ARCH §5.5).
#[derive(Default)]
pub struct CancelRegistry {
    inner: Arc<Mutex<Requests>>,
}

/// Unregisters its request on drop.
pub struct RequestGuard {
    id: String,
    token: CancelToken,
    inner: Arc<Mutex<Requests>>,
}

impl RequestGuard {
    pub fn token(&self) -> &CancelToken {
        &self.token
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let mut r = lock(&self.inner);
        if r.live.get(&self.id).is_some_and(|t| Arc::ptr_eq(t.flag(), self.token.flag())) {
            r.live.remove(&self.id);
        }
    }
}

/// `requestId`: 1–64 chars of `[A-Za-z0-9_-]` (the UI sends uuid-ish strings).
pub fn validate_request_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err(Error::InvalidInput("invalid requestId".into()));
    }
    Ok(())
}

impl CancelRegistry {
    pub fn register(&self, id: &str) -> Result<RequestGuard> {
        validate_request_id(id)?;
        let token = CancelToken::new();
        let mut r = lock(&self.inner);
        let now = Instant::now();
        r.early.retain(|(_, at)| now.duration_since(*at) < EARLY_CANCEL_TTL);
        if let Some(pos) = r.early.iter().position(|(e, _)| e == id) {
            r.early.remove(pos);
            token.cancel();
        }
        r.live.insert(id.to_string(), token.clone());
        Ok(RequestGuard { id: id.to_string(), token, inner: self.inner.clone() })
    }

    /// `true` if a live request was cancelled; an unknown id is remembered briefly (it may still arrive).
    pub fn cancel(&self, id: &str) -> Result<bool> {
        validate_request_id(id)?;
        let mut r = lock(&self.inner);
        if let Some(t) = r.live.get(id) {
            t.cancel();
            return Ok(true);
        }
        if r.early.len() >= MAX_EARLY_CANCELS {
            r.early.remove(0);
        }
        r.early.push((id.to_string(), Instant::now()));
        Ok(false)
    }

    pub fn live(&self) -> usize {
        lock(&self.inner).live.len()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Job progress subscribers

/// Live `jobs_subscribe` channels, one per webview label (a reload re-subscribes and replaces its old channel).
#[derive(Default)]
pub struct Subscribers {
    list: Mutex<Vec<(String, Channel<JobInfo>)>>,
}

impl Subscribers {
    pub fn add(&self, key: &str, ch: Channel<JobInfo>) {
        let mut l = lock(&self.list);
        l.retain(|(k, _)| k != key);
        l.push((key.to_string(), ch));
    }
    /// Pushes to every channel; channels whose send fails (webview gone) are dropped.
    pub fn push(&self, info: &JobInfo) {
        let chans: Vec<(String, Channel<JobInfo>)> = lock(&self.list).clone();
        let mut dead = Vec::new();
        for (_, ch) in &chans {
            if ch.send(info.clone()).is_err() {
                dead.push(ch.id());
            }
        }
        if !dead.is_empty() {
            lock(&self.list).retain(|(_, c)| !dead.contains(&c.id()));
        }
    }
    pub fn len(&self) -> usize {
        lock(&self.list).len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Worker

/// Host named in friendly job errors.
fn job_host(shelf: &Shelf, kind: &JobKind) -> Option<String> {
    match kind {
        JobKind::Clone { url } => rs_core::validate::clone_url(url).ok().map(|u| u.host),
        JobKind::Fetch { repo_id } => rs_core::repo::summary(shelf, *repo_id).ok().and_then(|s| s.origin_host),
        JobKind::FetchAll => None,
    }
}

/// Wraps an rs-core worker so failed jobs carry the friendly IPC wording (credentials, 404, "Could not reach
/// <host>", insteadOf): the runner stores `Error::to_string()`, so the text is re-wrapped as `Refused(text)`.
/// `Cancelled` and non-network errors pass through unchanged.
pub fn friendly_worker(shelf: Arc<Shelf>, base: Worker) -> Worker {
    Arc::new(move |kind: &JobKind, ctx: &JobCtx| match base(kind, ctx) {
        Err(e @ (Error::Git(_) | Error::Network(_) | Error::Refused(_))) => {
            Err(Error::Refused(friendly(&e, job_host(&shelf, kind).as_deref())))
        }
        other => other,
    })
}

/// Production worker, or (e2e build only, `test_file_clone`) the file:// seam (D-14, D-20).
pub fn make_worker(shelf: Arc<Shelf>, test_file_clone: bool) -> Worker {
    #[cfg(feature = "e2e")]
    if test_file_clone {
        return friendly_worker(shelf.clone(), rs_core::jobs::shelf_worker_unchecked(shelf));
    }
    let _ = test_file_clone;
    friendly_worker(shelf.clone(), rs_core::jobs::shelf_worker(shelf))
}

// ---------------------------------------------------------------------------------------------------------------
// AppState

pub struct AppState {
    pub shelf: Arc<Shelf>,
    pub jobs: JobRunner,
    pub requests: CancelRegistry,
    pub compute: ComputeGate,
    pub subscribers: Arc<Subscribers>,
    pub import_tokens: Mutex<HashMap<String, (PathBuf, Instant)>>,
    pub trasher: Arc<dyn Trasher>,
    pub picker: Arc<dyn Picker>,
    pub opener: Arc<dyn Opener>,
    /// e2e build + `RS_TEST_FILE_CLONE=1` (read once in `run()`); always `false` in shipped builds.
    pub test_file_clone: bool,
    /// Cancels the startup summary refresh on exit.
    pub startup_refresh: CancelToken,
}

impl AppState {
    pub fn new(
        shelf: Arc<Shelf>,
        test_file_clone: bool,
        picker: Arc<dyn Picker>,
        trasher: Arc<dyn Trasher>,
        opener: Arc<dyn Opener>,
    ) -> AppState {
        let worker = make_worker(shelf.clone(), test_file_clone);
        AppState::with_worker(shelf, test_file_clone, worker, picker, trasher, opener)
    }

    /// `new` with an explicit job worker (tests use a fake so no job touches the network).
    pub fn with_worker(
        shelf: Arc<Shelf>,
        test_file_clone: bool,
        worker: Worker,
        picker: Arc<dyn Picker>,
        trasher: Arc<dyn Trasher>,
        opener: Arc<dyn Opener>,
    ) -> AppState {
        let subscribers = Arc::new(Subscribers::default());
        let subs = subscribers.clone();
        let jobs = JobRunner::new(shelf.db.clone(), MAX_JOBS, worker, Arc::new(move |i: &JobInfo| subs.push(i)));
        AppState {
            shelf,
            jobs,
            requests: CancelRegistry::default(),
            compute: ComputeGate::new(),
            subscribers,
            import_tokens: Mutex::new(HashMap::new()),
            trasher,
            picker,
            opener,
            test_file_clone,
            startup_refresh: CancelToken::new(),
        }
    }

    /// D-18: remembers the dialog's folder under a fresh opaque token.
    pub fn issue_import_token(&self, path: PathBuf) -> String {
        let token = new_token();
        let mut t = lock(&self.import_tokens);
        let now = Instant::now();
        t.retain(|_, (_, at)| now.duration_since(*at) < IMPORT_TOKEN_TTL);
        if t.len() >= MAX_IMPORT_TOKENS
            && let Some(oldest) = t.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| k.clone())
        {
            t.remove(&oldest);
        }
        t.insert(token.clone(), (path, now));
        token
    }

    /// One-shot: the path for `token`, or `InvalidInput` when unknown or expired.
    pub fn take_import_token(&self, token: &str) -> Result<PathBuf> {
        let entry = lock(&self.import_tokens).remove(token);
        match entry {
            Some((p, at)) if at.elapsed() < IMPORT_TOKEN_TTL => Ok(p),
            Some(_) => Err(Error::InvalidInput("The folder selection expired. Choose the folder again.".into())),
            None => Err(Error::InvalidInput("Unknown folder selection. Choose the folder again.".into())),
        }
    }
}

/// 128-bit opaque token from std's randomly keyed hasher (no extra dependency; not a secret beyond this process).
fn new_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let part = |salt: u64| {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u64(seq);
        h.write_u128(nanos);
        h.finish()
    };
    format!("{:016x}{:016x}", part(1), part(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_registry_register_cancel_unregister() {
        let r = CancelRegistry::default();
        let g = r.register("req-1").unwrap();
        assert!(!g.token().is_cancelled());
        assert!(r.cancel("req-1").unwrap());
        assert!(g.token().is_cancelled());
        drop(g);
        assert_eq!(r.live(), 0);
        // cancel before register: remembered, the request starts cancelled
        assert!(!r.cancel("req-2").unwrap());
        assert!(r.register("req-2").unwrap().token().is_cancelled());
        assert!(!r.register("req-2").unwrap().token().is_cancelled(), "early cancel is consumed once");
        for bad in ["", "a b", "x/y", &"z".repeat(65)] {
            assert_eq!(r.register(bad).err().unwrap().code(), "invalidInput");
            assert_eq!(r.cancel(bad).unwrap_err().code(), "invalidInput");
        }
    }

    #[test]
    fn re_registering_an_id_keeps_the_newer_guard() {
        let r = CancelRegistry::default();
        let a = r.register("same").unwrap();
        let b = r.register("same").unwrap();
        drop(a);
        assert_eq!(r.live(), 1);
        assert!(r.cancel("same").unwrap());
        assert!(b.token().is_cancelled());
    }

    #[test]
    fn tokens_are_unique_hex() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn subscribers_drop_dead_channels_and_replace_per_key() {
        let s = Subscribers::default();
        let got = Arc::new(Mutex::new(0));
        let g = got.clone();
        s.add("main", Channel::new(move |_| {
            *g.lock().unwrap() += 1;
            Ok(())
        }));
        s.add("dead", Channel::new(|_| Err(tauri::Error::WebviewNotFound)));
        assert_eq!(s.len(), 2);
        let info = JobInfo {
            id: 1,
            kind: JobKind::FetchAll,
            state: rs_core::jobs::JobState::Waiting,
            progress: None,
            title: "t".into(),
            result: None,
            error: None,
            started_at: 0,
        };
        s.push(&info);
        assert_eq!(s.len(), 1);
        assert_eq!(*got.lock().unwrap(), 1);
        s.add("main", Channel::new(|_| Ok(())));
        assert_eq!(s.len(), 1);
    }
}
