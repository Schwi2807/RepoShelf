//! Latest commit, counts and paging (M2.6; FR-14, FR-26, NFR-P5, D-9, ARCH §6.3–6.4).

use crate::Shelf;
use crate::error::{Error, Result};
use crate::jobs::CancelToken;
use crate::types::{CommitSummary, CommitsPage, LatestCommit, ResolvedRev, Signature, Time};
use crate::validate::GitPath;
use git2::{Commit, Oid, Repository, Sort};

/// Page size of the commits list (FR-26).
pub const PAGE_LIMIT: u32 = 35;
/// Max commits scanned per call when a path filter is active (ARCH §6.4).
pub const SCAN_CAP: u32 = 20_000;

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

pub fn signature_of(s: &git2::Signature) -> Signature {
    let t = s.when();
    Signature { name: lossy(s.name_bytes()), email: lossy(s.email_bytes()), time: Time { secs: t.seconds(), offset_min: t.offset_minutes() } }
}

pub fn summary_of(c: &Commit) -> CommitSummary {
    CommitSummary {
        oid: c.id().to_string(),
        summary: c.summary_bytes().map(lossy).unwrap_or_default(),
        author: signature_of(&c.author()),
        committer: signature_of(&c.committer()),
        parents: c.parent_ids().map(|p| p.to_string()).collect(),
    }
}

pub(crate) fn oid_of(s: &str) -> Result<Oid> {
    Oid::from_str(s).map_err(|e| Error::InvalidRev(e.message().to_string()))
}

/// The commit at `rev` plus the cached count for its tip (`None` = call `count`). `repo_id` keys the count cache
/// (D-36: ARCH §3.4 had no repo id here).
pub fn latest(s: &Shelf, repo_id: i64, repo: &Repository, rev: &ResolvedRev) -> Result<LatestCommit> {
    let tip = oid_of(&rev.commit)?;
    let c = repo.find_commit(tip)?;
    Ok(LatestCommit { commit: summary_of(&c), count: cached_count(s, repo_id, tip)? })
}

pub fn cached_count(s: &Shelf, repo_id: i64, tip: Oid) -> Result<Option<u64>> {
    s.db.with(|c| {
        let mut st = c.prepare_cached("SELECT count FROM cache_commit_count WHERE repo_id = ?1 AND tip = ?2")?;
        let mut rows = st.query(rusqlite::params![repo_id, tip.to_string()])?;
        Ok(match rows.next()? {
            Some(r) => Some(r.get::<_, i64>(0)? as u64),
            None => None,
        })
    })
}

/// Number of commits reachable from `tip`; cached per `(repo_id, tip)`; cancellable every 256 commits.
pub fn count(s: &Shelf, repo_id: i64, repo: &Repository, tip: Oid, cancel: &CancelToken) -> Result<u64> {
    if let Some(n) = cached_count(s, repo_id, tip)? {
        return Ok(n);
    }
    // Walk without holding the DB lock.
    let mut w = repo.revwalk()?;
    w.push(tip)?;
    let mut n = 0u64;
    for oid in w {
        oid?;
        n += 1;
        if n.is_multiple_of(256) {
            cancel.check()?;
        }
    }
    cancel.check()?;
    s.db.with(|c| {
        c.execute(
            "INSERT OR REPLACE INTO cache_commit_count (repo_id, tip, count) VALUES (?1, ?2, ?3)",
            rusqlite::params![repo_id, tip.to_string(), n as i64],
        )?;
        Ok(())
    })?;
    Ok(n)
}

/// One page of the commits list (D-9: offset paging over a pinned tip).
#[derive(Debug, Clone)]
pub struct PageQuery {
    pub tip: Oid,
    /// "Since previous tag": commits reachable from this oid are hidden.
    pub hide: Option<Oid>,
    pub path: Option<GitPath>,
    /// Walk positions to skip (D-36). Use `CommitsPage.skip` of the previous page to continue.
    pub skip: u32,
    pub limit: u32,
    pub scan_cap: u32,
}

impl PageQuery {
    pub fn new(tip: Oid) -> PageQuery {
        PageQuery { tip, hide: None, path: None, skip: 0, limit: PAGE_LIMIT, scan_cap: SCAN_CAP }
    }
}

/// Entry oid at `path` in the commit's tree (`None` = absent).
fn oid_at(repo: &Repository, c: &Commit, path: &GitPath) -> Result<Option<Oid>> {
    let tree = c.tree()?;
    Ok(crate::tree::lookup(repo, &tree, path)?.map(|(oid, _)| oid))
}

