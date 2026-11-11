//! Test fixtures (feature `testkit` or `cfg(test)`).
//!
//! M1: the repo builder and `register_raw` (D-21). M2: fixture repos (D-23) and `fixtures(root)`.
//! Objects are written with git2 directly (no git CLI, no user config): fixed signatures and strictly increasing
//! commit times, so libgit2's time order equals `git log`. Large fixtures write through an in-memory ODB backend
//! and land as one packfile (fast to build, realistic to read).

use crate::Shelf;
use crate::error::Result;
use crate::types::Source;
use git2::{Oid, Repository, RepositoryInitOptions, Signature, Time};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub const AUTHOR_NAME: &str = "Fixture Author";
pub const AUTHOR_EMAIL: &str = "fixture@example.com";
pub const DEFAULT_BRANCH: &str = "main";
/// Base commit time of every fixture (2023-11-14).
pub const T0: i64 = 1_700_000_000;

pub const MODE_BLOB: i32 = 0o100644;
pub const MODE_EXEC: i32 = 0o100755;
pub const MODE_LINK: i32 = 0o120000;
pub const MODE_COMMIT: i32 = 0o160000;
pub const MODE_TREE: i32 = 0o040000;

/// Hostile README and SVG (NFR-S1, NFR-S7), shared with the mock UI.
pub const SECURITY_README: &[u8] = include_bytes!("../../../tests/fixtures/security/README.md");
pub const SECURITY_SVG: &[u8] = include_bytes!("../../../tests/fixtures/security/svg-with-script.svg");

/// Smallest valid PNG (1x1, transparent).
pub const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

pub const MIT_LICENSE: &str = "MIT License\n\nCopyright (c) 2023 Fixture Author\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND.\n";

pub fn sig(time: i64) -> Signature<'static> {
    Signature::new(AUTHOR_NAME, AUTHOR_EMAIL, &Time::new(time, 0)).expect("valid signature")
}

/// Builder over a fixture repository. Lazily initialised so `.bare()` can be chosen first:
/// `repo(dir).bare().file("a", b"x").commit("init", T0)`.
pub struct TestRepo {
    dir: PathBuf,
    bare: bool,
    repo: Option<Repository>,
    /// Flat view of the current branch's tree: raw path → (oid, mode).
    files: BTreeMap<Vec<u8>, (Oid, i32)>,
    branch: String,
    last: Option<Oid>,
}

