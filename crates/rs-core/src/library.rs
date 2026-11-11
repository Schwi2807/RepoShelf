//! Shelf management (M3.1; FR-1..5, FR-20, FR-36, L-12, NFR-O3, D-12, D-27, D-28).
//!
//! Imported repositories are never written: import/preview/refresh only open them read-only (no index, lock,
//! config or ref writes). Summary refreshes compute outside the DB mutex (read row → release → git → write row).

use crate::Shelf;
use crate::error::{Error, Result};
use crate::jobs::CancelToken;
use crate::paths;
use crate::repo;
use crate::types::{ImportPreview, RepoSummary, Removed};
use crate::validate::{CloneUrl, MetadataInput};
use git2::{ErrorCode, Repository, RepositoryOpenFlags};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

/// Every shelf repo from the DB only (no git; NFR-P1), in insertion order. Sorting/filtering is the UI's (D-27).
pub fn list(s: &Shelf) -> Result<Vec<RepoSummary>> {
    let ids: Vec<i64> = s.db.with(|c| {
        let mut st = c.prepare_cached("SELECT id FROM repos ORDER BY id")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    })?;
    ids.into_iter().map(|id| repo::summary(s, id)).collect()
}

/// Repo by its `owner/name` slug (exact match first, then case-insensitive).
pub fn by_slug(s: &Shelf, owner: &str, name: &str) -> Result<RepoSummary> {
    let id: Option<i64> = s.db.with(|c| {
        let exact = c
            .query_row("SELECT id FROM repos WHERE slug_owner = ?1 AND slug_name = ?2", params![owner, name], |r| r.get(0))
            .optional()?;
        if exact.is_some() {
            return Ok(exact);
        }
        Ok(c.query_row(
            "SELECT id FROM repos WHERE slug_owner = ?1 COLLATE NOCASE AND slug_name = ?2 COLLATE NOCASE ORDER BY id LIMIT 1",
            params![owner, name],
            |r| r.get(0),
        )
        .optional()?)
    })?;
    match id {
        Some(id) => repo::summary(s, id),
        None => Err(Error::not_found("repo")),
    }
}

