//! Last commit per directory entry (M2.8; FR-16, NFR-P3, R-1, ARCH §6.1).
//!
//! One time-ordered walk from the tip over all parents. A commit is skipped when its subtree for the directory is
//! identical to any parent's (TREESAME). Otherwise an entry is attributed to the commit when the commit holds the
//! tip's content for it (the extra `oid_C == pending[e]` rule, which ignores side changes a merge dropped) and every
//! parent holds something else. Known divergence from `git log -1 -- <path>`: a change made and reverted on a side
//! branch before a TREESAME merge is attributed to the revert commit (git reports an older first-parent commit).

use crate::Shelf;
use crate::commits::summary_of;
use crate::error::{Error, Result};
use crate::jobs::CancelToken;
use crate::types::{CommitSummary, EntryCommit, LastCommits};
use crate::validate::GitPath;
use git2::{Commit, Oid, Repository, Sort};
use std::collections::HashMap;

/// Max commits walked per directory (FR-16: "older than 10,000 commits").
pub const CAP: u32 = 10_000;

struct Walker<'r> {
    repo: &'r Repository,
    dir: GitPath,
    /// commit oid → subtree oid of `dir` (None = absent)
    subtrees: HashMap<Oid, Option<Oid>>,
    /// subtree oid → raw name → entry oid
    entries: HashMap<Oid, HashMap<Vec<u8>, Oid>>,
}

impl<'r> Walker<'r> {
    fn subtree(&mut self, commit: Oid) -> Result<Option<Oid>> {
        if let Some(s) = self.subtrees.get(&commit) {
            return Ok(*s);
        }
        let c = self.repo.find_commit(commit)?;
        let s = if self.dir.is_root() {
            Some(c.tree_id())
        } else {
            match crate::tree::lookup(self.repo, &c.tree()?, &self.dir)? {
                Some((oid, 0o040000)) => Some(oid),
                _ => None,
            }
        };
        self.subtrees.insert(commit, s);
        Ok(s)
    }

    fn entries(&mut self, tree: Oid) -> Result<&HashMap<Vec<u8>, Oid>> {
        if !self.entries.contains_key(&tree) {
            let t = self.repo.find_tree(tree)?;
            let m = t.iter().map(|e| (e.name_bytes().to_vec(), e.id())).collect();
            self.entries.insert(tree, m);
        }
        Ok(&self.entries[&tree])
    }

    fn entry_in(&mut self, sub: Option<Oid>, name: &[u8]) -> Result<Option<Oid>> {
        match sub {
            Some(t) => Ok(self.entries(t)?.get(name).copied()),
            None => Ok(None),
        }
    }
}

/// The walk: `pending` maps raw entry names to their oid at the tip. Returns attributions and `capped`.
fn walk(
    repo: &Repository,
    tip: Oid,
    dir: &GitPath,
    mut pending: HashMap<Vec<u8>, Oid>,
    cap: u32,
    cancel: &CancelToken,
) -> Result<(HashMap<Vec<u8>, CommitSummary>, bool)> {
    let mut w = Walker { repo, dir: dir.clone(), subtrees: HashMap::new(), entries: HashMap::new() };
    let mut found = HashMap::new();
    let mut rw = repo.revwalk()?;
    rw.set_sorting(Sort::TIME)?;
    rw.push(tip)?;
    let mut visited = 0u32;
    let mut capped = false;
    for oid in rw {
        if pending.is_empty() {
            break;
        }
        let oid = oid?;
        if visited >= cap {
            capped = true;
            break;
        }
        visited += 1;
        if visited.is_multiple_of(256) {
            cancel.check()?;
        }
        let sub_c = w.subtree(oid)?;
        let c: Commit = repo.find_commit(oid)?;
        let parents: Vec<Oid> = c.parent_ids().collect();
        let mut psubs = Vec::with_capacity(parents.len());
        for p in &parents {
            psubs.push(w.subtree(*p)?);
        }
        if psubs.contains(&sub_c) {
            continue; // TREESAME for dir vs some parent: nothing in dir changed through this commit
        }
        let Some(sub_c) = sub_c else { continue };
        let names: Vec<(Vec<u8>, Oid)> = pending.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let mut hit = Vec::new();
        for (name, tip_oid) in names {
            let oid_c = w.entry_in(Some(sub_c), &name)?;
            if oid_c != Some(tip_oid) {
                continue; // C did not produce the content the tip has
            }
            let mut changed = true;
            for ps in &psubs {
                if w.entry_in(*ps, &name)? == oid_c {
                    changed = false;
                    break;
                }
            }
            if changed {
                hit.push(name);
            }
        }
        if !hit.is_empty() {
            let s = summary_of(&c);
            for name in hit {
                pending.remove(&name);
                found.insert(name, s.clone());
            }
        }
    }
    Ok((found, capped && !pending.is_empty()))
}

