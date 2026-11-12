//! Open a shelf repo by id; detail and dirty note (M2.1; FR-4, FR-12, FR-18, NFR-O3, D-16).
//!
//! Paths come from the DB only (never from the UI). Imported repos are read-only by convention: nothing here
//! writes the index, a lock, the config or a ref.

use crate::Shelf;
use crate::error::{Error, Result};
use crate::refs::{HEADS, TAGS};
use crate::types::{HeadState, RepoDetail, RepoStatus, RepoSummary, Source};
use git2::{ErrorCode, Repository, RepositoryOpenFlags, StatusOptions};
use rusqlite::OptionalExtension;
use std::path::{Path, PathBuf};

fn status_of(s: &str, message: Option<String>) -> RepoStatus {
    match s {
        "missing" => RepoStatus::Missing,
        "corrupt" => RepoStatus::Corrupt { message: message.unwrap_or_default() },
        "empty" => RepoStatus::Empty,
        _ => RepoStatus::Ok,
    }
}

/// The `repos` row (+ metadata, topics) as a `RepoSummary`; `NotFound{what:"repo"}` for unknown ids.
pub fn summary(s: &Shelf, id: i64) -> Result<RepoSummary> {
    s.db.with(|c| {
        let mut st = c.prepare_cached(
            "SELECT r.id, r.slug_owner, r.slug_name, r.source, r.path, m.description, m.homepage, r.origin_url,
                    r.origin_host, r.default_branch, r.primary_language, r.last_commit_at, r.added_at, r.cloned_at,
                    r.last_opened_at, r.last_fetched_at, r.status, r.status_message
             FROM repos r LEFT JOIN repo_meta m ON m.repo_id = r.id WHERE r.id = ?1",
        )?;
        let mut rows = st.query([id])?;
        let Some(r) = rows.next()? else { return Err(Error::not_found("repo")) };
        let source: String = r.get(3)?;
        let status: String = r.get(16)?;
        let mut out = RepoSummary {
            id: r.get(0)?,
            owner: r.get(1)?,
            name: r.get(2)?,
            source: Source::from_db(&source).unwrap_or(Source::Imported),
            path: r.get(4)?,
            description: r.get(5)?,
            topics: Vec::new(),
            homepage: r.get(6)?,
            origin_url: r.get(7)?,
            origin_host: r.get(8)?,
            default_branch: r.get(9)?,
            primary_language: r.get(10)?,
            last_commit_at: r.get(11)?,
            added_at: r.get(12)?,
            cloned_at: r.get(13)?,
            last_opened_at: r.get(14)?,
            last_fetched_at: r.get(15)?,
            status: status_of(&status, r.get(17)?),
            about_user: Default::default(),
            about_github: None,
        };
        let mut t = c.prepare_cached("SELECT topic FROM repo_topics WHERE repo_id = ?1 ORDER BY pos")?;
        out.topics = t.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        let mut g = c.prepare_cached("SELECT description, homepage, topics_json, html_url, fetched_at FROM gh_repo_info WHERE repo_id = ?1")?;
        out.about_github = g
            .query_row([id], |r| {
                let topics: String = r.get(2)?;
                Ok(crate::types::GithubAbout {
                    description: r.get(0)?,
                    homepage: r.get(1)?,
                    topics: serde_json::from_str(&topics).unwrap_or_default(),
                    html_url: r.get(3)?,
                    fetched_at: r.get(4)?,
                })
            })
            .optional()?;
        merge_about(&mut out);
        Ok(out)
    })
}

/// D-102: per field, the user's value wins when set (description / homepage non-empty, topics non-empty);
/// otherwise the fetched GitHub value. `out.description/homepage/topics` hold the user's values on entry.
fn merge_about(out: &mut RepoSummary) {
    let set = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.trim().is_empty());
    out.about_user = crate::types::AboutValues { description: out.description.clone(), homepage: out.homepage.clone(), topics: out.topics.clone() };
    let Some(gh) = &out.about_github else { return };
    if !set(&out.description) {
        out.description = gh.description.clone().filter(|d| !d.trim().is_empty());
    }
    if !set(&out.homepage) {
        out.homepage = gh.homepage.clone().filter(|h| !h.trim().is_empty());
    }
    if out.topics.is_empty() {
        out.topics = gh.topics.clone();
    }
}

/// Opens a repository folder without searching parents. Missing folder → `RepoMissing`; anything that is not a
/// readable repository → `RepoCorrupt`.
pub fn open_path(path: &Path) -> Result<Repository> {
    if !path.exists() {
        return Err(Error::RepoMissing(path.display().to_string()));
    }
    Repository::open_ext(path, RepositoryOpenFlags::NO_SEARCH, [] as [&std::ffi::OsStr; 0]).map_err(|e| Error::RepoCorrupt(e.message().to_string()))
}

/// DB lookup → open (ARCH §3.4).
pub fn open(s: &Shelf, id: i64) -> Result<(RepoSummary, Repository)> {
    let sum = summary(s, id)?;
    let repo = open_path(&PathBuf::from(&sum.path))?;
    Ok((sum, repo))
}

