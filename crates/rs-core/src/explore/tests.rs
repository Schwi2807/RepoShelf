//! Explore tests: the client (cache, ETag, freshness, rate limit, token, 301), `GithubSource` over canned GitHub
//! JSON (no real HTTP, ever) and a parity run of the same assertions against `LocalSource` and `GithubSource`.

use super::cred::{MemoryStore, Token};
use super::github::{GithubSource, search};
use super::source::{LocalSource, RepoSource};
use super::{Freshness, api_url, get, get_json};
use crate::Shelf;
use crate::net::{Canned, Net};
use crate::testkit;
use crate::types::{BlobKind, ChangeKind, EntryKind, LineKind, RefKind, StaleReason};
use crate::validate::{git_path, rev};
use base64::Engine;
use serde_json::{Value, json};
use std::sync::Arc;

const C1: &str = "1111111111111111111111111111111111111111";
const C2: &str = "2222222222222222222222222222222222222222";
const T2: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const B_README: &str = "cccccccccccccccccccccccccccccccccccccccc";
const B_SRC: &str = "dddddddddddddddddddddddddddddddddddddddd";
const B_MAIN: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const API: &str = "https://api.github.com/repos/octo/demo";
const README: &str = "# Demo\n\nhello **world**\n";
const MAIN_RS: &str = "fn main() {}\n";
const TOKEN: &str = "ghp_SecretTokenValue123";

fn b64(s: &str) -> String {
    // GitHub wraps the base64 at 60 characters with newlines.
    let raw = base64::engine::general_purpose::STANDARD.encode(s);
    raw.as_bytes().chunks(60).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("\n") + "\n"
}

fn sig(date: &str) -> Value {
    json!({"name": "Mona Octocat", "email": "mona@example.com", "date": date})
}

fn commit_json(sha: &str, msg: &str, date: &str, parents: &[&str], tree: &str) -> Value {
    json!({
        "sha": sha,
        "node_id": "C_x",
        "commit": {
            "author": sig(date), "committer": sig(date), "message": msg,
            "tree": {"sha": tree, "url": "https://api.github.com/repos/octo/demo/git/trees/x"},
            "comment_count": 0
        },
        "author": {"login": "octocat", "id": 1, "avatar_url": "https://avatars.githubusercontent.com/u/1?v=4"},
        "parents": parents.iter().map(|p| json!({"sha": p, "url": "u", "html_url": "h"})).collect::<Vec<_>>()
    })
}

fn c2() -> Value {
    commit_json(C2, "Add main\n\nSecond commit body.", "2023-11-14T22:13:20Z", &[C1], T2)
}

fn c1() -> Value {
    commit_json(C1, "Initial", "2023-11-14T22:12:20Z", &[], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
}

fn repo_json() -> Value {
    json!({
        "id": 1, "name": "demo", "full_name": "octo/demo", "private": false,
        "owner": {"login": "octo", "id": 9, "avatar_url": "https://avatars.githubusercontent.com/u/9?v=4"},
        "html_url": "https://github.com/octo/demo", "description": "A demo repo", "fork": false,
        "created_at": "2020-09-13T12:26:40Z", "updated_at": "2023-11-14T22:13:20Z", "pushed_at": "2023-11-14T22:13:20Z",
        "homepage": "https://demo.example", "size": 2048, "stargazers_count": 120, "language": "Rust", "forks_count": 7,
        "open_issues_count": 3, "license": {"key": "mit", "name": "MIT License", "spdx_id": "MIT"},
        "topics": ["cli", "rust"], "default_branch": "main", "archived": false
    })
}

fn file_entry(name: &str, path: &str, sha: &str, size: usize, kind: &str) -> Value {
    json!({"type": kind, "size": size, "name": name, "path": path, "sha": sha, "html_url": "h", "download_url": null})
}

fn file_json(name: &str, path: &str, sha: &str, text: &str) -> Value {
    let mut v = file_entry(name, path, sha, text.len(), "file");
    v["content"] = json!(b64(text));
    v["encoding"] = json!("base64");
    v
}

fn put(c: &Canned, tail: &str, status: u16, headers: &[(&str, &str)], body: &Value) {
    c.on(&format!("{API}{tail}"), status, headers, body.to_string().into_bytes());
}

/// The canned `octo/demo` repository: two commits, README.md, src/main.rs, one tag, one release.
fn demo(c: &Canned) {
    put(c, "", 200, &[("etag", "\"repo1\"")], &repo_json());
    for sha in ["main", C2] {
        put(
            c,
            &format!("/commits?sha={sha}&per_page=1"),
            200,
            &[("link", "<https://api.github.com/repositories/1/commits?sha=main&per_page=1&page=2>; rel=\"next\", <https://api.github.com/repositories/1/commits?sha=main&per_page=1&page=2>; rel=\"last\"")],
            &json!([c2()]),
        );
    }
    put(c, &format!("/contents?ref={C2}"), 200, &[], &json!([file_entry("README.md", "README.md", B_README, README.len(), "file"), file_entry("src", "src", B_SRC, 0, "dir")]));
    put(c, &format!("/contents/src?ref={C2}"), 200, &[], &json!([file_entry("main.rs", "src/main.rs", B_MAIN, MAIN_RS.len(), "file")]));
    put(c, &format!("/contents/nope?ref={C2}"), 404, &[], &json!({"message": "Not Found"}));
    put(c, &format!("/contents/README.md?ref={C2}"), 200, &[], &file_json("README.md", "README.md", B_README, README));
    put(c, &format!("/contents/src/main.rs?ref={C2}"), 200, &[], &file_json("main.rs", "src/main.rs", B_MAIN, MAIN_RS));
    put(c, &format!("/commits?sha={C2}&per_page=35&page=1"), 200, &[], &json!([c2(), c1()]));
    let mut full = c2();
    full["stats"] = json!({"additions": 1, "deletions": 0, "total": 1});
    full["files"] = json!([{
        "sha": B_MAIN, "filename": "src/main.rs", "status": "added", "additions": 1, "deletions": 0, "changes": 1,
        "patch": "@@ -0,0 +1 @@\n+fn main() {}"
    }]);
    put(c, &format!("/commits/{C2}"), 200, &[], &full);
    put(c, "/branches?per_page=100", 200, &[], &json!([{"name": "dev", "commit": {"sha": C1, "url": "u"}, "protected": false}, {"name": "main", "commit": {"sha": C2, "url": "u"}, "protected": true}]));
    put(c, "/tags?per_page=100", 200, &[], &json!([{"name": "v1.0.0", "commit": {"sha": C1, "url": "u"}, "zipball_url": "z", "tarball_url": "t"}]));
    put(
        c,
        "/releases?per_page=100",
        200,
        &[],
        &json!([
            {"tag_name": "v2.0.0-rc1", "name": "", "body": "rc notes", "draft": false, "prerelease": true, "published_at": "2023-11-14T22:13:20Z", "html_url": "https://github.com/octo/demo/releases/tag/v2.0.0-rc1", "assets": []},
            {"tag_name": "v1.0.0", "name": "First", "body": "# Notes\n\nhello", "draft": false, "prerelease": false, "published_at": "2023-11-14T22:12:20Z",
             "target_commitish": "main", "html_url": "https://github.com/octo/demo/releases/tag/v1.0.0",
             "assets": [
               {"name": "demo-1.0.0-x86_64-pc-windows-msvc.zip", "size": 22, "download_count": 4, "browser_download_url": "https://github.com/octo/demo/releases/download/v1.0.0/demo-1.0.0-x86_64-pc-windows-msvc.zip", "content_type": "application/zip"},
               {"name": "evil.exe", "size": 1, "browser_download_url": "https://evil.example.com/evil.exe"}
             ]},
            {"tag_name": "draft", "draft": true, "prerelease": false, "assets": []}
        ]),
    );
    put(c, "/languages", 200, &[], &json!({"Rust": 900, "Shell": 100}));
}

fn shelf(t: &std::path::Path, c: &Arc<Canned>) -> Shelf {
    testkit::shelf(t).with_net(Net::new(c.clone()))
}

fn age_cache(s: &Shelf, secs: i64) {
    s.db.with(|c| Ok(c.execute("UPDATE cache_gh_api SET fetched_at = fetched_at - ?1", [secs])?)).unwrap();
}

fn hdr<'a>(seen: &'a [(String, Vec<(String, String)>)], i: usize, name: &str) -> Option<&'a str> {
    seen[i].1.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

