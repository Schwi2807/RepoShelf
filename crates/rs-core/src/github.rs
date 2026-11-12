//! GitHub API calls and the metadata hook that runs after Clone / Import / Fetch (v0.2.0 F4/F6; D-99..D-103).
//!
//! - `api_get`: one `api.github.com` GET (`net::Profile::API`: no redirect, D-49) with `If-None-Match`.
//!   403/429 with an exhausted limit (or 429) → `Refused` + no further API call from this shelf until the reset
//!   time; 301 (renamed/moved) → `Refused`; 404 → `Refused` (private, renamed or deleted).
//! - `fetch_repo_info`: description / homepage / topics into `gh_repo_info` (separate from the user's values).
//! - `sync`: what a job runs after its own work succeeded. It never fails: every problem is a note on the job.

use crate::Shelf;
use crate::error::{Error, Result};
use crate::ghreleases;
use crate::jobs::JobCtx;
use crate::net::Profile;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

/// Without `X-RateLimit-Reset`, back off this long after a rate-limit answer.
pub const RATE_LIMIT_BACKOFF_SECS: i64 = 15 * 60;

pub enum ApiAnswer {
    Fresh { body: String, etag: Option<String> },
    NotModified,
}

fn rate_limit_error(until: i64) -> Error {
    let mins = ((until - crate::now_secs()).max(60) + 59) / 60;
    Error::Refused(format!("GitHub's request limit for apps without an account is used up; try again in about {mins} min"))
}

/// `true` for the error `api_get` returns while rate-limited.
pub fn is_rate_limited(s: &Shelf) -> bool {
    s.net.rate_limited_until().is_some_and(|t| t > crate::now_secs())
}

/// One conditional API GET.
pub fn api_get(s: &Shelf, url: &str, etag: Option<&str>) -> Result<ApiAnswer> {
    if let Some(until) = s.net.rate_limited_until().filter(|t| *t > crate::now_secs()) {
        return Err(rate_limit_error(until));
    }
    let mut headers = vec![("Accept", "application/vnd.github+json".to_string()), ("X-GitHub-Api-Version", "2022-11-28".to_string())];
    if let Some(e) = etag.filter(|e| !e.is_empty() && e.len() < 200 && !e.contains(['\r', '\n'])) {
        headers.push(("If-None-Match", e.to_string()));
    }
    let resp = s.net.get(url, &headers, Profile::API)?;
    let status = resp.status;
    match status {
        200 => {
            let etag = resp.header("etag").map(str::to_string);
            let body = String::from_utf8(resp.bytes()?).map_err(|_| Error::InvalidInput("GitHub sent non-UTF-8 JSON".into()))?;
            Ok(ApiAnswer::Fresh { body, etag })
        }
        304 => Ok(ApiAnswer::NotModified),
        403 | 429 if status == 429 || resp.header("x-ratelimit-remaining") == Some("0") => {
            let reset = resp.header("x-ratelimit-reset").and_then(|v| v.trim().parse::<i64>().ok());
            let retry = resp.header("retry-after").and_then(|v| v.trim().parse::<i64>().ok()).map(|d| crate::now_secs() + d);
            let until = reset.or(retry).filter(|t| *t > crate::now_secs()).unwrap_or(crate::now_secs() + RATE_LIMIT_BACKOFF_SECS);
            s.net.set_rate_limited_until(Some(until));
            Err(rate_limit_error(until))
        }
        301 | 302 | 307 | 308 => Err(Error::Refused("the repository was renamed or moved on GitHub (update the origin to fetch its info)".into())),
        404 => Err(Error::Refused("not found on GitHub (private, renamed or deleted)".into())),
        _ => Err(Error::Network(format!("GitHub answered {status}"))),
    }
}

#[derive(Debug, Deserialize)]
struct RepoInfoJson {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    html_url: Option<String>,
}

