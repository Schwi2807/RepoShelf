//! Fetch and fast-forward (M3.6, M3.7; ARCH §5.4, §0.2; FR-10, FR-11, R-5, D-3, D-20, D-44).
//!
//! The one bounded write exception to NFR-O2: fetch writes `refs/remotes/origin/*`, new tags whose names are free,
//! `FETCH_HEAD`, objects/packs and reflogs of remote-tracking refs. Never `refs/heads/*`, HEAD, the index, the
//! config or the work tree (fetches go through an anonymous remote, so not even `remote.origin.*` is touched).

use crate::Shelf;
use crate::clone::{self, check_effective_url};
use crate::error::{Error, Result};
use crate::jobs::{JobCtx, Progress};
use crate::refs::{HEADS, ORIGIN, TAGS};
use crate::repo;
use crate::types::{FetchOutcome, JobResult, Source};
use crate::validate;
use crate::winpath;
use git2::{Oid, Repository};
use std::collections::BTreeMap;

/// Tags are fetched here first, then adopted into `refs/tags/` only when the name is free (§5.4 step 3–4).
pub const STAGING: &str = "refs/reposhelf-staging/tags/";
/// `new_commits` stops counting here.
pub const NEW_COMMITS_CAP: u32 = 100_000;
pub const NEEDS_HTTPS: &str = "Fetch needs an https origin";

/// Where a fetch goes. Only built by `fetch` (validated https origin) or, in test/e2e builds, `fetch_unchecked`.
pub struct FetchSource {
    url: String,
}

/// FR-10: fetch the repo's origin, which must be a public https URL passing the clone rules (re-validated here).
pub fn fetch(s: &Shelf, repo_id: i64, ctx: &JobCtx) -> Result<FetchOutcome> {
    let (_, r) = repo::open(s, repo_id)?;
    let origin = repo::origin_url(&r);
    drop(r);
    let url = match origin.as_deref() {
        Some(u) if u.starts_with("https://") => validate::clone_url(u).map_err(|_| Error::Refused(NEEDS_HTTPS.into()))?,
        _ => return Err(Error::Refused(NEEDS_HTTPS.into())),
    };
    fetch_from(s, repo_id, FetchSource { url: url.url }, ctx)
}

/// Fetch from whatever the origin is (`file://` upstreams in tests and the e2e build; D-20).
#[cfg(any(test, feature = "testkit"))]
pub fn fetch_unchecked(s: &Shelf, repo_id: i64, ctx: &JobCtx) -> Result<FetchOutcome> {
    let (_, r) = repo::open(s, repo_id)?;
    let url = repo::origin_url(&r).ok_or_else(|| Error::Refused("No origin remote".into()))?;
    drop(r);
    fetch_from(s, repo_id, FetchSource { url }, ctx)
}

/// Raw (unpeeled) targets of the direct refs under `prefix`, keyed by short name.
fn direct_refs(repo: &Repository, prefix: &str) -> Result<BTreeMap<String, Oid>> {
    let mut out = BTreeMap::new();
    for r in repo.references_glob(&format!("{prefix}*"))? {
        let r = r?;
        if let Some(t) = r.target() {
            out.insert(String::from_utf8_lossy(&r.name_bytes()[prefix.len()..]).into_owned(), t);
        }
    }
    Ok(out)
}

/// Deletes every staging ref when created and again when dropped (also on error and cancel).
struct Staging<'r>(&'r Repository);

impl<'r> Staging<'r> {
    fn new(repo: &'r Repository) -> Result<Staging<'r>> {
        clear_staging(repo)?;
        Ok(Staging(repo))
    }
}

impl Drop for Staging<'_> {
    fn drop(&mut self) {
        let _ = clear_staging(self.0);
    }
}