fn corrupt(e: git2::Error) -> Error {
    Error::RepoCorrupt(e.message().to_string())
}

/// HEAD as branch / detached / unborn.
pub fn head_state(repo: &Repository) -> Result<HeadState> {
    match repo.head() {
        Ok(h) if h.is_branch() => Ok(HeadState::Branch { name: String::from_utf8_lossy(&h.name_bytes()[HEADS.len()..]).into_owned() }),
        Ok(h) => match h.target() {
            Some(oid) => Ok(HeadState::Detached { oid: oid.to_string() }),
            None => Err(Error::RepoCorrupt("HEAD has no target".into())),
        },
        Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => {
            Ok(HeadState::Unborn { name: crate::refs::head_branch(repo).unwrap_or_else(|| "main".into()) })
        }
        Err(e) => Err(corrupt(e)),
    }
}

/// `origin` remote URL, if configured.
pub fn origin_url(repo: &Repository) -> Option<String> {
    let r = repo.find_remote("origin").ok()?;
    Some(String::from_utf8_lossy(r.url_bytes()).into_owned()).filter(|u| !u.is_empty())
}

/// Fetch needs an https origin that passes the clone URL rules (D-3, §5.4).
pub fn is_fetchable(origin: Option<&str>) -> bool {
    origin.is_some_and(|u| u.starts_with("https://") && crate::validate::clone_url(u).is_ok())
}

fn count_refs(repo: &Repository, prefix: &str, keep: impl Fn(&str) -> bool) -> Result<u32> {
    let mut n = 0;
    for r in repo.references_glob(&format!("{prefix}*")).map_err(corrupt)? {
        let r = r.map_err(corrupt)?;
        if r.kind() == Some(git2::ReferenceType::Symbolic) {
            continue;
        }
        if keep(&String::from_utf8_lossy(&r.name_bytes()[prefix.len()..])) {
            n += 1;
        }
    }
    Ok(n)
}

/// S2 header + About sidebar (FR-12, FR-18). A repo whose HEAD commit or tree cannot be read → `RepoCorrupt`.
pub fn detail(s: &Shelf, id: i64) -> Result<RepoDetail> {
    let (summary, repo) = open(s, id)?;
    let settings = crate::settings::load(&s.db)?;
    let head = head_state(&repo)?;
    let head_commit = match &head {
        HeadState::Unborn { .. } => None,
        _ => Some(repo.head().and_then(|h| h.peel_to_commit()).map_err(corrupt)?),
    };
    let license = match &head_commit {
        Some(c) => {
            let tree = c.tree().map_err(corrupt)?;
            crate::license::detect_license(&repo, &tree)
        }
        None => None,
    };
    let branch_count = count_refs(&repo, HEADS, |_| true)?;
    let tag_count = count_refs(&repo, TAGS, |_| true)?;
    let release_count = count_refs(&repo, TAGS, |n| crate::releases::is_version_tag(n).is_some())?;
    let fetchable = is_fetchable(origin_url(&repo).as_deref());
    let can_fast_forward = match summary.source {
        Source::Cloned => fast_forward_possible(&repo),
        Source::Imported => None,
    };
    let dirty = if settings.show_dirty_note && !repo.is_bare() { Some(dirty(&repo)?) } else { None };
    Ok(RepoDetail { is_bare: repo.is_bare(), head, branch_count, tag_count, release_count, license, fetchable, can_fast_forward, dirty, summary })
}

/// Managed clones: `Some(true)` when `origin/<default>` is strictly ahead of the local default branch.
fn fast_forward_possible(repo: &Repository) -> Option<bool> {
    let d = crate::refs::head_branch(repo)?;
    let local = repo.refname_to_id(&format!("{HEADS}{d}")).ok()?;
    let remote = repo.refname_to_id(&format!("{}{d}", crate::refs::ORIGIN)).ok()?;
    Some(local != remote && repo.graph_descendant_of(remote, local).unwrap_or(false))
}

