//! Bare managed clones (M3.3; ARCH §5.4; FR-6..9, NFR-W1, R-4, R-6, D-2, D-14, D-44).
//!
//! Also home of the single place that connects a git2 remote (`advertised`, `download`, shared with `fetch.rs`)
//! and of the per-thread network-call counter behind `testkit::net_calls()` (NFR-O1).

use crate::Shelf;
use crate::error::{Error, Result};
use crate::jobs::{JobCtx, Progress};
use crate::paths;
use crate::refs::{HEADS, ORIGIN, TAGS};
use crate::types::{ClonePreview, JobResult};
use crate::validate::{self, CloneUrl};
use crate::winpath;
use git2::{AutotagOption, Direction, FetchOptions, FetchPrune, ProxyOptions, Remote, RemoteCallbacks, RemoteRedirect, Repository};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------------------------------------------
// Network boundary

thread_local! {
    static NET_CALLS: Cell<u64> = const { Cell::new(0) };
}

/// Called right before every network connection attempt (git2 connect/fetch here, the GitHub GET in ghreleases).
pub(crate) fn note_net_call() {
    NET_CALLS.with(|c| c.set(c.get() + 1));
}

/// Connection attempts made on the calling thread (see `testkit::net_calls`).
pub fn net_calls_on_this_thread() -> u64 {
    NET_CALLS.with(Cell::get)
}

/// What the server advertised (short names) and its default branch.
pub(crate) struct Advertised {
    pub heads: Vec<String>,
    pub tags: Vec<String>,
    pub default: Option<String>,
}

/// D-44: git config can rewrite a remote URL (`url.<base>.insteadOf`, repo-local or user-global) to another
/// scheme or host. The effective URL must be exactly the one we validated, else the job fails before connecting.
pub(crate) fn check_effective_url(remote: &Remote<'_>, expected: &str) -> Result<()> {
    let got = String::from_utf8_lossy(remote.url_bytes());
    if got != expected {
        return Err(Error::Refused(format!(
            "Git configuration rewrites {expected} to {got} (url.*.insteadOf). RepoShelf only connects to the address itself."
        )));
    }
    Ok(())
}

/// Progress → `ctx` (Resolving only once every object is received, D-35 note), cancel → abort. No credentials
/// callback is ever registered (D-1): a server asking for credentials fails the job.
fn callbacks(ctx: &JobCtx) -> RemoteCallbacks<'_> {
    let mut cb = RemoteCallbacks::new();
    cb.transfer_progress(move |p| {
        if p.total_deltas() > 0 && p.received_objects() >= p.total_objects() {
            ctx.progress(Progress::Resolving { done: p.indexed_deltas(), total: p.total_deltas() });
        } else {
            ctx.progress(Progress::Receiving { received: p.received_objects(), total: p.total_objects(), bytes: p.received_bytes() });
        }
        !ctx.cancelled()
    });
    cb.sideband_progress(move |_| !ctx.cancelled());
    cb
}

fn proxy() -> ProxyOptions<'static> {
    let mut p = ProxyOptions::new();
    p.auto();
    p
}

fn net_err(e: git2::Error, ctx: &JobCtx) -> Error {
    if ctx.cancelled() { Error::Cancelled } else { Error::Git(e) }
}

