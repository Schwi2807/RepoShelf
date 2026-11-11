//! M2.12 (NFR-O2, FR-13): every read function leaves an imported non-bare repository untouched — no index
//! refresh, no lock files, no config or ref writes. Every file under the repo folder (`.git` and work tree) is
//! hashed before and after, with the dirty note off and on. (`snapshot` / `read_everything` live in testkit since
//! M3 so the network-boundary test shares them.)

use rs_core::settings::{self, Settings};
use rs_core::testkit::{self, read_everything, snapshot};
use rs_core::types::Source;
use rs_core::repo;
use std::path::{Path, PathBuf};

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
        let changed = testkit::snapshot_diff(&before, &after);
        assert!(changed.is_empty(), "files changed (dirty note {show_dirty_note}): {changed:?}");
    }
}
