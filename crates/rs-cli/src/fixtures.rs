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
