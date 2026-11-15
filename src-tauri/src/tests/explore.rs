//! M10a command-layer tests for Explore (v0.3.0): validation, the off switch, the token (never leaks), Save to shelf,
//! settings / cache integration. Everything runs over canned GitHub answers and an in-memory credential store.

use super::*;
use crate::cmd::explore as ex;
use rs_core::explore::cred::MemoryStore;
use rs_core::net::{Canned, Net};
use rs_core::paths::Dirs;
use serde_json::json;

const TOKEN: &str = "ghp_Sup3rSecretTokenValue_0123456789";
const C2: &str = "2222222222222222222222222222222222222222";

fn repo_body(owner: &str, name: &str) -> Vec<u8> {
    json!({"name": name, "owner": {"login": owner}, "default_branch": "main", "description": "demo", "stargazers_count": 3}).to_string().into_bytes()
}

struct GhEnv {
    _t: tempfile::TempDir,
    st: AppState,
    canned: Arc<Canned>,
    store: Arc<MemoryStore>,
    worked: Arc<Mutex<Vec<JobKind>>>,
    dirs: Dirs,
}

/// File-backed shelf (so the DB files can be scanned) with a canned network and a fake credential store.
fn gh_env() -> GhEnv {
    let t = tempfile::tempdir().unwrap();
    let dirs = Dirs { data: t.path().join("data"), archive: t.path().join("archive") };
    let canned = Canned::new();
    let store = Arc::new(MemoryStore::default());
    let shelf = Arc::new(Shelf::open(dirs.clone()).unwrap().with_net(Net::new(canned.clone())).with_creds(store.clone()));
    std::fs::create_dir_all(&shelf.dirs.archive).unwrap();
    let worked = Arc::new(Mutex::new(Vec::new()));
    let st = AppState::with_worker(
        shelf,
        false,
        fake_worker(worked.clone()),
        Arc::new(FakePicker::default()),
        Arc::new(FakeTrash::default()),
        Arc::new(FakeOpener::default()),
    );
    GhEnv { _t: t, st, canned, store, worked, dirs }
}

fn route_demo(c: &Canned) {
    c.on("https://api.github.com/repos/octo/demo", 200, &[("etag", "\"r\"")], repo_body("octo", "demo"));
    c.on(
        "https://api.github.com/repos/octo/demo/commits?sha=main&per_page=1",
        200,
        &[],
        json!([{"sha": C2, "commit": {"message": "m", "author": {"name": "A", "email": "a@x", "date": "2023-11-14T22:13:20Z"}, "tree": {"sha": "b".repeat(40)}}, "parents": []}]).to_string().into_bytes(),
    );
    c.on(&format!("https://api.github.com/repos/octo/demo/contents?ref={C2}"), 200, &[], b"[]".to_vec());
}

/// Every explore command that takes an address, as a closure over (owner, name).
type Call = Box<dyn Fn(&AppState, &str, &str) -> Result<()>>;
fn address_calls() -> Vec<(&'static str, Call)> {
    vec![
        ("explore_repo", Box::new(|st, o, n| ok(ex::explore_repo_impl(st, o, n)))),
        ("explore_refs", Box::new(|st, o, n| ok(ex::explore_refs_impl(st, o, n)))),
        ("explore_languages", Box::new(|st, o, n| ok(ex::explore_languages_impl(st, o, n)))),
        ("explore_resolve", Box::new(|st, o, n| ok(ex::explore_resolve_impl(st, o, n, "main")))),
        ("explore_commit_latest", Box::new(|st, o, n| ok(ex::explore_commit_latest_impl(st, o, n, "main")))),
        ("explore_tree", Box::new(|st, o, n| ok(ex::explore_tree_impl(st, o, n, "main", "")))),
        ("explore_readme", Box::new(|st, o, n| ok(ex::explore_readme_impl(st, o, n, "main", "")))),
        ("explore_render_file", Box::new(|st, o, n| ok(ex::explore_render_file_impl(st, o, n, "main", "README.md")))),
        ("explore_blob_meta", Box::new(|st, o, n| ok(ex::explore_blob_meta_impl(st, o, n, "main", "a.rs")))),
        ("explore_blob_text", Box::new(|st, o, n| ok(ex::explore_blob_text_impl(st, o, n, "main", "a.rs")))),
        ("explore_blob_bytes", Box::new(|st, o, n| ok(ex::explore_blob_bytes_impl(st, o, n, "main", "a.rs")))),
        ("explore_commits", Box::new(|st, o, n| ok(ex::explore_commits_impl(st, o, n, "main", None, 0, None)))),
        ("explore_commit", Box::new(|st, o, n| ok(ex::explore_commit_impl(st, o, n, C2)))),
        ("explore_commit_file_diff", Box::new(|st, o, n| ok(ex::explore_commit_file_diff_impl(st, o, n, C2, 0, false)))),
        ("explore_releases", Box::new(|st, o, n| ok(ex::explore_releases_impl(st, o, n)))),
        ("explore_save_to_shelf", Box::new(|st, o, n| ok(ex::explore_save_to_shelf_impl(st, o, n, None)))),
    ]
}

