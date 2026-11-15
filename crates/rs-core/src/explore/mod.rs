//! Explore mode backend (v0.3.0 M10a): browse GitHub repositories online through `api.github.com`, read-only.
//!
//! - `Explore` (held by `Shelf`): the credential store, the token cache, per-bucket rate-limit state and the
//!   "stale data served" marker. Nothing here runs unless an `explore_*` command asks.
//! - `get`: the one choke point for Explore HTTP (`net::Profile::API`: api.github.com only, https, no redirects —
//!   GitHub's documented 301 for a renamed repo is followed once by hand, re-checked). It adds
//!   `Authorization: Bearer <token>` only here, answers from the disk cache (`cache_gh_api`) when it can (sha-addressed
//!   answers never change; others are re-asked with `If-None-Match` after 60 s, a 304 is free), tracks the
//!   `X-RateLimit-*` headers per bucket (`core` / `search`) and serves saved data when the limit is used up or
//!   GitHub is unreachable.
//! - `github::GithubSource` / `source::LocalSource`: the `RepoSource` abstraction over the GitHub API and the local
//!   git object DB (D-122).
//!
//! The token never reaches SQLite, logs, job text, errors or IPC (see `cred`).

pub mod cache;
pub mod cred;
pub mod github;
pub mod json;
pub mod source;

use crate::Shelf;
use crate::error::{Error, Result};
use crate::net::Profile;
use crate::types::{ExploreStatus, Quota, Stale, StaleReason};
use cred::{CredentialStore, Token};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use url::Url;

/// An unchanged-looking answer younger than this is served without asking GitHub again.
pub const FRESH_SECS: i64 = 60;
/// Without reset headers, back off this long after a rate-limit answer.
pub const RATE_LIMIT_BACKOFF_SECS: i64 = 15 * 60;
pub const API_BASE: &str = "https://api.github.com";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Per-shelf Explore state.
pub struct Explore {
    creds: Arc<dyn CredentialStore>,
    /// `None` = the store was not asked yet; `Some(None)` = asked, no token.
    token: Mutex<Option<Option<Token>>>,
    /// bucket → Unix time before which no call is made.
    limited: Mutex<HashMap<&'static str, i64>>,
    stale: Mutex<Option<Stale>>,
}

impl Explore {
    pub fn new(creds: Arc<dyn CredentialStore>) -> Explore {
        Explore { creds, token: Mutex::new(None), limited: Mutex::new(HashMap::new()), stale: Mutex::new(None) }
    }

    /// The saved token, read from the store once and kept in memory. A store failure counts as "no token".
    fn token(&self) -> Option<Token> {
        let mut slot = lock(&self.token);
        if slot.is_none() {
            *slot = Some(self.creds.get().ok().flatten());
        }
        slot.clone().flatten()
    }

    pub fn token_saved(&self) -> bool {
        self.token().is_some()
    }

    /// Saves a validated token (store first, then the in-memory copy) and starts a fresh quota / limit state.
    pub fn set_token(&self, s: &Shelf, token: &Token) -> Result<()> {
        self.creds.set(token)?;
        *lock(&self.token) = Some(Some(token.clone()));
        self.reset_identity(s)
    }

    /// Removes the token. Cached answers go too: they may hold what only the token could see (private repos).
    pub fn remove_token(&self, s: &Shelf) -> Result<()> {
        self.creds.delete()?;
        *lock(&self.token) = Some(None);
        self.reset_identity(s)
    }

    fn reset_identity(&self, s: &Shelf) -> Result<()> {
        lock(&self.limited).clear();
        *lock(&self.stale) = None;
        cache::clear_quota(&s.db)?;
        cache::clear(&s.db)?;
        Ok(())
    }

    /// No call to `resource` before this Unix time (in-memory state, else the persisted headers).
    pub fn limited_until(&self, s: &Shelf, resource: &str) -> Option<i64> {
        let now = crate::now_secs();
        if let Some(t) = lock(&self.limited).get(resource).copied().filter(|t| *t > now) {
            return Some(t);
        }
        cache::load_quota(&s.db, resource).ok().flatten().filter(|q| q.remaining == 0 && q.reset_at > now).map(|q| q.reset_at)
    }

    fn set_limited(&self, resource: &'static str, until: i64) {
        lock(&self.limited).insert(resource, until);
    }

    fn note_stale(&self, fetched_at: i64, reason: StaleReason) {
        let mut s = lock(&self.stale);
        let oldest = s.map(|x| x.fetched_at.min(fetched_at)).unwrap_or(fetched_at);
        *s = Some(Stale { fetched_at: oldest, reason });
    }

    fn clear_stale(&self) {
        *lock(&self.stale) = None;
    }

    pub fn stale(&self) -> Option<Stale> {
        *lock(&self.stale)
    }