/// Path filter rule (ARCH §6.4): the entry differs from the first parent's, or the commit is a root and has it.
fn touches(repo: &Repository, c: &Commit, path: &GitPath) -> Result<bool> {
    let mine = oid_at(repo, c, path)?;
    Ok(match c.parents().next() {
        None => mine.is_some(),
        Some(p) => oid_at(repo, &p, path)? != mine,
    })
}

/// Walks from the pinned tip in time order, skips `skip` walk positions, then collects up to `limit` commits
/// (with a path filter: matching commits, scanning at most `scan_cap`). The returned `skip` is where the next page
/// starts (D-36), so "Older" and "Search older commits" (after `capped`) both continue with it.
pub fn page(repo: &Repository, q: &PageQuery, cancel: &CancelToken) -> Result<CommitsPage> {
    let mut w = repo.revwalk()?;
    w.set_sorting(Sort::TIME)?;
    w.push(q.tip)?;
    if let Some(h) = q.hide {
        w.hide(h)?;
    }
    let mut w = w.peekable();
    let mut pos = 0u32;
    while pos < q.skip {
        match w.next() {
            Some(o) => {
                o?;
            }
            None => break,
        }
        pos += 1;
        if pos.is_multiple_of(4096) {
            cancel.check()?;
        }
    }
    let limit = q.limit.max(1);
    let mut commits = Vec::new();
    let mut scanned = 0u32;
    let mut capped = false;
    while (commits.len() as u32) < limit {
        let Some(oid) = w.next() else { break };
        let oid = oid?;
        pos += 1;
        scanned += 1;
        if scanned.is_multiple_of(256) {
            cancel.check()?;
        }
        let c = repo.find_commit(oid)?;
        let keep = match &q.path {
            Some(p) => touches(repo, &c, p)?,
            None => true,
        };
        if keep {
            commits.push(summary_of(&c));
        }
        if q.path.is_some() && scanned >= q.scan_cap && (commits.len() as u32) < limit {
            capped = true;
            break;
        }
    }
    let has_more = w.peek().is_some();
    Ok(CommitsPage { tip: q.tip.to_string(), skip: pos, commits, has_more, scanned, capped: capped && has_more })
}