// ---------------------------------------------------------------------------------------------------------------
// Client

#[test]
fn url_builder_encodes_segments_and_queries() {
    let u = api_url(&["repos", "o", "r", "contents", "a b", "c#d?e", "ü"], &[("ref", "feature/x y&z")]);
    assert_eq!(u.as_str(), "https://api.github.com/repos/o/r/contents/a%20b/c%23d%3Fe/%C3%BC?ref=feature%2Fx+y%26z");
    assert_eq!(api_url(&["repos", "o", "r", "contents", "a/b"], &[]).path(), "/repos/o/r/contents/a%2Fb");
}

#[test]
fn cache_etag_and_freshness() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let url = api_url(&["repos", "octo", "demo"], &[]);
    c.on(url.as_str(), 200, &[("etag", "\"e1\""), ("x-ratelimit-limit", "60"), ("x-ratelimit-remaining", "59"), ("x-ratelimit-reset", "4102444800"), ("x-ratelimit-resource", "core")], b"{\"a\":1}".to_vec())
        .on(url.as_str(), 304, &[("x-ratelimit-limit", "60"), ("x-ratelimit-remaining", "59"), ("x-ratelimit-reset", "4102444800")], Vec::new());
    let v: Value = get_json(&s, &url, Freshness::Revalidate).unwrap();
    assert_eq!(v["a"], 1);
    // within 60 s: no request
    get(&s, &url, Freshness::Revalidate).unwrap();
    assert_eq!(c.seen().len(), 1);
    // older than 60 s: conditional request, 304 serves the saved body and renews it
    age_cache(&s, 120);
    get(&s, &url, Freshness::Revalidate).unwrap();
    assert_eq!(c.seen().len(), 1, "anonymous: a 304 would cost a request, so the saved copy is used for 15 minutes");
    age_cache(&s, 1000);
    let a = get(&s, &url, Freshness::Revalidate).unwrap();
    assert_eq!(a.body, b"{\"a\":1}");
    assert_eq!(c.seen().len(), 2);
    assert_eq!(hdr(&c.seen(), 1, "If-None-Match"), Some("\"e1\""));
    get(&s, &url, Freshness::Revalidate).unwrap();
    assert_eq!(c.seen().len(), 2, "the 304 renewed the copy");
    // immutable answers are never asked again, however old
    age_cache(&s, 10_000_000);
    get(&s, &url, Freshness::Immutable).unwrap();
    assert_eq!(c.seen().len(), 2);
    // quota from the headers, persisted
    let q = super::cache::load_quota(&s.db, "core").unwrap().unwrap();
    assert_eq!((q.limit, q.remaining, q.reset_at), (60, 59, 4_102_444_800));
    let st = s.explore.status(&s).unwrap();
    assert_eq!(st.core.unwrap().remaining, 59);
    assert_eq!((st.cache_entries, st.limited_until, st.stale), (1, None, None));
    // the standard request headers
    assert_eq!(hdr(&c.seen(), 0, "Accept"), Some("application/vnd.github+json"));
    assert_eq!(hdr(&c.seen(), 0, "X-GitHub-Api-Version"), Some("2022-11-28"));
    assert!(hdr(&c.seen(), 0, "Authorization").is_none());
}

#[test]
fn rate_limit_blocks_calls_serves_saved_data_and_search_is_its_own_bucket() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let a = api_url(&["repos", "octo", "demo"], &[]);
    let b = api_url(&["repos", "octo", "other"], &[]);
    let reset = (crate::now_secs() + 1800).to_string();
    // 200 first (consumed by the first call), then the limit answer for every later call
    c.on(a.as_str(), 200, &[], b"{}".to_vec());
    c.on(a.as_str(), 403, &[("x-ratelimit-limit", "60"), ("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", reset.as_str()), ("x-ratelimit-resource", "core")], b"{\"message\":\"API rate limit exceeded\"}".to_vec());
    get(&s, &a, Freshness::Revalidate).unwrap();
    age_cache(&s, 1000);
    // limited while a saved copy exists: the copy is served, marked stale
    let got = get(&s, &a, Freshness::Revalidate).unwrap();
    assert_eq!(got.body, b"{}");
    let st = s.explore.status(&s).unwrap();
    assert_eq!(st.limited_until, Some(reset.parse().unwrap()));
    assert_eq!(st.stale.unwrap().reason, StaleReason::RateLimited);
    assert_eq!(st.core.unwrap().remaining, 0);
    // no saved copy: a clear error with the reset time, and no request is made
    let n = c.seen().len();
    let e = get(&s, &b, Freshness::Revalidate).unwrap_err();
    assert_eq!(e.code(), "rateLimited");
    assert!(e.to_string().contains("30 min") || e.to_string().contains("31 min"), "{e}");
    assert_eq!(c.seen().len(), n);
    // the saved copy is still served while limited (even past its freshness)
    age_cache(&s, 500);
    assert_eq!(get(&s, &a, Freshness::Revalidate).unwrap().body, b"{}");
    assert_eq!(c.seen().len(), n);
    // search is a separate bucket: not blocked
    let q = api_url(&["search", "repositories"], &[("q", "x")]);
    c.on(q.as_str(), 200, &[("x-ratelimit-limit", "10"), ("x-ratelimit-remaining", "9"), ("x-ratelimit-reset", reset.as_str()), ("x-ratelimit-resource", "search")], b"{\"items\":[]}".to_vec());
    get(&s, &q, Freshness::Revalidate).unwrap();
    let st = s.explore.status(&s).unwrap();
    assert_eq!(st.search.unwrap().remaining, 9);
    assert_eq!(st.search_limited_until, None);
    // a 429 on search blocks search only
    let q2 = api_url(&["search", "repositories"], &[("q", "y")]);
    c.on(q2.as_str(), 429, &[("retry-after", "120")], Vec::new());
    assert_eq!(get(&s, &q2, Freshness::Revalidate).unwrap_err().code(), "rateLimited");
    let st = s.explore.status(&s).unwrap();
    assert!(st.search_limited_until.unwrap() > crate::now_secs());
    assert!(st.limited_until.is_some(), "core stays limited");
    // limit state survives a restart through the persisted headers (core only: remaining == 0)
    let s2 = testkit::shelf(t.path()); // a different in-memory DB: nothing persisted
    assert!(s2.explore.status(&s2).unwrap().limited_until.is_none());
}