fn cached(s: &Shelf, repo_id: i64, tip: Oid, dir: &GitPath) -> Result<Option<LastCommits>> {
    s.db.with(|c| {
        let mut st = c.prepare_cached("SELECT json FROM cache_last_commit WHERE repo_id = ?1 AND tip = ?2 AND dir = ?3")?;
        let mut rows = st.query(rusqlite::params![repo_id, tip.to_string(), dir.as_str()])?;
        Ok(match rows.next()? {
            Some(r) => serde_json::from_str(&r.get::<_, String>(0)?).ok(),
            None => None,
        })
    })
}

/// Raw (name, oid) of the entries of `dir` at `tip`.
fn tip_entries(repo: &Repository, tip: Oid, dir: &GitPath) -> Result<Vec<(Vec<u8>, Oid)>> {
    let root = repo.find_commit(tip)?.tree()?;
    let tree = crate::tree::subtree(repo, &root, dir)?.ok_or_else(|| Error::not_found("directory"))?;
    Ok(tree.iter().map(|e| (e.name_bytes().to_vec(), e.id())).collect())
}

/// Last commit of every entry of `dir` at `tip`, cached per `(repo_id, tip, dir)`. The walk runs without holding
/// the DB lock (read cache → release → compute → write).
pub fn for_dir(s: &Shelf, repo_id: i64, repo: &Repository, tip: Oid, dir: &GitPath, cap: u32, cancel: &CancelToken) -> Result<LastCommits> {
    if let Some(hit) = cached(s, repo_id, tip, dir)? {
        return Ok(hit);
    }
    let names = tip_entries(repo, tip, dir)?;
    let (mut found, capped) = walk(repo, tip, dir, names.iter().cloned().collect(), cap, cancel)?;
    let entries = names
        .iter()
        .map(|(n, _)| EntryCommit { name: String::from_utf8_lossy(n).into_owned(), commit: found.remove(n) })
        .collect();
    let out = LastCommits { tip: tip.to_string(), path: dir.as_str().to_string(), entries, capped };
    let json = serde_json::to_string(&out).map_err(|e| Error::InvalidInput(e.to_string()))?;
    s.db.with(|c| {
        c.execute(
            "INSERT OR REPLACE INTO cache_last_commit (repo_id, tip, dir, json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![repo_id, tip.to_string(), dir.as_str(), json, crate::now_secs()],
        )?;
        Ok(())
    })?;
    Ok(out)
}