/// Connects once and lists advertised heads/tags. Default branch: HEAD's symref target, else
/// `git_remote_default_branch`, else the branch whose tip equals HEAD (main/master first), else main/master.
pub(crate) fn advertised(remote: &mut Remote<'_>, ctx: &JobCtx) -> Result<Advertised> {
    ctx.check()?;
    ctx.progress(Progress::Connecting);
    note_net_call();
    let conn = remote.connect_auth(Direction::Fetch, Some(callbacks(ctx)), Some(proxy())).map_err(|e| net_err(e, ctx))?;
    let mut heads: Vec<(String, git2::Oid)> = Vec::new();
    let mut tags = Vec::new();
    let mut head_sym = None;
    let mut head_oid = None;
    for h in conn.list().map_err(|e| net_err(e, ctx))? {
        let n = h.name();
        if n == "HEAD" {
            head_sym = h.symref_target().and_then(|t| t.strip_prefix(HEADS)).map(str::to_string);
            head_oid = Some(h.oid());
        } else if let Some(b) = n.strip_prefix(HEADS) {
            heads.push((b.to_string(), h.oid()));
        } else if let Some(t) = n.strip_prefix(TAGS)
            && !t.ends_with("^{}")
        {
            tags.push(t.to_string());
        }
    }
    let lib_default = conn.default_branch().ok().and_then(|b| b.as_str().ok().and_then(|s| s.strip_prefix(HEADS)).map(str::to_string));
    drop(conn);
    let has = |n: &str| heads.iter().any(|(h, _)| h == n);
    let preferred = ["main", "master"];
    let default = head_sym
        .filter(|d| has(d))
        .or(lib_default.filter(|d| has(d)))
        .or_else(|| {
            let oid = head_oid?;
            let mut at: Vec<&String> = heads.iter().filter(|(_, o)| *o == oid).map(|(n, _)| n).collect();
            at.sort_by_key(|n| preferred.iter().position(|p| p == n).unwrap_or(preferred.len()));
            at.first().map(|n| n.to_string())
        })
        .or_else(|| preferred.iter().find(|p| has(p)).map(|p| p.to_string()));
    Ok(Advertised { heads: heads.into_iter().map(|(n, _)| n).collect(), tags, default })
}

/// Fetches exactly `specs` (never the configured refspecs: nothing is downloaded when `specs` is empty).
/// No auto-follow of tags, no prune, redirects only on the initial request (D-44).
pub(crate) fn download(remote: &mut Remote<'_>, specs: &[String], ctx: &JobCtx) -> Result<()> {
    ctx.check()?;
    if specs.is_empty() {
        return Ok(());
    }
    note_net_call();
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks(ctx))
        .proxy_options(proxy())
        .follow_redirects(RemoteRedirect::Initial)
        .download_tags(AutotagOption::None)
        .prune(FetchPrune::Off)
        .update_fetchhead(true);
    remote.fetch(specs, Some(&mut fo), Some("reposhelf: fetch")).map_err(|e| net_err(e, ctx))
}

// D-45: libgit2 (Windows) refuses a git dir whose path + the longest internal suffix (`objects/pack/pack-
// <64 hex>.pack.lock`, 92 chars) exceeds MAX_PATH, and any loose ref/reflog path over MAX_PATH, whatever
// `core.longpaths` says (repository.c `validate_repo_path`, refdb_fs.c `loose_path`). The first is refused before
// anything is created; refs whose loose file would be too long are skipped and reported (FR-9). (The local
// `file://` transport additionally DWIM-looks-up every advertised name in the receiving repo — local.c
// `local_negotiate_fetch` — so there a very long name fails the whole test clone; https never does that lookup.)
const MAX_PATH: usize = 260;
const REPO_SUFFIX: usize = 92;

fn git_dir_chars(git_dir: &Path) -> usize {
    paths::strip_verbatim(git_dir).to_string_lossy().chars().count() + 1
}

/// Can libgit2 open a repository at `git_dir` on this platform?
pub(crate) fn repo_path_ok(git_dir: &Path) -> bool {
    !cfg!(windows) || git_dir_chars(git_dir) + REPO_SUFFIX <= MAX_PATH
}

/// Moves names whose loose ref (`<git dir>/<prefix><name>`) or reflog (`logs/…`) plus `.lock` would exceed MAX_PATH
/// from `keep` to `skipped` (Windows only).
pub(crate) fn drop_long_refs(git_dir: &Path, prefix: &str, keep: Vec<String>, skipped: &mut Vec<String>) -> Vec<String> {
    if !cfg!(windows) {
        return keep;
    }
    long_ref_split(git_dir_chars(git_dir), prefix, keep, skipped)
}

fn long_ref_split(git_dir_chars: usize, prefix: &str, keep: Vec<String>, skipped: &mut Vec<String>) -> Vec<String> {
    let base = git_dir_chars + "logs/".len() + prefix.chars().count() + ".lock".len();
    let (ok, long): (Vec<String>, Vec<String>) = keep.into_iter().partition(|n| base + n.chars().count() <= MAX_PATH);
    skipped.extend(long);
    ok
}

pub(crate) fn ignore_case(repo: &Repository) -> bool {
    repo.config().and_then(|c| c.get_bool("core.ignorecase")).unwrap_or(cfg!(windows))
}

// ---------------------------------------------------------------------------------------------------------------
// Filesystem helpers