#[test]
fn hostile_addresses_revs_paths_and_oids_are_refused_before_any_request() {
    let e = gh_env();
    for (name, call) in address_calls() {
        for (o, n) in [("..", "demo"), ("octo", ".."), ("a/b", "demo"), ("octo", "a/b"), ("octo", "a\\b"), ("-x", "demo"), ("octo", "-x"), ("", "demo"), ("octo", ""), ("o c", "demo"), ("octo", "demo.git"), ("octo", "d?x"), ("o\0", "d")] {
            assert_eq!(code(call(&e.st, o, n).map(|_| ())), "invalidInput", "{name} {o:?}/{n:?}");
        }
    }
    // revs / paths / oids / skip
    for bad in ["", "..", "a..b", "HEAD~1", "@{-1}", "--upload-pack=x", "a b", "a\\b", "a:b"] {
        assert_eq!(code(ex::explore_tree_impl(&e.st, "octo", "demo", bad, "")), "invalidRev", "rev {bad:?}");
        assert_eq!(code(ex::explore_resolve_impl(&e.st, "octo", "demo", bad)), "invalidRev", "rev {bad:?}");
        assert_eq!(code(ex::explore_commits_impl(&e.st, "octo", "demo", bad, None, 0, None)), "invalidRev", "rev {bad:?}");
    }
    for bad in ["..", "a/../b", "/etc", "C:\\x", "c:/x", "a\\b", "a\0b", "a//b", "a/"] {
        assert_eq!(code(ex::explore_tree_impl(&e.st, "octo", "demo", "main", bad)), "invalidPath", "path {bad:?}");
        assert_eq!(code(ex::explore_readme_impl(&e.st, "octo", "demo", "main", bad)), "invalidPath", "path {bad:?}");
        assert_eq!(code(ex::explore_render_file_impl(&e.st, "octo", "demo", "main", bad)), "invalidPath", "path {bad:?}");
        assert_eq!(code(ex::explore_blob_meta_impl(&e.st, "octo", "demo", "main", bad)), "invalidPath", "path {bad:?}");
        assert_eq!(code(ex::explore_blob_text_impl(&e.st, "octo", "demo", "main", bad)), "invalidPath", "path {bad:?}");
        assert_eq!(code(ex::explore_blob_bytes_impl(&e.st, "octo", "demo", "main", bad)), "invalidPath", "path {bad:?}");
        assert_eq!(code(ex::explore_commits_impl(&e.st, "octo", "demo", "main", Some(bad), 0, None)), "invalidPath", "path {bad:?}");
    }
    for bad in ["", "main", "abc", &"g".repeat(40), &"a".repeat(41)] {
        assert_eq!(code(ex::explore_commit_impl(&e.st, "octo", "demo", bad)), "invalidRev", "oid {bad:?}");
        assert_eq!(code(ex::explore_commit_file_diff_impl(&e.st, "octo", "demo", bad, 0, false)), "invalidRev", "oid {bad:?}");
        assert_eq!(code(ex::explore_commits_impl(&e.st, "octo", "demo", "main", None, 0, Some(bad))), "invalidRev", "tip {bad:?}");
    }
    assert_eq!(code(ex::explore_commits_impl(&e.st, "octo", "demo", "main", None, super::super::cmd::MAX_SKIP + 1, None)), "invalidInput");
    assert_eq!(code(ex::explore_commit_file_diff_impl(&e.st, "octo", "demo", C2, super::super::cmd::MAX_FILE_INDEX + 1, false)), "invalidInput");
    // search words and token text
    for (q, sort, page) in [("", None, 1), ("a\nb", None, 1), ("x", Some("bogus"), 1), ("x", None, 0), ("x", None, 99)] {
        assert_eq!(code(ex::explore_search_impl(&e.st, q, sort, page)), "invalidInput", "{q:?}");
    }
    for bad in ["", "has space", "a\nb", "a/b", &"x".repeat(300)] {
        assert_eq!(code(ex::explore_token_set_impl(&e.st, bad)), "invalidInput");
    }
    assert!(e.canned.seen().is_empty(), "no refusal reached the transport: {:?}", e.canned.seen_urls());
    assert!(e.store.peek().is_none());
}