#[test]
fn quota_zero_is_remembered_in_the_db_for_the_next_run() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let reset = crate::now_secs() + 600;
    let dirs = crate::paths::Dirs { data: t.path().join("d"), archive: t.path().join("a") };
    let s = Shelf::open(dirs.clone()).unwrap().with_net(Net::new(c.clone()));
    let u = api_url(&["repos", "octo", "demo"], &[]);
    c.on(u.as_str(), 200, &[("x-ratelimit-limit", "60"), ("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", &reset.to_string())], b"{}".to_vec());
    get(&s, &u, Freshness::Revalidate).unwrap();
    drop(s);
    let s = Shelf::open(dirs).unwrap().with_net(Net::new(c.clone()));
    assert_eq!(s.explore.status(&s).unwrap().limited_until, Some(reset));
    let other = api_url(&["repos", "octo", "x"], &[]);
    assert_eq!(get(&s, &other, Freshness::Revalidate).unwrap_err().code(), "rateLimited");
}

#[test]
fn offline_serves_saved_data_marked_stale_and_errors_without() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let u = api_url(&["repos", "octo", "demo"], &[]);
    c.on(u.as_str(), 200, &[], b"{}".to_vec());
    get(&s, &u, Freshness::Revalidate).unwrap();
    age_cache(&s, 300);
    // the next answer for this URL is a transport failure: drop the route by asking an unrouted URL for the other case
    let s2 = shelf(t.path(), &Canned::new());
    s2.db.with(|c| Ok(c.execute("INSERT INTO cache_gh_api SELECT * FROM cache_gh_api WHERE 0", [])?)).unwrap();
    // same DB is not shared; emulate by planting a row in s2 and asking with no route
    s2.db
        .with(|c| {
            c.execute("INSERT INTO cache_gh_api (url, body, size, fetched_at, last_used_at) VALUES (?1, x'7b7d', 2, 5, 5)", [u.as_str()])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(get(&s2, &u, Freshness::Revalidate).unwrap().body, b"{}");
    let st = s2.explore.status(&s2).unwrap();
    assert_eq!((st.stale.unwrap().reason, st.stale.unwrap().fetched_at), (StaleReason::Offline, 5));
    let other = api_url(&["repos", "octo", "none"], &[]);
    assert_eq!(get(&s2, &other, Freshness::Revalidate).unwrap_err().code(), "network");
}

#[test]
fn token_goes_to_api_github_com_only_and_changes_clear_the_cache() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let store = Arc::new(MemoryStore::default());
    let s = shelf(t.path(), &c).with_creds(store.clone());
    demo(&c);
    GithubSource::new(&s, "octo", "demo").unwrap().repo().unwrap();
    assert!(hdr(&c.seen(), 0, "Authorization").is_none());
    assert!(!s.explore.token_saved());
    // saving: store written, cache + quota reset, header on the next request
    s.explore.set_token(&s, &Token::parse(TOKEN).unwrap()).unwrap();
    assert_eq!(store.peek().unwrap().expose(), TOKEN);
    assert!(s.explore.token_saved());
    assert_eq!(s.explore.status(&s).unwrap().cache_entries, 0);
    GithubSource::new(&s, "octo", "demo").unwrap().repo().unwrap();
    let seen = c.seen();
    assert_eq!(hdr(&seen, 1, "Authorization"), Some(format!("Bearer {TOKEN}").as_str()));
    assert!(seen[1].0.starts_with("https://api.github.com/"));
    // removing
    s.explore.remove_token(&s).unwrap();
    assert!(store.peek().is_none() && !s.explore.token_saved());
    assert_eq!(s.explore.status(&s).unwrap().cache_entries, 0);
    GithubSource::new(&s, "octo", "demo").unwrap().repo().unwrap();
    assert!(hdr(&c.seen(), 2, "Authorization").is_none());
    // a saved token is picked up from the store at the first use (as after a restart)
    let s2 = shelf(t.path(), &c).with_creds(MemoryStore::with_token(TOKEN));
    assert!(s2.explore.token_saved());
    // images / assets never carry it: a README image fetch after saving the token
    c.on("https://img.example.com/a.png", 200, &[("content-type", "image/png")], b"\x89PNG\r\n\x1a\n0000".to_vec());
    let n = c.seen().len();
    crate::imgcache::fetch(&s2, "https://img.example.com/a.png").unwrap();
    assert!(hdr(&c.seen(), n, "Authorization").is_none());
}

#[test]
fn unauthorised_token_is_reported_without_echoing_it() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c).with_creds(MemoryStore::with_token(TOKEN));
    let u = api_url(&["repos", "octo", "demo"], &[]);
    c.on(u.as_str(), 401, &[], b"{\"message\":\"Bad credentials\"}".to_vec());
    let e = get(&s, &u, Freshness::Revalidate).unwrap_err();
    assert_eq!(e.code(), "refused");
    assert!(!e.to_string().contains(TOKEN) && e.to_string().contains("token"), "{e}");
}

