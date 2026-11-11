//! Directory listings (M2.4; FR-15, NFR-P2, NFR-W3, R-7).
//!
//! Git paths are resolved inside the object database by walking tree entries one segment at a time. They are
//! never turned into `std::path::Path` and never joined onto a filesystem path (D-34: `a:b.txt` or `CON.txt` are
//! legal git names that must stay browsable, and a git path must never be able to name a file on disk).

use crate::error::{Error, Result};
use crate::types::{EntryKind, ResolvedRev, TreeEntry, TreeListing};
use crate::validate::GitPath;
use git2::{Oid, Repository, Tree};

const MODE_TREE: i32 = 0o040000;
const MODE_EXEC: i32 = 0o100755;
const MODE_LINK: i32 = 0o120000;
const MODE_COMMIT: i32 = 0o160000;

pub fn kind_of_mode(mode: i32) -> EntryKind {
    match mode {
        MODE_TREE => EntryKind::Dir,
        MODE_EXEC => EntryKind::Exec,
        MODE_LINK => EntryKind::Symlink,
        MODE_COMMIT => EntryKind::Submodule,
        _ => EntryKind::File,
    }
}

/// Child `seg` of `tree`: exact bytes first, then (for non-UTF-8 names shown lossily, R-7) the first entry whose
/// lossy name equals `seg`.
fn child(tree: &Tree, seg: &str) -> Option<(Oid, i32)> {
    if let Some(e) = tree.get_name_bytes(seg.as_bytes()) {
        return Some((e.id(), e.filemode()));
    }
    tree.iter().find(|e| String::from_utf8_lossy(e.name_bytes()) == seg).map(|e| (e.id(), e.filemode()))
}

/// (oid, mode) of `path` inside `tree`; root → the tree itself. `None` when absent (or a parent is not a tree).
pub fn lookup(repo: &Repository, tree: &Tree, path: &GitPath) -> Result<Option<(Oid, i32)>> {
    if path.is_root() {
        return Ok(Some((tree.id(), MODE_TREE)));
    }
    let mut segs = path.as_str().split('/').peekable();
    let mut cur: Option<Tree> = None;
    while let Some(seg) = segs.next() {
        let t = cur.as_ref().unwrap_or(tree);
        let Some((oid, mode)) = child(t, seg) else { return Ok(None) };
        if segs.peek().is_none() {
            return Ok(Some((oid, mode)));
        }
        if mode != MODE_TREE {
            return Ok(None);
        }
        cur = Some(repo.find_tree(oid)?);
    }
    Ok(None)
}

/// Subtree at `path` (`None` if absent or not a directory).
pub fn subtree<'r>(repo: &'r Repository, root: &Tree, path: &GitPath) -> Result<Option<Tree<'r>>> {
    match lookup(repo, root, path)? {
        Some((oid, MODE_TREE)) => Ok(Some(repo.find_tree(oid)?)),
        _ => Ok(None),
    }
}

pub(crate) fn root_tree<'r>(repo: &'r Repository, rev: &ResolvedRev) -> Result<Tree<'r>> {
    let oid = Oid::from_str(&rev.tree).map_err(|e| Error::InvalidRev(e.message().to_string()))?;
    Ok(repo.find_tree(oid)?)
}

pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