fn clear_staging(repo: &Repository) -> Result<()> {
    let names: Vec<Vec<u8>> = repo.references_glob(&format!("{STAGING}*"))?.filter_map(|r| r.ok().map(|r| r.name_bytes().to_vec())).collect();
    for n in names {
        if let Ok(mut r) = repo.find_reference(&String::from_utf8_lossy(&n)) {
            r.delete()?;
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    /// Test hook: fail right after the download (staging refs exist at that point).
    static FAIL_AFTER_DOWNLOAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn injected_failure() -> Result<()> {
    #[cfg(test)]
    if FAIL_AFTER_DOWNLOAD.with(|f| f.get()) {
        return Err(Error::Refused("injected failure".into()));
    }
    Ok(())
}

/// §5.4 fetch steps 1–6.
fn fetch_from(s: &Shelf, repo_id: i64, src: FetchSource, ctx: &JobCtx) -> Result<FetchOutcome> {
    let (_, repo) = repo::open(s, repo_id)?;
    let ic = clone::ignore_case(&repo);
    // 1. snapshot
    let before_remote = direct_refs(&repo, ORIGIN)?;
    let local_tags = direct_refs(&repo, TAGS)?;
    // 2. advertised refs → storable sets (existing local names win)
    let mut remote = repo.remote_anonymous(&src.url)?;
    check_effective_url(&remote, &src.url)?;
    let adv = clone::advertised(&mut remote, ctx)?;
    let remote_names: Vec<String> = before_remote.keys().cloned().collect();
    let tag_names: Vec<String> = local_tags.keys().cloned().collect();
    let (hkeep, hskip) = winpath::storable_refs(&adv.heads, &remote_names, adv.default.as_deref(), ic);
    let (tkeep, tskip) = winpath::storable_refs(&adv.tags, &tag_names, None, ic);
    // 3. staging namespace, fetch
    let staging = Staging::new(&repo)?;
    let mut specs = Vec::new();
    if !hkeep.is_empty() {
        specs.extend(winpath::refspecs(HEADS, ORIGIN, &hkeep, &hskip));
    }
    if !tkeep.is_empty() {
        specs.extend(winpath::refspecs(TAGS, STAGING, &tkeep, &tskip));
    }
    clone::download(&mut remote, &specs, ctx)?;
    drop(remote);
    injected_failure()?;
    ctx.check()?;
    ctx.progress(Progress::Finalizing);
    // 4. adopt free tag names; differing local tags are kept and reported
    let mut out = FetchOutcome {
        skipped_refs: hskip.iter().map(|n| format!("{HEADS}{n}")).chain(tskip.iter().map(|n| format!("{TAGS}{n}"))).collect(),
        ..Default::default()
    };
    for (name, target) in direct_refs(&repo, STAGING)? {
        match local_tags.get(&name) {
            Some(t) if *t == target => {}
            Some(_) => out.kept_local_tags.push(name),
            None => {
                if repo.find_reference(&format!("{TAGS}{name}")).is_err() {
                    repo.reference(&format!("{TAGS}{name}"), target, false, "reposhelf: fetch (new tag)")?;
                    out.new_tags.push(name);
                }
            }
        }
    }
    drop(staging);
    // 5. new commits over the changed remote-tracking refs
    let after_remote = direct_refs(&repo, ORIGIN)?;
    let mut walk = repo.revwalk()?;
    let mut any = false;
    for (name, oid) in &after_remote {
        if before_remote.get(name) != Some(oid) {
            out.updated_branches.push(name.clone());
            if walk.push(*oid).is_ok() {
                any = true;
            }
        }
    }
    if any {
        for oid in before_remote.values().chain(direct_refs(&repo, HEADS)?.values()) {
            let _ = walk.hide(*oid);
        }
        for c in walk {
            c?;
            out.new_commits += 1;
            if out.new_commits >= NEW_COMMITS_CAP {
                break;
            }
        }
    } else {
        drop(walk);
    }
    drop(repo);
    // 6. bookkeeping
    s.db.with(|c| Ok(c.execute("UPDATE repos SET last_fetched_at = ?1 WHERE id = ?2", rusqlite::params![crate::now_secs(), repo_id])?))?;
    crate::library::refresh_one(s, repo_id, &crate::jobs::CancelToken::new())?;
    Ok(out)
}

/// Job entry for `JobKind::Fetch`.
pub fn job(s: &Shelf, repo_id: i64, ctx: &JobCtx) -> Result<JobResult> {
    fetch(s, repo_id, ctx).map(JobResult::Fetched)
}

/// `JobKind::FetchAll`: every fetchable repo in turn (cancellable between and inside repos). Outcomes are summed;
/// new/kept tags and skipped refs are prefixed with the slug. Individual failures do not stop the run; they are
/// reported together at the end.
pub fn fetch_all(s: &Shelf, ctx: &JobCtx) -> Result<FetchOutcome> {
    fetch_all_with(s, ctx, |o| repo::is_fetchable(Some(o)), fetch)
}

#[cfg(any(test, feature = "testkit"))]
pub fn fetch_all_unchecked(s: &Shelf, ctx: &JobCtx) -> Result<FetchOutcome> {
    fetch_all_with(
        s,
        ctx,
        |o| o.starts_with("file:") || repo::is_fetchable(Some(o)),
        |s, id, ctx| {
            let file = repo::summary(s, id)?.origin_url.is_some_and(|o| o.starts_with("file:"));
            if file { fetch_unchecked(s, id, ctx) } else { fetch(s, id, ctx) }
        },
    )
}

fn fetch_all_with(
    s: &Shelf,
    ctx: &JobCtx,
    eligible: impl Fn(&str) -> bool,
    one: impl Fn(&Shelf, i64, &JobCtx) -> Result<FetchOutcome>,
) -> Result<FetchOutcome> {
    let mut total = FetchOutcome::default();
    let mut failures = Vec::new();
    for sum in crate::library::list(s)? {
        ctx.check()?;
        if !sum.origin_url.as_deref().is_some_and(&eligible) {
            continue;
        }
        let slug = format!("{}/{}", sum.owner, sum.name);
        match one(s, sum.id, ctx) {
            Ok(o) => {
                total.new_commits = total.new_commits.saturating_add(o.new_commits);
                total.new_tags.extend(o.new_tags.into_iter().map(|t| format!("{slug}: {t}")));
                total.kept_local_tags.extend(o.kept_local_tags.into_iter().map(|t| format!("{slug}: {t}")));
                total.skipped_refs.extend(o.skipped_refs.into_iter().map(|t| format!("{slug}: {t}")));
                total.updated_branches.extend(o.updated_branches.into_iter().map(|t| format!("{slug}: {t}")));
            }
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(e) => failures.push(format!("{slug} ({e})")),
        }
    }
    if failures.is_empty() {
        Ok(total)
    } else {
        Err(Error::Refused(format!("Fetch failed for {}: {}", failures.len(), failures.join("; "))))
    }
}

/// FR-11 (managed clones only): move `refs/heads/<default>` to `origin/<default>` when that is a fast-forward.
/// Equal → no-op. Returns the new (or unchanged) tip.
pub fn fast_forward_default(s: &Shelf, repo_id: i64) -> Result<String> {
    let (sum, r) = repo::open(s, repo_id)?;
    if sum.source != Source::Cloned {
        return Err(Error::Refused("Fast-forward is only available for repositories cloned by RepoShelf".into()));
    }
    let d = crate::refs::head_branch(&r)
        .or(sum.default_branch.clone())
        .ok_or_else(|| Error::Refused("HEAD is detached; there is no default branch to fast-forward".into()))?;
    let local = r.refname_to_id(&format!("{HEADS}{d}")).map_err(|_| Error::not_found("default branch"))?;
    let remote = r.refname_to_id(&format!("{ORIGIN}{d}")).map_err(|_| Error::not_found("origin branch"))?;
    if local == remote {
        return Ok(local.to_string());
    }
    if !r.graph_descendant_of(remote, local)? {
        return Err(Error::Refused("Branch has diverged from origin".into()));
    }
    r.reference(&format!("{HEADS}{d}"), remote, true, "reposhelf: fast-forward")?;
    drop(r);
    crate::library::refresh_one(s, repo_id, &crate::jobs::CancelToken::new())?;
    Ok(remote.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};

    /// Managed clone of a fresh upstream; returns (shelf tempdir, shelf, upstream builder, repo id).
    fn managed() -> (tempfile::TempDir, Shelf, testkit::TestRepo, i64) {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let (up, url) = testkit::upstream_downstream(&t.path().join("o").join("up"));
        let JobResult::Cloned { repo_id, .. } = clone::job_unchecked(&s, &url, &JobCtx::noop()).unwrap() else { panic!() };
        (t, s, up, repo_id)
    }

    #[test]
    fn new_commits_and_tags_and_kept_local_tag() {
        let (_t, s, mut up, id) = managed();
        let path = repo::summary(&s, id).unwrap().path;
        // local tag "v2" pointing somewhere else than upstream's "v2"
        let local = testkit::open(std::path::Path::new(&path));
        let first_obj = local.revparse_single("refs/tags/v1.0.0^{commit}").unwrap();
        local.tag_lightweight("v2", &first_obj, false).unwrap();
        let first = first_obj.id();
        drop(first_obj);
        drop(local);
        up.file("c.txt", b"3\n").commit("three", T0 + 600).file("d.txt", b"4\n").commit("four", T0 + 700);
        up.tag("v1.1.0", true).tag("v2", false);
        let before = testkit::net_calls();
        let o = fetch_unchecked(&s, id, &JobCtx::noop()).unwrap();
        assert!(testkit::net_calls() > before);
        assert_eq!(o.new_commits, 2);
        assert_eq!(o.new_tags, ["v1.1.0"]);
        assert_eq!(o.kept_local_tags, ["v2"]);
        assert_eq!(o.updated_branches, ["main"]);
        assert!(o.skipped_refs.is_empty());
        let r = testkit::open(std::path::Path::new(&path));
        assert_eq!(r.refname_to_id("refs/remotes/origin/main").unwrap(), up.head());
        assert_eq!(r.refname_to_id("refs/tags/v2").unwrap(), first, "local tag untouched");
        assert_ne!(r.refname_to_id("refs/heads/main").unwrap(), up.head(), "fetch never moves local branches");
        assert_eq!(r.references_glob(&format!("{STAGING}*")).unwrap().count(), 0);
        let sum = repo::summary(&s, id).unwrap();
        assert!(sum.last_fetched_at.is_some());
        // second fetch: nothing new
        let o = fetch_unchecked(&s, id, &JobCtx::noop()).unwrap();
        assert_eq!((o.new_commits, o.new_tags.len(), o.updated_branches.len()), (0, 0, 0));
        assert_eq!(o.kept_local_tags, ["v2"]);
    }

    #[test]
    fn imported_non_bare_changes_only_allowed_refs() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let (mut up, url) = testkit::upstream_downstream(&t.path().join("o").join("up"));
        let down = t.path().join("down");
        git2::build::RepoBuilder::new().clone(&url, &down).unwrap();
        {
            let r = testkit::open(&down);
            let head = r.head().unwrap().peel_to_commit().unwrap();
            r.tag_lightweight("mine", head.as_object(), false).unwrap();
        }
        // the user's own uncommitted work
        std::fs::write(down.join("README.md"), b"# local edit\n").unwrap();
        let id = crate::library::import(&s, &down).unwrap().id;
        up.file("n.txt", b"n\n").commit("new 1", T0 + 900).file("m.txt", b"m\n").commit("new 2", T0 + 960).tag("v9", false);
        let before = testkit::snapshot(&down);
        let o = fetch_unchecked(&s, id, &JobCtx::noop()).unwrap();
        assert_eq!(o.new_commits, 2);
        assert_eq!(o.new_tags, ["v9"]);
        let after = testkit::snapshot(&down);
        let changed = testkit::snapshot_diff(&before, &after);
        assert!(!changed.is_empty());
        let allowed = |p: &str| {
            p.starts_with(".git/objects/")
                || p.starts_with(".git/refs/remotes/")
                || p.starts_with(".git/logs/refs/remotes/")
                || p == ".git/FETCH_HEAD"
                || p == ".git/refs/tags/v9"
        };
        let bad: Vec<&String> = changed.iter().filter(|p| !allowed(p)).collect();
        assert!(bad.is_empty(), "fetch changed {bad:?}");
        for keep in [".git/index", ".git/HEAD", ".git/config", ".git/refs/heads/main", ".git/refs/tags/mine", "README.md"] {
            assert!(after.contains_key(std::path::Path::new(keep)), "{keep}");
        }
    }

    #[test]
    fn staging_refs_gone_after_injected_error() {
        let (_t, s, mut up, id) = managed();
        up.file("x", b"x").commit("x", T0 + 999).tag("vX", false);
        FAIL_AFTER_DOWNLOAD.with(|f| f.set(true));
        let e = fetch_unchecked(&s, id, &JobCtx::noop()).unwrap_err();
        FAIL_AFTER_DOWNLOAD.with(|f| f.set(false));
        assert!(e.to_string().contains("injected"), "{e}");
        let r = testkit::open(std::path::Path::new(&repo::summary(&s, id).unwrap().path));
        assert_eq!(r.references_glob(&format!("{STAGING}*")).unwrap().count(), 0);
        assert!(r.find_reference("refs/tags/vX").is_err(), "not adopted");
    }

    #[test]
    fn http_or_missing_origin_refused_without_network() {
        let (_t, s, _up, id) = managed();
        let path = repo::summary(&s, id).unwrap().path;
        let before = testkit::net_calls();
        // file:// origin through the checked path
        assert_eq!(fetch(&s, id, &JobCtx::noop()).unwrap_err().to_string(), NEEDS_HTTPS);
        let r = testkit::open(std::path::Path::new(&path));
        r.remote_set_url("origin", "http://github.com/o/r.git").unwrap();
        assert_eq!(fetch(&s, id, &JobCtx::noop()).unwrap_err().to_string(), NEEDS_HTTPS);
        r.remote_set_url("origin", "https://user@github.com/o/r.git").unwrap();
        assert_eq!(fetch(&s, id, &JobCtx::noop()).unwrap_err().code(), "refused");
        r.remote_delete("origin").unwrap();
        assert_eq!(fetch(&s, id, &JobCtx::noop()).unwrap_err().to_string(), NEEDS_HTTPS);
        assert_eq!(testkit::net_calls(), before);
    }

    #[test]
    fn fast_forward_cases() {
        let (_t, s, mut up, id) = managed();
        let path = repo::summary(&s, id).unwrap().path;
        // equal → no-op
        let tip = fast_forward_default(&s, id).unwrap();
        assert_eq!(tip, up.head().to_string());
        // ancestor → FF
        up.file("ff", b"1").commit("ff", T0 + 1000);
        fetch_unchecked(&s, id, &JobCtx::noop()).unwrap();
        assert!(crate::repo::detail(&s, id).unwrap().can_fast_forward == Some(true));
        assert_eq!(fast_forward_default(&s, id).unwrap(), up.head().to_string());
        let r = testkit::open(std::path::Path::new(&path));
        assert_eq!(r.refname_to_id("refs/heads/main").unwrap(), up.head());
        // diverged → refused, nothing moved
        let local_tip = r.find_commit(up.head()).unwrap();
        let tree = local_tip.tree().unwrap();
        let sig = testkit::sig(T0 + 2000);
        let mine = r.commit(Some("refs/heads/main"), &sig, &sig, "local", &tree, &[&local_tip]).unwrap();
        up.file("theirs", b"1").commit("theirs", T0 + 2001);
        fetch_unchecked(&s, id, &JobCtx::noop()).unwrap();
        let e = fast_forward_default(&s, id).unwrap_err();
        assert_eq!(e.to_string(), "Branch has diverged from origin");
        assert_eq!(r.refname_to_id("refs/heads/main").unwrap(), mine);
        // imported → refused
        let fx = testkit::basic(&_t.path().join("imp"));
        let iid = crate::library::import(&s, &fx.path).unwrap().id;
        assert_eq!(fast_forward_default(&s, iid).unwrap_err().code(), "refused");
    }

    #[test]
    fn fetch_all_sums_and_cancels() {
        let (t, s, mut up, id) = managed();
        let (mut up2, url2) = testkit::upstream_downstream(&t.path().join("o").join("second"));
        clone::job_unchecked(&s, &url2, &JobCtx::noop()).unwrap();
        // an imported repo without origin is not eligible (its https origin would need the network)
        let fx = testkit::basic(&t.path().join("imp"));
        testkit::open(&fx.path).remote_delete("origin").unwrap();
        crate::library::import(&s, &fx.path).unwrap();
        up.file("a1", b"1").commit("a1", T0 + 5000);
        up2.file("b1", b"1").commit("b1", T0 + 5000).file("b2", b"2").commit("b2", T0 + 5001);
        let o = fetch_all_unchecked(&s, &JobCtx::noop()).unwrap();
        assert_eq!(o.new_commits, 3);
        assert!(o.updated_branches.contains(&"o/up: main".to_string()), "{:?}", o.updated_branches);
        let _ = id;
        let ctx = JobCtx::noop();
        ctx.cancel_flag().store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(fetch_all_unchecked(&s, &ctx).unwrap_err().code(), "cancelled");
        // checked variant skips file:// origins entirely (no network, nothing eligible)
        let before = testkit::net_calls();
        assert_eq!(fetch_all(&s, &JobCtx::noop()).unwrap(), FetchOutcome::default());
        assert_eq!(testkit::net_calls(), before);
    }
}