/// Last commit of one path: the parent directory's cache entry, else the same walk for that single name (not cached).
pub fn for_path(s: &Shelf, repo_id: i64, repo: &Repository, tip: Oid, path: &GitPath, cap: u32, cancel: &CancelToken) -> Result<Option<CommitSummary>> {
    let (dir, name) = match path.as_str().rsplit_once('/') {
        Some((d, n)) => (crate::validate::git_path(d)?, n),
        None if path.is_root() => return Err(Error::not_found("file")),
        None => (GitPath::root(), path.as_str()),
    };
    if let Some(hit) = cached(s, repo_id, tip, &dir)? {
        return Ok(hit.entries.into_iter().find(|e| e.name == name).and_then(|e| e.commit));
    }
    let entries = tip_entries(repo, tip, &dir)?;
    let entry = entries
        .iter()
        .find(|(n, _)| n.as_slice() == name.as_bytes())
        .or_else(|| entries.iter().find(|(n, _)| String::from_utf8_lossy(n) == name))
        .ok_or_else(|| Error::not_found("path"))?;
    let (mut found, _) = walk(repo, tip, &dir, [entry.clone()].into_iter().collect(), cap, cancel)?;
    Ok(found.remove(&entry.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{self, MergesFx};
    use crate::types::Source;
    use crate::validate::git_path;

    fn setup(t: &std::path::Path) -> (Shelf, i64, MergesFx, Repository) {
        let fx = testkit::merges(&t.join("m"));
        let s = testkit::shelf(t);
        let id = testkit::register_raw(&s, &fx.path, Source::Imported).unwrap();
        let repo = testkit::open(&fx.path);
        (s, id, fx, repo)
    }

    fn of(l: &LastCommits, name: &str) -> Option<String> {
        l.entries.iter().find(|e| e.name == name).unwrap().commit.as_ref().map(|c| c.oid.clone())
    }

    fn git_last(fx: &MergesFx, path: &str) -> Option<String> {
        testkit::git_cli(&fx.path, &["log", "-1", "--format=%H", &fx.m3.to_string(), "--", path]).map(|s| s.trim().to_string())
    }

    #[test]
    fn matches_git_log_on_linear_and_merges() {
        let t = tempfile::tempdir().unwrap();
        let (s, id, fx, repo) = setup(t.path());
        let root = for_dir(&s, id, &repo, fx.m3, &GitPath::root(), CAP, &CancelToken::new()).unwrap();
        let dir = for_dir(&s, id, &repo, fx.m3, &git_path("dir").unwrap(), CAP, &CancelToken::new()).unwrap();
        let rv = for_dir(&s, id, &repo, fx.m3, &git_path("rv").unwrap(), CAP, &CancelToken::new()).unwrap();
        assert!(!root.capped && !dir.capped);
        // linear (first-parent changes)
        assert_eq!(of(&root, "a.txt"), Some(fx.c4.to_string()));
        assert_eq!(of(&root, "keep.txt"), Some(fx.c3.to_string()));
        // ordinary merge: the side commit that made the change
        assert_eq!(of(&dir, "x.txt"), Some(fx.s1.to_string()));
        // merge that dropped a side change: the older first-parent commit
        assert_eq!(of(&dir, "y.txt"), Some(fx.c1.to_string()));
        // directories as entries (m1 is TREESAME to s1 for dir/)
        assert_eq!(of(&root, "dir"), Some(fx.s1.to_string()));
        let checks = [
            ("a.txt", of(&root, "a.txt")),
            ("keep.txt", of(&root, "keep.txt")),
            ("dir", of(&root, "dir")),
            ("dir/x.txt", of(&dir, "x.txt")),
            ("dir/y.txt", of(&dir, "y.txt")),
        ];
        let mut compared = 0;
        for (path, ours) in checks {
            if let Some(g) = git_last(&fx, path) {
                assert_eq!(ours.as_deref(), Some(g.as_str()), "{path}");
                compared += 1;
            }
        }
        if compared == 0 {
            eprintln!("git not on PATH: comparison skipped");
        }
        // Documented divergence (side change + revert, merged TREESAME): we report the revert commit, git reports
        // the older first-parent commit.
        assert_eq!(of(&rv, "z.txt"), Some(fx.r2.to_string()));
        if let Some(g) = git_last(&fx, "rv/z.txt") {
            assert_eq!(g, fx.c1.to_string(), "git's answer");
        }
    }

    #[test]
    fn cache_cap_cancel_and_single_path() {
        let t = tempfile::tempdir().unwrap();
        let (s, id, fx, repo) = setup(t.path());
        let c = CancelToken::new();
        c.cancel();
        // cap (a capped result is cached like any other: older entries stay "older than the cap")
        let capped = for_dir(&s, id, &repo, fx.m3, &git_path("rv").unwrap(), 2, &CancelToken::new()).unwrap();
        assert!(capped.capped);
        assert!(capped.entries.iter().all(|e| e.commit.is_none()));
        // single path without a cache row runs its own walk
        let x = for_path(&s, id, &repo, fx.m3, &git_path("dir/x.txt").unwrap(), CAP, &CancelToken::new()).unwrap();
        assert_eq!(x.unwrap().oid, fx.s1.to_string());
        // cache: second call is served from the DB row even with a cancelled token
        let first = for_dir(&s, id, &repo, fx.m3, &GitPath::root(), CAP, &CancelToken::new()).unwrap();
        let again = for_dir(&s, id, &repo, fx.m3, &GitPath::root(), CAP, &c).unwrap();
        assert_eq!(first, again);
        let a = for_path(&s, id, &repo, fx.m3, &git_path("a.txt").unwrap(), CAP, &c).unwrap();
        assert_eq!(a.unwrap().oid, fx.c4.to_string(), "from the parent dir's cache row");
        assert_eq!(for_dir(&s, id, &repo, fx.m3, &git_path("nope").unwrap(), CAP, &c).unwrap_err().code(), "notFound");
    }

    #[test]
    fn cancel_on_long_walk() {
        let t = tempfile::tempdir().unwrap();
        let mut b = testkit::repo(&t.path().join("r"));
        b.bare().file("old.txt", b"o").commit("old", testkit::T0);
        for i in 1..400 {
            b.file("hot.txt", format!("{i}").as_bytes()).commit("hot", testkit::T0 + i);
        }
        let s = testkit::shelf(t.path());
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Cloned).unwrap();
        let repo = testkit::open(&t.path().join("r"));
        let c = CancelToken::new();
        c.cancel();
        assert_eq!(for_dir(&s, id, &repo, b.head(), &GitPath::root(), CAP, &c).unwrap_err().code(), "cancelled");
        let rows: i64 = s.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM cache_last_commit", [], |r| r.get(0))?)).unwrap();
        assert_eq!(rows, 0, "nothing cached after cancel");
        let l = for_dir(&s, id, &repo, b.head(), &GitPath::root(), CAP, &CancelToken::new()).unwrap();
        assert_eq!(l.entries.iter().find(|e| e.name == "old.txt").unwrap().commit.as_ref().unwrap().summary, "old");
    }
}
