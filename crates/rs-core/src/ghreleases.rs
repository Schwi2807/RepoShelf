//! GitHub release notes and assets (M3.8 FR-31 optional; v0.2.0 F4/F5/F6, D-22, D-49, D-99).
//!
//! One GET of `api.github.com/repos/<o>/<r>/releases?per_page=100` for github.com origins only (`net::Profile::API`:
//! no redirect, 10 s), with `If-None-Match` from the stored ETag. The raw JSON is cached in `cache_gh_releases`;
//! bodies go through the one markdown sanitiser (`markdown::render_release_notes`, avatars stripped) when merged.

use crate::Shelf;
use crate::error::{Error, Result};
use crate::github::{self, ApiAnswer};
use crate::repo;
use crate::types::{Release, ReleaseAsset};
use crate::validate;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

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
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

/// One entry of a release's `assets`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GhAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub download_count: Option<u64>,
    #[serde(default)]
    pub browser_download_url: String,
    #[serde(default)]
    pub content_type: Option<String>,
    /// `sha256:<hex>` when GitHub computed one.
    #[serde(default)]
    pub digest: Option<String>,
}

impl GhAsset {
    /// Lower-case hex of a `sha256:` digest (64 hex chars), else `None`.
    pub fn sha256(&self) -> Option<String> {
        let d = self.digest.as_deref()?.strip_prefix("sha256:")?;
        (d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit())).then(|| d.to_ascii_lowercase())
    }
}

/// Parses the API JSON (drafts dropped). Bad JSON → `InvalidInput`.
pub fn parse_releases(json: &str) -> Result<Vec<GhRelease>> {
    let v: Vec<GhRelease> = serde_json::from_str(json).map_err(|e| Error::InvalidInput(format!("Unexpected GitHub answer: {e}")))?;
    Ok(v.into_iter().filter(|r| !r.draft).collect())
}

/// GitHub's own "latest release": the first non-draft, non-prerelease entry (the API lists newest first).
pub fn latest(gh: &[GhRelease]) -> Option<&GhRelease> {
    gh.iter().find(|r| !r.draft && !r.prerelease)
}

/// Merges GitHub notes into the tag-based release cards by tag name: a non-empty body replaces the notes
/// (sanitised, avatars stripped, `gh_notes = true`); a non-empty release name replaces the title; the release
/// page link and assets are added. Tags GitHub does not know stay; GitHub-only releases are not added.
pub fn merge_into(mut releases: Vec<Release>, gh: &[GhRelease]) -> Vec<Release> {
    for r in &mut releases {
        let Some(g) = gh.iter().find(|g| g.tag_name == r.tag) else { continue };
        if let Some(n) = g.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            r.title = n.to_string();
        }
        if let Some(b) = g.body.as_deref().filter(|b| !b.trim().is_empty()) {
            r.notes_html = Some(crate::markdown::render_release_notes(b));
            r.gh_notes = true;
        }
        r.html_url = g.html_url.clone().filter(|u| u.starts_with("https://github.com/"));
        r.assets = g
            .assets
            .iter()
            .map(|a| ReleaseAsset {
                name: a.name.clone(),
                size: a.size,
                download_count: a.download_count,
                url: a.browser_download_url.clone(),
                content_type: a.content_type.clone(),
                sha256: a.sha256(),
                for_this_pc: false,
                downloaded: None,
            })
            .collect();
        if let crate::assets::Pick::One(i) = crate::assets::pick(&g.assets, std::env::consts::OS, std::env::consts::ARCH) {
            r.assets[i].for_this_pc = true;
        }
    }
    releases
}

/// `(owner, name)` of a plain `https://github.com/<owner>/<name>` origin, else `Refused`.
pub fn github_repo(origin: Option<&str>) -> Result<(String, String)> {
    let refused = || Error::Refused("Only repositories hosted on github.com have GitHub release notes and info".into());
    let u = origin.filter(|o| o.starts_with("https://")).and_then(|o| validate::clone_url(o).ok()).ok_or_else(refused)?;
    if !u.host.eq_ignore_ascii_case("github.com") || u.rel_dir.components().count() != 3 || u.url.matches('/').count() != 4 {
        return Err(refused());
    }
    Ok((u.owner, u.name))
}

/// The repository's current `origin` URL, read from git (the DB copy may be older).
pub fn origin_of(s: &Shelf, repo_id: i64) -> Result<Option<String>> {
    let (_, r) = repo::open(s, repo_id)?;
    Ok(repo::origin_url(&r))
}