#[test]
fn a_renamed_repository_follows_one_301_to_api_github_com_only() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let old = api_url(&["repos", "octo", "old"], &[]);
    c.on(old.as_str(), 301, &[("location", "https://api.github.com/repositories/1")], Vec::new())
        .on("https://api.github.com/repositories/1", 200, &[], b"{\"ok\":true}".to_vec());
    assert_eq!(get(&s, &old, Freshness::Revalidate).unwrap().body, b"{\"ok\":true}");
    assert_eq!(c.seen().len(), 2);
    // cached under the address that was asked
    assert_eq!(s.explore.status(&s).unwrap().cache_entries, 1);
    // other targets are never followed
    for (i, loc) in ["https://evil.example.com/x", "http://api.github.com/repositories/1", "https://api.github.com/user", "https://api.github.com:8443/repositories/1", "/repositories/1"].iter().enumerate() {
        let u = api_url(&["repos", "octo", &format!("bad{i}")], &[]);
        c.on(u.as_str(), 301, &[("location", loc)], Vec::new());
        let e = get(&s, &u, Freshness::Revalidate).unwrap_err();
        assert_eq!(e.code(), "refused", "{loc}");
    }
    assert!(!c.seen_urls().iter().any(|u| u.contains("evil") || u.starts_with("http:")));
}

#[test]
fn statuses_map_to_clear_errors() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    for (status, code) in [(404, "notFound"), (409, "notFound"), (410, "notFound"), (422, "invalidInput"), (451, "refused"), (403, "refused"), (500, "network"), (502, "network")] {
        let u = api_url(&["repos", "octo", &format!("s{status}")], &[]);
        c.on(u.as_str(), status, &[], Vec::new());
        assert_eq!(get(&s, &u, Freshness::Revalidate).unwrap_err().code(), code, "{status}");
    }
    let u = api_url(&["repos", "octo", "s404"], &[]);
    assert!(get(&s, &u, Freshness::Revalidate).unwrap_err().to_string().contains("private repositories need a token"));
    // an oversized answer is refused (profile cap) and not cached
    let big = api_url(&["repos", "octo", "big"], &[]);
    c.on(big.as_str(), 200, &[("content-length", "99999999")], vec![0u8; 10]);
    assert_eq!(get(&s, &big, Freshness::Revalidate).unwrap_err().code(), "refused");
}

#[test]
fn explore_off_refuses_everything_without_a_request() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    demo(&c);
    crate::settings::save(&s.db, &crate::settings::Settings { explore_enabled: false, ..Default::default() }).unwrap();
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let r = rev("main").unwrap();
    let resolved = crate::types::ResolvedRev { input: "main".into(), kind: RefKind::Branch, full_name: None, commit: C2.into(), tree: T2.into() };
    for res in [
        src.repo().map(|_| ()),
        src.refs().map(|_| ()),
        src.languages().map(|_| ()),
        src.resolve(&r).map(|_| ()),
        src.latest_commit(&resolved).map(|_| ()),
        src.tree(&resolved, &git_path("").unwrap()).map(|_| ()),
        src.readme(&resolved, &git_path("").unwrap()).map(|_| ()),
        src.blob_text(&resolved, &git_path("README.md").unwrap()).map(|_| ()),
        src.commits_page(&resolved, None, 0).map(|_| ()),
        src.commit_detail(C2).map(|_| ()),
        src.commit_file_diff(C2, 0, false).map(|_| ()),
        src.releases().map(|_| ()),
        search(&s, "demo", None, 1).map(|_| ()),
    ] {
        assert_eq!(res.unwrap_err().code(), "refused");
    }
    assert!(c.seen().is_empty());
    // the status still answers
    assert!(!s.explore.status(&s).unwrap().enabled);
}

// ---------------------------------------------------------------------------------------------------------------
// GithubSource

#[test]
fn repo_info_is_cleaned_and_checked() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let mut j = repo_json();
    j["description"] = json!("  a\u{0}b\u{1b}  ");
    j["homepage"] = json!("javascript:alert(1)");
    j["owner"]["avatar_url"] = json!("https://evil.example.com/a.png");
    j["topics"] = json!(["ok", "", "x".repeat(200)]);
    j["license"] = json!({"spdx_id": "NOASSERTION", "name": "Other"});
    put(&c, "", 200, &[], &j);
    let r = GithubSource::new(&s, "octo", "demo").unwrap().repo().unwrap();
    assert_eq!(r.description.as_deref(), Some("ab"));
    assert_eq!((r.homepage, r.owner_avatar_url), (None, None));
    assert_eq!(r.topics.len(), 2);
    assert_eq!(r.topics[1].len(), 50);
    assert_eq!(r.license.as_deref(), Some("Other"));
    assert_eq!((r.full_name.as_str(), r.clone_url.as_str()), ("octo/demo", "https://github.com/octo/demo.git"));
    // GitHub answering with a hostile name is refused
    let mut bad = repo_json();
    bad["name"] = json!("../x");
    let c2_ = Canned::new();
    put(&c2_, "", 200, &[], &bad);
    let s2 = shelf(t.path(), &c2_);
    assert_eq!(GithubSource::new(&s2, "octo", "demo").unwrap().repo().unwrap_err().code(), "invalidInput");
}

#[test]
fn hostile_addresses_are_refused_before_any_request() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    for (o, n) in [("..", "x"), ("a/b", "x"), ("octo", ".."), ("octo", "a/b"), ("octo", "a\\b"), ("-x", "r"), ("octo", "-r"), ("octo", "r.git"), ("o c", "r"), ("octo", "r?x=1"), ("", "r"), ("octo", "")] {
        assert_eq!(GithubSource::new(&s, o, n).err().unwrap().code(), "invalidInput", "{o}/{n}");
    }
    assert!(c.seen().is_empty());
}