/// New non-bare fixture repo at `dir` (initial branch `main`). Non-bare repos get no work tree and no index unless
/// `write_worktree()` is called.
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
            let r = Repository::init_opts(&self.dir, &o).expect("init fixture repo");
            // Pin line-ending handling so a checked-out work tree is clean regardless of the user's git config.
            let mut cfg = r.config().expect("config");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
            self.repo = Some(r);
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

    fn put(&mut self, path: &[u8], bytes: &[u8], mode: i32) -> &mut Self {
        let oid = self.git().blob(bytes).expect("write blob");
        self.files.insert(path.to_vec(), (oid, mode));
        self
    }

    /// Stage a regular file (nested paths create trees).
    pub fn file(&mut self, path: &str, bytes: &[u8]) -> &mut Self {
        self.put(path.as_bytes(), bytes, MODE_BLOB)
    }
    /// Stage a regular file whose path is raw bytes (e.g. a non-UTF-8 name).
    pub fn file_raw(&mut self, path: &[u8], bytes: &[u8]) -> &mut Self {
        self.put(path, bytes, MODE_BLOB)
    }
    pub fn exec(&mut self, path: &str, bytes: &[u8]) -> &mut Self {
        self.put(path.as_bytes(), bytes, MODE_EXEC)
    }
    pub fn symlink(&mut self, path: &str, target: &str) -> &mut Self {
        self.put(path.as_bytes(), target.as_bytes(), MODE_LINK)
    }
    /// Submodule gitlink entry pointing at `commit` (object need not exist).
    pub fn gitlink(&mut self, path: &str, commit: Oid) -> &mut Self {
        self.git();
        self.files.insert(path.as_bytes().to_vec(), (commit, MODE_COMMIT));
        self
    }
    pub fn remove(&mut self, path: &str) -> &mut Self {
        self.files.remove(path.as_bytes());
        self
    }

    fn build_tree(&mut self) -> Oid {
        let files = self.files.clone();
        write_tree(self.git(), &files)
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
                    let mut p = dir.as_bytes().to_vec();
                    p.extend_from_slice(e.name_bytes());
                    files.insert(p, (e.id(), e.filemode()));
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
        if annotated { self.tag_msg(name, name) } else { self.tag_with(name, None) }
    }

    /// Annotated tag with a custom message (tagger time = tip commit time).
    pub fn tag_msg(&mut self, name: &str, msg: &str) -> &mut Self {
        self.tag_with(name, Some(msg))
    }

    fn tag_with(&mut self, name: &str, msg: Option<&str>) -> &mut Self {
        let tip = self.head();
        {
            let repo = self.git();
            let obj = repo.find_object(tip, None).expect("tip object");
            match msg {
                Some(m) => {
                    let time = repo.find_commit(tip).expect("commit").time().seconds();
                    repo.tag(name, &obj, &sig(time), m, true).expect("annotated tag");
                }
                None => {
                    repo.tag_lightweight(name, &obj, true).expect("lightweight tag");
                }
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

    /// Point HEAD at the current branch (non-bare: also used before `write_worktree`).
    pub fn set_head(&mut self) -> &mut Self {
        let r = format!("refs/heads/{}", self.branch);
        self.git().set_head(&r).expect("set HEAD");
        self
    }

    /// Non-bare only: check out HEAD into the work tree and write the index (for dirty/immutability tests).
    pub fn write_worktree(&mut self) -> &mut Self {
        assert!(!self.bare, "write_worktree needs a non-bare repo");
        let mut co = git2::build::CheckoutBuilder::new();
        co.force();
        self.git().checkout_head(Some(&mut co)).expect("checkout HEAD");
        self
    }
}

/// Flat tree view: raw path → (oid, mode).
pub type Flat = BTreeMap<Vec<u8>, (Oid, i32)>;

/// Writes nested trees bottom-up from a flat raw-path map.
pub fn write_tree(repo: &Repository, entries: &Flat) -> Oid {
    let mut here: Flat = BTreeMap::new();
    let mut sub: BTreeMap<Vec<u8>, Flat> = BTreeMap::new();
    for (p, v) in entries {
        match p.iter().position(|b| *b == b'/') {
            Some(i) => {
                sub.entry(p[..i].to_vec()).or_default().insert(p[i + 1..].to_vec(), *v);
            }
            None => {
                here.insert(p.clone(), *v);
            }
        }
    }
    let mut tb = repo.treebuilder(None).expect("treebuilder");
    for (name, (oid, mode)) in &here {
        tb.insert(name.as_slice(), *oid, *mode).expect("tree insert");
    }
    for (name, children) in &sub {
        let t = write_tree(repo, children);
        tb.insert(name.as_slice(), t, MODE_TREE).expect("tree insert");
    }
    tb.write().expect("write tree")
}

/// Runs `f` with an in-memory ODB backend in front, then writes everything it created as one packfile.
pub fn packed<T>(repo: &Repository, f: impl FnOnce(&Repository) -> T) -> T {
    let odb = repo.odb().expect("odb");
    let mempack = odb.add_new_mempack_backend(1000).expect("mempack");
    let out = f(repo);
    let mut buf = git2::Buf::new();
    mempack.dump(repo, &mut buf).expect("dump mempack");
    {
        let mut pw = odb.packwriter().expect("packwriter");
        pw.write_all(&buf).expect("write pack");
        pw.commit().expect("index pack");
    }
    mempack.reset().expect("reset mempack");
    odb.refresh().expect("refresh odb");
    out
}

fn init(dir: &Path, bare: bool) -> Repository {
    let mut t = repo(dir);
    if bare {
        t.bare();
    }
    t.git();
    t.repo.take().expect("repo")
}

fn commit_tree(repo: &Repository, tree: Oid, parents: &[Oid], msg: &str, time: i64) -> Oid {
    let tree = repo.find_tree(tree).expect("tree");
    let ps: Vec<git2::Commit> = parents.iter().map(|p| repo.find_commit(*p).expect("parent")).collect();
    let pr: Vec<&git2::Commit> = ps.iter().collect();
    let s = sig(time);
    repo.commit(None, &s, &s, msg, &tree, &pr).expect("commit")
}

fn set_branch(repo: &Repository, name: &str, oid: Oid) {
    repo.reference(&format!("refs/heads/{name}"), oid, true, "fixture").expect("branch ref");
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures (ARCH §3.4, TASKS M2.2, D-23)

pub const BASIC_README: &str = "# Basic fixture\n\nA small project used by the tests.\n\n| Language | Files |\n|----------|-------|\n| Rust     | 2     |\n| TypeScript | 2   |\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n![logo](docs/logo.png)\n\nSee the [guide](docs/guide.md) and the [usage](#usage) section.\n\n## Usage\n\nRun it.\n\n- [x] tables\n- [ ] more\n";

/// Sizes chosen so languages are exactly Rust 60 % / TypeScript 40 %.
pub fn basic_files() -> Vec<(&'static str, Vec<u8>)> {
    let pad = |head: &str, n: usize| {
        let mut v = head.as_bytes().to_vec();
        v.resize(n - 1, b' ');
        v.push(b'\n');
        v
    };
    vec![
        ("README.md", BASIC_README.as_bytes().to_vec()),
        ("LICENSE", MIT_LICENSE.as_bytes().to_vec()),
        ("src/main.rs", pad("fn main() { basic::run(); }", 300)),
        ("src/lib.rs", pad("pub fn run() {}", 300)),
        ("web/app.ts", pad("export const app = 1;", 200)),
        ("web/util.ts", pad("export function u(): number { return 2; }", 200)),
        ("vendor/lib.js", pad("var vendored = true;", 1000)),
        ("docs/guide.md", b"# Guide\n\nBack to [readme](../README.md).\n".to_vec()),
        ("docs/logo.png", PNG_1X1.to_vec()),
    ]
}

pub struct BasicFx {
    pub path: PathBuf,
    pub first: Oid,
    pub head: Oid,
    pub feature: Oid,
}

/// Non-bare repo with work tree + index: README (table, code, relative image + link), `.rs` + `.ts`, MIT LICENSE,
/// vendored JS, an annotated tag `v0.1.0`, branch `feature`, https origin.
pub fn basic(dir: &Path) -> BasicFx {
    let mut b = repo(dir);
    for (p, c) in basic_files() {
        b.file(p, &c);
    }
    b.commit("Initial import", T0);
    let first = b.head();
    b.file("src/lib.rs", &{
        let mut v = b"pub fn run() { /* v2 */ }".to_vec();
        v.resize(299, b' ');
        v.push(b'\n');
        v
    })
    .commit("Improve run", T0 + 3600);
    b.tag("v0.1.0", true);
    let head = b.head();
    b.branch("feature").file("web/feature.ts", b"export const f = 1;\n").commit("Add feature", T0 + 7200);
    let feature = b.head();
    b.checkout("main").set_head().origin("https://github.com/example/basic.git").write_worktree();
    BasicFx { path: dir.to_path_buf(), first, head, feature }
}

/// Root with 1,000 entries (100 dirs + 900 files, mixed case), 20 commits touching different entries.
pub fn wide_dir(dir: &Path) -> PathBuf {
    let r = init(dir, true);
    packed(&r, |r| {
        let mut files: BTreeMap<Vec<u8>, (Oid, i32)> = BTreeMap::new();
        let mut parent: Option<Oid> = None;
        for c in 0..20u32 {
            for i in 0..1000u32 {
                if c > 0 && i % 20 != c {
                    continue;
                }
                let name = if i < 100 {
                    format!("{}dir{i:04}/inner.txt", if i % 2 == 0 { "D" } else { "d" })
                } else {
                    format!("{}file{i:04}.txt", if i % 3 == 0 { "F" } else { "f" })
                };
                let oid = r.blob(format!("entry {i} rev {c}\n").as_bytes()).expect("blob");
                files.insert(name.into_bytes(), (oid, MODE_BLOB));
            }
            let tree = write_tree(r, &files);
            let ps: Vec<Oid> = parent.into_iter().collect();
            parent = Some(commit_tree(r, tree, &ps, &format!("Wide commit {c}"), T0 + c as i64 * 60));
        }
        set_branch(r, DEFAULT_BRANCH, parent.expect("commits"));
    });
    dir.to_path_buf()
}

pub const LONG_HISTORY_COMMITS: u32 = 10_000;

/// 10k linear commits; each changes `counter.txt` and one of 50 files under `src/`.
pub fn long_history(dir: &Path) -> PathBuf {
    let r = init(dir, true);
    packed(&r, |r| {
        let mut parent: Option<Oid> = None;
        let mut src: BTreeMap<String, Oid> = BTreeMap::new();
        for i in 0..LONG_HISTORY_COMMITS {
            let counter = r.blob(format!("{i}\n").as_bytes()).expect("blob");
            src.insert(format!("f{:02}.rs", i % 50), r.blob(format!("// rev {i}\n").as_bytes()).expect("blob"));
            let mut stb = r.treebuilder(None).expect("tb");
            for (n, o) in &src {
                stb.insert(n.as_str(), *o, MODE_BLOB).expect("insert");
            }
            let st = stb.write().expect("subtree");
            let mut tb = r.treebuilder(None).expect("tb");
            tb.insert("counter.txt", counter, MODE_BLOB).expect("insert");
            tb.insert("src", st, MODE_TREE).expect("insert");
            let tree = tb.write().expect("tree");
            let ps: Vec<Oid> = parent.into_iter().collect();
            parent = Some(commit_tree(r, tree, &ps, &format!("Commit number {i}"), T0 + i as i64 * 60));
        }
        set_branch(r, DEFAULT_BRANCH, parent.expect("commits"));
    });
    dir.to_path_buf()
}

/// Merge-history fixture; see `merges`.
pub struct MergesFx {
    pub path: PathBuf,
    /// root: a.txt, keep.txt, dir/x.txt, dir/y.txt, rv/z.txt
    pub c1: Oid,
    /// main: a.txt
    pub c2: Oid,
    /// side: dir/x.txt
    pub s1: Oid,
    /// merge of side (keeps x change)
    pub m1: Oid,
    /// drop: dir/y.txt (dropped by m2)
    pub d1: Oid,
    /// main: keep.txt
    pub c3: Oid,
    /// merge of drop, y.txt kept at c1's content
    pub m2: Oid,
    /// rev: rv/z.txt changed then reverted
    pub r1: Oid,
    pub r2: Oid,
    /// main: a.txt
    pub c4: Oid,
    /// TREESAME merge of rev
    pub m3: Oid,
}

/// Ordinary merge, a merge dropping a side change, a side change+revert merged TREESAME (ARCH §6.1). Commit times
/// strictly increase in walk order; tag `v1` (annotated) at m1. Non-bare without work tree.
pub fn merges(dir: &Path) -> MergesFx {
    let mut b = repo(dir);
    let t = |n: i64| T0 + n * 600;
    b.file("a.txt", b"a1\n").file("keep.txt", b"k1\n").file("dir/x.txt", b"x1\n").file("dir/y.txt", b"y1\n").file("rv/z.txt", b"z1\n");
    b.commit("c1 root", t(1));
    let c1 = b.head();
    b.branch("side").file("dir/x.txt", b"x-side\n").commit("s1 side changes x", t(3));
    let s1 = b.head();
    b.checkout("main").file("a.txt", b"a2\n").commit("c2 main changes a", t(2));
    let c2 = b.head();
    b.file("dir/x.txt", b"x-side\n").merge("side", "m1 merge side", t(4));
    let m1 = b.head();
    b.tag("v1", true);
    b.branch("drop").file("dir/y.txt", b"y-dropped\n").commit("d1 drop changes y", t(5));
    let d1 = b.head();
    b.checkout("main").file("keep.txt", b"k2\n").commit("c3 main changes keep", t(6));
    let c3 = b.head();
    b.merge("drop", "m2 merge drop, keep y", t(7));
    let m2 = b.head();
    b.branch("rev").file("rv/z.txt", b"z-temp\n").commit("r1 change z", t(8));
    let r1 = b.head();
    b.file("rv/z.txt", b"z1\n").commit("r2 revert z", t(9));
    let r2 = b.head();
    b.checkout("main").file("a.txt", b"a3\n").commit("c4 main changes a", t(10));
    let c4 = b.head();
    b.merge("rev", "m3 merge rev (treesame)", t(11));
    let m3 = b.head();
    b.set_head();
    MergesFx { path: dir.to_path_buf(), c1, c2, s1, m1, d1, c3, m2, r1, r2, c4, m3 }
}

pub const LFS_POINTER: &str = "version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 12345\n";
pub const LARGE_TEXT_BYTES: usize = 1_500_000;
pub const HUGE_TEXT_BYTES: usize = 6_000_000;
pub const NON_UTF8_NAME: &[u8] = b"names/caf\xe9.txt";

fn text_of(len: usize) -> Vec<u8> {
    let line = b"0123456789 abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789\n";
    line.iter().copied().cycle().take(len).collect()
}

/// Bare repo with every special blob kind (FR-25, R-7, NFR-W3).
pub fn binaries(dir: &Path) -> PathBuf {
    let mut b = repo(dir);
    b.bare();
    let mut data = vec![0u8; 4096];
    for (i, v) in data.iter_mut().enumerate() {
        *v = (i % 251) as u8;
    }
    b.file("README.md", b"# Binaries\n")
        .file("img/logo.png", PNG_1X1)
        .file("img/evil.svg", SECURITY_SVG)
        .file("lfs/model.bin", LFS_POINTER.as_bytes())
        .symlink("link", "README.md")
        .file(".gitmodules", b"[submodule \"sub/lib\"]\n\tpath = sub/lib\n\turl = https://example.com/lib.git\n")
        .gitlink("sub/lib", Oid::from_str("0123456789abcdef0123456789abcdef01234567").expect("oid"))
        .file("big/large.txt", &text_of(LARGE_TEXT_BYTES))
        .file("big/huge.txt", &text_of(HUGE_TEXT_BYTES))
        .file("text/latin1.txt", b"caf\xe9 au lait\n")
        .file("text/bom.txt", "\u{feff}hello bom\nline two\n".as_bytes())
        .file("text/crlf.txt", b"one\r\ntwo\r\n")
        .file("text/blank.txt", b"a\n\n  \nb")
        .file("bin/data.bin", &data)
        .file_raw(NON_UTF8_NAME, b"latin-1 name\n")
        .file("win/a:b.txt", b"colon name\n")
        .file("win/CON.txt", b"device name\n")
        .exec("run", b"#!/usr/bin/env python3\nprint('hi')\n")
        .commit("Binaries", T0);
    dir.to_path_buf()
}

/// Bare repo with the hostile README at the root (NFR-S1).
pub fn security_readme(dir: &Path) -> PathBuf {
    let mut b = repo(dir);
    b.bare()
        .file("README.md", SECURITY_README)
        .file("svg-with-script.svg", SECURITY_SVG)
        .file("docs/page.md", b"# Page\n")
        .commit("Security fixture", T0);
    dir.to_path_buf()
}

/// Bare repo with releases: annotated `v0.9.0`, `v1.0.0`, `v2.0.0-rc1`, lightweight `v1.1.0`, annotated non-version
/// tag `snapshot` (4 version cards: rc1 newest, then v1.1.0 = latest stable).
pub fn releases(dir: &Path) -> PathBuf {
    let mut b = repo(dir);
    b.bare();
    let h = 3600;
    b.file("a.txt", b"1\n").commit("first", T0);
    b.tag_msg("v0.9.0", "Beta release\n\nFirst **public** beta.\n");
    b.file("a.txt", b"2\n").commit("second", T0 + h);
    b.tag_msg("v1.0.0", "v1.0.0\n\nStable at last.\n\n- item one\n- item two\n");
    b.file("a.txt", b"3\n").commit("third", T0 + 2 * h);
    b.tag("v1.1.0", false);
    b.file("a.txt", b"4\n").commit("fourth", T0 + 3 * h);
    b.tag_msg("snapshot", "Not a version\n");
    b.file("a.txt", b"5\n").commit("fifth", T0 + 4 * h);
    b.tag_msg("v2.0.0-rc1", "Release candidate 1\n\nTry it <script>alert(1)</script>.\n");
    dir.to_path_buf()
}

pub const BIG_TREE_FILES: u32 = 50_000;

/// One commit with 50k files in 500 dirs (mixed languages, some vendored/docs).
pub fn big_tree(dir: &Path) -> PathBuf {
    let r = init(dir, true);
    let exts = ["rs", "ts", "py", "go", "c", "js", "md", "json", "java", "css"];
    packed(&r, |r| {
        let mut root = r.treebuilder(None).expect("tb");
        for d in 0..500u32 {
            let mut tb = r.treebuilder(None).expect("tb");
            for f in 0..(BIG_TREE_FILES / 500) {
                let ext = exts[((d + f) % exts.len() as u32) as usize];
                let body = format!("// dir {d} file {f}\n{}", "x".repeat(((d * 7 + f * 13) % 400) as usize));
                let oid = r.blob(body.as_bytes()).expect("blob");
                tb.insert(format!("file{f:03}.{ext}"), oid, MODE_BLOB).expect("insert");
            }
            let sub = tb.write().expect("subtree");
            let name = match d % 50 {
                0 => format!("vendor{d}"),
                _ => format!("pkg{d:03}"),
            };
            root.insert(name, sub, MODE_TREE).expect("insert");
        }
        let tree = root.write().expect("tree");
        let c = commit_tree(r, tree, &[], "Big tree", T0);
        set_branch(r, DEFAULT_BRANCH, c);
    });
    dir.to_path_buf()
}

pub const HUGE_COMMIT_FILES: u32 = 400;

/// Two commits; the second rewrites 400 files (30 → 60 lines each, > 20k changed lines). Returns the big commit.
pub fn huge_commit(dir: &Path) -> (PathBuf, Oid) {
    let r = init(dir, true);
    let oid = packed(&r, |r| {
        let mut files: BTreeMap<Vec<u8>, (Oid, i32)> = BTreeMap::new();
        let write = |files: &mut BTreeMap<Vec<u8>, (Oid, i32)>, rev: u32, lines: u32| {
            for i in 0..HUGE_COMMIT_FILES {
                let body: String = (0..lines).map(|l| format!("rev {rev} file {i} line {l}\n")).collect();
                let oid = r.blob(body.as_bytes()).expect("blob");
                files.insert(format!("mod{:02}/file{i:03}.rs", i % 20).into_bytes(), (oid, MODE_BLOB));
            }
        };
        write(&mut files, 0, 30);
        let base = commit_tree(r, write_tree(r, &files), &[], "Base", T0);
        write(&mut files, 1, 60);
        let big = commit_tree(r, write_tree(r, &files), &[base], "Rewrite everything", T0 + 60);
        set_branch(r, DEFAULT_BRANCH, big);
        big
    });
    (dir.to_path_buf(), oid)
}

/// Loose-object repo whose tip commit's root tree object file was deleted (NFR-O3). Returns the deleted tree oid.
pub fn corrupt(dir: &Path) -> (PathBuf, Oid) {
    let mut b = repo(dir);
    b.file("a.txt", b"one\n").commit("one", T0).file("b.txt", b"two\n").commit("two", T0 + 60).set_head();
    let tip = b.head();
    let tree = b.git().find_commit(tip).expect("tip").tree_id();
    let hex = tree.to_string();
    let obj = dir.join(".git").join("objects").join(&hex[..2]).join(&hex[2..]);
    // Loose objects are read-only on disk.
    let mut perm = std::fs::metadata(&obj).expect("loose tree object").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&obj, perm).expect("chmod");
    std::fs::remove_file(&obj).expect("delete tree object");
    (dir.to_path_buf(), tree)
}

/// A small repo for the "registered then renamed" case: callers register `dir`, then rename it away.
pub fn missing(dir: &Path) -> PathBuf {
    let mut b = repo(dir);
    b.file("a.txt", b"x\n").commit("one", T0).set_head();
    dir.to_path_buf()
}

/// Every fixture under `root` (built by `rs fixtures`). Unit tests call the individual builders instead.
pub struct Fixtures {
    pub root: PathBuf,
    pub basic: PathBuf,
    pub wide_dir: PathBuf,
    pub long_history: PathBuf,
    pub merges: PathBuf,
    pub binaries: PathBuf,
    pub security_readme: PathBuf,
    pub releases: PathBuf,
    pub big_tree: PathBuf,
    pub huge_commit: PathBuf,
    pub huge_commit_oid: Oid,
    pub corrupt: PathBuf,
    /// Does not exist: the repo was moved to `missing.moved`.
    pub missing: PathBuf,
}

/// Fixture directory names under the `fixtures` root.
pub const FIXTURE_NAMES: &[&str] = &[
    "basic", "wide-dir", "long-history", "merges", "binaries", "security-readme", "releases", "big-tree", "huge-commit",
    "corrupt", "missing", "missing.moved",
];

/// Builds every fixture into `root/<name>`. Each target must not exist yet.
pub fn fixtures(root: &Path) -> Fixtures {
    let p = |n: &str| root.join(n);
    for n in FIXTURE_NAMES {
        assert!(!p(n).exists(), "fixture target already exists: {}", p(n).display());
    }
    basic(&p("basic"));
    wide_dir(&p("wide-dir"));
    long_history(&p("long-history"));
    merges(&p("merges"));
    binaries(&p("binaries"));
    security_readme(&p("security-readme"));
    releases(&p("releases"));
    big_tree(&p("big-tree"));
    let (_, huge_commit_oid) = huge_commit(&p("huge-commit"));
    corrupt(&p("corrupt"));
    missing(&p("missing"));
    std::fs::rename(p("missing"), p("missing.moved")).expect("move missing fixture");
    Fixtures {
        root: root.to_path_buf(),
        basic: p("basic"),
        wide_dir: p("wide-dir"),
        long_history: p("long-history"),
        merges: p("merges"),
        binaries: p("binaries"),
        security_readme: p("security-readme"),
        releases: p("releases"),
        big_tree: p("big-tree"),
        huge_commit: p("huge-commit"),
        huge_commit_oid,
        corrupt: p("corrupt"),
        missing: p("missing"),
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

/// In-memory shelf over a temp dir (tests).
pub fn shelf(t: &Path) -> Shelf {
    Shelf::in_memory(crate::paths::Dirs { data: t.join("data"), archive: t.join("archive") }).expect("shelf")
}

/// Opens a fixture repo the way `repo::open` does.
pub fn open(path: &Path) -> Repository {
    Repository::open_ext(path, git2::RepositoryOpenFlags::NO_SEARCH, [] as [&std::ffi::OsStr; 0]).expect("open fixture")
}

/// `git` CLI isolated from the user's/system config, run in `repo`. `None` when git is not on PATH.
pub fn git_cli(repo: &Path, args: &[&str]) -> Option<String> {
    let empty = std::env::temp_dir().join("rs-core-empty-gitconfig");
    if !empty.exists() {
        let _ = std::fs::write(&empty, b"");
    }
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.autocrlf=false", "-c", "core.quotepath=false", "-c", "safe.directory=*"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", &empty)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .ok()?;
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---------------------------------------------------------------------------------------------------------------
// M3: snapshots, read-everything, upstream fixtures, network counter

/// Every file under `root`: relative path → (length, blob hash, mtime). Directories are not listed.
pub type Snapshot = BTreeMap<PathBuf, (u64, Oid, Option<std::time::SystemTime>)>;

pub fn snapshot(root: &Path) -> Snapshot {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("read_dir") {
            let e = e.expect("dir entry");
            let p = e.path();
            let md = e.metadata().expect("metadata");
            if md.is_dir() {
                stack.push(p);
            } else {
                let bytes = std::fs::read(&p).expect("read file");
                let hash = Oid::hash_object(git2::ObjectType::Blob, &bytes).expect("hash");
                out.insert(p.strip_prefix(root).expect("under root").to_path_buf(), (md.len(), hash, md.modified().ok()));
            }
        }
    }
    out
}

/// Paths added, removed or changed between two snapshots (sorted, `/`-separated).
pub fn snapshot_diff(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (p, v) in before {
        if after.get(p) != Some(v) {
            out.push(p.to_string_lossy().replace('\\', "/"));
        }
    }
    for p in after.keys() {
        if !before.contains_key(p) {
            out.push(p.to_string_lossy().replace('\\', "/"));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Calls every read function of M2.1–M2.11 on a registered repo (every rev, every directory and file). Returns the
/// number of calls. Used by the immutability (NFR-O2) and network-boundary (NFR-O1) tests.
pub fn read_everything(s: &Shelf, id: i64) -> usize {
    use crate::jobs::CancelToken;
    use crate::types::EntryKind;
    use crate::validate::{GitPath, git_path, rev};
    use crate::{blob, commits, diff, langs, lastcommit, license, readme, refs, releases, repo as repo_mod, tree};
    let mut calls = 0;
    let c = CancelToken::new();
    let _ = repo_mod::detail(s, id).expect("detail");
    let (_, r) = repo_mod::open(s, id).expect("open");
    let _ = repo_mod::dirty(&r).expect("dirty");
    let _ = repo_mod::head_state(&r).expect("head_state");
    calls += 4;

    let list = refs::list(&r).expect("refs list");
    let _ = refs::branches(&r).expect("branches");
    let _ = refs::tags(&r).expect("tags");
    let names: Vec<String> = list.branches.iter().map(|b| b.name.clone()).collect();
    let base = list.default_branch.clone().unwrap_or_else(|| DEFAULT_BRANCH.into());
    let _ = refs::ahead_behind(&r, &base, &names, 10_000, &c).expect("ahead_behind");
    let _ = releases::list(&r).expect("releases");
    calls += 5;

    let revs: Vec<String> = list
        .branches
        .iter()
        .map(|b| b.name.clone())
        .chain(list.tags.iter().map(|t| format!("refs/tags/{}", t.name)))
        .chain(["HEAD".to_string()])
        .collect();
    for rv in &revs {
        let res = refs::resolve(&r, &rev(rv).expect("rev")).expect("resolve");
        let tip = Oid::from_str(&res.commit).expect("oid");
        let tree_oid = Oid::from_str(&res.tree).expect("oid");
        let _ = commits::latest(s, id, &r, &res).expect("latest");
        let _ = commits::count(s, id, &r, tip, &c).expect("count");
        let mut q = commits::PageQuery::new(tip);
        let _ = commits::page(&r, &q, &c).expect("page");
        q.path = Some(git_path("src").expect("path"));
        let _ = commits::page(&r, &q, &c).expect("page path");
        let _ = langs::for_tree(s, id, &r, tree_oid, &c).expect("langs");
        let _ = license::detect_license(&r, &r.find_tree(tree_oid).expect("tree"));
        let _ = readme::render_readme(&r, &res, &GitPath::root()).expect("readme");
        calls += 8;
        let mut dirs = vec![GitPath::root()];
        while let Some(d) = dirs.pop() {
            let l = tree::list_dir(&r, &res, &d).expect("list_dir");
            let _ = lastcommit::for_dir(s, id, &r, tip, &d, lastcommit::CAP, &c).expect("for_dir");
            calls += 2;
            for e in &l.entries {
                let p = git_path(&e.path).expect("entry path");
                let _ = tree::entry_kind(&r, &res, &p).expect("entry_kind");
                match e.kind {
                    EntryKind::Dir => dirs.push(p),
                    _ => {
                        let _ = blob::meta(&r, &res, &p).expect("meta");
                        let _ = blob::text(&r, &res, &p).expect("text");
                        let _ = blob::bytes(&r, &res, &p, u64::MAX).expect("bytes");
                        let _ = commits::file_last_commit(s, id, &r, &res, &p, &c).expect("file_last_commit");
                        if readme::is_markdown(&e.path) {
                            let _ = readme::render_file(&r, &res, &p).expect("render_file");
                        }
                        calls += 5;
                    }
                }
            }
        }
        let d = diff::commit_detail(&r, tip, &diff::DiffLimits::default()).expect("commit_detail");
        for f in &d.files {
            let _ = diff::file_diff(&r, tip, f.index, true);
        }
        calls += 1 + d.files.len();
    }
    calls
}

/// Network connection attempts made on **this thread** (git2 remote connects and ghreleases GETs). Thread-local so
/// parallel clone tests cannot disturb a "stays 0" assertion (NFR-O1).
pub fn net_calls() -> u64 {
    crate::clone::net_calls_on_this_thread()
}

/// `file:///C:/…` URL of a local folder (percent-encoded), for the unchecked clone/fetch seam (D-14, D-20).
pub fn file_url(path: &Path) -> String {
    let canon = crate::paths::canonical(path).expect("canonical path");
    url::Url::from_file_path(&canon).expect("file url").to_string()
}

/// Appends refs to `<git dir>/packed-refs` (creating it). Needed for names that cannot exist as loose ref files on
/// Windows (`llm` + `LLM/x`, `con`, `a|b`).
pub fn add_packed_refs(git_dir: &Path, refs: &[(&str, Oid)]) {
    let file = git_dir.join("packed-refs");
    let mut text = std::fs::read_to_string(&file).unwrap_or_else(|_| "# pack-refs with: peeled fully-peeled \n".into());
    for (name, oid) in refs {
        text.push_str(&format!("{oid} {name}\n"));
    }
    std::fs::write(&file, text).expect("write packed-refs");
}

/// Points HEAD at `refname` by writing the HEAD file directly (the ref may be unstorable here).
pub fn set_head_file(git_dir: &Path, refname: &str) {
    std::fs::write(git_dir.join("HEAD"), format!("ref: {refname}\n")).expect("write HEAD");
}

pub const LONG_PATH_DEPTH: usize = 30;

/// Bare upstream with file names Windows cannot store: `a:b.txt`, `CON.txt`, `trailing.`, and a > 300-char path.
pub fn winnames(dir: &Path) -> PathBuf {
    let long: String = (0..LONG_PATH_DEPTH).map(|i| format!("directory{i:02}/")).collect::<String>() + "file.txt";
    let mut b = repo(dir);
    b.bare()
        .file("README.md", b"# winnames\n")
        .file("a:b.txt", b"colon\n")
        .file("CON.txt", b"device\n")
        .file("trailing.", b"dot\n")
        .file(&long, b"deep\n")
        .commit("Windows-hostile names", T0)
        .set_head();
    dir.to_path_buf()
}

/// Bare upstream with ref names that clash or are invalid on Windows: branches `main`, `llm`, `LLM/x`; tags `v1.0`
/// (fine), `con` and `a|b` (unstorable). Clashing refs live in `packed-refs`.
pub fn refclash(dir: &Path) -> PathBuf {
    let mut b = repo(dir);
    b.bare().file("a.txt", b"1\n").commit("one", T0).set_head();
    let one = b.head();
    b.file("a.txt", b"2\n").commit("two", T0 + 60);
    let two = b.head();
    b.tag("v1.0", false);
    add_packed_refs(dir, &[("refs/heads/LLM/x", one), ("refs/heads/llm", two), ("refs/tags/a|b", one), ("refs/tags/con", two)]);
    dir.to_path_buf()
}

/// Upstream half of an upstream/downstream pair: bare repo with `main` (2 commits), `dev`, annotated `v1.0.0`,
/// lightweight `light`. Returns the builder (on `main`, for adding commits later) and its `file://` URL. The
/// downstream is created by the test (unchecked clone, or `git2::build::RepoBuilder` for an imported work tree).
pub fn upstream_downstream(dir: &Path) -> (TestRepo, String) {
    let mut b = repo(dir);
    b.bare().file("README.md", b"# Upstream\n").file("src/main.rs", b"fn main() {}\n").commit("Initial", T0);
    b.file("src/lib.rs", b"pub fn x() {}\n").commit("Add lib", T0 + 60).tag("v1.0.0", true).tag("light", false);
    b.branch("dev").file("dev.txt", b"dev\n").commit("Dev work", T0 + 120);
    b.checkout("main").set_head();
    let url = file_url(dir);
    (b, url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Dirs;

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
        assert!(!b.files.contains_key(b"side.txt".as_slice()));
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

    #[test]
    fn packed_writes_one_readable_pack() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("p.git");
        let r = init(&dir, true);
        let tip = packed(&r, |r| {
            let mut files = BTreeMap::new();
            for i in 0..100 {
                files.insert(format!("f{i}.txt").into_bytes(), (r.blob(format!("{i}\n").as_bytes()).unwrap(), MODE_BLOB));
            }
            let c = commit_tree(r, write_tree(r, &files), &[], "packed", T0);
            set_branch(r, "main", c);
            c
        });
        let packs: Vec<_> = std::fs::read_dir(dir.join("objects").join("pack")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert!(packs.iter().any(|n| n.to_string_lossy().ends_with(".pack")), "{packs:?}");
        let loose = std::fs::read_dir(dir.join("objects")).unwrap().filter(|e| e.as_ref().unwrap().file_name().len() == 2).count();
        assert_eq!(loose, 0, "nothing loose");
        // A fresh handle reads everything from the pack.
        let fresh = open(&dir);
        let tree = fresh.find_commit(tip).unwrap().tree().unwrap();
        assert_eq!(tree.len(), 100);
        let blob = fresh.find_blob(tree.get_name("f42.txt").unwrap().id()).unwrap();
        assert_eq!(blob.content(), b"42\n");
    }

    #[test]
    fn basic_fixture_has_clean_worktree_and_index() {
        let t = tempfile::tempdir().unwrap();
        let fx = basic(&t.path().join("basic"));
        assert!(fx.path.join("README.md").is_file());
        assert!(fx.path.join(".git").join("index").is_file());
        let r = open(&fx.path);
        assert_eq!(r.head().unwrap().target(), Some(fx.head));
        let st = r.statuses(None).unwrap();
        assert!(st.is_empty(), "clean checkout");
    }
}