/// `https://api.github.com/repos/<owner>/<name>/releases?per_page=100` for a github.com origin, else `Refused`.
pub fn api_url(origin: Option<&str>) -> Result<String> {
    let (o, n) = github_repo(origin)?;
    Ok(format!("https://api.github.com/repos/{o}/{n}/releases?per_page=100"))
}

/// Stores the raw API JSON (+ ETag) for a repo (validated by parsing first).
pub fn store_with_etag(s: &Shelf, repo_id: i64, json: &str, etag: Option<&str>) -> Result<Vec<GhRelease>> {
    let parsed = parse_releases(json)?;
    s.db.with(|c| {
        c.execute(
            "INSERT INTO cache_gh_releases (repo_id, fetched_at, json, etag) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(repo_id) DO UPDATE SET fetched_at = excluded.fetched_at, json = excluded.json, etag = excluded.etag",
            params![repo_id, crate::now_secs(), json, etag],
        )?;
        Ok(())
    })?;
    Ok(parsed)
}

/// `store_with_etag` without an ETag (tests, v0.1.0 behaviour).
pub fn store(s: &Shelf, repo_id: i64, json: &str) -> Result<Vec<GhRelease>> {
    store_with_etag(s, repo_id, json, None)
}

/// Cached notes, if any were fetched before.
pub fn cached(s: &Shelf, repo_id: i64) -> Result<Option<Vec<GhRelease>>> {
    let json: Option<String> =
        s.db.with(|c| Ok(c.query_row("SELECT json FROM cache_gh_releases WHERE repo_id = ?1", [repo_id], |r| r.get(0)).optional()?))?;
    json.map(|j| parse_releases(&j)).transpose()
}

fn cached_etag(s: &Shelf, repo_id: i64) -> Result<Option<String>> {
    s.db.with(|c| Ok(c.query_row("SELECT etag FROM cache_gh_releases WHERE repo_id = ?1", [repo_id], |r| r.get(0)).optional()?.flatten()))
}

/// `releases::list` with cached GitHub notes merged, source-code links for github.com origins and the
/// download state of each asset (no network; backs `releases_list`).
pub fn releases_with_cached(s: &Shelf, repo_id: i64) -> Result<Vec<Release>> {
    let (_, r) = repo::open(s, repo_id)?;
    let origin = repo::origin_url(&r);
    let list = crate::releases::list(&r)?;
    drop(r);
    let mut list = match cached(s, repo_id)? {
        Some(gh) => merge_into(list, &gh),
        None => list,
    };
    if let Ok((o, n)) = github_repo(origin.as_deref()) {
        for rel in &mut list {
            let tag = crate::assets::url_path_segment(&rel.tag);
            rel.source_zip_url = Some(format!("https://github.com/{o}/{n}/archive/refs/tags/{tag}.zip"));
            rel.source_tar_url = Some(format!("https://github.com/{o}/{n}/archive/refs/tags/{tag}.tar.gz"));
            for a in &mut rel.assets {
                let f = crate::assets::store_file(s, origin.as_deref(), &rel.tag, &a.name).ok();
                a.downloaded = f.filter(|f| crate::paths::long(f).is_file()).map(|f| f.display().to_string());
            }
        }
    }
    Ok(list)
}

/// One conditional GET (`If-None-Match`) of the releases list; 304 keeps the cache. Returns the parsed list.
pub fn refresh(s: &Shelf, repo_id: i64) -> Result<Vec<GhRelease>> {
    let (_, r) = repo::open(s, repo_id)?;
    let url = api_url(repo::origin_url(&r).as_deref())?;
    drop(r);
    match github::api_get(s, &url, cached_etag(s, repo_id)?.as_deref())? {
        ApiAnswer::Fresh { body, etag } => store_with_etag(s, repo_id, &body, etag.as_deref()),
        ApiAnswer::NotModified => {
            s.db.with(|c| Ok(c.execute("UPDATE cache_gh_releases SET fetched_at = ?2 WHERE repo_id = ?1", params![repo_id, crate::now_secs()])?))?;
            Ok(cached(s, repo_id)?.unwrap_or_default())
        }
    }
}