#[test]
fn browse_a_repo_end_to_end() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    demo(&c);
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let info = src.repo().unwrap();
    assert_eq!((info.stars, info.language.as_deref(), info.default_branch.as_deref()), (120, Some("Rust"), Some("main")));
    assert_eq!(info.created_at, Some(1_600_000_000));
    assert_eq!(info.on_shelf, None);

    // refs first, so the kind of "main" / "v1.0.0" is known from the saved lists
    let refs = src.refs_ex().unwrap();
    assert_eq!(refs.refs.default_branch.as_deref(), Some("main"));
    assert_eq!(refs.refs.branches.iter().map(|b| (b.name.as_str(), b.is_default)).collect::<Vec<_>>(), [("main", true), ("dev", false)]);
    assert_eq!(refs.refs.branches[0].full_name, "refs/heads/main");
    assert_eq!((refs.refs.tags[0].name.as_str(), refs.refs.tags[0].kind, refs.refs.tags[0].target.as_str()), ("v1.0.0", RefKind::Tag, C1));
    assert!(!refs.branches_truncated && !refs.tags_truncated);

    let r = src.resolve(&rev("main").unwrap()).unwrap();
    assert_eq!((r.kind, r.full_name.as_deref(), r.commit.as_str(), r.tree.as_str()), (RefKind::Branch, Some("refs/heads/main"), C2, T2));
    let head = src.resolve(&rev("HEAD").unwrap()).unwrap();
    assert_eq!(head.commit, C2);

    let latest = src.latest_commit(&r).unwrap();
    assert_eq!((latest.commit.summary.as_str(), latest.count), ("Add main", Some(2)));
    assert_eq!(latest.commit.author.time.secs, 1_700_000_000);

    let root = src.tree(&r, &git_path("").unwrap()).unwrap();
    assert_eq!(root.entries.iter().map(|e| (e.name.as_str(), e.kind)).collect::<Vec<_>>(), [("src", EntryKind::Dir), ("README.md", EntryKind::File)]);
    assert_eq!(root.readme.as_deref(), Some("README.md"));
    assert_eq!(root.entries[1].size, Some(README.len() as u64));
    assert_eq!(root.entries[0].size, None);
    let sub = src.tree(&r, &git_path("src").unwrap()).unwrap();
    assert_eq!((sub.entries[0].path.as_str(), sub.entries[0].storable_here), ("src/main.rs", true));
    assert_eq!(src.tree(&r, &git_path("README.md").unwrap()).unwrap_err().code(), "notFound");
    assert_eq!(src.blob_text(&r, &git_path("src").unwrap()).unwrap_err().code(), "notFound");

    let bt = src.blob_text(&r, &git_path("src/main.rs").unwrap()).unwrap();
    assert_eq!(bt.text.as_deref(), Some(MAIN_RS));
    assert_eq!((bt.meta.kind, bt.meta.lines, bt.meta.lang_hint.as_deref(), bt.meta.size), (BlobKind::Text, Some(1), Some("rust"), 13));
    assert_eq!(src.blob_bytes(&r, &git_path("src/main.rs").unwrap(), 100).unwrap(), MAIN_RS.as_bytes());
    assert_eq!(src.blob_bytes(&r, &git_path("src/main.rs").unwrap(), 5).unwrap_err().code(), "refused");

    let rd = src.readme(&r, &git_path("").unwrap()).unwrap().unwrap();
    assert!(rd.html.contains("<h1") && rd.html.contains("<strong>world</strong>"), "{}", rd.html);
    assert_eq!(rd.source_path, "README.md");
    assert_eq!(rd.headings[0].text, "Demo");
    assert!(src.readme(&r, &git_path("src").unwrap()).is_err() || src.readme(&r, &git_path("src").unwrap()).unwrap().is_none());
    assert!(src.render_file(&r, &git_path("README.md").unwrap()).unwrap().html.contains("<h1"));

    let page = src.commits_page(&r, None, 0).unwrap();
    assert_eq!(page.commits.iter().map(|x| x.summary.as_str()).collect::<Vec<_>>(), ["Add main", "Initial"]);
    assert_eq!((page.tip.as_str(), page.skip, page.has_more), (C2, 2, false));
    assert_eq!(page.commits[1].parents.len(), 0);
    assert_eq!(src.commits_page(&r, None, 7).unwrap_err().code(), "invalidInput");

    let d = src.commit_detail(C2).unwrap();
    assert_eq!((d.commit.summary.as_str(), d.message.as_str()), ("Add main", "Add main\n\nSecond commit body."));
    assert_eq!((d.parent_compared.as_deref(), d.is_merge, d.additions, d.deletions, d.diffs_deferred), (Some(C1), false, 1, 0, false));
    assert_eq!((d.files[0].kind, d.files[0].new_path.as_deref(), d.files[0].old_path.clone()), (ChangeKind::Added, Some("src/main.rs"), None));
    assert_eq!(d.diffs.len(), 1);
    assert_eq!((d.diffs[0].hunks[0].lines[0].kind, d.diffs[0].hunks[0].lines[0].text.as_str()), (LineKind::Add, "fn main() {}"));
    let fd = src.commit_file_diff(C2, 0, false).unwrap();
    assert_eq!(fd.hunks.len(), 1);
    assert_eq!(src.commit_file_diff(C2, 5, false).unwrap_err().code(), "invalidInput");
    assert_eq!(src.commit_detail("main").unwrap_err().code(), "invalidRev");

    let l = src.languages().unwrap();
    assert_eq!(l.items.iter().map(|x| (x.name.as_str(), x.bytes)).collect::<Vec<_>>(), [("Rust", 900), ("Shell", 100)]);
    assert!((l.items[0].percent - 90.0).abs() < 0.01);

    let rel = src.releases().unwrap();
    assert_eq!(rel.iter().map(|x| x.tag.as_str()).collect::<Vec<_>>(), ["v2.0.0-rc1", "v1.0.0"], "drafts are dropped");
    assert_eq!((rel[0].title.as_str(), rel[0].latest, rel[0].prerelease, rel[1].latest), ("v2.0.0-rc1", false, true, true));
    assert_eq!((rel[0].previous_tag.as_deref(), rel[1].previous_tag.as_deref()), (Some("v1.0.0"), None));
    assert_eq!((rel[1].title.as_str(), rel[1].date, rel[1].commit.as_str()), ("First", 1_700_000_000 - 60, ""));
    assert!(rel[1].notes_html.as_deref().unwrap().contains("<h1"));
    assert_eq!(rel[1].assets.len(), 1, "an asset outside github.com is dropped");
    assert_eq!(rel[1].assets[0].download_count, Some(4));
    assert_eq!(rel[1].source_zip_url.as_deref(), Some("https://github.com/octo/demo/archive/refs/tags/v1.0.0.zip"));
    assert_eq!(rel[1].html_url.as_deref(), Some("https://github.com/octo/demo/releases/tag/v1.0.0"));
}

#[test]
fn sha_addressed_requests_are_made_once_and_the_commit_count_comes_from_the_link_header() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    demo(&c);
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let r = src.resolve(&rev("main").unwrap()).unwrap();
    src.latest_commit(&r).unwrap(); // same saved answer as the resolve (name-addressed)
    assert_eq!(c.seen().len(), 1, "resolve + latest + count = one request: {:?}", c.seen_urls());
    src.tree(&r, &git_path("").unwrap()).unwrap();
    src.blob_text(&r, &git_path("README.md").unwrap()).unwrap();
    let n = c.seen().len();
    age_cache(&s, 10_000);
    src.tree(&r, &git_path("").unwrap()).unwrap();
    src.blob_text(&r, &git_path("README.md").unwrap()).unwrap();
    src.commits_page(&r, None, 0).unwrap();
    src.commits_page(&r, None, 0).unwrap();
    assert_eq!(c.seen().len(), n + 1, "only the first commits page was requested: {:?}", c.seen_urls());
    // a repo page cold costs: resolve(1) tree(1) readme(1) + repo(1) + languages(1)
    let c2_ = Canned::new();
    demo(&c2_);
    let s2 = shelf(t.path(), &c2_);
    let src = GithubSource::new(&s2, "octo", "demo").unwrap();
    src.repo().unwrap();
    let r = src.resolve(&rev("main").unwrap()).unwrap();
    src.latest_commit(&r).unwrap();
    src.tree(&r, &git_path("").unwrap()).unwrap();
    src.readme(&r, &git_path("").unwrap()).unwrap();
    src.languages().unwrap();
    assert_eq!(c2_.seen().len(), 5, "{:?}", c2_.seen_urls());
}

