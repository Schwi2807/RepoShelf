//! RepoShelf core (no Tauri). ARCH §3.
//!
//! Every module of ARCH §2 is declared here (M1.1) so later milestones never edit this file.

// Framework (M1)
pub mod db;
pub mod error;
pub mod jobs;
pub mod paths;
pub mod settings;
pub mod types;
pub mod validate;
pub mod winpath;

// Data: git read model (M2)
pub mod blob;
pub mod commits;
pub mod diff;
pub mod langs;
pub mod langs_table;
pub mod lastcommit;
pub mod license;
pub mod markdown;
pub mod readme;
pub mod refs;
pub mod releases;
pub mod repo;
pub mod tree;

// Data: library, clone, fetch (M3)
pub mod clone;
pub mod fetch;
#[cfg(feature = "ghreleases")]
pub mod ghreleases;
pub mod library;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use error::{Error, Result};

use db::Db;
use paths::Dirs;

/// The facade the command layer holds (`Arc<Shelf>`).
pub struct Shelf {
    pub db: Db,
    pub dirs: Dirs,
}

impl Shelf {
    /// Creates the data dir if needed and opens `<data>/reposhelf.db` (migrations run).
    /// The archive dir is created lazily by clone.
    pub fn open(dirs: Dirs) -> Result<Shelf> {
        std::fs::create_dir_all(paths::long(&dirs.data))?;
        let db = Db::open(&paths::db_file(&dirs))?;
        Ok(Shelf { db, dirs })
    }

    /// In-memory DB (tests); `dirs` are only recorded, nothing is created.
    pub fn in_memory(dirs: Dirs) -> Result<Shelf> {
        Ok(Shelf { db: Db::open_in_memory()?, dirs })
    }
}

/// Seconds since the Unix epoch.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Smoke check for the D-4 pipeline: comrak (GFM) → ammonia allow-list. Superseded by `markdown::render` (M2.10).
pub fn render_smoke(md: &str) -> String {
    let mut opts = comrak::Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.autolink = true;
    // Raw HTML passes comrak so ammonia (not comrak's escaping) is the one sanitiser.
    opts.render.r#unsafe = true;
    let html = comrak::markdown_to_html(md, &opts);
    ammonia::clean(&html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_markdown_strips_script() {
        let out = render_smoke("# Title\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(out.contains("<h1>"), "{out}");
        assert!(out.contains("<table>"), "{out}");
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("alert(1)"), "{out}");
    }

    #[test]
    fn shelf_open_creates_data_dir_and_db() {
        let t = tempfile::tempdir().unwrap();
        let dirs = Dirs { data: t.path().join("data dir"), archive: t.path().join("archive") };
        let s = Shelf::open(dirs.clone()).unwrap();
        assert!(paths::db_file(&dirs).is_file());
        assert!(!dirs.archive.exists());
        assert!(!s.db.read_only());
    }
}