/// The `n` most recently opened repos (D-28: "Recent" = `last_opened_at`).
pub fn recent(s: &Shelf, n: u32) -> Result<Vec<RepoSummary>> {
    let ids: Vec<i64> = s.db.with(|c| {
        let mut st = c.prepare_cached(
            "SELECT id FROM repos WHERE last_opened_at IS NOT NULL ORDER BY last_opened_at DESC, id DESC LIMIT ?1",
        )?;
        let v = st.query_map([n], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    })?;
    ids.into_iter().map(|id| repo::summary(s, id)).collect()
}

pub fn touch_opened(s: &Shelf, id: i64) -> Result<()> {
    let n = s.db.with(|c| Ok(c.execute("UPDATE repos SET last_opened_at = ?1 WHERE id = ?2", params![crate::now_secs(), id])?))?;
    if n == 0 { Err(Error::not_found("repo")) } else { Ok(()) }
}

// ---------------------------------------------------------------------------------------------------------------
// Slugs (D-12)

/// Slug-safe part: `[A-Za-z0-9._-]`, other characters → `-`; empty, `.` or `..` → None.
fn slug_part(s: &str) -> Option<String> {
    let mut out = String::new();
    for c in s.chars() {
        let c = if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' };
        if !(c == '-' && out.ends_with('-')) {
            out.push(c);
        }
    }
    let out = out.trim_matches('-').to_string();
    (!out.is_empty() && out != "." && out != "..").then_some(out)
}

/// Lenient `(owner, name)` from any origin form (https, ssh://, scp-like `git@host:o/r.git`); None for local paths.
pub fn origin_slug(url: &str) -> Option<(String, String)> {
    let u = url.trim();
    let path = if let Some(i) = u.find("://") {
        let rest = &u[i + 3..];
        if u[..i].eq_ignore_ascii_case("file") {
            return None;
        }
        rest.split_once('/')?.1
    } else {
        // scp-like `user@host:path`; a Windows drive (`C:\x`, `C:/x`) is not an origin with an owner.
        let (host, p) = u.split_once(':')?;
        if host.len() <= 1 || host.contains('/') || host.contains('\\') {
            return None;
        }
        p
    };
    let path = path.split(['?', '#']).next().unwrap_or("");
    let segs: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
    if segs.len() < 2 {
        return None;
    }
    let last = segs[segs.len() - 1];
    Some((slug_part(segs[0])?, slug_part(last.strip_suffix(".git").unwrap_or(last))?))
}

/// Host of an origin URL (https/ssh URL or scp-like form).
fn origin_host(url: &str) -> Option<String> {
    if let Ok(u) = url::Url::parse(url)
        && let Some(h) = u.host_str()
        && !h.is_empty()
    {
        return Some(h.to_string());
    }
    let (left, _) = url.split_once(':')?;
    let host = left.rsplit('@').next()?;
    (host.len() > 1 && !host.contains('/')).then(|| host.to_string())
}

/// First free `name`, `name-2`, `name-3`… for `owner` (case-insensitive), ignoring `except`.
fn unique_name(c: &Connection, owner: &str, base: &str) -> Result<String> {
    let mut name = base.to_string();
    let mut n = 2;
    loop {
        let taken: i64 = c.query_row(
            "SELECT COUNT(*) FROM repos WHERE slug_owner = ?1 COLLATE NOCASE AND slug_name = ?2 COLLATE NOCASE",
            params![owner, name],
            |r| r.get(0),
        )?;
        if taken == 0 {
            return Ok(name);
        }
        name = format!("{base}-{n}");
        n += 1;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Import (FR-5)

/// Opens a picked folder without searching parents. Returns the canonical repo root (work tree for non-bare,
/// the git dir for bare) and the repo. Anything else → `NotARepo` with a readable reason.
fn open_folder(dir: &Path) -> Result<(PathBuf, Repository)> {
    let canon = paths::canonical(dir).map_err(|_| Error::NotARepo(format!("{} does not exist", dir.display())))?;
    let r = Repository::open_ext(&canon, RepositoryOpenFlags::NO_SEARCH, [] as [&std::ffi::OsStr; 0]).map_err(|e| {
        let why = if e.code() == ErrorCode::Owner {
            format!("git refuses to open it because another user owns it ({})", e.message())
        } else {
            "pick the top folder of a git repository (the one that contains .git) or a bare repository".to_string()
        };
        Error::NotARepo(format!("{}: {why}", canon.display()))
    })?;
    let root = if r.is_bare() { r.path().to_path_buf() } else { r.workdir().map(Path::to_path_buf).unwrap_or_else(|| r.path().to_path_buf()) };
    Ok((paths::canonical(&root)?, r))
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn duplicate_of(c: &Connection, path: &str, except: Option<i64>) -> Result<Option<i64>> {
    Ok(c.query_row(
        "SELECT id FROM repos WHERE path = ?1 COLLATE NOCASE AND id != ?2",
        params![path, except.unwrap_or(-1)],
        |r| r.get(0),
    )
    .optional()?)
}

/// `(owner, name)` proposal for an imported repo: origin owner/name, else `local/<folder>`.
fn import_slug(root: &Path, origin: Option<&str>) -> (String, String) {
    if let Some(sl) = origin.and_then(origin_slug) {
        return sl;
    }
    let folder = root.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = folder.strip_suffix(".git").unwrap_or(&folder);
    ("local".into(), slug_part(folder).unwrap_or_else(|| "repo".into()))
}

/// What importing `dir` would do (FR-5). `token` is filled in by src-tauri (D-18).
pub fn preview_import(s: &Shelf, dir: &Path) -> Result<ImportPreview> {
    let (root, r) = open_folder(dir)?;
    let origin = repo::origin_url(&r);
    let (_, name) = import_slug(&root, origin.as_deref());
    let p = path_str(&root);
    let duplicate_of = s.db.with(|c| duplicate_of(c, &p, None))?;
    Ok(ImportPreview { path: p, name, is_bare: r.is_bare(), origin_url: origin, duplicate_of, token: String::new() })
}

/// Adds an existing repository folder (no writes to it). Duplicate canonical path → `Duplicate`.
pub fn import(s: &Shelf, dir: &Path) -> Result<RepoSummary> {
    let (root, r) = open_folder(dir)?;
    let origin = repo::origin_url(&r);
    let is_bare = r.is_bare();
    drop(r);
    let (owner, base) = import_slug(&root, origin.as_deref());
    let p = path_str(&root);
    let host = origin.as_deref().and_then(origin_host);
    let id = s.db.tx(|tx| {
        if duplicate_of(tx, &p, None)?.is_some() {
            return Err(Error::Duplicate(p.clone()));
        }
        let name = unique_name(tx, &owner, &base)?;
        tx.execute(
            "INSERT INTO repos (slug_owner, slug_name, source, path, is_bare, origin_url, origin_host, added_at)
             VALUES (?1, ?2, 'imported', ?3, ?4, ?5, ?6, ?7)",
            params![owner, name, p, is_bare, origin, host, crate::now_secs()],
        )?;
        Ok(tx.last_insert_rowid())
    })?;
    refresh_one(s, id, &CancelToken::new())
}

/// Registers a finished managed clone (called by `clone::job`). Slug = URL owner/name (`-2` on collision).
pub fn register_clone(s: &Shelf, path: &Path, url: &CloneUrl) -> Result<RepoSummary> {
    let p = path_str(&paths::canonical(path)?);
    let owner = slug_part(&url.owner).unwrap_or_else(|| "local".into());
    let base = slug_part(&url.name).unwrap_or_else(|| "repo".into());
    let now = crate::now_secs();
    let id = s.db.tx(|tx| {
        if duplicate_of(tx, &p, None)?.is_some() {
            return Err(Error::Duplicate(p.clone()));
        }
        let name = unique_name(tx, &owner, &base)?;
        tx.execute(
            "INSERT INTO repos (slug_owner, slug_name, source, path, is_bare, origin_url, origin_host, added_at, cloned_at)
             VALUES (?1, ?2, 'cloned', ?3, 1, ?4, ?5, ?6, ?6)",
            params![owner, name, p, url.url, url.host, now],
        )?;
        Ok(tx.last_insert_rowid())
    })?;
    refresh_one(s, id, &CancelToken::new())
}

/// FR-4 "Locate…": point an existing entry at a new folder (validated like an import).
pub fn relocate(s: &Shelf, id: i64, dir: &Path) -> Result<RepoSummary> {
    repo::summary(s, id)?;
    let (root, r) = open_folder(dir)?;
    let origin = repo::origin_url(&r);
    let is_bare = r.is_bare();
    drop(r);
    let p = path_str(&root);
    let host = origin.as_deref().and_then(origin_host);
    s.db.tx(|tx| {
        if duplicate_of(tx, &p, Some(id))?.is_some() {
            return Err(Error::Duplicate(p.clone()));
        }
        tx.execute(
            "UPDATE repos SET path = ?1, is_bare = ?2, origin_url = ?3, origin_host = ?4, head_tip = NULL,
                              status = 'ok', status_message = NULL WHERE id = ?5",
            params![p, is_bare, origin, host, id],
        )?;
        Ok(())
    })?;
    refresh_one(s, id, &CancelToken::new())
}

/// FR-20: user-edited description, homepage, topics (validated). Stored in the DB only.
pub fn set_metadata(s: &Shelf, id: i64, m: MetadataInput) -> Result<RepoSummary> {
    let m = m.validate()?;
    repo::summary(s, id)?;
    s.db.tx(|tx| {
        tx.execute(
            "INSERT INTO repo_meta (repo_id, description, homepage, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(repo_id) DO UPDATE SET description = excluded.description, homepage = excluded.homepage,
                                                updated_at = excluded.updated_at",
            params![id, m.description, m.homepage, crate::now_secs()],
        )?;
        tx.execute("DELETE FROM repo_topics WHERE repo_id = ?1", [id])?;
        for (pos, t) in m.topics.iter().enumerate() {
            tx.execute("INSERT INTO repo_topics (repo_id, topic, pos) VALUES (?1, ?2, ?3)", params![id, t, pos as i64])?;
        }
        Ok(())
    })?;
    repo::summary(s, id)
}

/// FR-36: drops the entry (metadata, topics and caches cascade). Trashing a managed clone is src-tauri's job.
pub fn remove(s: &Shelf, id: i64) -> Result<Removed> {
    let sum = repo::summary(s, id)?;
    s.db.with(|c| Ok(c.execute("DELETE FROM repos WHERE id = ?1", [id])?))?;
    Ok(Removed { path: sum.path, source: sum.source })
}

// ---------------------------------------------------------------------------------------------------------------
// Summary refresh (FR-1, FR-4, NFR-O3, R-9)

enum Probe {
    Missing,
    Corrupt(String),
    Empty { default_branch: Option<String> },
    Ok { tip: String, last_commit_at: i64, default_branch: Option<String>, language: Option<String> },
}

fn message(e: Error) -> String {
    match e {
        Error::RepoCorrupt(m) => m,
        other => other.to_string(),
    }
}

fn probe(s: &Shelf, id: i64, path: &Path, known_tip: Option<&str>, known_lang: Option<String>, cancel: &CancelToken) -> Result<(Probe, Option<String>)> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok((Probe::Missing, None));
    }
    let r = match repo::open_path(path) {
        Ok(r) => r,
        Err(Error::RepoMissing(_)) => return Ok((Probe::Missing, None)),
        Err(e) => return Ok((Probe::Corrupt(message(e)), None)),
    };
    let origin = repo::origin_url(&r);
    let head = match r.head() {
        Ok(h) => h,
        Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => {
            return Ok((Probe::Empty { default_branch: crate::refs::head_branch(&r) }, origin));
        }
        Err(e) => return Ok((Probe::Corrupt(e.message().to_string()), origin)),
    };
    let commit = match head.peel_to_commit() {
        Ok(c) => c,
        Err(e) => return Ok((Probe::Corrupt(e.message().to_string()), origin)),
    };
    let tree = match commit.tree() {
        Ok(t) => t,
        Err(e) => return Ok((Probe::Corrupt(e.message().to_string()), origin)),
    };
    let tip = commit.id().to_string();
    let language = if known_tip == Some(tip.as_str()) && known_lang.is_some() {
        known_lang
    } else {
        match crate::langs::for_tree(s, id, &r, tree.id(), cancel) {
            Ok(l) => l.items.into_iter().map(|i| i.name).find(|n| n != crate::langs::OTHER),
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(e) => return Ok((Probe::Corrupt(message(e)), origin)),
        }
    };
    let probe = Probe::Ok { tip, last_commit_at: commit.time().seconds(), default_branch: crate::refs::head_branch(&r), language };
    Ok((probe, origin))
}

/// Re-reads one repo: exists? opens? HEAD tip changed → `last_commit_at`, `default_branch`, `primary_language`;
/// status ok/missing/corrupt/empty. Never fails for a broken repo (the status says why); `Cancelled` passes through.
pub fn refresh_one(s: &Shelf, id: i64, cancel: &CancelToken) -> Result<RepoSummary> {
    cancel.check()?;
    let (path, tip, lang): (String, Option<String>, Option<String>) = s.db.with(|c| {
        c.query_row("SELECT path, head_tip, primary_language FROM repos WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?
            .ok_or_else(|| Error::not_found("repo"))
    })?;
    let (p, origin) = probe(s, id, Path::new(&path), tip.as_deref(), lang, cancel)?;
    let host = origin.as_deref().and_then(origin_host);
    s.db.with(|c| {
        match p {
            Probe::Missing => c.execute("UPDATE repos SET status = 'missing', status_message = NULL WHERE id = ?1", [id])?,
            Probe::Corrupt(m) => c.execute("UPDATE repos SET status = 'corrupt', status_message = ?1 WHERE id = ?2", params![m, id])?,
            Probe::Empty { default_branch } => c.execute(
                "UPDATE repos SET status = 'empty', status_message = NULL, head_tip = NULL, last_commit_at = NULL,
                        primary_language = NULL, default_branch = COALESCE(?1, default_branch), origin_url = ?2, origin_host = ?3
                 WHERE id = ?4",
                params![default_branch, origin, host, id],
            )?,
            Probe::Ok { tip, last_commit_at, default_branch, language } => c.execute(
                "UPDATE repos SET status = 'ok', status_message = NULL, head_tip = ?1, last_commit_at = ?2,
                        default_branch = COALESCE(?3, default_branch), primary_language = ?4, origin_url = ?5, origin_host = ?6
                 WHERE id = ?7",
                params![tip, last_commit_at, default_branch, language, origin, host, id],
            )?,
        };
        Ok(())
    })?;
    repo::summary(s, id)
}

/// Refreshes every repo (background on start and on `library_refresh`; cancellable between and inside repos).
pub fn refresh_summaries(s: &Shelf, cancel: &CancelToken) -> Result<Vec<RepoSummary>> {
    let ids: Vec<i64> = s.db.with(|c| {
        let mut st = c.prepare_cached("SELECT id FROM repos ORDER BY id")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    })?;
    for id in ids {
        cancel.check()?;
        match refresh_one(s, id, cancel) {
            Ok(_) | Err(Error::NotFound { .. }) => {}
            Err(e) => return Err(e),
        }
    }
    list(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Dirs;
    use crate::testkit::{self, T0};
    use crate::types::{RepoStatus, Source};

    fn import_err(s: &Shelf, p: &Path) -> Error {
        import(s, p).unwrap_err()
    }

    #[test]
    fn import_non_bare_and_bare() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let fx = testkit::basic(&t.path().join("basic"));
        let pv = preview_import(&s, &fx.path).unwrap();
        assert!(!pv.is_bare);
        assert_eq!(pv.name, "basic");
        assert_eq!(pv.origin_url.as_deref(), Some("https://github.com/example/basic.git"));
        assert_eq!(pv.duplicate_of, None);
        assert!(pv.token.is_empty());
        let sum = import(&s, &fx.path).unwrap();
        assert_eq!(sum.source, Source::Imported);
        assert_eq!((sum.owner.as_str(), sum.name.as_str()), ("example", "basic"));
        assert_eq!(sum.path, path_str(&paths::canonical(&fx.path).unwrap()));
        assert_eq!(sum.status, RepoStatus::Ok);
        assert_eq!(sum.default_branch.as_deref(), Some("main"));
        assert_eq!(sum.primary_language.as_deref(), Some("Rust"));
        assert_eq!(sum.last_commit_at, Some(T0 + 3600));
        assert_eq!(sum.origin_host.as_deref(), Some("github.com"));
        assert_eq!(sum.cloned_at, None);

        let mut b = testkit::repo(&t.path().join("tool.git"));
        b.bare().file("a.ts", b"export {};\n").commit("one", T0).set_head();
        let sum = import(&s, &t.path().join("tool.git")).unwrap();
        assert_eq!((sum.owner.as_str(), sum.name.as_str()), ("local", "tool"));
        assert_eq!(by_slug(&s, "local", "tool").unwrap().id, sum.id);
        assert_eq!(by_slug(&s, "LOCAL", "Tool").unwrap().id, sum.id);
        assert_eq!(by_slug(&s, "local", "nope").unwrap_err().code(), "notFound");
        assert_eq!(list(&s).unwrap().len(), 2);
        // picking the .git folder of a work tree imports the work tree (duplicate here)
        assert_eq!(preview_import(&s, &fx.path.join(".git")).unwrap().duplicate_of, Some(list(&s).unwrap()[0].id));
    }

    #[test]
    fn duplicates_by_canonical_path() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let fx = testkit::basic(&t.path().join("Basic"));
        let first = import(&s, &fx.path).unwrap();
        let variants = [
            PathBuf::from(format!("{}{}", fx.path.display(), std::path::MAIN_SEPARATOR)),
            fx.path.join("src").join(".."),
            PathBuf::from(fx.path.display().to_string().to_lowercase()),
            PathBuf::from(fx.path.display().to_string().to_uppercase()),
        ];
        for v in variants {
            if !v.exists() {
                continue; // case variants only exist on case-insensitive filesystems
            }
            assert_eq!(preview_import(&s, &v).unwrap().duplicate_of, Some(first.id), "{}", v.display());
            let e = import_err(&s, &v);
            assert_eq!(e.code(), "duplicate", "{}: {e}", v.display());
        }
        assert_eq!(list(&s).unwrap().len(), 1);
    }

    #[test]
    fn non_repo_refused_with_message() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let plain = t.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let e = import_err(&s, &plain);
        assert_eq!(e.code(), "notARepo");
        assert!(e.to_string().contains("top folder of a git repository"), "{e}");
        assert_eq!(preview_import(&s, &plain).unwrap_err().code(), "notARepo");
        assert_eq!(import_err(&s, &t.path().join("absent")).code(), "notARepo");
        // a subfolder of a work tree is not searched upwards
        let fx = testkit::basic(&t.path().join("basic"));
        assert_eq!(import_err(&s, &fx.path.join("src")).code(), "notARepo");
        assert!(list(&s).unwrap().is_empty());
    }

    #[test]
    fn library_calls_leave_repo_files_untouched() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let fx = testkit::basic(&t.path().join("basic"));
        let before = testkit::snapshot(&fx.path);
        assert!(before.keys().any(|p| p.ends_with("index")));
        let _ = preview_import(&s, &fx.path).unwrap();
        let sum = import(&s, &fx.path).unwrap();
        let _ = import(&s, &fx.path).unwrap_err();
        refresh_summaries(&s, &CancelToken::new()).unwrap();
        refresh_one(&s, sum.id, &CancelToken::new()).unwrap();
        touch_opened(&s, sum.id).unwrap();
        set_metadata(&s, sum.id, MetadataInput { description: Some("d".into()), homepage: None, topics: vec!["x".into()] }).unwrap();
        relocate(&s, sum.id, &fx.path).unwrap();
        let _ = list(&s).unwrap();
        let _ = by_slug(&s, &sum.owner, &sum.name).unwrap();
        let _ = recent(&s, 5).unwrap();
        let after = testkit::snapshot(&fx.path);
        assert_eq!(testkit::snapshot_diff(&before, &after), Vec::<String>::new());
    }

    #[test]
    fn slug_collision_gets_dash_two() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let a = testkit::basic(&t.path().join("a").join("basic"));
        let b = testkit::basic(&t.path().join("b").join("basic"));
        let c = testkit::basic(&t.path().join("c").join("basic"));
        let names: Vec<String> = [a, b, c].iter().map(|fx| import(&s, &fx.path).unwrap().name).collect();
        assert_eq!(names, ["basic", "basic-2", "basic-3"]);
        // origin owner differs → no collision
        let mut d = testkit::repo(&t.path().join("d").join("basic"));
        d.file("x", b"1").commit("one", T0).set_head().origin("git@gitlab.com:other/basic.git");
        let sum = import(&s, &t.path().join("d").join("basic")).unwrap();
        assert_eq!((sum.owner.as_str(), sum.name.as_str()), ("other", "basic"));
        assert_eq!(sum.origin_host.as_deref(), Some("gitlab.com"));
    }

    #[test]
    fn origin_slug_forms() {
        let o = |u: &str| origin_slug(u).map(|(a, b)| format!("{a}/{b}"));
        assert_eq!(o("https://github.com/o/r.git").as_deref(), Some("o/r"));
        assert_eq!(o("https://gitlab.com/g/sub/r").as_deref(), Some("g/r"));
        assert_eq!(o("git@github.com:o/r.git").as_deref(), Some("o/r"));
        assert_eq!(o("ssh://git@host:22/o/r.git").as_deref(), Some("o/r"));
        assert_eq!(o("https://h.example/my org/my repo.git").as_deref(), Some("my-org/my-repo"));
        for none in ["file:///C:/x/y", "C:\\x\\y", "C:/x/y", "../other", "/srv/git/r.git", "https://github.com/onlyone", ""] {
            assert_eq!(o(none), None, "{none}");
        }
        assert_eq!(origin_host("git@github.com:o/r.git").as_deref(), Some("github.com"));
        assert_eq!(origin_host("https://codeberg.org/o/r").as_deref(), Some("codeberg.org"));
    }

    #[test]
    fn metadata_persists_across_reopen_and_is_validated() {
        let t = tempfile::tempdir().unwrap();
        let dirs = Dirs { data: t.path().join("data"), archive: t.path().join("archive") };
        let fx = testkit::basic(&t.path().join("basic"));
        let id = {
            let s = Shelf::open(dirs.clone()).unwrap();
            let id = import(&s, &fx.path).unwrap().id;
            let m = MetadataInput {
                description: Some("  A shelf test  ".into()),
                homepage: Some("https://example.com/x".into()),
                topics: vec!["Rust".into(), "cli".into(), "rust".into()],
            };
            let sum = set_metadata(&s, id, m).unwrap();
            assert_eq!(sum.topics, ["rust", "cli"]);
            for bad in [
                MetadataInput { topics: vec!["bad topic".into()], ..Default::default() },
                MetadataInput { homepage: Some("javascript:alert(1)".into()), ..Default::default() },
                MetadataInput { homepage: Some("https://u:p@example.com".into()), ..Default::default() },
                MetadataInput { description: Some("x".repeat(351)), ..Default::default() },
            ] {
                assert_eq!(set_metadata(&s, id, bad).unwrap_err().code(), "invalidInput");
            }
            assert_eq!(set_metadata(&s, 999, MetadataInput::default()).unwrap_err().code(), "notFound");
            id
        };
        let s = Shelf::open(dirs).unwrap();
        let sum = repo::summary(&s, id).unwrap();
        assert_eq!(sum.description.as_deref(), Some("A shelf test"));
        assert_eq!(sum.homepage.as_deref(), Some("https://example.com/x"));
        assert_eq!(sum.topics, ["rust", "cli"]);
    }

    #[test]
    fn missing_corrupt_and_empty_status() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let m = testkit::missing(&t.path().join("gone"));
        let mid = import(&s, &m).unwrap().id;
        std::fs::rename(&m, t.path().join("gone.moved")).unwrap();
        let (cp, _) = testkit::corrupt(&t.path().join("corrupt"));
        let cid = import(&s, &cp).unwrap().id;
        let mut e = testkit::repo(&t.path().join("empty"));
        e.git();
        let eid = import(&s, &t.path().join("empty")).unwrap().id;

        let all = refresh_summaries(&s, &CancelToken::new()).unwrap();
        let st = |id: i64| all.iter().find(|r| r.id == id).unwrap().status.clone();
        assert_eq!(st(mid), RepoStatus::Missing);
        match st(cid) {
            RepoStatus::Corrupt { message } => assert!(!message.is_empty()),
            other => panic!("{other:?}"),
        }
        assert_eq!(st(eid), RepoStatus::Empty);
        assert_eq!(repo::summary(&s, eid).unwrap().default_branch.as_deref(), Some("main"));

        // relocate the missing one → ok again
        let sum = relocate(&s, mid, &t.path().join("gone.moved")).unwrap();
        assert_eq!(sum.status, RepoStatus::Ok);
        assert!(sum.path.ends_with("gone.moved"));
        // relocating onto a folder another entry already uses → duplicate
        assert_eq!(relocate(&s, mid, &cp).unwrap_err().code(), "duplicate");
        assert_eq!(relocate(&s, mid, &t.path().join("nothing")).unwrap_err().code(), "notARepo");
        // cancelled refresh
        let c = CancelToken::new();
        c.cancel();
        assert_eq!(refresh_summaries(&s, &c).unwrap_err().code(), "cancelled");
    }

    #[test]
    fn tip_change_updates_summary() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a.py", b"print(1)\n").commit("one", T0).set_head();
        let id = import(&s, &t.path().join("r")).unwrap().id;
        assert_eq!(repo::summary(&s, id).unwrap().primary_language.as_deref(), Some("Python"));
        b.file("b.go", &b"package main\n".repeat(50)).commit("two", T0 + 99);
        let sum = refresh_one(&s, id, &CancelToken::new()).unwrap();
        assert_eq!(sum.last_commit_at, Some(T0 + 99));
        assert_eq!(sum.primary_language.as_deref(), Some("Go"));
    }

    #[test]
    fn remove_touch_recent() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let a = import(&s, &testkit::missing(&t.path().join("a"))).unwrap();
        let b = import(&s, &testkit::missing(&t.path().join("b"))).unwrap();
        set_metadata(&s, a.id, MetadataInput { topics: vec!["t".into()], ..Default::default() }).unwrap();
        assert!(recent(&s, 5).unwrap().is_empty());
        touch_opened(&s, a.id).unwrap();
        touch_opened(&s, b.id).unwrap();
        s.db.with(|c| Ok(c.execute("UPDATE repos SET last_opened_at = last_opened_at + 10 WHERE id = ?1", [b.id])?)).unwrap();
        let r: Vec<i64> = recent(&s, 5).unwrap().iter().map(|x| x.id).collect();
        assert_eq!(r, [b.id, a.id]);
        assert_eq!(recent(&s, 1).unwrap().len(), 1);
        assert_eq!(touch_opened(&s, 999).unwrap_err().code(), "notFound");
        let rm = remove(&s, a.id).unwrap();
        assert_eq!(rm, Removed { path: a.path.clone(), source: Source::Imported });
        let topics: i64 = s.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM repo_topics", [], |r| r.get(0))?)).unwrap();
        assert_eq!(topics, 0, "cascade");
        assert!(Path::new(&a.path).exists(), "imported folders are never deleted");
        assert_eq!(remove(&s, a.id).unwrap_err().code(), "notFound");
        assert_eq!(list(&s).unwrap().len(), 1);
    }
}
