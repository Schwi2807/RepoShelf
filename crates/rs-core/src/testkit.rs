//! Test fixtures (feature `testkit` or `cfg(test)`). M1: the repo builder and `register_raw` (D-21).
//! Objects are written with git2 directly (no checkout, no git CLI, no user config): fixed signatures and times.

use crate::Shelf;
use crate::error::Result;
use crate::types::Source;
use git2::{Oid, Repository, RepositoryInitOptions, Signature, Time};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const AUTHOR_NAME: &str = "Fixture Author";
pub const AUTHOR_EMAIL: &str = "fixture@example.com";
pub const DEFAULT_BRANCH: &str = "main";

const MODE_BLOB: i32 = 0o100644;
const MODE_EXEC: i32 = 0o100755;
const MODE_LINK: i32 = 0o120000;
const MODE_COMMIT: i32 = 0o160000;
const MODE_TREE: i32 = 0o040000;

pub fn sig(time: i64) -> Signature<'static> {
    Signature::new(AUTHOR_NAME, AUTHOR_EMAIL, &Time::new(time, 0)).expect("valid signature")
}

/// Builder over a fixture repository. Lazily initialised so `.bare()` can be chosen first:
/// `repo(dir).bare().file("a", b"x").commit("init", 1_700_000_000)`.
pub struct TestRepo {
    dir: PathBuf,
    bare: bool,
    repo: Option<Repository>,
    /// Flat view of the current branch's tree: path → (oid, mode).
    files: BTreeMap<String, (Oid, i32)>,
    branch: String,
    last: Option<Oid>,
}

/// New non-bare fixture repo at `dir` (initial branch `main`).
pub fn repo(dir: &Path) -> TestRepo {
    TestRepo { dir: dir.to_path_buf(), bare: false, repo: None, files: BTreeMap::new(), branch: DEFAULT_BRANCH.into(), last: None }
}

impl TestRepo {
    /// Make it a bare repository (call before any other step).
    pub fn bare(&mut self) -> &mut Self {
        assert!(self.repo.is_none(), "bare() must be the first call");
        self.bare = true;
        self
    }

