//! Optional GitHub release notes (M3.8; feature `ghreleases`, D-22; FR-31 optional, NFR-O1).
//!
//! One user-initiated GET to `api.github.com` for github.com origins only. The raw JSON is cached in
//! `cache_gh_releases`; bodies go through the one markdown sanitiser (`markdown::render`) when merged.

use crate::Shelf;
use crate::error::{Error, Result};
use crate::repo;
use crate::types::Release;
use crate::validate::{self, GitPath};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use std::time::Duration;

pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const USER_AGENT: &str = "RepoShelf";

/// The fields we use from `GET /repos/{o}/{r}/releases`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
}

/// Parses the API JSON (drafts dropped). Bad JSON → `InvalidInput`.
pub fn parse_releases(json: &str) -> Result<Vec<GhRelease>> {
    let v: Vec<GhRelease> = serde_json::from_str(json).map_err(|e| Error::InvalidInput(format!("Unexpected GitHub answer: {e}")))?;
    Ok(v.into_iter().filter(|r| !r.draft).collect())
}

/// Merges GitHub notes into the tag-based release cards by tag name: a non-empty body replaces the notes
/// (sanitised, `gh_notes = true`); a non-empty release name replaces the title. Tags GitHub does not know stay.
pub fn merge_into(mut releases: Vec<Release>, gh: &[GhRelease]) -> Vec<Release> {
    for r in &mut releases {
        let Some(g) = gh.iter().find(|g| g.tag_name == r.tag) else { continue };
        if let Some(n) = g.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            r.title = n.to_string();
        }
        if let Some(b) = g.body.as_deref().filter(|b| !b.trim().is_empty()) {
            r.notes_html = Some(crate::markdown::render(b, &GitPath::root()).html);
            r.gh_notes = true;
        }
    }
    releases
}

/// `https://api.github.com/repos/<owner>/<name>/releases?per_page=100` for a github.com origin, else `Refused`.
pub fn api_url(origin: Option<&str>) -> Result<String> {
    let refused = || Error::Refused("Release notes can only be fetched for repositories hosted on github.com".into());
    let u = origin.filter(|o| o.starts_with("https://")).and_then(|o| validate::clone_url(o).ok()).ok_or_else(refused)?;
    if !u.host.eq_ignore_ascii_case("github.com") || u.rel_dir.components().count() != 3 || u.url.matches('/').count() != 4 {
        return Err(refused());
    }
    Ok(format!("https://api.github.com/repos/{}/{}/releases?per_page=100", u.owner, u.name))
}

/// Stores the raw API JSON for a repo (validated by parsing first).
pub fn store(s: &Shelf, repo_id: i64, json: &str) -> Result<Vec<GhRelease>> {
    let parsed = parse_releases(json)?;
    s.db.with(|c| {
        c.execute(
            "INSERT INTO cache_gh_releases (repo_id, fetched_at, json) VALUES (?1, ?2, ?3)
             ON CONFLICT(repo_id) DO UPDATE SET fetched_at = excluded.fetched_at, json = excluded.json",
            params![repo_id, crate::now_secs(), json],
        )?;
        Ok(())
    })?;
    Ok(parsed)
}

/// Cached notes, if any were fetched before.
pub fn cached(s: &Shelf, repo_id: i64) -> Result<Option<Vec<GhRelease>>> {
    let json: Option<String> =
        s.db.with(|c| Ok(c.query_row("SELECT json FROM cache_gh_releases WHERE repo_id = ?1", [repo_id], |r| r.get(0)).optional()?))?;
    json.map(|j| parse_releases(&j)).transpose()
}

/// `releases::list` with cached GitHub notes merged (no network; backs `releases_list` in M4).
pub fn releases_with_cached(s: &Shelf, repo_id: i64) -> Result<Vec<Release>> {
    let (_, r) = repo::open(s, repo_id)?;
    let list = crate::releases::list(&r)?;
    Ok(match cached(s, repo_id)? {
        Some(gh) => merge_into(list, &gh),
        None => list,
    })
}