#[test]
fn big_binary_symlink_and_submodule_files() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    demo(&c);
    let big = json!({"type": "file", "size": 5_000_000, "name": "big.bin", "path": "big.bin", "sha": B_MAIN, "content": "", "encoding": "none"});
    put(&c, &format!("/contents/big.bin?ref={C2}"), 200, &[], &big);
    let mut big_png = big.clone();
    big_png["name"] = json!("big.png");
    put(&c, &format!("/contents/big.png?ref={C2}"), 200, &[], &big_png);
    let mut bin = file_json("a.bin", "a.bin", B_MAIN, "x");
    bin["content"] = json!(base64::engine::general_purpose::STANDARD.encode([0u8, 1, 2]));
    bin["size"] = json!(3);
    put(&c, &format!("/contents/a.bin?ref={C2}"), 200, &[], &bin);
    put(&c, &format!("/contents/link?ref={C2}"), 200, &[], &json!({"type": "symlink", "size": 9, "name": "link", "path": "link", "sha": B_MAIN, "target": "README.md"}));
    put(&c, &format!("/contents/sub?ref={C2}"), 200, &[], &json!({"type": "submodule", "size": 0, "name": "sub", "path": "sub", "sha": B_MAIN, "submodule_git_url": "https://github.com/o/s.git"}));
    let mut lfs = file_json("m.dat", "m.dat", B_MAIN, "version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 12345\n");
    lfs["size"] = json!("version https://git-lfs.github.com/spec/v1
oid sha256:abc
size 12345
".len());
    put(&c, &format!("/contents/m.dat?ref={C2}"), 200, &[], &lfs);
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let r = crate::types::ResolvedRev { input: C2.into(), kind: RefKind::Commit, full_name: None, commit: C2.into(), tree: T2.into() };
    let m = src.blob_text(&r, &git_path("big.bin").unwrap()).unwrap();
    assert!(!m.meta.displayable && m.text.is_none() && m.meta.size == 5_000_000, "{m:?}");
    assert_eq!(src.blob_bytes(&r, &git_path("big.bin").unwrap(), u64::MAX).unwrap_err().code(), "refused");
    assert_eq!(src.blob_meta(&r, &git_path("big.png").unwrap()).unwrap().kind, BlobKind::Image { mime: "image/png".into() });
    assert_eq!(src.blob_meta(&r, &git_path("a.bin").unwrap()).unwrap().kind, BlobKind::Binary);
    assert_eq!(src.blob_meta(&r, &git_path("link").unwrap()).unwrap().kind, BlobKind::Symlink { target: "README.md".into() });
    assert_eq!(src.blob_meta(&r, &git_path("sub").unwrap()).unwrap().kind, BlobKind::Submodule { url: Some("https://github.com/o/s.git".into()) });
    assert_eq!(src.blob_bytes(&r, &git_path("sub").unwrap(), 10).unwrap_err().code(), "refused");
    assert_eq!(src.blob_meta(&r, &git_path("m.dat").unwrap()).unwrap().kind, BlobKind::LfsPointer { size: Some(12345) });
    // a truncated / corrupt content is refused
    let mut short = file_json("s.txt", "s.txt", B_MAIN, "hello");
    short["size"] = json!(99);
    put(&c, &format!("/contents/s.txt?ref={C2}"), 200, &[], &short);
    assert_eq!(src.blob_text(&r, &git_path("s.txt").unwrap()).unwrap_err().code(), "invalidInput");
}

#[test]
fn hostile_entries_in_a_listing_are_skipped() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    put(
        &c,
        &format!("/contents?ref={C2}"),
        200,
        &[],
        &json!([
            file_entry("ok.txt", "ok.txt", B_MAIN, 1, "file"),
            file_entry("../evil", "../evil", B_MAIN, 1, "file"),
            file_entry("a/b", "a/b", B_MAIN, 1, "file"),
            file_entry("", "", B_MAIN, 1, "file"),
            file_entry("bad-sha", "bad-sha", "zzz", 1, "file"),
            file_entry("weird", "weird", B_MAIN, 1, "unknown"),
            file_entry("aux", "aux", B_MAIN, 1, "file")
        ]),
    );
    let r = crate::types::ResolvedRev { input: C2.into(), kind: RefKind::Commit, full_name: None, commit: C2.into(), tree: T2.into() };
    let l = GithubSource::new(&s, "octo", "demo").unwrap().tree(&r, &git_path("").unwrap()).unwrap();
    assert_eq!(l.entries.iter().map(|e| (e.name.as_str(), e.storable_here)).collect::<Vec<_>>(), [("aux", false), ("ok.txt", true)]);
}

#[test]
fn big_commits_defer_diffs_and_files_without_patch_are_marked() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let mut full = c2();
    full["stats"] = json!({"additions": 30_000, "deletions": 0, "total": 30_000});
    full["files"] = json!([
        {"filename": "a.rs", "status": "modified", "additions": 30_000, "deletions": 0, "changes": 30_000, "patch": "@@ -1 +1 @@\n+x"},
        {"filename": "img.png", "status": "modified", "additions": 0, "deletions": 0, "changes": 0},
        {"filename": "huge.json", "status": "added", "additions": 9000, "deletions": 0, "changes": 9000},
        {"filename": "new.rs", "previous_filename": "old.rs", "status": "renamed", "additions": 0, "deletions": 0, "changes": 0},
        {"filename": "gone.rs", "status": "removed", "additions": 0, "deletions": 3, "changes": 3, "patch": "@@ -1,3 +0,0 @@\n-a\n-b\n-c"}
    ]);
    put(&c, &format!("/commits/{C2}"), 200, &[], &full);
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let d = src.commit_detail(C2).unwrap();
    assert!(d.diffs_deferred && d.diffs.is_empty());
    let f = &d.files;
    assert_eq!((f[1].binary, f[1].too_large), (true, false));
    assert_eq!((f[2].binary, f[2].too_large), (false, true));
    assert_eq!((f[3].kind, f[3].old_path.as_deref(), f[3].new_path.as_deref(), f[3].binary), (ChangeKind::Renamed, Some("old.rs"), Some("new.rs"), false));
    assert_eq!((f[4].kind, f[4].new_path.clone()), (ChangeKind::Deleted, None));
    // "Load diff" still works for a file that has a patch, and refuses one without
    assert_eq!(src.commit_file_diff(C2, 4, false).unwrap().hunks[0].lines.len(), 3);
    assert_eq!(src.commit_file_diff(C2, 1, true).unwrap_err().code(), "refused");
}