/// D-16: any change in the work tree or index (untracked included, ignored and submodules excluded). Never
/// refreshes or writes the index (`UPDATE_INDEX` off).
pub fn dirty(repo: &Repository) -> Result<bool> {
    if repo.is_bare() {
        return Ok(false);
    }
    let mut o = StatusOptions::new();
    o.include_untracked(true).recurse_untracked_dirs(false).include_ignored(false).exclude_submodules(true).update_index(false);
    Ok(!repo.statuses(Some(&mut o))?.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{self, Settings};
    use crate::testkit::{self, T0};

    #[test]
    fn summary_row_and_unknown_id() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let p = testkit::missing(&t.path().join("r"));
        let id = testkit::register_raw(&s, &p, Source::Imported).unwrap();
        s.db.with(|c| {
            c.execute("INSERT INTO repo_meta (repo_id, description, homepage, updated_at) VALUES (?1, 'd', 'https://x.example', 1)", [id])?;
            c.execute("INSERT INTO repo_topics (repo_id, topic, pos) VALUES (?1, 'zeta', 0), (?1, 'alpha', 1)", [id])?;
            c.execute("UPDATE repos SET status = 'corrupt', status_message = 'bad' WHERE id = ?1", [id])?;
            Ok(())
        })
        .unwrap();
        let sum = summary(&s, id).unwrap();
        assert_eq!((sum.name.as_str(), sum.owner.as_str(), sum.source), ("r", "local", Source::Imported));
        assert_eq!(sum.description.as_deref(), Some("d"));
        assert_eq!(sum.topics, ["zeta", "alpha"]);
        assert_eq!(sum.status, RepoStatus::Corrupt { message: "bad".into() });
        assert_eq!(summary(&s, 999).unwrap_err().code(), "notFound");
        assert_eq!(open(&s, 999).err().unwrap().code(), "notFound");
    }

    #[test]
    fn missing_and_corrupt() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        // registered, then renamed away
        let p = testkit::missing(&t.path().join("gone"));
        let id = testkit::register_raw(&s, &p, Source::Imported).unwrap();
        std::fs::rename(&p, t.path().join("gone.moved")).unwrap();
        assert_eq!(open(&s, id).err().unwrap().code(), "repoMissing");
        assert_eq!(detail(&s, id).unwrap_err().code(), "repoMissing");
        // one object deleted: opens, but detail cannot read the tree
        let (cp, tree) = testkit::corrupt(&t.path().join("corrupt"));
        let id = testkit::register_raw(&s, &cp, Source::Imported).unwrap();
        assert!(open(&s, id).is_ok());
        let e = detail(&s, id).unwrap_err();
        assert_eq!(e.code(), "repoCorrupt", "{e}");
        assert!(testkit::open(&cp).find_tree(tree).is_err());
        // a folder that is not a repository
        let plain = t.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let id = testkit::register_raw(&s, &plain, Source::Imported).unwrap();
        assert_eq!(open(&s, id).err().unwrap().code(), "repoCorrupt");
    }

    #[test]
    fn detail_of_basic_with_dirty_note() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let fx = testkit::basic(&t.path().join("basic"));
        let id = testkit::register_raw(&s, &fx.path, Source::Imported).unwrap();
        let d = detail(&s, id).unwrap();
        assert!(!d.is_bare);
        assert_eq!(d.head, HeadState::Branch { name: "main".into() });
        assert_eq!((d.branch_count, d.tag_count, d.release_count), (2, 1, 1));
        assert_eq!(d.license.as_deref(), Some("MIT"));
        assert!(d.fetchable);
        assert_eq!(d.can_fast_forward, None, "imported");
        assert_eq!(d.dirty, None, "note is off by default");
        settings::save(&s.db, &Settings { show_dirty_note: true, ..Default::default() }).unwrap();
        assert_eq!(detail(&s, id).unwrap().dirty, Some(false));
        std::fs::write(fx.path.join("src").join("main.rs"), b"changed\n").unwrap();
        assert_eq!(detail(&s, id).unwrap().dirty, Some(true));
    }

    #[test]
    fn bare_unborn_detached_and_fast_forward() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        settings::save(&s.db, &Settings { show_dirty_note: true, ..Default::default() }).unwrap();

        let mut b = testkit::repo(&t.path().join("c.git"));
        b.bare().file("a", b"1").commit("one", T0);
        let one = b.head();
        b.file("a", b"2").commit("two", T0 + 60);
        let two = b.head();
        b.git().reference("refs/heads/main", one, true, "rewind").unwrap();
        b.git().reference("refs/remotes/origin/main", two, true, "remote").unwrap();
        b.origin("http://insecure.example.com/o/c.git");
        let id = testkit::register_raw(&s, &t.path().join("c.git"), Source::Cloned).unwrap();
        let d = detail(&s, id).unwrap();
        assert!(d.is_bare);
        assert_eq!(d.dirty, None, "bare repos have no dirty note");
        assert_eq!(d.can_fast_forward, Some(true));
        assert!(!d.fetchable, "http origin");

        // unborn HEAD
        let mut u = testkit::repo(&t.path().join("empty"));
        u.git();
        let id = testkit::register_raw(&s, &t.path().join("empty"), Source::Imported).unwrap();
        let d = detail(&s, id).unwrap();
        assert_eq!(d.head, HeadState::Unborn { name: "main".into() });
        assert_eq!((d.branch_count, d.tag_count, d.license), (0, 0, None));
        assert_eq!(d.dirty, Some(false));

        // detached HEAD
        let fx = testkit::merges(&t.path().join("m"));
        testkit::open(&fx.path).set_head_detached(fx.c2).unwrap();
        let id = testkit::register_raw(&s, &fx.path, Source::Imported).unwrap();
        let d = detail(&s, id).unwrap();
        assert_eq!(d.head, HeadState::Detached { oid: fx.c2.to_string() });
        assert!(!d.fetchable, "no origin");
    }
}