/// Trims and bounds a fetched string; empty → `None`.
fn clean(v: Option<String>, max: usize) -> Option<String> {
    let t: String = v?.chars().filter(|c| !c.is_control()).take(max).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// `GET /repos/{o}/{r}` (conditional) into `gh_repo_info`. The homepage is kept only when it is http(s).
pub fn fetch_repo_info(s: &Shelf, repo_id: i64) -> Result<()> {
    let (o, n) = ghreleases::github_repo(ghreleases::origin_of(s, repo_id)?.as_deref())?;
    let etag: Option<String> =
        s.db.with(|c| Ok(c.query_row("SELECT etag FROM gh_repo_info WHERE repo_id = ?1", [repo_id], |r| r.get(0)).optional()?.flatten()))?;
    match api_get(s, &format!("https://api.github.com/repos/{o}/{n}"), etag.as_deref())? {
        ApiAnswer::NotModified => {
            s.db.with(|c| Ok(c.execute("UPDATE gh_repo_info SET fetched_at = ?2 WHERE repo_id = ?1", params![repo_id, crate::now_secs()])?))?;
        }
        ApiAnswer::Fresh { body, etag } => {
            let j: RepoInfoJson = serde_json::from_str(&body).map_err(|e| Error::InvalidInput(format!("Unexpected GitHub answer: {e}")))?;
            let homepage = clean(j.homepage, 500).filter(|h| h.starts_with("https://") || h.starts_with("http://"));
            let topics: Vec<String> = j.topics.into_iter().filter_map(|t| clean(Some(t), 50)).take(20).collect();
            let html_url = j.html_url.filter(|u| u.starts_with("https://github.com/"));
            s.db.with(|c| {
                c.execute(
                    "INSERT INTO gh_repo_info (repo_id, etag, fetched_at, description, homepage, topics_json, html_url)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(repo_id) DO UPDATE SET etag = excluded.etag, fetched_at = excluded.fetched_at,
                       description = excluded.description, homepage = excluded.homepage, topics_json = excluded.topics_json,
                       html_url = excluded.html_url",
                    params![
                        repo_id,
                        etag,
                        crate::now_secs(),
                        clean(j.description, 1000),
                        homepage,
                        serde_json::to_string(&topics).unwrap_or_else(|_| "[]".into()),
                        html_url
                    ],
                )?;
                Ok(())
            })?;
        }
    }
    Ok(())
}

/// What `sync` does (from the settings + the action's own option).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncOpts {
    pub info: bool,
    pub notes: bool,
    pub download: bool,
}

impl SyncOpts {
    /// `download`: the action's checkbox (`None` = the setting `autoDownloadLatestRelease`).
    pub fn from_settings(s: &Shelf, download: Option<bool>) -> SyncOpts {
        let st = crate::settings::load(&s.db).unwrap_or_default();
        SyncOpts {
            info: st.fetch_repo_info,
            notes: st.fetch_release_notes,
            download: download.unwrap_or(st.auto_download_latest_release),
        }
    }
    pub fn any(&self) -> bool {
        self.info || self.notes || self.download
    }
}

/// A download the hook asks the job runner to queue (`JobKind::ReleaseDownload`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadReq {
    pub repo_id: i64,
    pub tag: String,
    pub asset: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub notes: Vec<String>,
    pub download: Option<DownloadReq>,
}

/// `true` when the repo's origin is a plain github.com repository (the hook applies).
pub fn applies(s: &Shelf, repo_id: i64) -> bool {
    ghreleases::origin_of(s, repo_id).is_ok_and(|o| ghreleases::github_repo(o.as_deref()).is_ok())
}

/// The post-action hook (Clone / Import / Fetch / Fetch all). Never fails; never touches git.
pub fn sync(s: &Shelf, repo_id: i64, opts: SyncOpts, ctx: &JobCtx) -> SyncReport {
    let mut rep = SyncReport::default();
    if !opts.any() || !s.net.available() || !applies(s, repo_id) {
        return rep;
    }
    let mut note = |what: &str, e: &Error| rep.notes.push(format!("{what}: {e}"));
    if opts.info {
        if ctx.cancelled() {
            rep.notes.push("GitHub info skipped (cancelled)".into());
            return rep;
        }
        if let Err(e) = fetch_repo_info(s, repo_id) {
            note("GitHub info", &e);
            if is_rate_limited(s) {
                return rep;
            }
        }
    }
    if opts.notes || opts.download {
        if ctx.cancelled() {
            rep.notes.push("GitHub releases skipped (cancelled)".into());
            return rep;
        }
        let gh = match ghreleases::refresh(s, repo_id) {
            Ok(gh) => gh,
            Err(e) => {
                note("GitHub releases", &e);
                return rep;
            }
        };
        if opts.download {
            match crate::assets::latest_for_this_pc(s, repo_id, &gh) {
                Ok(crate::assets::Latest::Download { tag, asset }) => rep.download = Some(DownloadReq { repo_id, tag, asset }),
                Ok(crate::assets::Latest::Already { tag, asset }) => rep.notes.push(format!("Latest release {tag}: {asset} is already downloaded")),
                Ok(crate::assets::Latest::Nothing(why)) => rep.notes.push(format!("No release download: {why}")),
                Err(e) => note("Release download", &e),
            }
        }
    }
    rep
}