#[test]
fn search_pages_validation_and_on_shelf() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let mut item = repo_json();
    let mut bad = repo_json();
    bad["name"] = json!("a/b");
    item["stargazers_count"] = json!(5);
    let body = json!({"total_count": 1234, "incomplete_results": false, "items": [item, bad]});
    let url = "https://api.github.com/search/repositories?q=demo+language%3Arust&per_page=30&page=1&sort=stars&order=desc";
    c.on(url, 200, &[("link", "<https://api.github.com/search/repositories?page=2>; rel=\"next\""), ("x-ratelimit-resource", "search"), ("x-ratelimit-limit", "10"), ("x-ratelimit-remaining", "9"), ("x-ratelimit-reset", "4102444800")], body.to_string().into_bytes());
    // an imported repo with the same github origin is "on the shelf"
    let mut b = testkit::repo(&t.path().join("r"));
    b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://github.com/octo/demo.git");
    let id = crate::library::import(&s, &t.path().join("r")).unwrap().id;
    let r = search(&s, "  demo language:rust ", Some("stars"), 1).unwrap();
    assert_eq!((r.total_count, r.page, r.has_more, r.items.len()), (1234, 1, true, 1), "the oddly spelled row is skipped");
    assert_eq!((r.items[0].full_name.as_str(), r.items[0].stars, r.items[0].on_shelf), ("octo/demo", 5, Some(id)));
    assert_eq!(r.query, "demo language:rust");
    assert_eq!(s.explore.status(&s).unwrap().search.unwrap().limit, 10);
    for (q, sort, page) in [("", None, 1), ("   ", None, 1), ("a\nb", None, 1), (&"x".repeat(257), None, 1), ("ok", Some("nonsense"), 1), ("ok", None, 0), ("ok", None, 35)] {
        assert_eq!(search(&s, q, sort, page).unwrap_err().code(), "invalidInput", "{q:?} {sort:?} {page}");
    }
    assert_eq!(c.seen().len(), 1);
}

#[test]
fn refs_are_truncated_after_three_pages() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    put(&c, "", 200, &[], &repo_json());
    put(&c, "/branches?per_page=100", 200, &[], &json!([]));
    let next = [("link", "<https://api.github.com/x?page=2>; rel=\"next\"")];
    let many = |n: usize, p: usize| Value::Array((0..n).map(|i| json!({"name": format!("t{p}-{i}"), "commit": {"sha": C1, "url": "u"}})).collect());
    put(&c, "/tags?per_page=100", 200, &next, &many(100, 1));
    put(&c, "/tags?per_page=100&page=2", 200, &next, &many(100, 2));
    put(&c, "/tags?per_page=100&page=3", 200, &next, &many(100, 3));
    let r = GithubSource::new(&s, "octo", "demo").unwrap().refs_ex().unwrap();
    assert_eq!(r.refs.tags.len(), 300);
    assert!(r.tags_truncated && !r.branches_truncated);
}

// ---------------------------------------------------------------------------------------------------------------
// Parity: the same assertions against the git object DB and the GitHub API

fn parity(src: &dyn RepoSource, author: &str) {
    let r = src.resolve(&rev("main").unwrap()).unwrap();
    let root = src.tree(&r, &git_path("").unwrap()).unwrap();
    assert_eq!(root.entries.iter().map(|e| (e.name.as_str(), e.kind, e.path.as_str())).collect::<Vec<_>>(), [("src", EntryKind::Dir, "src"), ("README.md", EntryKind::File, "README.md")]);
    assert_eq!(root.readme.as_deref(), Some("README.md"));
    assert_eq!(root.path, "");
    let sub = src.tree(&r, &git_path("src").unwrap()).unwrap();
    assert_eq!(sub.entries.iter().map(|e| (e.name.as_str(), e.path.as_str(), e.size)).collect::<Vec<_>>(), [("main.rs", "src/main.rs", Some(13))]);
    assert_eq!(src.tree(&r, &git_path("nope").unwrap()).unwrap_err().code(), "notFound");
    let bt = src.blob_text(&r, &git_path("src/main.rs").unwrap()).unwrap();
    assert_eq!(bt.text.as_deref(), Some(MAIN_RS));
    assert_eq!((bt.meta.kind, bt.meta.lines, bt.meta.sloc, bt.meta.lang_hint.as_deref(), bt.meta.displayable, bt.meta.lossy), (BlobKind::Text, Some(1), Some(1), Some("rust"), true, false));
    assert_eq!(src.blob_bytes(&r, &git_path("README.md").unwrap(), 1000).unwrap(), README.as_bytes());
    assert_eq!(src.blob_text(&r, &git_path("src").unwrap()).unwrap_err().code(), "notFound");
    let rd = src.readme(&r, &git_path("").unwrap()).unwrap().unwrap();
    assert_eq!((rd.source_path.as_str(), rd.headings.iter().map(|h| h.text.as_str()).collect::<Vec<_>>()), ("README.md", vec!["Demo"]));
    assert!(rd.html.contains("<strong>world</strong>"));
    assert_eq!(src.render_file(&r, &git_path("README.md").unwrap()).unwrap().html, rd.html);
    let latest = src.latest_commit(&r).unwrap();
    assert_eq!(latest.commit.summary.as_str(), "Add main");
    assert!(matches!(latest.count, None | Some(2)), "{:?}", latest.count);
    let page = src.commits_page(&r, None, 0).unwrap();
    assert_eq!(page.commits.iter().map(|c| c.summary.as_str()).collect::<Vec<_>>(), ["Add main", "Initial"]);
    assert_eq!((page.skip, page.has_more, page.tip.as_str()), (2, false, r.commit.as_str()));
    assert_eq!(page.commits[0].parents, vec![page.commits[1].oid.clone()]);
    assert_eq!(page.commits[0].author.name, author);
    let d = src.commit_detail(&r.commit).unwrap();
    assert_eq!((d.additions, d.deletions, d.is_merge, d.diffs_deferred, d.files.len()), (1, 0, false, false, 1));
    assert_eq!((d.files[0].kind, d.files[0].new_path.as_deref(), d.files[0].old_path.as_deref()), (ChangeKind::Added, Some("src/main.rs"), None));
    assert_eq!(d.diffs.len(), 1);
    let line = &d.diffs[0].hunks[0].lines[0];
    assert_eq!((line.kind, line.old_no, line.new_no, line.text.as_str()), (LineKind::Add, None, Some(1), "fn main() {}"));
    assert_eq!(d.message.trim_end(), "Add main\n\nSecond commit body.");
    let refs = src.refs().unwrap();
    assert_eq!(refs.default_branch.as_deref(), Some("main"));
    assert!(refs.branches.iter().any(|b| b.name == "main" && b.is_default));
    assert!(refs.tags.iter().any(|t| t.name == "v1.0.0"));
}