/// `releases_fetch_github`: refresh, then the merged cards.
pub fn fetch_github_notes(s: &Shelf, repo_id: i64) -> Result<Vec<Release>> {
    refresh(s, repo_id)?;
    releases_with_cached(s, repo_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Canned, Net};
    use crate::testkit;
    use crate::types::Source;

    const CANNED: &str = r#"[
      {"tag_name":"v1.0.0","name":"First stable","body":"Hello **world**<script>alert(1)</script>","draft":false,"prerelease":false,"extra":1,
       "html_url":"https://github.com/o/r/releases/tag/v1.0.0",
       "assets":[{"name":"r-1.0.0-x86_64-pc-windows-msvc.zip","size":10,"download_count":3,
                  "browser_download_url":"https://github.com/o/r/releases/download/v1.0.0/r-1.0.0-x86_64-pc-windows-msvc.zip",
                  "content_type":"application/zip","digest":"sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789"},
                 {"name":"SHA256SUMS","size":100,"browser_download_url":"https://github.com/o/r/releases/download/v1.0.0/SHA256SUMS"}]},
      {"tag_name":"v2.0.0-rc1","name":"","body":null,"draft":false,"prerelease":true},
      {"tag_name":"v9.9.9","name":"Draft","body":"secret","draft":true,"prerelease":false},
      {"tag_name":"v0.0.1"}
    ]"#;

    #[test]
    fn parse_drops_drafts_and_tolerates_missing_fields() {
        let v = parse_releases(CANNED).unwrap();
        assert_eq!(v.iter().map(|r| r.tag_name.as_str()).collect::<Vec<_>>(), ["v1.0.0", "v2.0.0-rc1", "v0.0.1"]);
        assert_eq!(v[1].body, None);
        assert_eq!(v[0].assets.len(), 2);
        assert_eq!(v[0].assets[0].sha256().as_deref(), Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"));
        assert_eq!(v[0].assets[1].sha256(), None);
        assert_eq!(latest(&v).unwrap().tag_name, "v1.0.0");
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
        assert!(plain.iter().all(|r| !r.gh_notes && r.assets.is_empty() && r.source_zip_url.is_none()));
        assert_eq!(cached(&s, id).unwrap(), None);
        store(&s, id, CANNED).unwrap();
        let merged = releases_with_cached(&s, id).unwrap();
        assert_eq!(merged.len(), plain.len(), "GitHub never adds or removes cards");
        let v1 = merged.iter().find(|r| r.tag == "v1.0.0").expect("fixture has v1.0.0");
        assert!(v1.gh_notes);
        assert_eq!(v1.title, "First stable");
        assert_eq!(v1.html_url.as_deref(), Some("https://github.com/o/r/releases/tag/v1.0.0"));
        assert_eq!(v1.assets.len(), 2);
        assert_eq!(v1.assets[0].download_count, Some(3));
        assert!(v1.assets[0].downloaded.is_none());
        assert!(!v1.assets[1].for_this_pc, "checksum lists are never picked");
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
    fn refresh_uses_etag_and_keeps_cache_on_304() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let s = testkit::shelf(t.path()).with_net(Net::new(c.clone()));
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).tag("v1.0.0", false).set_head().origin("https://github.com/o/r.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        let url = "https://api.github.com/repos/o/r/releases?per_page=100";
        c.on(url, 200, &[("ETag", "W/\"abc\"")], CANNED.as_bytes().to_vec()).on(url, 304, &[], Vec::new());
        let first = fetch_github_notes(&s, id).unwrap();
        assert!(first[0].gh_notes);
        assert!(first[0].source_zip_url.as_deref() == Some("https://github.com/o/r/archive/refs/tags/v1.0.0.zip"));
        let second = fetch_github_notes(&s, id).unwrap();
        assert_eq!(first, second, "304 keeps the cached answer");
        let seen = c.seen();
        assert_eq!(seen.len(), 2);
        assert!(!seen[0].1.iter().any(|(k, _)| k == "If-None-Match"));
        assert!(seen[1].1.iter().any(|(k, v)| k == "If-None-Match" && v == "W/\"abc\""), "{seen:?}");
        assert!(seen[1].1.iter().any(|(k, v)| k == "Accept" && v == "application/vnd.github+json"));
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
        let s = testkit::shelf(t.path()).with_net(Net::real());
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://github.com/rust-lang/rust.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        fetch_github_notes(&s, id).unwrap();
        assert!(!cached(&s, id).unwrap().unwrap().is_empty());
        crate::github::fetch_repo_info(&s, id).unwrap();
        assert!(crate::repo::summary(&s, id).unwrap().about_github.is_some());
    }
}