/// One GET to the GitHub API (10 s, `User-Agent: RepoShelf`, https only, no redirects), cached, merged.
pub fn fetch_github_notes(s: &Shelf, repo_id: i64) -> Result<Vec<Release>> {
    let (_, r) = repo::open(s, repo_id)?;
    let url = api_url(repo::origin_url(&r).as_deref())?;
    drop(r);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(USER_AGENT)
        .https_only(true)
        .max_redirects(0)
        .build()
        .into();
    crate::clone::note_net_call();
    let body = agent
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .and_then(|mut resp| resp.body_mut().read_to_string())
        .map_err(|e| Error::Network(format!("GitHub release notes: {e}")))?;
    store(s, repo_id, &body)?;
    releases_with_cached(s, repo_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit;
    use crate::types::Source;

    const CANNED: &str = r#"[
      {"tag_name":"v1.0.0","name":"First stable","body":"Hello **world**<script>alert(1)</script>","draft":false,"prerelease":false,"extra":1},
      {"tag_name":"v2.0.0-rc1","name":"","body":null,"draft":false,"prerelease":true},
      {"tag_name":"v9.9.9","name":"Draft","body":"secret","draft":true,"prerelease":false},
      {"tag_name":"v0.0.1"}
    ]"#;

    #[test]
    fn parse_drops_drafts_and_tolerates_missing_fields() {
        let v = parse_releases(CANNED).unwrap();
        assert_eq!(v.iter().map(|r| r.tag_name.as_str()).collect::<Vec<_>>(), ["v1.0.0", "v2.0.0-rc1", "v0.0.1"]);
        assert_eq!(v[1].body, None);
        assert_eq!(parse_releases("{not json").unwrap_err().code(), "invalidInput");
        assert_eq!(parse_releases(r#"{"message":"Not Found"}"#).unwrap_err().code(), "invalidInput");
    }

    #[test]
    fn merge_by_tag_name_and_cache_round_trip() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let p = testkit::releases(&t.path().join("rel"));
        let id = testkit::register_raw(&s, &p, Source::Imported).unwrap();
        let plain = releases_with_cached(&s, id).unwrap();
        assert!(plain.iter().all(|r| !r.gh_notes));
        assert_eq!(cached(&s, id).unwrap(), None);
        store(&s, id, CANNED).unwrap();
        let merged = releases_with_cached(&s, id).unwrap();
        assert_eq!(merged.len(), plain.len(), "GitHub never adds or removes cards");
        let v1 = merged.iter().find(|r| r.tag == "v1.0.0").expect("fixture has v1.0.0");
        assert!(v1.gh_notes);
        assert_eq!(v1.title, "First stable");
        let html = v1.notes_html.as_deref().unwrap();
        assert!(html.contains("<strong>world</strong>") && !html.contains("<script"), "{html}");
        for r in merged.iter().filter(|r| r.tag != "v1.0.0") {
            let before = plain.iter().find(|x| x.tag == r.tag).unwrap();
            assert_eq!(r, before, "{} unchanged (empty/absent GitHub body)", r.tag);
        }
        assert_eq!(store(&s, id, "nope").unwrap_err().code(), "invalidInput");
        assert!(cached(&s, id).unwrap().is_some(), "bad JSON never replaces the cache");
    }

    #[test]
    fn non_github_origins_refused_without_network() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let before = testkit::net_calls();
        for origin in [
            Some("https://gitlab.com/o/r.git"),
            Some("http://github.com/o/r.git"),
            Some("git@github.com:o/r.git"),
            Some("https://github.com/g/sub/r.git"),
            Some("file:///C:/x/r"),
            None,
        ] {
            assert_eq!(api_url(origin).unwrap_err().code(), "refused", "{origin:?}");
        }
        assert_eq!(api_url(Some("https://github.com/o/r")).unwrap(), "https://api.github.com/repos/o/r/releases?per_page=100");
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://codeberg.org/o/r.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        assert_eq!(fetch_github_notes(&s, id).unwrap_err().code(), "refused");
        assert_eq!(testkit::net_calls(), before);
    }

    /// Optional live smoke (the only network test; never in the gate).
    #[test]
    #[ignore = "network: hits api.github.com"]
    fn live_github_smoke() {
        let t = tempfile::tempdir().unwrap();
        let s = testkit::shelf(t.path());
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://github.com/rust-lang/rust.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        fetch_github_notes(&s, id).unwrap();
        assert!(!cached(&s, id).unwrap().unwrap().is_empty());
    }
}