/// Queues the auto-download (if any) as a follow-up job and returns the notes.
pub fn finish(report: SyncReport, ctx: &JobCtx) -> Vec<String> {
    if let Some(d) = report.download {
        let title = format!("Download {} ({})", d.asset, d.tag);
        ctx.follow_up(crate::jobs::JobKind::ReleaseDownload { repo_id: d.repo_id, tag: d.tag, asset: d.asset }, title);
    }
    report.notes
}

/// The hook after a successful Clone / Fetch / Import job of one repo (`auto_download`: the action's option).
pub fn after_action(s: &Shelf, repo_id: i64, auto_download: Option<bool>, ctx: &JobCtx) -> Vec<String> {
    finish(sync(s, repo_id, SyncOpts::from_settings(s, auto_download), ctx), ctx)
}

/// Adds the hook's notes to a finished clone / fetch result.
pub fn with_notes(mut r: crate::types::JobResult, notes: Vec<String>) -> crate::types::JobResult {
    use crate::types::JobResult;
    match &mut r {
        JobResult::Cloned { notes: n, .. } | JobResult::Synced { notes: n, .. } => n.extend(notes),
        JobResult::Fetched(o) => o.notes.extend(notes),
        JobResult::Downloaded { .. } => {}
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Canned, Net};
    use crate::repo;
    use crate::testkit;
    use crate::types::Source;
    use crate::validate::MetadataInput;

    fn gh_repo(t: &std::path::Path, c: &std::sync::Arc<Canned>) -> (Shelf, i64) {
        let s = testkit::shelf(t).with_net(Net::new(c.clone()));
        let mut b = testkit::repo(&t.join("r"));
        b.file("a", b"1").commit("one", testkit::T0).tag("v1.0.0", false).set_head().origin("https://github.com/o/r.git");
        let id = testkit::register_raw(&s, &t.join("r"), Source::Imported).unwrap();
        (s, id)
    }

    const INFO: &str = r#"{"description":"Fetched description","homepage":"https://r.example","topics":["cli","rust"],
        "html_url":"https://github.com/o/r","stargazers_count":5}"#;

    #[test]
    fn repo_info_is_stored_apart_and_user_values_win() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let (s, id) = gh_repo(t.path(), &c);
        let url = "https://api.github.com/repos/o/r";
        c.on(url, 200, &[("etag", "\"e1\"")], INFO.as_bytes().to_vec()).on(url, 304, &[], Vec::new());
        fetch_repo_info(&s, id).unwrap();
        let sum = repo::summary(&s, id).unwrap();
        assert_eq!(sum.description.as_deref(), Some("Fetched description"));
        assert_eq!(sum.homepage.as_deref(), Some("https://r.example"));
        assert_eq!(sum.topics, ["cli", "rust"]);
        assert_eq!(sum.about_user, Default::default());
        let gh = sum.about_github.clone().unwrap();
        assert_eq!(gh.html_url.as_deref(), Some("https://github.com/o/r"));

        // the user's own values win per field; GitHub's stay visible apart
        let sum = crate::library::set_metadata(&s, id, MetadataInput { description: Some("Mine".into()), homepage: None, topics: vec![] }).unwrap();
        assert_eq!(sum.description.as_deref(), Some("Mine"));
        assert_eq!(sum.homepage.as_deref(), Some("https://r.example"), "unset user homepage → GitHub's");
        assert_eq!(sum.topics, ["cli", "rust"]);
        assert_eq!(sum.about_user.description.as_deref(), Some("Mine"));
        assert_eq!(sum.about_github.as_ref().unwrap().description.as_deref(), Some("Fetched description"));

        // 304: kept, conditional header sent
        fetch_repo_info(&s, id).unwrap();
        assert_eq!(repo::summary(&s, id).unwrap().about_github.unwrap().description.as_deref(), Some("Fetched description"));
        assert!(c.seen()[1].1.iter().any(|(k, v)| k == "If-None-Match" && v == "\"e1\""));
        // a fetch never touches the user's rows
        let user: (Option<String>, i64) = s
            .db
            .with(|c| Ok(c.query_row("SELECT description, (SELECT COUNT(*) FROM repo_topics) FROM repo_meta WHERE repo_id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?))
            .unwrap();
        assert_eq!(user, (Some("Mine".into()), 0));
    }

    #[test]
    fn hostile_info_values_are_cleaned() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let (s, id) = gh_repo(t.path(), &c);
        let long = "x".repeat(5000);
        let body = format!(
            r#"{{"description":"  a\u0000b\u001b  ","homepage":"javascript:alert(1)","topics":["ok","","{long}"],"html_url":"https://evil.example/x"}}"#
        );
        c.on("https://api.github.com/repos/o/r", 200, &[], body.into_bytes());
        fetch_repo_info(&s, id).unwrap();
        let gh = repo::summary(&s, id).unwrap().about_github.unwrap();
        assert_eq!(gh.description.as_deref(), Some("ab"));
        assert_eq!(gh.homepage, None);
        assert_eq!(gh.topics.len(), 2);
        assert_eq!(gh.topics[1].len(), 50);
        assert_eq!(gh.html_url, None);
    }

    #[test]
    fn rate_limit_and_moved_are_notes_and_stop_further_calls() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let (s, id) = gh_repo(t.path(), &c);
        let reset = (crate::now_secs() + 600).to_string();
        c.on("https://api.github.com/repos/o/r", 403, &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", reset.as_str())], b"{}".to_vec());
        let opts = SyncOpts { info: true, notes: true, download: true };
        let rep = sync(&s, id, opts, &JobCtx::noop());
        assert_eq!(rep.download, None);
        assert_eq!(rep.notes.len(), 1, "{:?}", rep.notes);
        assert!(rep.notes[0].contains("request limit") && rep.notes[0].contains("10 min"), "{:?}", rep.notes);
        assert!(is_rate_limited(&s));
        assert_eq!(c.seen().len(), 1, "the releases call was never made");
        // a second sync makes no request at all
        let rep = sync(&s, id, opts, &JobCtx::noop());
        assert_eq!(rep.notes.len(), 1);
        assert_eq!(c.seen().len(), 1);

        let t2 = tempfile::tempdir().unwrap();
        let c2 = Canned::new();
        let (s2, id2) = gh_repo(t2.path(), &c2);
        c2.on("https://api.github.com/repos/o/r", 301, &[("location", "https://api.github.com/repositories/9")], Vec::new())
            .on("https://api.github.com/repos/o/r/releases?per_page=100", 429, &[], Vec::new());
        let rep = sync(&s2, id2, SyncOpts { info: true, notes: true, download: false }, &JobCtx::noop());
        assert_eq!(rep.notes.len(), 2, "{:?}", rep.notes);
        assert!(rep.notes[0].contains("renamed or moved"), "{:?}", rep.notes);
        assert!(rep.notes[1].contains("request limit"), "{:?}", rep.notes);
        assert_eq!(c2.seen().len(), 2, "the 301 was not followed");
    }

    #[test]
    fn non_github_or_all_off_does_nothing() {
        let t = tempfile::tempdir().unwrap();
        let c = Canned::new();
        let s = testkit::shelf(t.path()).with_net(Net::new(c.clone()));
        let mut b = testkit::repo(&t.path().join("r"));
        b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://gitlab.com/o/r.git");
        let id = testkit::register_raw(&s, &t.path().join("r"), Source::Imported).unwrap();
        assert_eq!(sync(&s, id, SyncOpts { info: true, notes: true, download: true }, &JobCtx::noop()), SyncReport::default());
        let (s2, id2) = gh_repo(&t.path().join("2"), &c);
        assert_eq!(sync(&s2, id2, SyncOpts { info: false, notes: false, download: false }, &JobCtx::noop()), SyncReport::default());
        assert!(c.seen().is_empty());
        // settings drive the defaults
        let o = SyncOpts::from_settings(&s, None);
        assert_eq!(o, SyncOpts { info: true, notes: true, download: true });
        assert!(!SyncOpts::from_settings(&s, Some(false)).download);
    }
}
