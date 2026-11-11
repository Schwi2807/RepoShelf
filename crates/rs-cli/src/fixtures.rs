//! `rs fixtures <dir>`: builds every testkit fixture under `<dir>` (M2.13).
//!
//! `<dir>` must be empty, new, or a previous fixtures dir (marker file `.rs-fixtures`); in the last case the known
//! fixture folders are deleted and rebuilt. Nothing else inside `<dir>` is touched.

use std::path::Path;
use std::time::Instant;

pub const MARKER: &str = ".rs-fixtures";

/// Deletes a fixture folder (loose git objects are read-only on Windows).
fn force_remove(p: &Path) -> std::io::Result<()> {
    if !p.exists() {
        return Ok(());
    }
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)? {
            let e = e?;
            let md = e.metadata()?;
            if md.is_dir() {
                stack.push(e.path());
            } else if md.permissions().readonly() {
                let mut perm = md.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perm.set_readonly(false);
                std::fs::set_permissions(e.path(), perm)?;
            }
        }
    }
    std::fs::remove_dir_all(p)
}

pub fn run(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let marker = dir.join(MARKER);
    let non_empty = std::fs::read_dir(dir).map_err(|e| e.to_string())?.next().is_some();
    if non_empty && !marker.exists() {
        return Err(format!("{} is not empty and has no {MARKER} marker; refusing to write into it", dir.display()));
    }
    for name in rs_core::testkit::FIXTURE_NAMES {
        force_remove(&dir.join(name)).map_err(|e| format!("cannot remove old fixture {name}: {e}"))?;
    }
    std::fs::write(&marker, b"built by `rs fixtures`\n").map_err(|e| e.to_string())?;
    let t = Instant::now();
    let fx = rs_core::testkit::fixtures(dir);
    println!("fixtures built in {:.1} s under {}", t.elapsed().as_secs_f64(), fx.root.display());
    for (name, p) in [
        ("basic", &fx.basic),
        ("wide-dir", &fx.wide_dir),
        ("long-history", &fx.long_history),
        ("merges", &fx.merges),
        ("binaries", &fx.binaries),
        ("security-readme", &fx.security_readme),
        ("releases", &fx.releases),
        ("big-tree", &fx.big_tree),
        ("huge-commit", &fx.huge_commit),
        ("corrupt", &fx.corrupt),
        ("missing", &fx.missing),
    ] {
        println!("  {name:<16} {}", p.display());
    }
    println!("  huge-commit oid  {}", fx.huge_commit_oid);
    Ok(())
}

/// Folder of the `--register` copies inside `<dir>`.
pub const REGISTER_DIR: &str = "registered";

/// `rs fixtures --register <n> <dir>` (M8.5, NFR-P1): builds `n` small copies of the `basic` fixture under
/// `<dir>\registered\basic-NN` and imports them into the shelf named by `RS_DATA_DIR` / `RS_ARCHIVE_DIR`. Both env
/// vars must be set, so the user's real shelf is never touched. Copies that already exist are reused; ones already
/// on the shelf are skipped.
pub fn register(dir: &Path, n: u32) -> Result<(), String> {
    use rs_core::paths::{ENV_ARCHIVE_DIR, ENV_DATA_DIR};
    for var in [ENV_DATA_DIR, ENV_ARCHIVE_DIR] {
        if std::env::var_os(var).is_none_or(|v| v.is_empty()) {
            return Err(format!("--register needs {ENV_DATA_DIR} and {ENV_ARCHIVE_DIR} set to scratch folders ({var} is not set)"));
        }
    }
    let shelf = rs_core::Shelf::open(rs_core::paths::resolve_dirs()).map_err(|e| e.to_string())?;
    let root = dir.join(REGISTER_DIR);
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let t = Instant::now();
    let (mut added, mut skipped) = (0, 0);
    for i in 1..=n {
        let p = root.join(format!("basic-{i:02}"));
        if !p.exists() {
            rs_core::testkit::basic(&p);
            // No github.com origin on the copies: they show up as `local/basic-NN` and nothing offers a fetch.
            rs_core::testkit::open(&p).remote_delete("origin").map_err(|e| format!("{}: {e}", p.display()))?;
        }
        match rs_core::library::import(&shelf, &p) {
            Ok(_) => added += 1,
            Err(rs_core::error::Error::Duplicate(_)) => skipped += 1,
            Err(e) => return Err(format!("import {}: {e}", p.display())),
        }
    }
    println!(
        "registered {added} copies ({skipped} already on the shelf) in {:.1} s under {} into {}",
        t.elapsed().as_secs_f64(),
        root.display(),
        shelf.dirs.data.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test owns both env vars (no parallel env races in this crate).
    #[test]
    fn register_needs_scratch_env_then_imports_once() {
        let t = tempfile::tempdir().unwrap();
        // SAFETY: the only test in this binary that touches these env vars.
        unsafe {
            std::env::remove_var(rs_core::paths::ENV_DATA_DIR);
            std::env::remove_var(rs_core::paths::ENV_ARCHIVE_DIR);
        }
        let err = register(t.path(), 2).unwrap_err();
        assert!(err.contains("RS_DATA_DIR"), "{err}");
        assert!(!t.path().join(REGISTER_DIR).exists(), "nothing built without the env vars");
        unsafe {
            std::env::set_var(rs_core::paths::ENV_DATA_DIR, t.path().join("data"));
            std::env::set_var(rs_core::paths::ENV_ARCHIVE_DIR, t.path().join("archive"));
        }
        register(t.path(), 3).unwrap();
        register(t.path(), 3).unwrap(); // second run: all duplicates, no error
        let shelf = rs_core::Shelf::open(rs_core::paths::resolve_dirs()).unwrap();
        let n: i64 = shelf.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM repos", [], |r| r.get(0))?)).unwrap();
        assert_eq!(n, 3);
        let owners: i64 = shelf.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM repos WHERE slug_owner = 'local' AND origin_url IS NULL", [], |r| r.get(0))?)).unwrap();
        assert_eq!(owners, 3, "copies have no origin");
        unsafe {
            std::env::remove_var(rs_core::paths::ENV_DATA_DIR);
            std::env::remove_var(rs_core::paths::ENV_ARCHIVE_DIR);
        }
    }
}