    pub fn git(&mut self) -> &Repository {
        if self.repo.is_none() {
            std::fs::create_dir_all(&self.dir).expect("create fixture dir");
            let mut o = RepositoryInitOptions::new();
            o.bare(self.bare).initial_head(DEFAULT_BRANCH).mkpath(true);
            self.repo = Some(Repository::init_opts(&self.dir, &o).expect("init fixture repo"));
        }
        self.repo.as_ref().expect("initialised")
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// Tip of the current branch after the last commit/merge.
    pub fn head(&self) -> Oid {
        self.last.expect("no commit yet")
    }

    pub fn current_branch(&self) -> &str {
        &self.branch
    }

    fn put(&mut self, path: &str, bytes: &[u8], mode: i32) -> &mut Self {
        let oid = self.git().blob(bytes).expect("write blob");
        self.files.insert(path.to_string(), (oid, mode));
        self
    }

    /// Stage a regular file (nested paths create trees).
    pub fn file(&mut self, path: &str, bytes: &[u8]) -> &mut Self {
        self.put(path, bytes, MODE_BLOB)
    }
    pub fn exec(&mut self, path: &str, bytes: &[u8]) -> &mut Self {
        self.put(path, bytes, MODE_EXEC)
    }
    pub fn symlink(&mut self, path: &str, target: &str) -> &mut Self {
        self.put(path, target.as_bytes(), MODE_LINK)
    }
    /// Submodule gitlink entry pointing at `commit` (object need not exist).
    pub fn gitlink(&mut self, path: &str, commit: Oid) -> &mut Self {
        self.git();
        self.files.insert(path.to_string(), (commit, MODE_COMMIT));
        self
    }
    pub fn remove(&mut self, path: &str) -> &mut Self {
        self.files.remove(path);
        self
    }

    fn build_tree(&mut self) -> Oid {
        // Group the flat map into nested trees bottom-up.
        fn write(repo: &Repository, entries: &BTreeMap<String, (Oid, i32)>) -> Oid {
            let mut here: BTreeMap<String, (Oid, i32)> = BTreeMap::new();
            let mut sub: BTreeMap<String, BTreeMap<String, (Oid, i32)>> = BTreeMap::new();
            for (p, v) in entries {
                match p.split_once('/') {
                    Some((d, rest)) => {
                        sub.entry(d.to_string()).or_default().insert(rest.to_string(), *v);
                    }
                    None => {
                        here.insert(p.clone(), *v);
                    }
                }
            }
            let mut tb = repo.treebuilder(None).expect("treebuilder");
            for (name, (oid, mode)) in &here {
                tb.insert(name, *oid, *mode).expect("tree insert");
            }
            for (name, children) in &sub {
                let t = write(repo, children);
                tb.insert(name, t, MODE_TREE).expect("tree insert");
            }
            tb.write().expect("write tree")
        }
        let files = self.files.clone();
        write(self.git(), &files)
    }

    fn commit_with(&mut self, msg: &str, time: i64, extra_parent: Option<Oid>) -> Oid {
        let tree_oid = self.build_tree();
        let refname = format!("refs/heads/{}", self.branch);
        let last = self.last;
        let oid = {
            let repo = self.git();
            let tree = repo.find_tree(tree_oid).expect("tree");
            let mut parents = Vec::new();
            if let Some(p) = last {
                parents.push(repo.find_commit(p).expect("parent"));
            }
            if let Some(p) = extra_parent {
                parents.push(repo.find_commit(p).expect("merge parent"));
            }
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
            let s = sig(time);
            // Write the commit object, then move the branch ref (works for bare and non-bare, no index involved).
            let oid = repo.commit(None, &s, &s, msg, &tree, &parent_refs).expect("commit");
            repo.reference(&refname, oid, true, msg).expect("update branch");
            oid
        };
        self.last = Some(oid);
        oid
    }

    /// Commit the staged tree on the current branch with fixed author/committer at `time` (epoch seconds).
    pub fn commit(&mut self, msg: &str, time: i64) -> &mut Self {
        self.commit_with(msg, time, None);
        self
    }

    /// Create branch `name` at the current tip and switch to it (refs only).
    pub fn branch(&mut self, name: &str) -> &mut Self {
        let tip = self.head();
        self.git().reference(&format!("refs/heads/{name}"), tip, true, "branch").expect("create branch");
        self.branch = name.to_string();
        self
    }

    /// Switch to an existing branch and load its tree as the staged state.
    pub fn checkout(&mut self, name: &str) -> &mut Self {
        let mut files = BTreeMap::new();
        let tip = {
            let repo = self.git();
            let tip = repo.refname_to_id(&format!("refs/heads/{name}")).expect("branch exists");
            let tree = repo.find_commit(tip).expect("commit").tree().expect("tree");
            tree.walk(git2::TreeWalkMode::PreOrder, |dir, e| {
                if e.kind() != Some(git2::ObjectType::Tree) {
                    files.insert(format!("{dir}{}", e.name().unwrap_or("")), (e.id(), e.filemode()));
                }
                git2::TreeWalkResult::Ok
            })
            .expect("walk");
            tip
        };
        self.files = files;
        self.branch = name.to_string();
        self.last = Some(tip);
        self
    }

    /// Tag the current tip. Annotated tags get a fixed tagger at the tip's commit time and message = name.
    pub fn tag(&mut self, name: &str, annotated: bool) -> &mut Self {
        let tip = self.head();
        {
            let repo = self.git();
            let obj = repo.find_object(tip, None).expect("tip object");
            if annotated {
                let time = repo.find_commit(tip).expect("commit").time().seconds();
                repo.tag(name, &obj, &sig(time), name, true).expect("annotated tag");
            } else {
                repo.tag_lightweight(name, &obj, true).expect("lightweight tag");
            }
        }
        self
    }

    /// Merge commit on the current branch with parents [current tip, tip of `other`]. The tree is the staged
    /// state, so callers decide the merge result (e.g. drop a side change).
    pub fn merge(&mut self, other: &str, msg: &str, time: i64) -> &mut Self {
        let theirs = self.git().refname_to_id(&format!("refs/heads/{other}")).expect("branch to merge");
        self.commit_with(msg, time, Some(theirs));
        self
    }

    /// Set the `origin` remote URL (config only).
    pub fn origin(&mut self, url: &str) -> &mut Self {
        self.git().remote("origin", url).expect("add origin");
        self
    }
}

/// Inserts a `repos` row by SQL (D-21) so M2 caches have their FK target before `library::import` exists.
/// Stores the canonical path (8.3 temp names expanded); slug `local/<dir name>` with `-2`, `-3`… on collision.
pub fn register_raw(s: &Shelf, path: &Path, source: Source) -> Result<i64> {
    let canon = crate::paths::canonical(path)?;
    let is_bare = Repository::open_ext(&canon, git2::RepositoryOpenFlags::NO_SEARCH, [] as [&std::ffi::OsStr; 0])
        .map(|r| r.is_bare())
        .unwrap_or(false);
    let base = canon
        .file_name()
        .map(|n| n.to_string_lossy().trim_end_matches(".git").to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "repo".into());
    let path_s = canon.to_string_lossy().to_string();
    s.db.tx(|tx| {
        let mut name = base.clone();
        let mut n = 2;
        while tx.query_row("SELECT COUNT(*) FROM repos WHERE slug_owner = 'local' AND slug_name = ?1", [&name], |r| r.get::<_, i64>(0))? > 0 {
            name = format!("{base}-{n}");
            n += 1;
        }
        tx.execute(
            "INSERT INTO repos (slug_owner, slug_name, source, path, is_bare, added_at) VALUES ('local', ?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![name, source.as_db(), path_s, is_bare, crate::now_secs()],
        )?;
        Ok(tx.last_insert_rowid())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Dirs;

    const T0: i64 = 1_700_000_000;

    #[test]
    fn builder_produces_expected_commits() {
        let t = tempfile::tempdir().unwrap();
        let mut b = repo(&t.path().join("demo"));
        b.file("README.md", b"# Demo\n").file("src/main.rs", b"fn main() {}\n").commit("init", T0);
        let c1 = b.head();
        b.file("src/lib.rs", b"pub fn x() {}\n").exec("run.sh", b"#!/bin/sh\n").symlink("link", "README.md").commit("second", T0 + 60);
        b.tag("v1.0.0", true).tag("light", false);
        b.branch("side").file("side.txt", b"side\n").commit("on side", T0 + 120);
        let side = b.head();
        b.checkout("main");
        assert!(!b.files.contains_key("side.txt"));
        b.file("side.txt", b"side\n").merge("side", "Merge side", T0 + 180);
        let merge = b.head();

        let repo = b.git();
        assert!(!repo.is_bare());
        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "main");
        let m = repo.find_commit(merge).unwrap();
        assert_eq!(m.parent_count(), 2);
        assert_eq!(m.parent_id(1).unwrap(), side);
        assert_eq!(m.author().name().unwrap(), AUTHOR_NAME);
        assert_eq!(m.time().seconds(), T0 + 180);

        let mut walk = repo.revwalk().unwrap();
        walk.push(merge).unwrap();
        let all: Vec<Oid> = walk.map(|o| o.unwrap()).collect();
        assert_eq!(all.len(), 4);
        assert_eq!(*all.last().unwrap(), c1);

        let tree = m.tree().unwrap();
        for p in ["README.md", "src/main.rs", "src/lib.rs", "run.sh", "link", "side.txt"] {
            assert!(tree.get_path(Path::new(p)).is_ok(), "{p}");
        }
        assert_eq!(tree.get_path(Path::new("run.sh")).unwrap().filemode(), MODE_EXEC);
        assert_eq!(tree.get_path(Path::new("link")).unwrap().filemode(), MODE_LINK);

        let tag = repo.revparse_single("refs/tags/v1.0.0").unwrap();
        assert!(tag.as_tag().is_some(), "annotated");
        assert_eq!(tag.peel_to_commit().unwrap().summary().unwrap(), Some("second"));
        let light = repo.revparse_single("refs/tags/light").unwrap();
        assert!(light.as_commit().is_some(), "lightweight points at the commit");
        // no work tree files are written
        assert!(!t.path().join("demo").join("README.md").exists());
    }

    #[test]
    fn bare_and_gitlink() {
        let t = tempfile::tempdir().unwrap();
        let mut b = repo(&t.path().join("x.git"));
        b.bare().file("a", b"1").gitlink("vendor/lib", Oid::from_str(&"1".repeat(40)).unwrap()).commit("c", T0);
        let head = b.head();
        let repo = b.git();
        assert!(repo.is_bare());
        let tree = repo.find_commit(head).unwrap().tree().unwrap();
        assert_eq!(tree.get_path(Path::new("vendor/lib")).unwrap().filemode(), MODE_COMMIT);
    }

    #[test]
    fn same_inputs_same_oids() {
        let t = tempfile::tempdir().unwrap();
        let mk = |name: &str| {
            let mut b = repo(&t.path().join(name));
            b.file("a.txt", b"x").commit("one", T0).file("b.txt", b"y").commit("two", T0 + 1);
            b.head()
        };
        assert_eq!(mk("r1"), mk("r2"), "fixtures are deterministic");
    }

    #[test]
    fn register_raw_inserts_row_with_canonical_path() {
        let t = tempfile::tempdir().unwrap();
        let shelf = Shelf::in_memory(Dirs { data: t.path().join("d"), archive: t.path().join("a") }).unwrap();
        let mut b = repo(&t.path().join("proj"));
        b.file("a", b"1").commit("c", T0);
        let mut bare = repo(&t.path().join("other").join("proj.git"));
        bare.bare().file("a", b"1").commit("c", T0);

        let id1 = register_raw(&shelf, &t.path().join("proj"), Source::Imported).unwrap();
        let id2 = register_raw(&shelf, &t.path().join("other").join("proj.git"), Source::Cloned).unwrap();
        assert_ne!(id1, id2);
        let rows: Vec<(String, String, String, bool)> = shelf
            .db
            .with(|c| {
                let mut st = c.prepare("SELECT slug_name, source, path, is_bare FROM repos ORDER BY id")?;
                Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?)
            })
            .unwrap();
        assert_eq!(rows[0].0, "proj");
        assert_eq!(rows[1].0, "proj-2");
        assert_eq!((rows[0].1.as_str(), rows[0].3), ("imported", false));
        assert_eq!((rows[1].1.as_str(), rows[1].3), ("cloned", true));
        let canon = crate::paths::canonical(&t.path().join("proj")).unwrap();
        assert_eq!(rows[0].2, canon.to_string_lossy());
        // same path twice violates the UNIQUE path constraint
        assert!(register_raw(&shelf, &t.path().join("proj"), Source::Imported).is_err());
    }
}
