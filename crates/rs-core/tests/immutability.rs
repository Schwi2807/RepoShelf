//! M2.12 (NFR-O2, FR-13): every read function leaves an imported non-bare repository untouched — no index
//! refresh, no lock files, no config or ref writes. Every file under the repo folder (`.git` and work tree) is
//! hashed before and after, with the dirty note off and on.

use rs_core::jobs::CancelToken;
use rs_core::settings::{self, Settings};
use rs_core::testkit;
use rs_core::types::{EntryKind, Source};
use rs_core::validate::{GitPath, git_path, rev};
use rs_core::{blob, commits, diff, langs, lastcommit, license, readme, refs, releases, repo, tree};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

type Snapshot = BTreeMap<PathBuf, (u64, git2::Oid, Option<SystemTime>)>;

fn snapshot(root: &Path) -> Snapshot {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let md = e.metadata().unwrap();
            if md.is_dir() {
                stack.push(p);
            } else {
                let bytes = std::fs::read(&p).unwrap();
                let hash = git2::Oid::hash_object(git2::ObjectType::Blob, &bytes).unwrap();
                out.insert(p.strip_prefix(root).unwrap().to_path_buf(), (md.len(), hash, md.modified().ok()));
            }
        }
    }
    out
}

/// Calls every read function of M2.1–M2.11 on the repo.
fn read_everything(s: &rs_core::Shelf, id: i64) -> usize {
    let mut calls = 0;
    let c = CancelToken::new();
    let _ = repo::detail(s, id).unwrap();
    let (_, r) = repo::open(s, id).unwrap();
    let _ = repo::dirty(&r).unwrap();
    let _ = repo::head_state(&r).unwrap();
    calls += 4;

    let list = refs::list(&r).unwrap();
    let _ = refs::branches(&r).unwrap();
    let _ = refs::tags(&r).unwrap();
    let names: Vec<String> = list.branches.iter().map(|b| b.name.clone()).collect();
    let _ = refs::ahead_behind(&r, "main", &names, 10_000, &c).unwrap();
    let _ = releases::list(&r).unwrap();
    calls += 5;

    let revs: Vec<String> = list.branches.iter().map(|b| b.name.clone()).chain(list.tags.iter().map(|t| format!("refs/tags/{}", t.name))).chain(["HEAD".to_string()]).collect();
    for rv in &revs {
        let res = refs::resolve(&r, &rev(rv).unwrap()).unwrap();
        let tip = git2::Oid::from_str(&res.commit).unwrap();
        let tree_oid = git2::Oid::from_str(&res.tree).unwrap();
        let _ = commits::latest(s, id, &r, &res).unwrap();
        let _ = commits::count(s, id, &r, tip, &c).unwrap();
        let mut q = commits::PageQuery::new(tip);
        let _ = commits::page(&r, &q, &c).unwrap();
        q.path = Some(git_path("src").unwrap());
        let _ = commits::page(&r, &q, &c).unwrap();
        let _ = langs::for_tree(s, id, &r, tree_oid, &c).unwrap();
        let _ = license::detect_license(&r, &r.find_tree(tree_oid).unwrap());
        let _ = readme::render_readme(&r, &res, &GitPath::root()).unwrap();
        calls += 8;
        // walk every directory
        let mut dirs = vec![GitPath::root()];
        while let Some(d) = dirs.pop() {
            let l = tree::list_dir(&r, &res, &d).unwrap();
            let _ = lastcommit::for_dir(s, id, &r, tip, &d, lastcommit::CAP, &c).unwrap();
            calls += 2;
            for e in &l.entries {
                let p = git_path(&e.path).unwrap();
                let _ = tree::entry_kind(&r, &res, &p).unwrap();
                match e.kind {
                    EntryKind::Dir => dirs.push(p),
                    _ => {
                        let _ = blob::meta(&r, &res, &p).unwrap();
                        let _ = blob::text(&r, &res, &p).unwrap();
                        let _ = blob::bytes(&r, &res, &p, u64::MAX).unwrap();
                        let _ = commits::file_last_commit(s, id, &r, &res, &p, &c).unwrap();
                        if readme::is_markdown(&e.path) {
                            let _ = readme::render_file(&r, &res, &p).unwrap();
                        }
                        calls += 5;
                    }
                }
            }
        }
        let d = diff::commit_detail(&r, tip, &diff::DiffLimits::default()).unwrap();
        for f in &d.files {
            let _ = diff::file_diff(&r, tip, f.index, true);
        }
        calls += 1 + d.files.len();
    }
    calls
}

#[test]
fn read_functions_never_write_into_an_imported_repo() {
    let t = tempfile::tempdir().unwrap();
    let fx = testkit::basic(&t.path().join("imported"));
    // An uncommitted change makes status compare contents (the dirty note must still not refresh the index).
    std::fs::write(fx.path.join("web").join("app.ts"), b"export const app = 2;\n").unwrap();
    std::fs::write(fx.path.join("untracked.txt"), b"new\n").unwrap();
    let path: PathBuf = rs_core::paths::canonical(&fx.path).unwrap();
    let s = testkit::shelf(t.path());
    let id = testkit::register_raw(&s, &path, Source::Imported).unwrap();

    let before = snapshot(&path);
    assert!(before.keys().any(|p| p.ends_with("index")), "fixture has an index");
    assert!(before.keys().any(|p| p == Path::new("README.md")), "fixture has a work tree");
    assert!(before.len() > 20, "{} files", before.len());

    for show_dirty_note in [false, true] {
        settings::save(&s.db, &Settings { show_dirty_note, ..Default::default() }).unwrap();
        assert_eq!(repo::detail(&s, id).unwrap().dirty, show_dirty_note.then_some(true));
        let calls = read_everything(&s, id);
        assert!(calls > 100, "{calls} calls");
        let after = snapshot(&path);
        let added: Vec<_> = after.keys().filter(|k| !before.contains_key(*k)).collect();
        let removed: Vec<_> = before.keys().filter(|k| !after.contains_key(*k)).collect();
        assert!(added.is_empty() && removed.is_empty(), "files added {added:?} removed {removed:?}");
        for (p, v) in &before {
            assert_eq!(after.get(p), Some(v), "{} changed (dirty note {show_dirty_note})", p.display());
        }
    }
}