#[test]
fn every_explore_command_refuses_when_explore_is_off() {
    let e = gh_env();
    route_demo(&e.canned);
    app::settings_set_impl(&e.st, rs_core::settings::Settings { explore_enabled: false, ..Default::default() }).unwrap();
    for (name, call) in address_calls() {
        assert_eq!(code(call(&e.st, "octo", "demo")), "refused", "{name}");
    }
    assert_eq!(code(ex::explore_search_impl(&e.st, "demo", None, 1)), "refused");
    assert!(e.canned.seen().is_empty());
    // status, token handling and the cache button still work while it is off
    assert!(!ex::explore_status_impl(&e.st).unwrap().enabled);
    ex::explore_token_set_impl(&e.st, TOKEN).unwrap();
    assert!(ex::explore_status_impl(&e.st).unwrap().token_saved);
    ex::explore_token_remove_impl(&e.st).unwrap();
    ex::explore_cache_clear_impl(&e.st).unwrap();
    assert!(e.canned.seen().is_empty(), "turning Explore off also skips the token check request");
    // and on again
    app::settings_set_impl(&e.st, rs_core::settings::Settings::default()).unwrap();
    assert_eq!(ex::explore_repo_impl(&e.st, "octo", "demo").unwrap().stars, 3);
}

#[test]
fn settings_carry_the_token_flag_from_the_store_and_ignore_what_the_ui_sends() {
    let e = gh_env();
    e.canned.on("https://api.github.com/rate_limit", 200, &[("x-ratelimit-limit", "5000"), ("x-ratelimit-remaining", "4999"), ("x-ratelimit-reset", "4102444800")], b"{}".to_vec());
    let s = app::settings_get_impl(&e.st).unwrap();
    assert!(s.explore_enabled && !s.github_token_saved);
    // the UI claiming a token is saved changes nothing
    let s = app::settings_set_impl(&e.st, rs_core::settings::Settings { github_token_saved: true, ..Default::default() }).unwrap();
    assert!(!s.github_token_saved);
    let st = ex::explore_token_set_impl(&e.st, &format!("  {TOKEN}  ")).unwrap();
    assert!(st.token_saved);
    assert_eq!(st.core.as_ref().map(|q| (q.limit, q.remaining)), Some((5000, 4999)), "quota of the token, from /rate_limit");
    assert_eq!(e.store.peek().unwrap().expose(), TOKEN);
    assert!(app::settings_get_impl(&e.st).unwrap().github_token_saved);
    // saving settings again (the UI sends the whole object) does not touch the token
    let s = app::settings_set_impl(&e.st, rs_core::settings::Settings { github_token_saved: false, theme: rs_core::types::Theme::Dark, ..Default::default() }).unwrap();
    assert!(s.github_token_saved && e.store.peek().is_some());
    ex::explore_token_remove_impl(&e.st).unwrap();
    assert!(!app::settings_get_impl(&e.st).unwrap().github_token_saved && e.store.peek().is_none());
}