#[test]
fn local_and_github_sources_agree() {
    let t = tempfile::tempdir().unwrap();
    // local: the same history as the canned GitHub fixture
    let s = testkit::shelf(t.path());
    let mut b = testkit::repo(&t.path().join("demo"));
    b.file("README.md", README.as_bytes()).commit("Initial", testkit::T0).tag("v1.0.0", false);
    b.file("src/main.rs", MAIN_RS.as_bytes()).commit("Add main\n\nSecond commit body.", testkit::T0 + 60).set_head();
    let id = testkit::register_raw(&s, &t.path().join("demo"), crate::types::Source::Imported).unwrap();
    let local = LocalSource::new(&s, id);
    let author = local.commits_page(&local.resolve(&rev("main").unwrap()).unwrap(), None, 0).unwrap().commits[0].author.name.clone();
    parity(&local, &author);

    let c = Canned::new();
    demo(&c);
    let s2 = shelf(t.path(), &c);
    parity(&GithubSource::new(&s2, "octo", "demo").unwrap(), "Mona Octocat");
}

#[test]
fn a_saved_token_revalidates_names_after_a_minute_anonymous_after_fifteen() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c).with_creds(MemoryStore::with_token(TOKEN));
    let u = api_url(&["repos", "octo", "demo"], &[]);
    c.on(u.as_str(), 200, &[("etag", "\"e\"")], b"{}".to_vec()).on(u.as_str(), 304, &[], Vec::new());
    get(&s, &u, Freshness::Revalidate).unwrap();
    age_cache(&s, 30);
    get(&s, &u, Freshness::Revalidate).unwrap();
    assert_eq!(c.seen().len(), 1);
    age_cache(&s, 90);
    get(&s, &u, Freshness::Revalidate).unwrap();
    assert_eq!(c.seen().len(), 2, "a 304 is free with Authorization, so 60 s is enough");
}

/// GitHub refuses files over 1 MB for the plain JSON type (403 `too_large`): the answer is built from the listing.
#[test]
fn a_file_over_one_megabyte_is_metadata_only_even_when_github_refuses_the_contents_call() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    put(&c, &format!("/contents?ref={C2}"), 200, &[], &json!([file_entry("big.bin", "big.bin", B_MAIN, 5_000_000, "file"), file_entry("small.txt", "small.txt", B_MAIN, 5, "file")]));
    let too_large = json!({"message": "This API returns blobs up to 1 MB in size. The requested blob is too large to fetch via the API.", "errors": [{"resource": "Blob", "field": "data", "code": "too_large"}]});
    put(&c, &format!("/contents/big.bin?ref={C2}"), 403, &[], &too_large);
    put(&c, &format!("/contents/small.txt?ref={C2}"), 403, &[], &too_large);
    put(&c, &format!("/contents/gone.bin?ref={C2}"), 403, &[], &too_large);
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let r = crate::types::ResolvedRev { input: C2.into(), kind: RefKind::Commit, full_name: None, commit: C2.into(), tree: T2.into() };
    let m = src.blob_text(&r, &git_path("big.bin").unwrap()).unwrap();
    assert!(!m.meta.displayable && m.text.is_none() && m.meta.size == 5_000_000 && m.meta.oid == B_MAIN, "{m:?}");
    assert_eq!(src.blob_bytes(&r, &git_path("big.bin").unwrap(), u64::MAX).unwrap_err().code(), "refused");
    // a small file GitHub refuses for another reason, and an unlisted one, keep the original refusal
    assert_eq!(src.blob_meta(&r, &git_path("small.txt").unwrap()).unwrap_err().code(), "refused");
    assert_eq!(src.blob_meta(&r, &git_path("gone.bin").unwrap()).unwrap_err().code(), "refused");
}

#[test]
fn hostile_markdown_is_sanitised_on_the_network_path_too() {
    let t = tempfile::tempdir().unwrap();
    let c = Canned::new();
    let s = shelf(t.path(), &c);
    let md = include_str!("../../../../tests/fixtures/security/README.md");
    put(&c, &format!("/contents?ref={C2}"), 200, &[], &json!([file_entry("README.md", "README.md", B_README, md.len(), "file")]));
    put(&c, &format!("/contents/README.md?ref={C2}"), 200, &[], &file_json("README.md", "README.md", B_README, md));
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    let r = crate::types::ResolvedRev { input: C2.into(), kind: RefKind::Commit, full_name: None, commit: C2.into(), tree: T2.into() };
    let rd = src.readme(&r, &git_path("").unwrap()).unwrap().unwrap();
    let local = crate::markdown::render(md, &git_path("").unwrap());
    assert_eq!(rd.html, local.html, "the same sanitiser output as a local README");
    let lower = rd.html.to_lowercase();
    for bad in ["<script", "onerror", "onclick", "javascript:", "<iframe", "<object", "<embed", "style="] {
        assert!(!lower.contains(bad), "{bad} survived: {}", rd.html);
    }
    // release bodies (and avatars) too
    let hostile = "<script>alert(1)</script><img src=x onerror=alert(2)> [x](javascript:alert(3)) <img src=\"https://avatars.githubusercontent.com/u/1\">";
    put(&c, "/releases?per_page=100", 200, &[], &json!([{"tag_name": "v1", "body": hostile, "draft": false, "prerelease": false, "assets": []}]));
    let rel = src.releases().unwrap();
    let html = rel[0].notes_html.clone().unwrap().to_lowercase();
    for bad in ["<script", "onerror", "href=\"javascript", "avatars.githubusercontent.com"] {
        assert!(!html.contains(bad), "{bad} survived: {html}");
    }
}

#[test]
fn a_read_only_library_still_browses_without_saving() {
    let t = tempfile::tempdir().unwrap();
    let dirs = crate::paths::Dirs { data: t.path().join("d"), archive: t.path().join("a") };
    {
        let s = Shelf::open(dirs.clone()).unwrap();
        s.db.with(|c| Ok(c.pragma_update(None, "user_version", crate::db::SCHEMA_VERSION + 1)?)).unwrap();
    }
    let c = Canned::new();
    demo(&c);
    let s = Shelf::open(dirs).unwrap().with_net(Net::new(c.clone()));
    assert!(s.db.read_only());
    let src = GithubSource::new(&s, "octo", "demo").unwrap();
    assert_eq!(src.repo().unwrap().stars, 120);
    src.repo().unwrap();
    assert_eq!(c.seen().len(), 2, "nothing could be saved, so the second call asked again");
    assert_eq!(s.explore.status(&s).unwrap().cache_entries, 0);
}