    /// What the Explore header / Settings show. Never contains the token.
    pub fn status(&self, s: &Shelf) -> Result<ExploreStatus> {
        let (cache_bytes, cache_entries) = cache::stats(&s.db)?;
        let quota = |r: &str| cache::load_quota(&s.db, r).ok().flatten();
        Ok(ExploreStatus {
            enabled: crate::settings::load(&s.db)?.explore_enabled,
            available: s.net.available(),
            token_saved: self.token_saved(),
            core: quota("core"),
            search: quota("search"),
            limited_until: self.limited_until(s, "core"),
            search_limited_until: self.limited_until(s, "search"),
            stale: self.stale(),
            cache_bytes,
            cache_entries,
        })
    }
}

/// How long a saved answer may be used without asking GitHub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Addressed by a commit / blob sha: the answer can never change.
    Immutable,
    /// Re-ask (conditionally) after `FRESH_SECS`.
    Revalidate,
    /// Never cached (`/rate_limit`).
    Never,
}

/// A checked answer.
#[derive(Debug, Clone)]
pub struct Answer {
    pub body: Vec<u8>,
    pub link: json::Link,
    pub fetched_at: i64,
}

impl Answer {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(|e| Error::InvalidInput(format!("Unexpected GitHub answer: {e}")))
    }
}

/// `https://api.github.com/<segments…>?<query>`: every segment percent-encoded (a `/` inside one stays one segment).
pub fn api_url(segments: &[&str], query: &[(&str, &str)]) -> Url {
    let mut u = Url::parse(API_BASE).expect("constant URL");
    if let Ok(mut p) = u.path_segments_mut() {
        p.extend(segments);
    }
    if !query.is_empty() {
        u.query_pairs_mut().extend_pairs(query);
    }
    u
}

fn resource_of(url: &Url) -> &'static str {
    if url.path().starts_with("/search") { "search" } else { "core" }
}

/// One raw answer (status + the headers we use + the body when the status is 200).
pub struct Raw {
    pub status: u16,
    pub etag: Option<String>,
    pub link: Option<String>,
    pub location: Option<String>,
    pub retry_after: Option<i64>,
    pub body: Vec<u8>,
}

fn header_i64(r: &crate::net::Response, name: &str) -> Option<i64> {
    r.header(name).and_then(|v| v.trim().parse::<i64>().ok())
}

/// One GET with the token, the quota bookkeeping and nothing else (no cache). The body is read only for 200.
pub fn fetch_raw(s: &Shelf, url: &Url, etag: Option<&str>) -> Result<Raw> {
    let mut headers: Vec<(&str, String)> = vec![("Accept", "application/vnd.github+json".into()), ("X-GitHub-Api-Version", "2022-11-28".into())];
    if let Some(e) = etag.filter(|e| !e.is_empty() && e.len() < 200 && !e.contains(['\r', '\n'])) {
        headers.push(("If-None-Match", e.to_string()));
    }
    if let Some(t) = s.explore.token() {
        headers.push(("Authorization", format!("Bearer {}", t.expose())));
    }
    let resp = s.net.get(url.as_str(), &headers, Profile::API)?;
    let resource = resource_of(url);
    // Rate-limit headers ride on every answer (also 304 / 403 / 404).
    let remaining = header_i64(&resp, "x-ratelimit-remaining");
    let reset = header_i64(&resp, "x-ratelimit-reset");
    if let (Some(limit), Some(rem), Some(reset)) = (header_i64(&resp, "x-ratelimit-limit"), remaining, reset) {
        let bucket: &'static str = match resp.header("x-ratelimit-resource") {
            Some("search") => "search",
            Some("core") => "core",
            _ => resource,
        };
        let q = Quota { resource: bucket.into(), limit: limit.clamp(0, u32::MAX as i64) as u32, remaining: rem.clamp(0, u32::MAX as i64) as u32, reset_at: reset, seen_at: crate::now_secs() };
        let _ = cache::save_quota(&s.db, &q);
        if rem == 0 && reset > crate::now_secs() {
            s.explore.set_limited(bucket, reset);
        }
    }
    let status = resp.status;
    let (etag, link, location) =
        (resp.header("etag").map(str::to_string), resp.header("link").map(str::to_string), resp.header("location").map(str::to_string));
    let retry_after = header_i64(&resp, "retry-after");
    let body = if status == 200 { resp.bytes()? } else { Vec::new() };
    Ok(Raw { status, etag, link, location, retry_after, body })
}

/// A renamed repository answers 301 with `Location: https://api.github.com/repositories/<id>/…`; anything else is
/// not followed.
fn moved_target(loc: &str) -> Option<Url> {
    let u = Url::parse(loc).ok()?;
    (u.scheme() == "https" && u.host_str() == Some("api.github.com") && u.port().is_none() && u.path().starts_with("/repositories/")).then_some(u)
}

fn refuse_off(s: &Shelf) -> Result<()> {
    if !crate::settings::load(&s.db)?.explore_enabled {
        return Err(Error::Refused("Explore is turned off (Settings → Network → Explore).".into()));
    }
    if !s.net.available() {
        return Err(Error::Unsupported("Explore is not included in this build.".into()));
    }
    Ok(())
}