#[test]
fn a_rejected_token_is_removed_again_and_never_echoed() {
    let e = gh_env();
    e.canned.on("https://api.github.com/rate_limit", 401, &[], b"{\"message\":\"Bad credentials\"}".to_vec());
    let err = ex::explore_token_set_impl(&e.st, TOKEN).unwrap_err();
    assert_eq!(err.code(), "refused");
    assert!(!err.to_string().contains(TOKEN));
    assert!(e.store.peek().is_none() && !ex::explore_status_impl(&e.st).unwrap().token_saved);
    // an unreachable GitHub (no canned answer) keeps the token: it may be fine
    let e = gh_env();
    ex::explore_token_set_impl(&e.st, TOKEN).unwrap();
    assert!(e.store.peek().is_some());
}

/// The token reaches only the credential store and the `Authorization` header of api.github.com requests: not the
/// SQLite files (incl. WAL), any job row, any returned value, any error text, nor any image / clone request.
#[test]
fn the_token_never_leaks_anywhere() {
    let e = gh_env();
    let c = &e.canned;
    route_demo(c);
    c.on("https://api.github.com/rate_limit", 200, &[("x-ratelimit-limit", "5000"), ("x-ratelimit-remaining", "4999"), ("x-ratelimit-reset", "4102444800")], b"{}".to_vec());
    c.on("https://api.github.com/repos/octo/private", 401, &[], b"{\"message\":\"Bad credentials\"}".to_vec());
    c.on("https://api.github.com/repos/octo/gone", 404, &[], b"{}".to_vec());
    c.on(
        "https://api.github.com/search/repositories?q=demo&per_page=30&page=1",
        200,
        &[("x-ratelimit-resource", "search"), ("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", "29"), ("x-ratelimit-reset", "4102444800")],
        json!({"total_count": 1, "items": [{"name": "demo", "owner": {"login": "octo"}}]}).to_string().into_bytes(),
    );
    c.on("https://api.github.com/search/repositories?q=limited&per_page=30&page=1", 403, &[("x-ratelimit-resource", "search"), ("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "4102444800")], b"{}".to_vec());
    c.on("https://img.example.com/a.png", 200, &[("content-type", "image/png")], b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec());

    let mut seen_text: Vec<String> = Vec::new();
    let mut keep = |label: &str, v: String| seen_text.push(format!("{label}: {v}"));
    keep("token_set", format!("{:?}", ex::explore_token_set_impl(&e.st, TOKEN).map(|s| serde_json::to_string(&s).unwrap())));
    // a 200, a 401, a 404, a transport error (no route), a rate-limited search, a good search
    keep("repo", format!("{:?}", ex::explore_repo_impl(&e.st, "octo", "demo").map(|r| serde_json::to_string(&r).unwrap())));
    keep("401", format!("{:?}", ex::explore_repo_impl(&e.st, "octo", "private")));
    keep("404", format!("{:?}", ex::explore_repo_impl(&e.st, "octo", "gone")));
    keep("net", format!("{:?}", ex::explore_repo_impl(&e.st, "octo", "unrouted")));
    keep("limited", format!("{:?}", ex::explore_search_impl(&e.st, "limited", None, 1)));
    keep("search", format!("{:?}", ex::explore_search_impl(&e.st, "demo", None, 1).map(|r| serde_json::to_string(&r).unwrap())));
    keep("tree", format!("{:?}", ex::explore_tree_impl(&e.st, "octo", "demo", "main", "").map(|r| serde_json::to_string(&r).unwrap())));
    // friendly IPC wording of errors (the command layer maps with the host)
    for err in [ex::explore_repo_impl(&e.st, "octo", "unrouted").unwrap_err(), ex::explore_repo_impl(&e.st, "octo", "private").unwrap_err()] {
        keep("ipc", format!("{:?}", crate::ipc_error::IpcError::from_core(&err, Some("api.github.com"))));
    }
    // Save to shelf -> a job row; settings, status, app info, images
    let job = ex::explore_save_to_shelf_impl(&e.st, "octo", "demo", None).unwrap();
    e.st.jobs.join();
    keep("jobs", serde_json::to_string(&e.st.jobs.list()).unwrap());
    keep("job_title", e.st.jobs.get(job).unwrap().title);
    keep("settings", serde_json::to_string(&app::settings_get_impl(&e.st).unwrap()).unwrap());
    keep("status", serde_json::to_string(&ex::explore_status_impl(&e.st).unwrap()).unwrap());
    keep("info", serde_json::to_string(&app::app_info_impl(&e.st).unwrap()).unwrap());
    images::image_fetch_impl(&e.st, "https://img.example.com/a.png").unwrap();
    for t in &seen_text {
        assert!(!t.contains(TOKEN), "token in returned text: {t}");
        assert!(!t.contains(&TOKEN[4..]), "token body in returned text: {t}");
    }

    // every file the app wrote (DB, WAL, SHM, caches, job rows) and a dump of every table
    let mut files = 0;
    let mut stack = vec![e.dirs.data.clone(), e.dirs.archive.clone()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for it in rd.flatten() {
            let p = it.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files += 1;
                let bytes = std::fs::read(&p).unwrap();
                assert!(!bytes.windows(TOKEN.len()).any(|w| w == TOKEN.as_bytes()), "token found in {}", p.display());
            }
        }
    }
    assert!(files >= 1);
    let dump = e
        .st
        .shelf
        .db
        .with(|c| {
            let mut out = String::new();
            let tables: Vec<String> = {
                let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
                st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
            };
            for t in tables {
                let mut st = c.prepare(&format!("SELECT * FROM \"{t}\""))?;
                let n = st.column_count();
                let mut rows = st.query([])?;
                while let Some(r) = rows.next()? {
                    for i in 0..n {
                        out.push_str(&format!("{:?}|", r.get_ref(i)?));
                    }
                    out.push('\n');
                }
            }
            Ok(out)
        })
        .unwrap();
    assert!(!dump.contains(TOKEN) && !dump.contains("Bearer"), "token in a table");
    assert!(dump.to_lowercase().contains("gh_quota") || !dump.is_empty());

    // where the header went: every Explore request carried it, to api.github.com only; nothing else did
    let seen = e.canned.seen();
    let mut with = 0;
    for (url, headers) in &seen {
        let auth = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("authorization"));
        if url.starts_with("https://api.github.com/") {
            assert_eq!(auth.map(|(_, v)| v.as_str()), Some(format!("Bearer {TOKEN}").as_str()), "{url}");
            with += 1;
        } else {
            assert!(auth.is_none(), "Authorization sent to {url}");
        }
    }
    assert!(with >= 6, "{with}");
    assert!(seen.iter().any(|(u, _)| u == "https://img.example.com/a.png"));
    // the clone job goes through the (fake) worker as a plain https URL: no credentials in it
    assert_eq!(e.worked.lock().unwrap().as_slice(), &[JobKind::Clone { url: "https://github.com/octo/demo.git".into(), auto_download: None }]);
    // removing the token empties the saved answers (private data must not outlive the credential)
    assert!(ex::explore_status_impl(&e.st).unwrap().cache_entries > 0);
    let st = ex::explore_token_remove_impl(&e.st).unwrap();
    assert_eq!((st.cache_entries, st.token_saved, st.core), (0, false, None));
}

#[test]
fn save_to_shelf_starts_the_existing_clone_job_once() {
    // a worker that holds the job open, so the second click finds it running
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let rx = Arc::new(Mutex::new(rx));
    let started = Arc::new(Mutex::new(Vec::<JobKind>::new()));
    let s2 = started.clone();
    let worker: Worker = Arc::new(move |kind: &JobKind, _ctx: &JobCtx| {
        s2.lock().unwrap().push(kind.clone());
        let _ = rx.lock().unwrap().recv_timeout(std::time::Duration::from_secs(20));
        Ok(JobResult::Cloned { repo_id: 99, skipped_refs: vec![], notes: vec![] })
    });
    let t = tempfile::tempdir().unwrap();
    let shelf = Arc::new(testkit::shelf(t.path()));
    std::fs::create_dir_all(&shelf.dirs.archive).unwrap();
    let st = AppState::with_worker(shelf, false, worker, Arc::new(FakePicker::default()), Arc::new(FakeTrash::default()), Arc::new(FakeOpener::default()));
    let a = ex::explore_save_to_shelf_impl(&st, "Octo", "Demo", Some(false)).unwrap();
    let b = ex::explore_save_to_shelf_impl(&st, "octo", "demo", None).unwrap();
    assert_eq!(a, b, "a double click is one job");
    assert_eq!(st.jobs.get(a).unwrap().title, "Clone Octo/Demo");
    tx.send(()).unwrap();
    st.jobs.join();
    assert_eq!(started.lock().unwrap().as_slice(), &[JobKind::Clone { url: "https://github.com/Octo/Demo.git".into(), auto_download: Some(false) }]);
    // finished: a new click starts a new job (the clone worker would register the repo; the fake does not)
    let c = ex::explore_save_to_shelf_impl(&st, "octo", "demo", None);
    assert!(c.is_ok());
    let _ = tx.send(());
    st.jobs.join();
    // names GitHub allows but Windows cannot store are refused with the clone rules' message, no job
    assert_eq!(code(ex::explore_save_to_shelf_impl(&st, "octo", "aux", None)), "invalidUrl");
}

#[test]
fn save_to_shelf_of_a_repo_already_on_the_shelf_is_a_duplicate() {
    let e = gh_env();
    let dir = github_repo(&e._t.path().join("r"));
    rs_core::library::import(&e.st.shelf, &dir).unwrap();
    assert_eq!(code(ex::explore_save_to_shelf_impl(&e.st, "o", "r", None)), "duplicate");
    // the Explore repo page knows it is on the shelf
    e.canned.on("https://api.github.com/repos/o/r", 200, &[], repo_body("o", "r"));
    let r = ex::explore_repo_impl(&e.st, "o", "r").unwrap();
    assert!(r.on_shelf.is_some());
    assert!(e.worked.lock().unwrap().is_empty());
}

#[test]
fn clear_cache_and_app_info_include_the_explore_cache() {
    let e = gh_env();
    route_demo(&e.canned);
    let before = app::app_info_impl(&e.st).unwrap().cache_bytes;
    ex::explore_repo_impl(&e.st, "octo", "demo").unwrap();
    let st = ex::explore_status_impl(&e.st).unwrap();
    assert!(st.cache_entries == 1 && st.cache_bytes > 0);
    assert!(app::app_info_impl(&e.st).unwrap().cache_bytes >= before + st.cache_bytes);
    // "Clear Explore cache" empties only that
    assert_eq!(ex::explore_cache_clear_impl(&e.st).unwrap().cache_entries, 0);
    ex::explore_repo_impl(&e.st, "octo", "demo").unwrap();
    // the general "Clear cache" empties it too
    app::cache_clear_impl(&e.st).unwrap();
    assert_eq!(ex::explore_status_impl(&e.st).unwrap().cache_entries, 0);
}

#[test]
fn explore_never_touches_the_local_library() {
    let e = gh_env();
    route_demo(&e.canned);
    let dir = github_repo(&e._t.path().join("r"));
    let id = rs_core::library::import(&e.st.shelf, &dir).unwrap().id;
    let before = rs_core::library::list(&e.st.shelf).unwrap();
    ex::explore_repo_impl(&e.st, "octo", "demo").unwrap();
    ex::explore_tree_impl(&e.st, "octo", "demo", "main", "").unwrap();
    assert_eq!(rs_core::library::list(&e.st.shelf).unwrap(), before);
    assert!(tree::tree_list_impl(&e.st, id, "main", "").is_ok(), "local screens read the local DB as before");
}

/// Every `Error::code()` rs-core can send is a member of the TS `IpcErrorCode` union and of `errors.ts`'s set
/// (an unknown code would be flattened to "Unexpected error" by `toIpcError`; `rateLimited` was added in M10a).
#[test]
fn every_error_code_is_known_to_the_frontend() {
    let err = include_str!("../../../crates/rs-core/src/error.rs");
    let body = err.split("pub fn code(&self)").nth(1).unwrap().split("pub fn not_found").next().unwrap();
    let codes: Vec<&str> = body.lines().filter_map(|l| l.split("=> \"").nth(1)).filter_map(|r| r.split('"').next()).collect();
    assert!(codes.len() >= 17, "{codes:?}");
    let types = include_str!("../../../src/ipc/types.ts");
    let errors = include_str!("../../../src/ipc/errors.ts");
    for c in codes {
        assert!(types.contains(&format!("| '{c}'")), "{c} missing from IpcErrorCode");
        assert!(errors.contains(&format!("'{c}',")), "{c} missing from errors.ts");
    }
}
