//! SQLite store (ARCH §4): one connection behind a mutex, WAL, forward-only migrations via `user_version`.
//!
//! `Db` is a cheap clone (shared connection) so the `JobRunner` and the `Shelf` use the same store.
//! Never hold the lock across a long git walk: read cache → release → compute → write cache.

use crate::error::Result;
use rusqlite::{Connection, OpenFlags, Transaction};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

const MIGRATIONS: &[&str] = &[include_str!("migrations/0001_init.sql"), include_str!("migrations/0002_github.sql"), include_str!("migrations/0003_explore.sql")];

/// Number of migrations this build knows.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Max rows kept in `cache_last_commit`.
pub const LAST_COMMIT_CACHE_ROWS: i64 = 2_000;
/// Cache rows older than this whose tip is not a current ref tip are dropped on start.
pub const CACHE_MAX_AGE_SECS: i64 = 30 * 24 * 3600;

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    read_only: bool,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").field("read_only", &self.read_only).finish()
    }
}

fn user_version(c: &Connection) -> Result<i64> {
    Ok(c.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

fn migrate(c: &mut Connection) -> Result<()> {
    let from = user_version(c)?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(from.max(0) as usize) {
        let tx = c.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

impl Db {
    /// Opens (creating if needed) and migrates. A DB written by a newer app (`user_version >
    /// SCHEMA_VERSION`) is reopened read-only and never touched (`read_only() == true`).
    pub fn open(file: &Path) -> Result<Db> {
        let mut c = Connection::open(file)?;
        c.busy_timeout(Duration::from_secs(5))?;
        // Read the version before any write pragma.
        if user_version(&c)? > SCHEMA_VERSION {
            drop(c);
            let c = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
            c.busy_timeout(Duration::from_secs(5))?;
            c.pragma_update(None, "foreign_keys", true)?;
            return Ok(Db { conn: Arc::new(Mutex::new(c)), read_only: true });
        }
        let _mode: String = c.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        c.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut c)?;
        Ok(Db { conn: Arc::new(Mutex::new(c)), read_only: false })
    }

    /// Fresh migrated in-memory DB (tests).
    pub fn open_in_memory() -> Result<Db> {
        let mut c = Connection::open_in_memory()?;
        c.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut c)?;
        Ok(Db { conn: Arc::new(Mutex::new(c)), read_only: false })
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let c = self.lock();
        f(&c)
    }

    /// Runs `f` in one transaction; commits on `Ok`, rolls back on `Err`.
    pub fn tx<T>(&self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T> {
        let mut c = self.lock();
        let tx = c.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

/// Startup cache pruning: keep the newest `LAST_COMMIT_CACHE_ROWS` rows of `cache_last_commit` and drop rows
/// older than 30 days whose tip is not in `keep_tips` (current ref tips). Returns rows deleted.
pub fn prune_caches(db: &Db, keep_tips: &[String]) -> Result<usize> {
    if db.read_only() {
        return Ok(0);
    }
    let keep: HashSet<&str> = keep_tips.iter().map(String::as_str).collect();
    let cutoff = crate::now_secs() - CACHE_MAX_AGE_SECS;
    db.tx(|tx| {
        let mut deleted = tx.execute(
            "DELETE FROM cache_last_commit WHERE rowid NOT IN
               (SELECT rowid FROM cache_last_commit ORDER BY created_at DESC, rowid DESC LIMIT ?1)",
            [LAST_COMMIT_CACHE_ROWS],
        )?;
        let old: Vec<(i64, String)> = {
            let mut st = tx.prepare("SELECT rowid, tip FROM cache_last_commit WHERE created_at < ?1")?;
            st.query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?
        };
        for (rowid, tip) in old {
            if !keep.contains(tip.as_str()) {
                deleted += tx.execute("DELETE FROM cache_last_commit WHERE rowid = ?1", [rowid])?;
            }
        }
        Ok(deleted)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(db: &Db) -> Vec<String> {
        db.with(|c| {
            let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
            Ok(st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
        })
        .unwrap()
    }

    fn insert_repo(db: &Db) -> i64 {
        db.with(|c| {
            c.execute(
                "INSERT INTO repos (slug_owner, slug_name, source, path, is_bare, added_at) VALUES ('local','r','imported','C:\\r',0,1)",
                [],
            )?;
            Ok(c.last_insert_rowid())
        })
        .unwrap()
    }

    #[test]
    fn fresh_open_runs_migrations() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        let db = Db::open(&f).unwrap();
        assert!(!db.read_only());
        assert_eq!(db.with(user_version).unwrap(), SCHEMA_VERSION);
        let names = tables(&db);
        for t in ["repos", "repo_meta", "repo_topics", "settings", "cache_commit_count", "cache_last_commit", "cache_languages", "cache_gh_releases", "jobs"] {
            assert!(names.iter().any(|n| n == t), "missing {t}: {names:?}");
        }
        let mode: String = db.with(|c| Ok(c.query_row("PRAGMA journal_mode", [], |r| r.get(0))?)).unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        let fk: i64 = db.with(|c| Ok(c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?)).unwrap();
        assert_eq!(fk, 1);
        // cloned_at column exists (D-19)
        db.with(|c| Ok(c.execute("UPDATE repos SET cloned_at = 1", [])?)).unwrap();
    }

    /// v0.2.0 (D-99): a DB written by v0.1.0 (0001 only, rows in every table) migrates in place and keeps them.
    #[test]
    fn v010_db_with_rows_migrates_and_keeps_everything() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        {
            let c = Connection::open(&f).unwrap();
            c.execute_batch(include_str!("migrations/0001_init.sql")).unwrap();
            c.pragma_update(None, "user_version", 1).unwrap();
            c.execute_batch(
                "INSERT INTO repos (id, slug_owner, slug_name, source, path, is_bare, origin_url, origin_host, added_at)
                   VALUES (1, 'o', 'r', 'cloned', 'C:\\a\\r.git', 1, 'https://github.com/o/r.git', 'github.com', 5);
                 INSERT INTO repo_meta (repo_id, description, homepage, updated_at) VALUES (1, NULL, 'https://h.example', 6);
                 INSERT INTO repo_topics (repo_id, topic, pos) VALUES (1, 'mine', 0);
                 INSERT INTO settings (key, value) VALUES ('theme', '\"dark\"');
                 INSERT INTO cache_commit_count (repo_id, tip, count) VALUES (1, 't', 3);
                 INSERT INTO cache_last_commit (repo_id, tip, dir, json, created_at) VALUES (1, 't', '', '{}', 7);
                 INSERT INTO cache_languages (repo_id, tree, json) VALUES (1, 'tr', '{}');
                 INSERT INTO cache_gh_releases (repo_id, fetched_at, json) VALUES (1, 8, '[]');
                 INSERT INTO jobs (kind_json, title, state, created_at) VALUES ('{\"type\":\"fetch\",\"repoId\":1}', 'Fetch o/r', 'done', 9);",
            )
            .unwrap();
        }
        let db = Db::open(&f).unwrap();
        assert!(!db.read_only());
        assert_eq!(db.with(user_version).unwrap(), SCHEMA_VERSION);
        assert_eq!(SCHEMA_VERSION, 3);
        let counts: Vec<i64> = db
            .with(|c| {
                let mut v = Vec::new();
                for t in ["repos", "repo_meta", "repo_topics", "settings", "cache_commit_count", "cache_last_commit", "cache_languages", "cache_gh_releases", "jobs"] {
                    v.push(c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?);
                }
                Ok(v)
            })
            .unwrap();
        assert_eq!(counts, vec![1; 9]);
        let (etag, json): (Option<String>, String) =
            db.with(|c| Ok(c.query_row("SELECT etag, json FROM cache_gh_releases", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
        assert_eq!((etag, json.as_str()), (None, "[]"));
        assert!(tables(&db).iter().any(|n| n == "gh_repo_info"));
        // the new table cascades with its repo
        db.with(|c| Ok(c.execute("INSERT INTO gh_repo_info (repo_id, fetched_at) VALUES (1, 1)", [])?)).unwrap();
        db.with(|c| Ok(c.execute("DELETE FROM repos", [])?)).unwrap();
        let left: i64 = db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM gh_repo_info", [], |r| r.get(0))?)).unwrap();
        assert_eq!(left, 0);
        drop(db);
        assert_eq!(Db::open(&f).unwrap().with(user_version).unwrap(), SCHEMA_VERSION, "reopen is a no-op");
    }

    /// v0.3.0 (D-118): a DB written by v0.2.0 (0001 + 0002, rows in every table incl. `gh_repo_info`) migrates in
    /// place, keeps every row and gets the empty Explore tables.
    #[test]
    fn v020_db_with_rows_migrates_and_keeps_everything() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        {
            let c = Connection::open(&f).unwrap();
            c.execute_batch(include_str!("migrations/0001_init.sql")).unwrap();
            c.execute_batch(include_str!("migrations/0002_github.sql")).unwrap();
            c.pragma_update(None, "user_version", 2).unwrap();
            c.execute_batch(
                "INSERT INTO repos (id, slug_owner, slug_name, source, path, is_bare, origin_url, origin_host, added_at)
                   VALUES (1, 'o', 'r', 'cloned', 'C:\\a\\r.git', 1, 'https://github.com/o/r.git', 'github.com', 5);
                 INSERT INTO repo_meta (repo_id, description, homepage, updated_at) VALUES (1, 'mine', 'https://h.example', 6);
                 INSERT INTO repo_topics (repo_id, topic, pos) VALUES (1, 'mine', 0);
                 INSERT INTO settings (key, value) VALUES ('theme', '\"dark\"'), ('loadRemoteImages', 'false');
                 INSERT INTO cache_commit_count (repo_id, tip, count) VALUES (1, 't', 3);
                 INSERT INTO cache_last_commit (repo_id, tip, dir, json, created_at) VALUES (1, 't', '', '{}', 7);
                 INSERT INTO cache_languages (repo_id, tree, json) VALUES (1, 'tr', '{}');
                 INSERT INTO cache_gh_releases (repo_id, fetched_at, json, etag) VALUES (1, 8, '[]', '\"e\"');
                 INSERT INTO gh_repo_info (repo_id, etag, fetched_at, description, topics_json, html_url)
                   VALUES (1, '\"i\"', 9, 'from github', '[\"cli\"]', 'https://github.com/o/r');
                 INSERT INTO jobs (kind_json, title, state, created_at) VALUES ('{\"type\":\"fetch\",\"repoId\":1}', 'Fetch o/r', 'done', 9);",
            )
            .unwrap();
        }
        let db = Db::open(&f).unwrap();
        assert!(!db.read_only());
        assert_eq!(db.with(user_version).unwrap(), 3);
        let counts: Vec<i64> = db
            .with(|c| {
                let mut v = Vec::new();
                for t in [
                    "repos", "repo_meta", "repo_topics", "cache_commit_count", "cache_last_commit", "cache_languages", "cache_gh_releases",
                    "gh_repo_info", "jobs",
                ] {
                    v.push(c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?);
                }
                v.push(c.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))?);
                Ok(v)
            })
            .unwrap();
        assert_eq!(counts, vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 2]);
        let (desc, etag): (String, String) = db
            .with(|c| Ok(c.query_row("SELECT description, etag FROM gh_repo_info", [], |r| Ok((r.get(0)?, r.get(1)?)))?))
            .unwrap();
        assert_eq!((desc.as_str(), etag.as_str()), ("from github", "\"i\""));
        // the v0.2.0 settings still load, the new keys take their defaults
        let s = crate::settings::load(&db).unwrap();
        assert_eq!(s.theme, crate::types::Theme::Dark);
        assert!(!s.load_remote_images);
        assert!(s.explore_enabled);
        // the Explore tables are new and empty, and work
        for t in ["cache_gh_api", "gh_quota"] {
            assert!(tables(&db).iter().any(|n| n == t), "{t}");
            let n: i64 = db.with(|c| Ok(c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))?)).unwrap();
            assert_eq!(n, 0, "{t}");
        }
        db.with(|c| {
            c.execute("INSERT INTO cache_gh_api (url, body, size, fetched_at, last_used_at) VALUES ('https://api.github.com/x', x'7b7d', 2, 1, 1)", [])?;
            c.execute("INSERT INTO gh_quota (resource, quota, remaining, reset_at, seen_at) VALUES ('core', 60, 59, 2, 1)", [])?;
            Ok(())
        })
        .unwrap();
        // existing data is not touched by "Clear cache" tables of the Explore side and vice versa
        db.with(|c| Ok(c.execute("DELETE FROM cache_gh_api", [])?)).unwrap();
        let left: i64 = db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM gh_repo_info", [], |r| r.get(0))?)).unwrap();
        assert_eq!(left, 1);
        drop(db);
        assert_eq!(Db::open(&f).unwrap().with(user_version).unwrap(), 3, "reopen is a no-op");
    }

    #[test]
    fn reopen_does_not_rerun_migrations() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        {
            let db = Db::open(&f).unwrap();
            insert_repo(&db);
        }
        let db = Db::open(&f).unwrap(); // would fail with "table repos already exists" if re-run
        assert_eq!(db.with(user_version).unwrap(), SCHEMA_VERSION);
        let n: i64 = db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM repos", [], |r| r.get(0))?)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn future_version_opens_read_only() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        {
            let db = Db::open(&f).unwrap();
            db.with(|c| Ok(c.pragma_update(None, "user_version", SCHEMA_VERSION + 1)?)).unwrap();
        }
        let db = Db::open(&f).unwrap();
        assert!(db.read_only());
        assert_eq!(db.with(user_version).unwrap(), SCHEMA_VERSION + 1);
        let n: i64 = db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM repos", [], |r| r.get(0))?)).unwrap();
        assert_eq!(n, 0);
        assert!(db.with(|c| Ok(c.execute("DELETE FROM repos", [])?)).is_err(), "write must fail");
        assert_eq!(prune_caches(&db, &[]).unwrap(), 0);
    }

    #[test]
    fn tx_rolls_back_on_error() {
        let db = Db::open_in_memory().unwrap();
        let r: Result<()> = db.tx(|tx| {
            tx.execute("INSERT INTO settings (key, value) VALUES ('a', '1')", [])?;
            Err(crate::Error::Refused("stop".into()))
        });
        assert!(r.is_err());
        let n: i64 = db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))?)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn prune_keeps_newest_2000() {
        let db = Db::open_in_memory().unwrap();
        let repo = insert_repo(&db);
        let now = crate::now_secs();
        db.tx(|tx| {
            let mut st = tx.prepare("INSERT INTO cache_last_commit (repo_id, tip, dir, json, created_at) VALUES (?1, ?2, '', '{}', ?3)")?;
            for i in 0..2_100i64 {
                // all recent, so only the row cap applies
                st.execute(rusqlite::params![repo, format!("tip{i}"), now - 2_100 + i])?;
            }
            Ok(())
        })
        .unwrap();
        let deleted = prune_caches(&db, &[]).unwrap();
        assert_eq!(deleted, 100);
        let (n, min): (i64, i64) = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*), MIN(created_at) FROM cache_last_commit", [], |r| Ok((r.get(0)?, r.get(1)?)))?))
            .unwrap();
        assert_eq!(n, 2_000);
        assert_eq!(min, now - 2_000);
    }

    #[test]
    fn prune_drops_old_rows_unless_tip_is_current() {
        let db = Db::open_in_memory().unwrap();
        let repo = insert_repo(&db);
        let old = crate::now_secs() - CACHE_MAX_AGE_SECS - 10;
        db.with(|c| {
            for (tip, at) in [("old-gone", old), ("old-kept", old), ("fresh", crate::now_secs())] {
                c.execute(
                    "INSERT INTO cache_last_commit (repo_id, tip, dir, json, created_at) VALUES (?1, ?2, '', '{}', ?3)",
                    rusqlite::params![repo, tip, at],
                )?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(prune_caches(&db, &["old-kept".to_string()]).unwrap(), 1);
        let tips: Vec<String> = db
            .with(|c| {
                let mut st = c.prepare("SELECT tip FROM cache_last_commit ORDER BY tip")?;
                Ok(st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
            })
            .unwrap();
        assert_eq!(tips, vec!["fresh", "old-kept"]);
    }
}