/// Marker inside partial clone folder names: `.<name>.git.partial-<ms>-<seq>`.
pub const PARTIAL_MARK: &str = ".partial-";

static FINALIZE: Mutex<()> = Mutex::new(());
static SEQ: AtomicU64 = AtomicU64::new(0);

fn clear_readonly(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => clear_readonly(&p),
            Ok(_) => {
                if let Ok(md) = std::fs::metadata(&p) {
                    let mut perm = md.permissions();
                    if perm.readonly() {
                        #[allow(clippy::permissions_set_readonly_false)]
                        perm.set_readonly(false);
                        let _ = std::fs::set_permissions(&p, perm);
                    }
                }
            }
            Err(_) => {}
        }
    }
}

/// `remove_dir_all` that also handles read-only git object/pack files (Windows) and deep paths (`\\?\`).
pub(crate) fn remove_tree(p: &Path) -> std::io::Result<()> {
    let lp = paths::long(p);
    if std::fs::symlink_metadata(&lp).is_err() {
        return Ok(());
    }
    match std::fs::remove_dir_all(&lp) {
        Ok(()) => Ok(()),
        Err(_) => {
            clear_readonly(&lp);
            std::fs::remove_dir_all(&lp)
        }
    }
}

fn now_millis() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// `<parent>/<stem>.git`, else `<stem>-2.git`, `-3`… (first free name).
fn free_target(parent: &Path, stem: &str) -> PathBuf {
    let mut n = 1u32;
    loop {
        let name = if n == 1 { format!("{stem}.git") } else { format!("{stem}-{n}.git") };
        let t = parent.join(name);
        if std::fs::symlink_metadata(paths::long(&t)).is_err() {
            return t;
        }
        n += 1;
    }
}

/// Removes leftover `.x.git.partial-*` folders (`<archive>/<host>/<owner>/`) from a killed process (FR-8).
pub fn cleanup_partials(archive: &Path) -> Result<u32> {
    let mut n = 0;
    let Ok(hosts) = std::fs::read_dir(paths::long(archive)) else { return Ok(0) };
    for h in hosts.flatten() {
        // the release store (D-104) has its own cleanup (`assets::cleanup_partials`)
        if !h.file_type().is_ok_and(|t| t.is_dir()) || h.file_name() == crate::assets::STORE_DIR {
            continue;
        }
        let Ok(owners) = std::fs::read_dir(h.path()) else { continue };
        for o in owners.flatten() {
            if !o.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let Ok(items) = std::fs::read_dir(o.path()) else { continue };
            for it in items.flatten() {
                let name = it.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') && name.contains(&format!(".git{PARTIAL_MARK}")) && it.file_type().is_ok_and(|t| t.is_dir()) {
                    remove_tree(&it.path())?;
                    n += 1;
                }
            }
        }
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------------------------
// Clone

/// Validates the input (pure, no network) and shows where the clone would land (FR-6).
pub fn preview(s: &Shelf, input: &str) -> Result<ClonePreview> {
    let u = validate::clone_url(input)?;
    let (parent, stem) = split_rel(&u.rel_dir);
    let target = free_target(&s.dirs.archive.join(parent), &stem);
    Ok(ClonePreview { url: u.url, host: u.host, owner: u.owner, name: u.name, target_dir: target.display().to_string() })
}

fn split_rel(rel_dir: &Path) -> (PathBuf, String) {
    let parent = rel_dir.parent().map(Path::to_path_buf).unwrap_or_default();
    let file = rel_dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "repo.git".into());
    let stem = file.strip_suffix(".git").unwrap_or(&file).to_string();
    (parent, stem)
}

/// Bare clone of a validated public https URL into the archive (ARCH §5.4 steps 1–7). Returns the final folder and
/// the skipped ref names (full names, FR-9). Only accepts a `CloneUrl` (type-level guarantee, M3.4).
pub fn clone(s: &Shelf, url: &CloneUrl, ctx: &JobCtx) -> Result<(PathBuf, Vec<String>)> {
    clone_inner(s, &url.url, &url.rel_dir, ctx)
}