/// Entries of the directory `path` at `rev`: dirs (and submodules) first, then case-insensitive name. Blob sizes
/// come from object headers (no content read).
pub fn list_dir(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<TreeListing> {
    let root = root_tree(repo, rev)?;
    let tree = match lookup(repo, &root, path)? {
        Some((oid, MODE_TREE)) => repo.find_tree(oid)?,
        Some(_) => return Err(Error::not_found("directory")),
        None => return Err(Error::not_found("path")),
    };
    let odb = repo.odb()?;
    let dir_raw = path.as_str().as_bytes();
    let mut entries = Vec::with_capacity(tree.len());
    for e in tree.iter() {
        let raw = e.name_bytes();
        let name = String::from_utf8_lossy(raw).into_owned();
        let kind = kind_of_mode(e.filemode());
        let full = join(path.as_str(), &name);
        let path_raw = std::str::from_utf8(raw).is_err().then(|| {
            let mut v = dir_raw.to_vec();
            if !v.is_empty() {
                v.push(b'/');
            }
            v.extend_from_slice(raw);
            v
        });
        let size = match kind {
            EntryKind::File | EntryKind::Exec | EntryKind::Symlink => odb.read_header(e.id()).ok().map(|(n, _)| n as u64),
            _ => None,
        };
        entries.push(TreeEntry {
            storable_here: crate::winpath::path_ok_here(&full),
            name,
            path: full,
            path_raw,
            kind,
            oid: e.id().to_string(),
            size,
        });
    }
    sort_entries(&mut entries);
    let readme = crate::readme::find_readme(&entries).map(|e| e.name.clone());
    Ok(TreeListing { rev: rev.clone(), path: path.as_str().to_string(), entries, readme })
}

fn is_dir_like(k: EntryKind) -> bool {
    matches!(k, EntryKind::Dir | EntryKind::Submodule)
}

/// FR-15 order: directories first, then case-insensitive name (ties by exact name).
pub fn sort_entries(entries: &mut [TreeEntry]) {
    entries.sort_by_cached_key(|e| (!is_dir_like(e.kind), e.name.to_lowercase(), e.name.clone()));
}

/// Kind of the entry at `path` (blob route vs tree route); root is a directory.
pub fn entry_kind(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<EntryKind> {
    let root = root_tree(repo, rev)?;
    match lookup(repo, &root, path)? {
        Some((_, mode)) => Ok(kind_of_mode(mode)),
        None => Err(Error::not_found("path")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit;
    use crate::validate::{git_path, rev};

    fn at(repo: &Repository, r: &str) -> ResolvedRev {
        crate::refs::resolve(repo, &rev(r).unwrap()).unwrap()
    }

    #[test]
    fn wide_dir_order_and_sizes() {
        let t = tempfile::tempdir().unwrap();
        let p = testkit::wide_dir(&t.path().join("w"));
        let repo = testkit::open(&p);
        let l = list_dir(&repo, &at(&repo, "main"), &GitPath::root()).unwrap();
        assert_eq!(l.entries.len(), 1000);
        assert!(l.entries[..100].iter().all(|e| e.kind == EntryKind::Dir));
        assert!(l.entries[100..].iter().all(|e| e.kind == EntryKind::File && e.size.is_some()));
        for w in l.entries.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if is_dir_like(a.kind) == is_dir_like(b.kind) {
                assert!(a.name.to_lowercase() <= b.name.to_lowercase(), "{} before {}", a.name, b.name);
            }
        }
        // mixed case interleaves: Ddir0000, ddir0001, Ddir0002…
        assert_eq!(l.entries[0].name, "Ddir0000");
        assert_eq!(l.entries[1].name, "ddir0001");
        assert!(l.entries.iter().all(|e| e.storable_here && e.path_raw.is_none()));
        assert_eq!(l.readme, None);
        let sub = list_dir(&repo, &at(&repo, "main"), &git_path("Ddir0000").unwrap()).unwrap();
        assert_eq!(sub.entries[0].path, "Ddir0000/inner.txt");
    }

    #[test]
    fn kinds_raw_names_and_windows_names() {
        let t = tempfile::tempdir().unwrap();
        let p = testkit::binaries(&t.path().join("b"));
        let repo = testkit::open(&p);
        let r = at(&repo, "main");
        let l = list_dir(&repo, &r, &GitPath::root()).unwrap();
        let kind = |n: &str| l.entries.iter().find(|e| e.name == n).unwrap().kind;
        assert_eq!(kind("img"), EntryKind::Dir);
        assert_eq!(kind("link"), EntryKind::Symlink);
        assert_eq!(kind("run"), EntryKind::Exec);
        assert_eq!(kind("README.md"), EntryKind::File);
        assert_eq!(l.readme.as_deref(), Some("README.md"));
        let link = l.entries.iter().find(|e| e.name == "link").unwrap();
        assert_eq!(link.size, Some("README.md".len() as u64));
        let sub = list_dir(&repo, &r, &git_path("sub").unwrap()).unwrap();
        assert_eq!(sub.entries[0].kind, EntryKind::Submodule);
        assert_eq!(sub.entries[0].size, None);
        // dirs (and submodules) first
        let first_file = l.entries.iter().position(|e| !is_dir_like(e.kind)).unwrap();
        assert!(l.entries[first_file..].iter().all(|e| !is_dir_like(e.kind)));

        // non-UTF-8 name: lossy name, raw bytes kept, no NUL anywhere, still browsable by its lossy path
        let names = list_dir(&repo, &r, &git_path("names").unwrap()).unwrap();
        let e = &names.entries[0];
        assert_eq!(e.name, "caf\u{fffd}.txt");
        assert_eq!(e.path_raw.as_deref(), Some(testkit::NON_UTF8_NAME));
        assert!(!e.name.contains('\0') && !e.path.contains('\0'));
        assert_eq!(entry_kind(&repo, &r, &git_path(&e.path).unwrap()).unwrap(), EntryKind::File);

        // D-34 / NFR-W3: names Windows cannot store are listed, flagged and resolvable inside the object DB.
        let win = list_dir(&repo, &r, &git_path("win").unwrap()).unwrap();
        for n in ["a:b.txt", "CON.txt"] {
            let e = win.entries.iter().find(|e| e.name == n).unwrap();
            assert!(!e.storable_here, "{n}");
            assert_eq!(entry_kind(&repo, &r, &git_path(&e.path).unwrap()).unwrap(), EntryKind::File);
        }
    }

    #[test]
    fn missing_paths_and_files() {
        let t = tempfile::tempdir().unwrap();
        let fx = testkit::merges(&t.path().join("m"));
        let repo = testkit::open(&fx.path);
        let r = at(&repo, "main");
        assert_eq!(list_dir(&repo, &r, &git_path("nope").unwrap()).unwrap_err().code(), "notFound");
        assert_eq!(list_dir(&repo, &r, &git_path("a.txt").unwrap()).unwrap_err().code(), "notFound");
        assert_eq!(list_dir(&repo, &r, &git_path("a.txt/x").unwrap()).unwrap_err().code(), "notFound");
        assert_eq!(entry_kind(&repo, &r, &GitPath::root()).unwrap(), EntryKind::Dir);
        assert_eq!(entry_kind(&repo, &r, &git_path("dir").unwrap()).unwrap(), EntryKind::Dir);
        assert_eq!(entry_kind(&repo, &r, &git_path("dir/x.txt").unwrap()).unwrap(), EntryKind::File);
    }
}