/// The cached-or-fetched answer for `url` (see the module docs).
pub fn get(s: &Shelf, url: &Url, fresh: Freshness) -> Result<Answer> {
    refuse_off(s)?;
    let key = url.as_str();
    let cached = if fresh == Freshness::Never { None } else { cache::get(&s.db, key)? };
    let now = crate::now_secs();
    let answer_of = |e: &cache::Entry| Answer { body: e.body.clone(), link: json::Link::parse(e.link.as_deref()), fetched_at: e.fetched_at };
    if let Some(e) = &cached
        && (fresh == Freshness::Immutable || now - e.fetched_at < FRESH_SECS)
    {
        return Ok(answer_of(e));
    }
    let resource = resource_of(url);
    let stale = |e: &cache::Entry, reason| {
        s.explore.note_stale(e.fetched_at, reason);
        answer_of(e)
    };
    if let Some(until) = s.explore.limited_until(s, resource) {
        return match &cached {
            Some(e) => Ok(stale(e, StaleReason::RateLimited)),
            None => Err(Error::rate_limited(until)),
        };
    }
    let etag = cached.as_ref().and_then(|e| e.etag.clone());
    let mut raw = match fetch_raw(s, url, etag.as_deref()) {
        Ok(r) => r,
        Err(e @ Error::Network(_)) => {
            return match &cached {
                Some(c) => Ok(stale(c, StaleReason::Offline)),
                None => Err(e),
            };
        }
        Err(e) => return Err(e),
    };
    if matches!(raw.status, 301 | 302 | 307 | 308)
        && let Some(target) = raw.location.as_deref().and_then(moved_target)
    {
        raw = fetch_raw(s, &target, etag.as_deref())?;
    }
    match raw.status {
        200 => {
            let entry = cache::Entry { body: raw.body, etag: raw.etag, link: raw.link, fetched_at: crate::now_secs() };
            if fresh != Freshness::Never {
                cache::put(&s.db, key, &entry, cache::CACHE_CAP_BYTES)?;
            }
            s.explore.clear_stale();
            Ok(Answer { body: entry.body, link: json::Link::parse(entry.link.as_deref()), fetched_at: entry.fetched_at })
        }
        304 => match &cached {
            Some(e) => {
                cache::touch(&s.db, key)?;
                s.explore.clear_stale();
                Ok(answer_of(e))
            }
            None => Err(Error::Network("GitHub answered 304 for a request without a saved copy".into())),
        },
        301 | 302 | 307 | 308 => Err(Error::Refused("This repository was renamed or moved on GitHub.".into())),
        401 => Err(Error::Refused("GitHub did not accept the saved token. Remove it in Settings and enter a new one.".into())),
        403 | 429 => {
            let limited = raw.status == 429 || raw.retry_after.is_some() || s.explore.limited_until(s, resource).is_some();
            if !limited {
                return Err(Error::Refused(format!("GitHub refused this request ({}).", raw.status)));
            }
            let until = s
                .explore
                .limited_until(s, resource)
                .or_else(|| raw.retry_after.map(|d| crate::now_secs() + d.max(1)))
                .unwrap_or(crate::now_secs() + RATE_LIMIT_BACKOFF_SECS);
            s.explore.set_limited(resource, until);
            match &cached {
                Some(e) => Ok(stale(e, StaleReason::RateLimited)),
                None => Err(Error::rate_limited(until)),
            }
        }
        404 => Err(Error::not_found(if s.explore.token_saved() { "repository, file or page on GitHub" } else { "repository, file or page on GitHub (private repositories need a token)" })),
        409 => Err(Error::not_found("commits (the repository is empty)")),
        410 => Err(Error::not_found("page on GitHub (it was deleted)")),
        422 => Err(Error::InvalidInput("GitHub could not use these search words.".into())),
        451 => Err(Error::Refused("GitHub blocked this repository for legal reasons (451).".into())),
        st => Err(Error::Network(format!("GitHub answered {st}"))),
    }
}

/// After saving a token: one free `GET /rate_limit` (it does not count against the limit) tells whether GitHub accepts
/// it and fills in the quota. Only a 401 is an error; an unreachable GitHub or Explore being off leaves the token saved.
pub fn check_token(s: &Shelf) -> Result<()> {
    if refuse_off(s).is_err() {
        return Ok(());
    }
    match fetch_raw(s, &api_url(&["rate_limit"], &[]), None) {
        Ok(r) if r.status == 401 => Err(Error::Refused("GitHub did not accept this token.".into())),
        _ => Ok(()),
    }
}

/// `get` + JSON.
pub fn get_json<T: DeserializeOwned>(s: &Shelf, url: &Url, fresh: Freshness) -> Result<T> {
    get(s, url, fresh)?.json()
}

#[cfg(test)]
mod tests;