fn clone_inner(s: &Shelf, url: &str, rel_dir: &Path, ctx: &JobCtx) -> Result<(PathBuf, Vec<String>)> {
    ctx.check()?;
    let (rel_parent, stem) = split_rel(rel_dir);
    let parent = s.dirs.archive.join(rel_parent);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let partial = parent.join(format!(".{stem}.git{PARTIAL_MARK}{}-{seq}", now_millis()));
    if !repo_path_ok(&partial) {
        return Err(Error::Refused(format!(
            "The archive path {} is too long for git on Windows (limit about {} characters). Use a shorter archive folder.",
            parent.display(),
            MAX_PATH - REPO_SUFFIX - 40
        )));
    }
    std::fs::create_dir_all(paths::long(&parent))?;
    let res = clone_into(&partial, url, ctx).and_then(|skipped| {
        ctx.check()?;
        ctx.progress(Progress::Finalizing);
        let _g = FINALIZE.lock().unwrap_or_else(|e| e.into_inner());
        let target = free_target(&parent, &stem);
        std::fs::rename(paths::long(&partial), paths::long(&target))?;
        Ok((target, skipped))
    });
    if res.is_err() {
        let _ = remove_tree(&partial);
    }
    res
}

/// Steps 1–5 inside the partial folder. Every git2 handle is dropped before returning (the folder is renamed or
/// deleted next; open pack files would block that on Windows).
fn clone_into(partial: &Path, url: &str, ctx: &JobCtx) -> Result<Vec<String>> {
    let repo = Repository::init_bare(paths::long(partial))?;
    if cfg!(windows) {
        repo.config()?.set_bool("core.longpaths", true)?;
    }
    let ic = ignore_case(&repo);
    let mut remote = repo.remote("origin", url)?;
    check_effective_url(&remote, url)?;
    let adv = advertised(&mut remote, ctx)?;
    let Some(default) = adv.default.clone() else {
        return Err(Error::Refused("The repository is empty: it has no branches to archive.".into()));
    };
    let (hkeep, mut hskip) = winpath::storable_refs(&adv.heads, &[], Some(&default), ic);
    let hkeep = drop_long_refs(partial, ORIGIN, hkeep, &mut hskip);
    if !hkeep.contains(&default) {
        return Err(Error::Refused(format!(
            "The default branch {default:?} cannot be stored on Windows, so this repository cannot be archived."
        )));
    }
    let (tkeep, mut tskip) = winpath::storable_refs(&adv.tags, &[], None, ic);
    let tkeep = drop_long_refs(partial, TAGS, tkeep, &mut tskip);
    let mut specs = winpath::refspecs(HEADS, ORIGIN, &hkeep, &hskip);
    if !tkeep.is_empty() {
        specs.extend(winpath::refspecs(TAGS, TAGS, &tkeep, &tskip));
    }
    download(&mut remote, &specs, ctx)?;
    ctx.check()?;
    drop(remote);
    let tip = repo.refname_to_id(&format!("{ORIGIN}{default}"))?;
    repo.reference(&format!("{HEADS}{default}"), tip, true, "reposhelf: clone")?;
    repo.set_head(&format!("{HEADS}{default}"))?;
    repo.reference_symbolic(&format!("{ORIGIN}HEAD"), &format!("{ORIGIN}{default}"), true, "reposhelf: clone")?;
    drop(repo);
    Ok(hskip.iter().map(|n| format!("{HEADS}{n}")).chain(tskip.iter().map(|n| format!("{TAGS}{n}"))).collect())
}

/// Job entry for `JobKind::Clone`: validate (pure; no network on rejection, M3.4) → clone → register.
pub fn job(s: &Shelf, input: &str, ctx: &JobCtx) -> Result<JobResult> {
    let u = validate::clone_url(input)?;
    let (path, skipped) = clone(s, &u, ctx)?;
    register(s, &path, &u, skipped)
}