/// Last commit that changed `path` at `rev` (file view header), via the parent directory's last-commit cache.
pub fn file_last_commit(
    s: &Shelf,
    repo_id: i64,
    repo: &Repository,
    rev: &ResolvedRev,
    path: &GitPath,
    cancel: &CancelToken,
) -> Result<Option<CommitSummary>> {
    let tip = oid_of(&rev.commit)?;
    crate::lastcommit::for_path(s, repo_id, repo, tip, path, crate::lastcommit::CAP, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, T0};
    use crate::types::Source;
    use crate::validate::git_path;

    fn all_pages(repo: &Repository, mut q: PageQuery) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            let p = page(repo, &q, &CancelToken::new()).unwrap();
            out.extend(p.commits.iter().map(|c| c.oid.clone()));
            if !p.has_more {
                break;
            }
            assert!(p.skip > q.skip, "progress");
            q.skip = p.skip;
        }
        out
    }

    #[test]
    fn pages_concatenate_to_full_walk_and_git_log() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::merges(&t.path().join("m"));
        let repo = testkit::open(&fx.path);
        let mut full = repo.revwalk().unwrap();
        full.set_sorting(Sort::TIME).unwrap();
        full.push(fx.m3).unwrap();
        let full: Vec<String> = full.map(|o| o.unwrap().to_string()).collect();
        assert_eq!(full.len(), 11);
        for limit in [1, 3, 4, 35] {
            let mut q = PageQuery::new(fx.m3);
            q.limit = limit;
            assert_eq!(all_pages(&repo, q), full, "limit {limit}");
        }
        match testkit::git_cli(&fx.path, &["log", "--format=%H", &fx.m3.to_string()]) {
            Some(out) => assert_eq!(out.lines().collect::<Vec<_>>(), full),
            None => eprintln!("git not on PATH: comparison skipped"),
        }
        let p = page(&repo, &PageQuery::new(fx.m3), &CancelToken::new()).unwrap();
        assert_eq!((p.commits.len(), p.has_more, p.skip, p.capped), (11, false, 11, false));
        assert_eq!(p.commits[0].summary, "m3 merge rev (treesame)");
        assert_eq!(p.commits[0].parents.len(), 2);
    }

    #[test]
    fn path_filter_and_since() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::merges(&t.path().join("m"));
        let repo = testkit::open(&fx.path);
        let mut q = PageQuery::new(fx.m3);
        q.path = Some(git_path("a.txt").unwrap());
        let got = all_pages(&repo, q.clone());
        assert_eq!(got, [fx.c4, fx.c2, fx.c1].map(|o| o.to_string()));
        // dir/x.txt: m1 differs from its first parent (c2), s1 from c1, and c1 adds it
        q.path = Some(git_path("dir/x.txt").unwrap());
        assert_eq!(all_pages(&repo, q.clone()), [fx.m1, fx.s1, fx.c1].map(|o| o.to_string()));
        // directory filter
        q.path = Some(git_path("rv").unwrap());
        assert_eq!(all_pages(&repo, q.clone()), [fx.r2, fx.r1, fx.c1].map(|o| o.to_string()));
        // since tag v1 (at m1)
        let mut q = PageQuery::new(fx.m3);
        q.hide = Some(fx.m1);
        let got = all_pages(&repo, q);
        assert_eq!(got.len(), 7);
        assert!(!got.contains(&fx.m1.to_string()) && !got.contains(&fx.s1.to_string()));
    }

    #[test]
    fn capped_scan_continues() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::merges(&t.path().join("m"));
        let repo = testkit::open(&fx.path);
        let mut q = PageQuery::new(fx.m3);
        q.path = Some(git_path("rv/z.txt").unwrap());
        q.scan_cap = 3;
        let p = page(&repo, &q, &CancelToken::new()).unwrap();
        // m3, c4, r2 scanned → r2 matches; cap reached with fewer than `limit` matches
        assert!(p.capped && p.has_more);
        assert_eq!(p.scanned, 3);
        assert_eq!(p.commits.iter().map(|c| c.oid.clone()).collect::<Vec<_>>(), [fx.r2.to_string()]);
        q.skip = p.skip;
        let p2 = page(&repo, &q, &CancelToken::new()).unwrap();
        assert_eq!(p2.skip, 6, "next scan window");
        assert_eq!(p2.commits.iter().map(|c| c.oid.clone()).collect::<Vec<_>>(), [fx.r1.to_string()]);
        let mut all = vec![fx.r2.to_string()];
        let mut q3 = q.clone();
        q3.skip = 0;
        q3.scan_cap = 2;
        let mut got = Vec::new();
        loop {
            let p = page(&repo, &q3, &CancelToken::new()).unwrap();
            got.extend(p.commits.iter().map(|c| c.oid.clone()));
            if !p.has_more {
                break;
            }
            q3.skip = p.skip;
        }
        all.push(fx.r1.to_string());
        all.push(fx.c1.to_string());
        assert_eq!(got, all);
    }

    #[test]
    fn count_cache_latest_and_cancel() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::merges(&t.path().join("m"));
        let s = testkit::shelf(t.path());
        let id = testkit::register_raw(&s, &fx.path, Source::Imported).unwrap();
        let repo = testkit::open(&fx.path);
        let rev = crate::refs::resolve(&repo, &crate::validate::rev("main").unwrap()).unwrap();
        assert_eq!(latest(&s, id, &repo, &rev).unwrap().count, None);
        let c = CancelToken::new();
        c.cancel();
        // 11 commits < 256: the final check still reports the cancel
        assert_eq!(count(&s, id, &repo, fx.m3, &c).unwrap_err().code(), "cancelled");
        assert_eq!(cached_count(&s, id, fx.m3).unwrap(), None, "nothing cached after cancel");
        assert_eq!(count(&s, id, &repo, fx.m3, &CancelToken::new()).unwrap(), 11);
        let l = latest(&s, id, &repo, &rev).unwrap();
        assert_eq!(l.count, Some(11));
        assert_eq!(l.commit.oid, fx.m3.to_string());
        // cache hit: served even when cancelled
        assert_eq!(count(&s, id, &repo, fx.m3, &c).unwrap(), 11);
        if let Some(out) = testkit::git_cli(&fx.path, &["rev-list", "--count", &fx.m3.to_string()]) {
            assert_eq!(out.trim(), "11");
        }
        // signature/summary fields
        assert_eq!(l.commit.author.name, testkit::AUTHOR_NAME);
        assert_eq!(l.commit.committer.time.secs, T0 + 11 * 600);
    }

    #[test]
    fn page_cancel() {
        let t = tempfile::tempdir().unwrap();
        let mut b = testkit::repo(&t.path().join("r"));
        b.bare();
        for i in 0..300 {
            b.file("a", format!("{i}").as_bytes()).commit("c", T0 + i);
        }
        let repo = testkit::open(&t.path().join("r"));
        let c = CancelToken::new();
        c.cancel();
        let mut q = PageQuery::new(b.head());
        q.limit = 1000;
        assert_eq!(page(&repo, &q, &c).unwrap_err().code(), "cancelled");
    }
}