fn register(s: &Shelf, path: &Path, u: &CloneUrl, skipped: Vec<String>) -> Result<JobResult> {
    match crate::library::register_clone(s, path, u) {
        Ok(sum) => Ok(JobResult::Cloned { repo_id: sum.id, skipped_refs: skipped, notes: Vec::new() }),
        Err(e) => {
            let _ = remove_tree(path);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Test seam (D-14): local `file://` sources. Never compiled into shipped builds.

#[cfg(any(test, feature = "testkit"))]
fn unchecked_url(url: &str) -> CloneUrl {
    let clean = |s: &str| -> String {
        let t: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect();
        let t = t.trim_end_matches(['.', ' ']).to_string();
        if t.is_empty() || !winpath::path_ok_here(&t) { "repo".into() } else { t }
    };
    let segs: Vec<&str> = url.trim_end_matches('/').rsplit('/').take(2).collect();
    let name = clean(segs.first().copied().unwrap_or("repo").trim_end_matches(".git"));
    let owner = clean(segs.get(1).copied().unwrap_or("local"));
    let rel_dir: PathBuf = ["file", owner.as_str(), &format!("{name}.git")].iter().collect();
    CloneUrl { url: url.to_string(), host: "file".into(), owner, name, rel_dir }
}

/// Same clone as `clone`, from any URL (tests and the e2e build only; D-14).
#[cfg(any(test, feature = "testkit"))]
pub fn clone_from_unchecked(s: &Shelf, url: &str, ctx: &JobCtx) -> Result<(PathBuf, Vec<String>)> {
    clone_inner(s, url, &unchecked_url(url).rel_dir, ctx)
}

/// `job` for a `file://` source (tests and the e2e build only).
#[cfg(any(test, feature = "testkit"))]
pub fn job_unchecked(s: &Shelf, url: &str, ctx: &JobCtx) -> Result<JobResult> {
    let u = unchecked_url(url);
    let (path, skipped) = clone_inner(s, url, &u.rel_dir, ctx)?;
    register(s, &path, &u, skipped)
}

/// e2e git map (tests and the e2e build only, M13): clone the validated https `input` (where it lands, slug,
/// origin) but download from the local `source` (`file://` upstream), then point `origin` back at the https URL, so
/// the GitHub hook and a later (mapped) Fetch see a github.com repo while git never leaves the machine.
#[cfg(any(test, feature = "testkit"))]
pub fn job_mapped(s: &Shelf, input: &str, source: &str, ctx: &JobCtx) -> Result<JobResult> {
    let u = validate::clone_url(input)?;
    let (path, skipped) = clone_inner(s, source, &u.rel_dir, ctx)?;
    let set = Repository::open_bare(paths::long(&path)).and_then(|r| r.remote_set_url("origin", &u.url));
    if let Err(e) = set {
        let _ = remove_tree(&path);
        return Err(e.into());
    }
    register(s, &path, &u, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs;
    use crate::testkit::{self, T0};
    use crate::types::Source;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn refnames(path: &Path) -> Vec<String> {
        let r = testkit::open(path);
        let mut v: Vec<String> = r.references().unwrap().map(|x| x.unwrap().name().unwrap().to_string()).collect();
        v.sort();
        v
    }

    fn leftovers(archive: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![archive.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.contains(PARTIAL_MARK) {
                    out.push(n);
                } else if e.file_type().unwrap().is_dir() && !n.ends_with(".git") {
                    stack.push(e.path());
                }
            }
        }
        out
    }

    #[test]
    fn clone_is_bare_with_expected_refs_and_registers() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let (up, url) = testkit::upstream_downstream(&t.path().join("up").join("proj"));
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let ctx = JobCtx::new(Arc::new(AtomicBool::new(false)), move |p| ev.lock().unwrap().push(p.clone()));
        let res = job_unchecked(&s, &url, &ctx).unwrap();
        let JobResult::Cloned { repo_id, skipped_refs, .. } = res else { panic!("{res:?}") };
        assert!(skipped_refs.is_empty());
        let sum = crate::repo::summary(&s, repo_id).unwrap();
        assert_eq!(sum.source, Source::Cloned);
        assert_eq!((sum.owner.as_str(), sum.name.as_str()), ("up", "proj"));
        assert_eq!(sum.default_branch.as_deref(), Some("main"));
        assert!(sum.cloned_at.is_some());
        let path = PathBuf::from(&sum.path);
        assert!(path.ends_with(Path::new("file").join("up").join("proj.git")), "{}", path.display());
        let r = testkit::open(&path);
        assert!(r.is_bare());
        assert_eq!(r.head().unwrap().name().unwrap(), "refs/heads/main");
        assert_eq!(r.find_reference("refs/remotes/origin/HEAD").unwrap().symbolic_target().unwrap(), Some("refs/remotes/origin/main"));
        assert_eq!(r.config().unwrap().get_string("remote.origin.fetch").unwrap(), "+refs/heads/*:refs/remotes/origin/*");
        assert_eq!(r.config().unwrap().get_string("remote.origin.url").unwrap(), url);
        if cfg!(windows) {
            assert!(r.config().unwrap().get_bool("core.longpaths").unwrap());
        }
        assert_eq!(
            refnames(&path),
            ["refs/heads/main", "refs/remotes/origin/HEAD", "refs/remotes/origin/dev", "refs/remotes/origin/main", "refs/tags/light", "refs/tags/v1.0.0"]
        );
        assert_eq!(r.refname_to_id("refs/heads/main").unwrap(), up.head());
        assert!(r.find_reference("refs/tags/v1.0.0").unwrap().peel_to_tag().is_ok(), "annotated tag object fetched");
        assert!(leftovers(&s.dirs.archive).is_empty());
        let ev = events.lock().unwrap();
        assert_eq!(ev.first(), Some(&Progress::Connecting));
        assert_eq!(ev.last(), Some(&Progress::Finalizing));
        // no checkout ever: only git internals in the folder
        assert!(!path.join("README.md").exists());
    }

    #[test]
    fn refclash_and_invalid_refs_are_skipped_and_listed() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let up = testkit::refclash(&t.path().join("o").join("clash"));
        let (path, skipped) = clone_from_unchecked(&s, &testkit::file_url(&up), &JobCtx::noop()).unwrap();
        assert_eq!(skipped, ["refs/heads/llm", "refs/tags/a|b", "refs/tags/con"]);
        let names = refnames(&path);
        assert!(names.contains(&"refs/remotes/origin/LLM/x".to_string()) || names.contains(&"refs/remotes/origin/llm".to_string()));
        assert!(names.contains(&"refs/tags/v1.0".to_string()));
        assert!(!names.iter().any(|n| n.ends_with("/con") || n.contains('|')), "{names:?}");
    }

    #[test]
    fn winnames_clone_fine_because_bare() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let up = testkit::winnames(&t.path().join("o").join("winnames"));
        let (path, skipped) = clone_from_unchecked(&s, &testkit::file_url(&up), &JobCtx::noop()).unwrap();
        assert!(skipped.is_empty());
        let r = testkit::open(&path);
        let tree = r.head().unwrap().peel_to_tree().unwrap();
        for name in ["a:b.txt", "CON.txt", "trailing."] {
            assert!(tree.get_name(name).is_some(), "{name}");
        }
        let res = refs::resolve(&r, &validate::rev("main").unwrap()).unwrap();
        let listing = crate::tree::list_dir(&r, &res, &validate::GitPath::root()).unwrap();
        assert!(listing.entries.iter().any(|e| e.name == "CON.txt" && !e.storable_here));
    }

    #[test]
    fn cancel_mid_transfer_leaves_nothing() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let mut b = testkit::repo(&t.path().join("o").join("big"));
        b.bare();
        for i in 0..200u32 {
            b.file(&format!("f{i:03}.txt"), format!("content {i} {}\n", "x".repeat(2000)).as_bytes());
        }
        b.commit("many files", T0).set_head();
        let url = testkit::file_url(b.path());
        let flag = Arc::new(AtomicBool::new(false));
        let f2 = flag.clone();
        let saw_transfer = Arc::new(AtomicBool::new(false));
        let st = saw_transfer.clone();
        let ctx = JobCtx::new(flag, move |p| {
            if matches!(p, Progress::Receiving { .. } | Progress::Resolving { .. }) {
                st.store(true, Ordering::SeqCst);
                f2.store(true, Ordering::SeqCst);
            }
        });
        let e = job_unchecked(&s, &url, &ctx).unwrap_err();
        assert_eq!(e.code(), "cancelled", "{e}");
        assert!(saw_transfer.load(Ordering::SeqCst), "cancel happened during the transfer");
        let owner_dir = s.dirs.archive.join("file").join("o");
        let left: Vec<_> = std::fs::read_dir(&owner_dir).unwrap().flatten().map(|e| e.file_name()).collect();
        assert!(left.is_empty(), "{left:?}");
        assert!(crate::library::list(&s).unwrap().is_empty());
    }

    #[test]
    fn cleanup_partials_removes_leftovers_only() {
        let t = tempfile::tempdir().unwrap();
        let archive = t.path().join("archive");
        let owner = archive.join("github.com").join("o");
        let partial = owner.join(".x.git.partial-123-0");
        std::fs::create_dir_all(partial.join("objects").join("pack")).unwrap();
        let ro = partial.join("objects").join("pack").join("p.pack");
        std::fs::write(&ro, b"x").unwrap();
        let mut perm = std::fs::metadata(&ro).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&ro, perm).unwrap();
        std::fs::create_dir_all(owner.join("x.git")).unwrap();
        std::fs::create_dir_all(owner.join(".hidden")).unwrap();
        assert_eq!(cleanup_partials(&archive).unwrap(), 1);
        assert!(!partial.exists());
        assert!(owner.join("x.git").exists() && owner.join(".hidden").exists());
        assert_eq!(cleanup_partials(&t.path().join("nope")).unwrap(), 0);
    }

    #[test]
    fn second_clone_gets_dash_two() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let (_, url) = testkit::upstream_downstream(&t.path().join("o").join("dup"));
        let a = job_unchecked(&s, &url, &JobCtx::noop()).unwrap();
        let b = job_unchecked(&s, &url, &JobCtx::noop()).unwrap();
        let (JobResult::Cloned { repo_id: ia, .. }, JobResult::Cloned { repo_id: ib, .. }) = (a, b) else { panic!() };
        let (sa, sb) = (crate::repo::summary(&s, ia).unwrap(), crate::repo::summary(&s, ib).unwrap());
        assert!(sa.path.ends_with("dup.git") && sb.path.ends_with("dup-2.git"), "{} {}", sa.path, sb.path);
        assert_eq!((sa.name.as_str(), sb.name.as_str()), ("dup", "dup-2"));
    }

    #[test]
    fn unstorable_default_branch_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let up = t.path().join("o").join("con-default");
        let mut b = testkit::repo(&up);
        b.bare().file("a", b"1").commit("one", T0);
        let tip = b.head();
        testkit::add_packed_refs(&up, &[("refs/heads/con", tip)]);
        testkit::set_head_file(&up, "refs/heads/con");
        let e = clone_from_unchecked(&s, &testkit::file_url(&up), &JobCtx::noop()).unwrap_err();
        assert_eq!(e.code(), "refused", "{e}");
        assert!(e.to_string().contains("\"con\""), "{e}");
        assert!(leftovers(&s.dirs.archive).is_empty());
    }

    #[test]
    fn archive_path_too_long_for_libgit2_is_refused_early() {
        // D-45: libgit2 cannot open a git dir longer than MAX_PATH - 92 even with core.longpaths.
        let t = tempfile::tempdir().unwrap();
        let mut s = testkit::shelf(t.path());
        s.dirs.archive = t.path().join("a".repeat(120)).join("b".repeat(120));
        let (_, url) = testkit::upstream_downstream(&t.path().join("o").join("deep"));
        let e = clone_from_unchecked(&s, &url, &JobCtx::noop()).unwrap_err();
        if cfg!(windows) {
            assert_eq!(e.code(), "refused", "{e}");
            assert!(e.to_string().contains("too long"), "{e}");
            assert!(!s.dirs.archive.exists(), "nothing created");
        }
    }

    #[test]
    fn long_ref_split_skips_names_whose_loose_file_is_too_long() {
        let mut skipped = vec!["refs/x".to_string()];
        let base = 100; // git dir chars incl. trailing slash
        let room = MAX_PATH - (base + "logs/".len() + ORIGIN.len() + ".lock".len());
        let fits = "a".repeat(room);
        let over = "b".repeat(room + 1);
        let keep = long_ref_split(base, ORIGIN, vec!["main".into(), fits.clone(), over.clone()], &mut skipped);
        assert_eq!(keep, ["main".to_string(), fits]);
        assert_eq!(skipped, ["refs/x".to_string(), over]);
    }

    #[test]
    fn overlong_advertised_ref_fails_cleanly_over_file_transport() {
        // D-45: local-transport behaviour only — `local_negotiate_fetch` DWIM-looks-up every advertised name in the
        // receiving repo, so a very long name fails the test clone inside libgit2 before any refspec filter (https
        // skips such names via `drop_long_refs`). The job leaves no folder behind.
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let up = t.path().join("o").join("longref");
        let mut b = testkit::repo(&up);
        b.bare().file("a", b"1").commit("one", T0).set_head();
        let long = format!("feature/{}", "x".repeat(152));
        testkit::add_packed_refs(&up, &[(&format!("refs/heads/{long}"), b.head())]);
        let res = clone_from_unchecked(&s, &testkit::file_url(&up), &JobCtx::noop());
        if cfg!(windows) {
            let e = res.unwrap_err();
            assert!(e.to_string().contains("path too long"), "{e}");
            assert!(leftovers(&s.dirs.archive).is_empty());
            let owner = s.dirs.archive.join("file").join("o");
            assert_eq!(std::fs::read_dir(owner).unwrap().count(), 0);
        }
    }

    #[test]
    fn remove_tree_handles_deep_read_only_trees() {
        let t = tempfile::tempdir().unwrap();
        let mut deep = t.path().join("root");
        for i in 0..12 {
            deep = deep.join(format!("segment-{i:02}-{}", "d".repeat(20)));
        }
        std::fs::create_dir_all(paths::long(&deep)).unwrap();
        let f = deep.join("pack.idx");
        std::fs::write(paths::long(&f), b"x").unwrap();
        let mut perm = std::fs::metadata(paths::long(&f)).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(paths::long(&f), perm).unwrap();
        assert!(f.to_string_lossy().len() > 260);
        remove_tree(&t.path().join("root")).unwrap();
        assert!(!t.path().join("root").exists());
    }

    #[test]
    fn insteadof_rewrite_is_refused_before_connecting() {
        // D-44: a config rewrite of the validated https URL (here to http://) fails the job with no connection.
        let t = tempfile::tempdir().unwrap();
        let r = Repository::init_bare(t.path().join("x.git")).unwrap();
        r.config().unwrap().set_str("url.http://insecure.example/.insteadOf", "https://github.com/").unwrap();
        let mut remote = r.remote_anonymous("https://github.com/o/r.git").unwrap();
        let before = testkit::net_calls();
        let e = check_effective_url(&remote, "https://github.com/o/r.git").unwrap_err();
        assert_eq!(e.code(), "refused", "{e}");
        assert!(e.to_string().contains("http://insecure.example/o/r.git"), "{e}");
        assert_eq!(testkit::net_calls(), before);
        // and the same URL without a rewrite passes
        r.config().unwrap().remove("url.http://insecure.example/.insteadOf").unwrap();
        remote = r.remote_anonymous("https://github.com/o/r.git").unwrap();
        check_effective_url(&remote, "https://github.com/o/r.git").unwrap();
    }

    #[test]
    fn preview_and_rejections_never_touch_the_network() {
        // M3.4: validator purity, for the preview and the job entry.
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let before = testkit::net_calls();
        for bad in [
            "git@github.com:o/r.git", "ssh://git@github.com/o/r", "git://github.com/o/r", "file:///C:/x", "http://github.com/o/r",
            "https://user:pw@github.com/o/r", "https://u@github.com/o/r", "C:\\repos\\x", "c:/repos/x", "/etc/x", "./x", "../x",
            "a\\b", "a/b/c", "https://github.com/o/../r", "", "   ", "https://github.com/onlyowner",
        ] {
            let e = preview(&s, bad).unwrap_err();
            assert_eq!(e.code(), "invalidUrl", "{bad:?}: {e}");
            let e = job(&s, bad, &JobCtx::noop()).unwrap_err();
            assert_eq!(e.code(), "invalidUrl", "{bad:?}: {e}");
        }
        assert_eq!(testkit::net_calls(), before);
        assert!(!s.dirs.archive.exists(), "nothing created");
        let p = preview(&s, "owner/repo").unwrap();
        assert_eq!(p.url, "https://github.com/owner/repo.git");
        assert!(p.target_dir.ends_with(&Path::new("github.com").join("owner").join("repo.git").display().to_string()));
        assert_eq!(testkit::net_calls(), before);
        // type-level: clone() accepts only a validated CloneUrl
        type CloneFn = fn(&Shelf, &CloneUrl, &JobCtx) -> Result<(PathBuf, Vec<String>)>;
        let _: CloneFn = clone;
    }

    #[test]
    fn clone_counts_its_connections() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let (_, url) = testkit::upstream_downstream(&t.path().join("o").join("n"));
        let before = testkit::net_calls();
        clone_from_unchecked(&s, &url, &JobCtx::noop()).unwrap();
        assert_eq!(testkit::net_calls() - before, 2, "one listing connect + one fetch");
    }
}
