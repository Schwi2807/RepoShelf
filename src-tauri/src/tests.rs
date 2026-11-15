//! Command-layer tests (M4.11): the `_impl` functions over a test `AppState` (in-memory shelf, scratch dirs,
//! fake picker/trash/opener, fake job worker), no runtime and no window. PoC criterion 4, NFR-S4.

mod explore;
mod ipc_samples;

use crate::cmd::{app, blob, commits, dialogs, images, jobs, library, refs, releases, tree};
use crate::ipc_error::CREDENTIALS;
use crate::state::{AppState, Opener, Picker, Trasher};
use rs_core::Shelf;
use rs_core::error::{Error, Result};
use rs_core::jobs::{JobCtx, JobInfo, JobKind, JobState, Worker};
use rs_core::testkit::{self, BasicFx};
use rs_core::types::{FetchOutcome, JobResult, Source};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};

// ---------------------------------------------------------------------------------------------------------------
// Fakes

#[derive(Default)]
pub struct FakePicker {
    pub folders: Mutex<VecDeque<Option<PathBuf>>>,
    pub saves: Mutex<VecDeque<Option<PathBuf>>>,
    pub calls: Mutex<Vec<String>>,
}

impl Picker for FakePicker {
    fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        self.calls.lock().unwrap().push(format!("folder:{title}"));
        self.folders.lock().unwrap().pop_front().flatten()
    }
    fn save_file(&self, title: &str, file_name: &str) -> Option<PathBuf> {
        self.calls.lock().unwrap().push(format!("save:{title}:{file_name}"));
        self.saves.lock().unwrap().pop_front().flatten()
    }
}

#[derive(Default)]
pub struct FakeTrash {
    pub got: Mutex<Vec<PathBuf>>,
}

impl Trasher for FakeTrash {
    fn delete(&self, path: &Path) -> std::result::Result<(), String> {
        self.got.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeOpener {
    pub urls: Mutex<Vec<String>>,
    pub folders: Mutex<Vec<PathBuf>>,
    pub reveals: Mutex<Vec<PathBuf>>,
}

impl Opener for FakeOpener {
    fn open_url(&self, url: &str) -> std::result::Result<(), String> {
        self.urls.lock().unwrap().push(url.to_string());
        Ok(())
    }
    fn open_folder(&self, path: &Path) -> std::result::Result<(), String> {
        self.folders.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
    fn reveal(&self, path: &Path) -> std::result::Result<(), String> {
        self.reveals.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// Job worker that never touches the network: records kinds, fails `Fetch { repo_id: -7 }` like a 401.
fn fake_worker(seen: Arc<Mutex<Vec<JobKind>>>) -> Worker {
    Arc::new(move |kind: &JobKind, _ctx: &JobCtx| {
        seen.lock().unwrap().push(kind.clone());
        match kind {
            JobKind::Clone { .. } => Ok(JobResult::Cloned { repo_id: 99, skipped_refs: vec![], notes: vec![] }),
            _ => Ok(JobResult::Fetched(FetchOutcome::default())),
        }
    })
}

struct Env {
    t: tempfile::TempDir,
    st: AppState,
    picker: Arc<FakePicker>,
    trash: Arc<FakeTrash>,
    opener: Arc<FakeOpener>,
    worked: Arc<Mutex<Vec<JobKind>>>,
    basic: BasicFx,
    id: i64,
}

fn env_with(worker: Option<Worker>, test_file_clone: bool) -> Env {
    let t = tempfile::tempdir().unwrap();
    let shelf = Arc::new(testkit::shelf(t.path()));
    std::fs::create_dir_all(&shelf.dirs.archive).unwrap();
    let basic = testkit::basic(&t.path().join("src").join("basic"));
    let id = rs_core::library::import(&shelf, &basic.path).unwrap().id;
    let picker = Arc::new(FakePicker::default());
    let trash = Arc::new(FakeTrash::default());
    let opener = Arc::new(FakeOpener::default());
    let worked = Arc::new(Mutex::new(Vec::new()));
    let worker = worker.unwrap_or_else(|| fake_worker(worked.clone()));
    let st = AppState::with_worker(shelf, test_file_clone, worker, picker.clone(), trash.clone(), opener.clone());
    Env { t, st, picker, trash, opener, worked, basic, id }
}

fn env() -> Env {
    env_with(None, false)
}

fn shelf(e: &Env) -> &Shelf {
    &e.st.shelf
}

fn code<T: std::fmt::Debug>(r: Result<T>) -> &'static str {
    match r {
        Ok(v) => panic!("expected an error, got {v:?}"),
        Err(e) => e.code(),
    }
}

/// Registers an extra fixture repo and returns its id.
fn add(e: &Env, path: &Path, source: Source) -> i64 {
    testkit::register_raw(shelf(e), path, source).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// Hostile inputs (§5.2, PoC criterion 4)

const HOSTILE: &[&str] = &["..", "a/../b", "/etc", "C:\\x", "c:/x", "a\\b", "a\0b", "refs/../x", "HEAD~1", "a..b", "@{-1}", "--upload-pack=x"];
/// Legal git file names (refused as revs only).
const PATH_OK: &[&str] = &["HEAD~1", "a..b", "@{-1}", "--upload-pack=x"];

type Probe = Box<dyn Fn(&AppState, i64, &str) -> Result<()>>;

fn ok<T>(r: Result<T>) -> Result<()> {
    r.map(|_| ())
}

/// Every command that takes a `rev`, with the hostile value in that argument.
fn rev_probes() -> Vec<(&'static str, Probe)> {
    vec![
        ("rev_resolve", Box::new(|s, id, v| ok(tree::rev_resolve_impl(s, id, v)))),
        ("tree_list", Box::new(|s, id, v| ok(tree::tree_list_impl(s, id, v, "")))),
        ("tree_last_commits", Box::new(|s, id, v| ok(tree::tree_last_commits_impl(s, id, v, "", "r1")))),
        ("commit_latest", Box::new(|s, id, v| ok(tree::commit_latest_impl(s, id, v)))),
        ("commit_count", Box::new(|s, id, v| ok(tree::commit_count_impl(s, id, v, "r2")))),
        ("readme_render", Box::new(|s, id, v| ok(tree::readme_render_impl(s, id, v, "")))),
        ("markdown_render_file", Box::new(|s, id, v| ok(tree::markdown_render_file_impl(s, id, v, "README.md")))),
        ("languages", Box::new(|s, id, v| ok(tree::languages_impl(s, id, v, "r3")))),
        ("blob_meta", Box::new(|s, id, v| ok(blob::blob_meta_impl(s, id, v, "README.md")))),
        ("blob_text", Box::new(|s, id, v| ok(blob::blob_text_impl(s, id, v, "README.md")))),
        ("blob_bytes", Box::new(|s, id, v| ok(blob::blob_bytes_impl(s, id, v, "README.md")))),
        ("blob_save_as", Box::new(|s, id, v| ok(dialogs::blob_save_as_impl(s, id, v, "README.md")))),
        ("file_last_commit", Box::new(|s, id, v| ok(blob::file_last_commit_impl(s, id, v, "README.md", "r4")))),
        ("commits_page.rev", Box::new(|s, id, v| ok(commits::commits_page_impl(s, id, v, None, None, 0, None)))),
        ("commits_page.since", Box::new(|s, id, v| ok(commits::commits_page_impl(s, id, "main", None, Some(v), 0, None)))),
        ("branches_ahead_behind", Box::new(|s, id, v| ok(refs::branches_ahead_behind_impl(s, id, &[v.to_string()], "r5")))),
    ]
}

/// Every command that takes a git `path`.
fn path_probes() -> Vec<(&'static str, Probe)> {
    vec![
        ("tree_list", Box::new(|s, id, v| ok(tree::tree_list_impl(s, id, "main", v)))),
        ("tree_last_commits", Box::new(|s, id, v| ok(tree::tree_last_commits_impl(s, id, "main", v, "p1")))),
        ("readme_render", Box::new(|s, id, v| ok(tree::readme_render_impl(s, id, "main", v)))),
        ("markdown_render_file", Box::new(|s, id, v| ok(tree::markdown_render_file_impl(s, id, "main", v)))),
        ("blob_meta", Box::new(|s, id, v| ok(blob::blob_meta_impl(s, id, "main", v)))),
        ("blob_text", Box::new(|s, id, v| ok(blob::blob_text_impl(s, id, "main", v)))),
        ("blob_bytes", Box::new(|s, id, v| ok(blob::blob_bytes_impl(s, id, "main", v)))),
        ("blob_save_as", Box::new(|s, id, v| ok(dialogs::blob_save_as_impl(s, id, "main", v)))),
        ("file_last_commit", Box::new(|s, id, v| ok(blob::file_last_commit_impl(s, id, "main", v, "p2")))),
        ("commits_page.path", Box::new(|s, id, v| ok(commits::commits_page_impl(s, id, "main", Some(v), None, 0, None)))),
    ]
}

/// Every command that takes a full commit id.
fn oid_probes() -> Vec<(&'static str, Probe)> {
    vec![
        ("commit_detail", Box::new(|s, id, v| ok(commits::commit_detail_impl(s, id, v)))),
        ("commit_file_diff", Box::new(|s, id, v| ok(commits::commit_file_diff_impl(s, id, v, 0, false)))),
        ("commits_page.tip", Box::new(|s, id, v| ok(commits::commits_page_impl(s, id, "main", None, None, 0, Some(v))))),
    ]
}

#[test]
fn hostile_revs_refused_for_every_rev_command() {
    let e = env();
    for (name, probe) in rev_probes() {
        for h in HOSTILE {
            assert_eq!(code(probe(&e.st, e.id, h)), "invalidRev", "{name} rev {h:?}");
            // validation happens before the repo lookup
            assert_eq!(code(probe(&e.st, 424242, h)), "invalidRev", "{name} rev {h:?} (unknown repo)");
        }
    }
}

#[test]
fn hostile_paths_refused_for_every_path_command() {
    let e = env();
    for (name, probe) in path_probes() {
        for h in HOSTILE.iter().filter(|h| !PATH_OK.contains(h)) {
            assert_eq!(code(probe(&e.st, e.id, h)), "invalidPath", "{name} path {h:?}");
            assert_eq!(code(probe(&e.st, 424242, h)), "invalidPath", "{name} path {h:?} (unknown repo)");
        }
        // legal-but-odd names are looked up, not refused: absent here → notFound
        for h in PATH_OK {
            if let Err(err) = probe(&e.st, e.id, h) {
                assert_ne!(err.code(), "invalidPath", "{name} refused legal file name {h:?}");
            }
        }
    }
}

#[test]
fn bad_oids_refused() {
    let e = env();
    for (name, probe) in oid_probes() {
        for bad in ["", "abc", &"a".repeat(39), &"a".repeat(41), &"g".repeat(40), "HEAD", "main", "..", &format!("{}\0", "a".repeat(39))] {
            assert_eq!(code(probe(&e.st, e.id, bad)), "invalidRev", "{name} oid {bad:?}");
        }
    }
}

#[test]
fn unknown_repo_id_is_not_found_everywhere() {
    let e = env();
    let x = 424242;
    let head = e.basic.head.to_string();
    let s = &e.st;
    let all: Vec<(&str, Result<()>)> = vec![
        ("repo_set_metadata", ok(library::repo_set_metadata_impl(s, x, None, None, vec![]))),
        ("repo_detail", ok(library::repo_detail_impl(s, x))),
        ("repo_open_folder", ok(library::repo_open_folder_impl(s, x))),
        ("repo_locate", ok(dialogs::repo_locate_impl(s, x))),
        ("repo_remove", ok(dialogs::repo_remove_impl(s, x))),
        ("refs_list", ok(tree::refs_list_impl(s, x))),
        ("branches_list", ok(refs::branches_list_impl(s, x))),
        ("tags_list", ok(refs::tags_list_impl(s, x))),
        ("releases_list", ok(releases::releases_list_impl(s, x))),
        ("releases_fetch_github", ok(releases::releases_fetch_github_impl(s, x))),
        ("commit_detail", ok(commits::commit_detail_impl(s, x, &head))),
        ("commit_file_diff", ok(commits::commit_file_diff_impl(s, x, &head, 0, false))),
        ("fetch_start", ok(jobs::fetch_start_impl(s, x, None))),
        ("fast_forward", ok(jobs::fast_forward_impl(s, x))),
        ("release_download", ok(jobs::release_download_impl(s, x, "v1", "a.zip"))),
        ("release_reveal", ok(releases::release_reveal_impl(s, x, "v1", "a.zip"))),
    ];
    for (name, r) in all {
        assert_eq!(code(r), "notFound", "{name}");
    }
    // valid rev/path arguments: the repo lookup is what fails
    for (name, probe) in rev_probes().into_iter().chain(path_probes()) {
        assert_eq!(code(probe(s, x, "main")), "notFound", "{name}");
    }
    assert_eq!(code(library::repo_by_slug_impl(s, "nobody", "nothing")), "notFound");
}

#[test]
fn request_ids_are_validated() {
    let e = env();
    for bad in ["", "a b", "../x", &"r".repeat(65)] {
        assert_eq!(code(tree::commit_count_impl(&e.st, e.id, "main", bad)), "invalidInput", "{bad:?}");
        assert_eq!(code(app::request_cancel_impl(&e.st, bad)), "invalidInput", "{bad:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// App commands

#[test]
fn app_info_settings_and_cache_clear() {
    let e = env();
    let info = app::app_info_impl(&e.st).unwrap();
    assert_eq!(info.repo_count, 1);
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.archive_bytes, Some(0));
    assert_eq!(info.archive_dir, shelf(&e).dirs.archive.display().to_string());

    // fill caches, then clear
    tree::languages_impl(&e.st, e.id, "main", "c1").unwrap();
    tree::tree_last_commits_impl(&e.st, e.id, "main", "", "c2").unwrap();
    tree::commit_count_impl(&e.st, e.id, "main", "c3").unwrap();
    assert!(app::app_info_impl(&e.st).unwrap().cache_bytes > 0);
    let after = app::cache_clear_impl(&e.st).unwrap();
    assert_eq!(after.cache_bytes, 0);
    assert_eq!(after.repo_count, 1, "repos survive cache_clear");

    let mut s = app::settings_get_impl(&e.st).unwrap();
    assert_eq!(s, rs_core::settings::Settings::default());
    s.theme = rs_core::types::Theme::Dark;
    s.show_dirty_note = true;
    assert_eq!(app::settings_set_impl(&e.st, s.clone()).unwrap(), s);
    assert_eq!(app::settings_get_impl(&e.st).unwrap(), s);
}

#[test]
fn app_open_dir_takes_a_keyword_only() {
    let e = env();
    app::app_open_dir_impl(&e.st, "data").unwrap();
    app::app_open_dir_impl(&e.st, "archive").unwrap();
    let opened = e.opener.folders.lock().unwrap().clone();
    assert_eq!(opened, vec![shelf(&e).dirs.data.clone(), shelf(&e).dirs.archive.clone()]);
    for bad in ["", "Data", "cache", "C:\\Windows", "..", "data/.."] {
        assert_eq!(code(app::app_open_dir_impl(&e.st, bad)), "invalidInput", "{bad:?}");
    }
    assert_eq!(e.opener.folders.lock().unwrap().len(), 2, "nothing opened for refused input");
}

#[test]
fn archive_size_is_capped() {
    let t = tempfile::tempdir().unwrap();
    for i in 0..5 {
        std::fs::write(t.path().join(format!("f{i}")), b"12345").unwrap();
    }
    assert_eq!(app::dir_bytes(t.path(), 100), Some(25));
    assert_eq!(app::dir_bytes(t.path(), 3), None);
    assert_eq!(app::dir_bytes(&t.path().join("absent"), 3), Some(0));
}

#[test]
fn open_external_refuses_non_http() {
    let e = env();
    for bad in [
        "javascript:alert(1)",
        "file:///C:/Windows/win.ini",
        "ftp://example.com/x",
        "https://user:pw@example.com/",
        "https://user@example.com/",
        "mailto:a@example.com",
        "data:text/html,<script>",
        "vbscript:x",
        "",
        "example.com",
        "\\\\server\\share",
    ] {
        assert_eq!(code(app::open_external_impl(&e.st, bad)), "invalidUrl", "{bad:?}");
    }
    assert!(e.opener.urls.lock().unwrap().is_empty());
    app::open_external_impl(&e.st, "https://example.com/a?b=1#c").unwrap();
    app::open_external_impl(&e.st, " http://example.org ").unwrap();
    assert_eq!(*e.opener.urls.lock().unwrap(), vec!["https://example.com/a?b=1#c".to_string(), "http://example.org/".to_string()]);
}

#[test]
fn request_cancel_stops_a_registered_request() {
    let e = env();
    // cancel first: the request starts cancelled and returns `cancelled`
    assert!(!app::request_cancel_impl(&e.st, "early-1").unwrap());
    assert_eq!(code(tree::commit_count_impl(&e.st, e.id, "main", "early-1")), "cancelled");
    // a live request is found while running
    let g = e.st.requests.register("live-1").unwrap();
    assert!(app::request_cancel_impl(&e.st, "live-1").unwrap());
    assert!(g.token().is_cancelled());
    drop(g);
    assert_eq!(e.st.requests.live(), 0);
}

// ---------------------------------------------------------------------------------------------------------------
// Library commands

#[test]
fn library_list_refresh_slug_metadata_detail() {
    let e = env();
    let list = library::library_list_impl(&e.st).unwrap();
    assert_eq!(list.len(), 1);
    let s = &list[0];
    assert_eq!((s.owner.as_str(), s.name.as_str()), ("example", "basic"));
    let refreshed = library::library_refresh_impl(&e.st, "lr-1").unwrap();
    assert_eq!(refreshed.len(), 1);
    assert_eq!(e.st.compute.in_use(), 0, "compute slot released");
    assert_eq!(library::repo_by_slug_impl(&e.st, "example", "basic").unwrap().id, e.id);
    assert_eq!(library::repo_by_slug_impl(&e.st, "EXAMPLE", "Basic").unwrap().id, e.id);
    assert_eq!(code(library::repo_by_slug_impl(&e.st, "", "basic")), "invalidInput");

    let m = library::repo_set_metadata_impl(&e.st, e.id, Some(" A shelf ".into()), Some("https://example.com".into()), vec!["Rust".into(), "cli".into()]).unwrap();
    assert_eq!(m.description.as_deref(), Some("A shelf"));
    assert_eq!(m.topics, vec!["rust", "cli"]);
    assert_eq!(code(library::repo_set_metadata_impl(&e.st, e.id, None, Some("javascript:x".into()), vec![])), "invalidInput");
    assert_eq!(code(library::repo_set_metadata_impl(&e.st, e.id, None, None, vec!["bad topic".into()])), "invalidInput");

    assert!(list[0].last_opened_at.is_none());
    let d = library::repo_detail_impl(&e.st, e.id).unwrap();
    assert!(!d.is_bare);
    assert_eq!(d.license.as_deref(), Some("MIT"));
    assert!(d.fetchable);
    assert!(d.summary.last_opened_at.is_some(), "repo_detail touches last_opened_at");
}

#[test]
fn repo_open_folder_opens_work_tree_or_reveals_bare() {
    let e = env();
    library::repo_open_folder_impl(&e.st, e.id).unwrap();
    let opened = e.opener.folders.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert!(e.opener.reveals.lock().unwrap().is_empty());
    let bare = add(&e, &testkit::releases(&e.t.path().join("rel.git")), Source::Imported);
    library::repo_open_folder_impl(&e.st, bare).unwrap();
    assert_eq!(e.opener.reveals.lock().unwrap().len(), 1);
    // missing folder → repoMissing, nothing opened
    let gone_dir = testkit::missing(&e.t.path().join("gone"));
    let gone = add(&e, &gone_dir, Source::Imported);
    std::fs::rename(&gone_dir, e.t.path().join("gone-renamed")).unwrap();
    assert_eq!(code(library::repo_open_folder_impl(&e.st, gone)), "repoMissing");
    assert_eq!(e.opener.folders.lock().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Dialog commands

#[test]
fn import_pick_and_confirm_with_token() {
    let e = env();
    let other = testkit::releases(&e.t.path().join("other.git"));
    // cancelled dialog → None
    e.picker.folders.lock().unwrap().push_back(None);
    assert!(dialogs::import_pick_impl(&e.st).unwrap().is_none());
    // picked folder → preview with token
    e.picker.folders.lock().unwrap().push_back(Some(other.clone()));
    let p = dialogs::import_pick_impl(&e.st).unwrap().unwrap();
    assert_eq!(p.token.len(), 32);
    assert!(p.is_bare);
    assert!(p.duplicate_of.is_none());
    // bad / unknown token
    for bad in ["", "nope", &"x".repeat(65)] {
        assert_eq!(code(dialogs::import_confirm_impl(&e.st, bad, None)), "invalidInput", "{bad:?}");
    }
    let s = dialogs::import_confirm_impl(&e.st, &p.token, None).unwrap();
    assert_eq!(s.source, Source::Imported);
    // one-shot
    assert_eq!(code(dialogs::import_confirm_impl(&e.st, &p.token, None)), "invalidInput");
    // a duplicate shows up in the preview
    let dup = dialogs::import_preview_for(&e.st, &e.basic.path).unwrap();
    assert_eq!(dup.duplicate_of, Some(e.id));
    assert_eq!(code(dialogs::import_confirm_impl(&e.st, &dup.token, None)), "duplicate");
    // non-repo folder refused at preview
    e.picker.folders.lock().unwrap().push_back(Some(e.t.path().to_path_buf()));
    assert_eq!(code(dialogs::import_pick_impl(&e.st)), "notARepo");
}

#[test]
fn expired_import_token_is_refused() {
    let e = env();
    let p = dialogs::import_preview_for(&e.st, &testkit::releases(&e.t.path().join("o.git"))).unwrap();
    {
        let mut t = e.st.import_tokens.lock().unwrap();
        let entry = t.get_mut(&p.token).unwrap();
        entry.1 = std::time::Instant::now() - crate::state::IMPORT_TOKEN_TTL - std::time::Duration::from_secs(1);
    }
    assert_eq!(code(dialogs::import_confirm_impl(&e.st, &p.token, None)), "invalidInput");
}

#[test]
fn repo_locate_relocates_or_cancels() {
    let e = env();
    let gone = testkit::missing(&e.t.path().join("moved"));
    let id = add(&e, &gone, Source::Imported);
    std::fs::rename(&gone, e.t.path().join("moved-away")).unwrap();
    assert_eq!(rs_core::library::refresh_one(shelf(&e), id, &rs_core::jobs::CancelToken::new()).unwrap().status, rs_core::types::RepoStatus::Missing);
    e.picker.folders.lock().unwrap().push_back(None);
    assert!(dialogs::repo_locate_impl(&e.st, id).unwrap().is_none());
    let new_home = testkit::releases(&e.t.path().join("found.git"));
    e.picker.folders.lock().unwrap().push_back(Some(new_home));
    let s = dialogs::repo_locate_impl(&e.st, id).unwrap().unwrap();
    assert!(s.path.ends_with("found.git"), "{}", s.path);
}

#[test]
fn repo_remove_trashes_only_cloned_folders_inside_the_archive() {
    let e = env();
    // imported → nothing trashed
    let r = dialogs::repo_remove_impl(&e.st, e.id).unwrap();
    assert!(!r.trashed);
    assert!(e.trash.got.lock().unwrap().is_empty());
    assert!(e.basic.path.join("README.md").exists());
    // cloned, inside the archive → trashed
    let inside = testkit::releases(&shelf(&e).dirs.archive.join("github.com").join("o").join("r.git"));
    let id = add(&e, &inside, Source::Cloned);
    assert!(dialogs::repo_remove_impl(&e.st, id).unwrap().trashed);
    let got = e.trash.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1);
    assert!(got[0].ends_with("r.git"), "{got:?}");
    assert_eq!(code(rs_core::repo::summary(shelf(&e), id)), "notFound");
    // cloned but relocated outside the archive → entry removed, folder left alone (D-50)
    let outside = testkit::releases(&e.t.path().join("elsewhere.git"));
    let id = add(&e, &outside, Source::Cloned);
    assert!(!dialogs::repo_remove_impl(&e.st, id).unwrap().trashed);
    assert_eq!(e.trash.got.lock().unwrap().len(), 1);
    // the archive dir itself is never "inside" itself
    assert!(!dialogs::inside_archive(&shelf(&e).dirs.archive, &shelf(&e).dirs.archive));
}

#[test]
fn blob_save_as_writes_or_cancels() {
    let e = env();
    let target = e.t.path().join("saved.png");
    e.picker.saves.lock().unwrap().push_back(Some(target.clone()));
    let out = dialogs::blob_save_as_impl(&e.st, e.id, "main", "docs/logo.png").unwrap().unwrap();
    assert_eq!(out, target.display().to_string());
    assert_eq!(std::fs::read(&target).unwrap(), testkit::PNG_1X1);
    assert!(e.picker.calls.lock().unwrap().last().unwrap().ends_with(":logo.png"));
    e.picker.saves.lock().unwrap().push_back(None);
    assert!(dialogs::blob_save_as_impl(&e.st, e.id, "main", "README.md").unwrap().is_none());
    // a missing file fails before the dialog opens
    let before = e.picker.calls.lock().unwrap().len();
    assert_eq!(code(dialogs::blob_save_as_impl(&e.st, e.id, "main", "nope.txt")), "notFound");
    assert_eq!(code(dialogs::blob_save_as_impl(&e.st, e.id, "main", "")), "invalidPath");
    assert_eq!(e.picker.calls.lock().unwrap().len(), before);
}

// ---------------------------------------------------------------------------------------------------------------
// Browse commands

#[test]
fn refs_tree_readme_languages() {
    let e = env();
    let s = &e.st;
    let refs = tree::refs_list_impl(s, e.id).unwrap();
    assert_eq!(refs.default_branch.as_deref(), Some("main"));
    let r = tree::rev_resolve_impl(s, e.id, "v0.1.0").unwrap();
    assert_eq!(r.commit, e.basic.head.to_string());
    assert_eq!(code(tree::rev_resolve_impl(s, e.id, "nope")), "notFound");
    let l = tree::tree_list_impl(s, e.id, "main", "").unwrap();
    assert_eq!(l.readme.as_deref(), Some("README.md"));
    assert!(l.entries.iter().any(|x| x.name == "src"));
    let sub = tree::tree_list_impl(s, e.id, "main", "src").unwrap();
    assert_eq!(sub.entries.len(), 2);
    let lc = tree::tree_last_commits_impl(s, e.id, "main", "src", "lc-1").unwrap();
    assert_eq!(lc.entries.len(), 2);
    let latest = tree::commit_latest_impl(s, e.id, "main").unwrap();
    assert_eq!(latest.commit.oid, e.basic.head.to_string());
    assert_eq!(tree::commit_count_impl(s, e.id, "main", "cc-1").unwrap(), 2);
    assert_eq!(tree::commit_count_impl(s, e.id, "feature", "cc-2").unwrap(), 3);
    let readme = tree::readme_render_impl(s, e.id, "main", "").unwrap().unwrap();
    assert!(readme.html.contains("<table>"));
    assert!(tree::readme_render_impl(s, e.id, "main", "src").unwrap().is_none(), "no README in src/");
    let md = tree::markdown_render_file_impl(s, e.id, "main", "docs/guide.md").unwrap();
    assert!(md.html.contains("Guide"));
    let langs = tree::languages_impl(s, e.id, "main", "lg-1").unwrap();
    assert_eq!(langs.items[0].name, "Rust");
    assert_eq!(e.st.requests.live(), 0);
    assert_eq!(e.st.compute.in_use(), 0);
}

#[test]
fn blob_commands_and_bytes_cap() {
    let e = env();
    let s = &e.st;
    let m = blob::blob_meta_impl(s, e.id, "main", "src/main.rs").unwrap();
    assert_eq!(m.lang_hint.as_deref(), Some("rust"));
    let t = blob::blob_text_impl(s, e.id, "main", "README.md").unwrap();
    assert!(t.text.unwrap().starts_with("# Basic fixture"));
    assert_eq!(blob::blob_bytes_impl(s, e.id, "main", "docs/logo.png").unwrap(), testkit::PNG_1X1);
    assert_eq!(code(blob::blob_bytes_impl(s, e.id, "main", "")), "invalidPath");
    let flc = blob::file_last_commit_impl(s, e.id, "main", "src/lib.rs", "fl-1").unwrap().unwrap();
    assert_eq!(flc.oid, e.basic.head.to_string());

    let bin = add(&e, &testkit::binaries(&e.t.path().join("bin.git")), Source::Imported);
    // 6 MB file: bytes refused, text null, meta says not displayable
    assert_eq!(code(blob::blob_bytes_impl(s, bin, "main", "big/huge.txt")), "refused");
    assert!(blob::blob_text_impl(s, bin, "main", "big/huge.txt").unwrap().text.is_none());
    assert!(!blob::blob_meta_impl(s, bin, "main", "big/huge.txt").unwrap().displayable);
    // 1.5 MB is under the cap
    assert_eq!(blob::blob_bytes_impl(s, bin, "main", "big/large.txt").unwrap().len(), testkit::LARGE_TEXT_BYTES);
    // svg is an image with the allow-listed mime; NFR-W3 names stay browsable
    let svg = blob::blob_meta_impl(s, bin, "main", "img/evil.svg").unwrap();
    assert_eq!(serde_json::to_value(&svg.kind).unwrap()["mime"], "image/svg+xml");
    assert!(blob::blob_text_impl(s, bin, "main", "win/a:b.txt").is_ok());
    assert!(blob::blob_text_impl(s, bin, "main", "win/CON.txt").is_ok());
}

#[test]
fn commit_commands() {
    let e = env();
    let s = &e.st;
    let p = commits::commits_page_impl(s, e.id, "feature", None, None, 0, None).unwrap();
    assert_eq!(p.commits.len(), 3);
    assert_eq!(p.tip, e.basic.feature.to_string());
    // pinned tip wins over rev; skip continues
    let p2 = commits::commits_page_impl(s, e.id, "main", None, None, 1, Some(&p.tip)).unwrap();
    assert_eq!(p2.commits.len(), 2);
    // since previous tag
    let since = commits::commits_page_impl(s, e.id, "feature", None, Some("v0.1.0"), 0, None).unwrap();
    assert_eq!(since.commits.len(), 1);
    // path filter ("" = none)
    let lib = commits::commits_page_impl(s, e.id, "main", Some("src/lib.rs"), None, 0, None).unwrap();
    assert_eq!(lib.commits.len(), 2);
    let all = commits::commits_page_impl(s, e.id, "main", Some(""), None, 0, None).unwrap();
    assert_eq!(all.commits.len(), 2);
    assert_eq!(code(commits::commits_page_impl(s, e.id, "main", None, None, crate::cmd::MAX_SKIP + 1, None)), "invalidInput");
    assert_eq!(code(commits::commits_page_impl(s, e.id, "main", None, None, 0, Some(&"0".repeat(40)))), "notFound");

    let d = commits::commit_detail_impl(s, e.id, &e.basic.head.to_string()).unwrap();
    assert_eq!(d.files.len(), 1);
    let upper = e.basic.head.to_string().to_uppercase();
    assert_eq!(commits::commit_detail_impl(s, e.id, &upper).unwrap().commit.oid, e.basic.head.to_string());
    let f = commits::commit_file_diff_impl(s, e.id, &e.basic.head.to_string(), 0, false).unwrap();
    assert_eq!(f.index, 0);
    assert_eq!(code(commits::commit_file_diff_impl(s, e.id, &e.basic.head.to_string(), crate::cmd::MAX_FILE_INDEX + 1, false)), "invalidInput");
}

#[test]
fn branch_tag_release_commands() {
    let e = env();
    let s = &e.st;
    let b = refs::branches_list_impl(s, e.id).unwrap();
    assert_eq!(b.len(), 2);
    let ab = refs::branches_ahead_behind_impl(s, e.id, &["feature".to_string()], "ab-1").unwrap();
    assert_eq!(ab[0].0, "feature");
    assert_eq!((ab[0].1.ahead, ab[0].1.behind), (1, 0));
    let json = serde_json::to_value(&ab).unwrap();
    assert_eq!(json[0][0], "feature", "[name, AheadBehind] pairs");
    assert_eq!(code(refs::branches_ahead_behind_impl(s, e.id, &vec!["x".to_string(); refs::MAX_NAMES + 1], "ab-2")), "invalidInput");
    let tags = refs::tags_list_impl(s, e.id).unwrap();
    assert_eq!(tags[0].item.name, "v0.1.0");
    let rel_id = add(&e, &testkit::releases(&e.t.path().join("rel.git")), Source::Imported);
    let rel = releases::releases_list_impl(s, rel_id).unwrap();
    assert_eq!(rel.len(), 4);
}

/// Feature on: cached (canned) GitHub notes are merged by `releases_list`; the network fetch refuses a non-GitHub
/// origin before connecting.
#[cfg(feature = "ghreleases")]
#[test]
fn releases_fetch_github_with_feature() {
    let e = env();
    let rel = testkit::releases(&e.t.path().join("gh.git"));
    let id = add(&e, &rel, Source::Imported);
    let json = r#"[{"tag_name":"v1.0.0","name":"One point oh","body":"From **GitHub**","draft":false,"prerelease":false}]"#;
    rs_core::ghreleases::store(shelf(&e), id, json).unwrap();
    let list = releases::releases_list_impl(&e.st, id).unwrap();
    let v1 = list.iter().find(|r| r.tag == "v1.0.0").unwrap();
    assert!(v1.gh_notes);
    assert_eq!(v1.title, "One point oh");
    let before = testkit::net_calls();
    assert_eq!(code(releases::releases_fetch_github_impl(&e.st, id)), "refused", "no github origin");
    assert_eq!(testkit::net_calls(), before);
}

#[cfg(not(feature = "ghreleases"))]
#[test]
fn releases_fetch_github_without_feature_is_unsupported() {
    let e = env();
    assert_eq!(code(releases::releases_fetch_github_impl(&e.st, e.id)), "unsupported");
}

// ---------------------------------------------------------------------------------------------------------------
// Jobs

fn recording_channel() -> (Channel<JobInfo>, Arc<Mutex<Vec<serde_json::Value>>>) {
    let got = Arc::new(Mutex::new(Vec::new()));
    let g = got.clone();
    let ch = Channel::new(move |body: InvokeResponseBody| {
        if let InvokeResponseBody::Json(s) = body {
            g.lock().unwrap().push(serde_json::from_str(&s).unwrap());
        }
        Ok(())
    });
    (ch, got)
}

#[test]
fn clone_preview_and_start() {
    let e = env();
    for bad in ["", "git@github.com:o/r.git", "http://example.com/o/r", "file:///C:/x", "a/b/c", "https://u:p@github.com/o/r", "C:\\repo"] {
        assert_eq!(code(jobs::clone_preview_impl(&e.st, bad)), "invalidUrl", "{bad:?}");
        assert_eq!(code(jobs::clone_start_impl(&e.st, bad, None)), "invalidUrl", "{bad:?}");
    }
    assert!(e.st.jobs.list().is_empty(), "rejected input never creates a job");
    let p = jobs::clone_preview_impl(&e.st, "owner/repo").unwrap();
    assert_eq!(p.url, "https://github.com/owner/repo.git");
    assert!(p.target_dir.ends_with("repo.git"), "{}", p.target_dir);

    let (ch, got) = recording_channel();
    let snap = jobs::jobs_subscribe_impl(&e.st, "main", ch);
    assert!(snap.is_empty());
    let id = jobs::clone_start_impl(&e.st, " owner/repo ", None).unwrap();
    e.st.jobs.join();
    let info = e.st.jobs.get(id).unwrap();
    assert_eq!(info.state, JobState::Done);
    assert_eq!(info.title, "Clone owner/repo");
    assert_eq!(*e.worked.lock().unwrap(), vec![JobKind::clone_url("https://github.com/owner/repo.git")]);
    // the channel saw waiting → running → done, camelCase JobInfo
    let events = got.lock().unwrap().clone();
    let states: Vec<&str> = events.iter().map(|v| v["state"].as_str().unwrap()).collect();
    assert_eq!(states.first(), Some(&"waiting"));
    assert_eq!(states.last(), Some(&"done"));
    assert_eq!(events.last().unwrap()["result"]["type"], "cloned");
    assert!(events.last().unwrap().get("startedAt").is_some());
    // a new subscription's snapshot contains the job
    let (ch2, _) = recording_channel();
    let snap = jobs::jobs_subscribe_impl(&e.st, "main", ch2);
    assert_eq!(snap.iter().map(|j| j.id).collect::<Vec<_>>(), vec![id]);
    assert_eq!(e.st.subscribers.len(), 1, "same webview replaces its channel");
}

#[test]
fn fetch_jobs_cancel_retry_dismiss() {
    let e = env();
    let id = jobs::fetch_start_impl(&e.st, e.id, None).unwrap();
    e.st.jobs.join();
    assert_eq!(e.st.jobs.get(id).unwrap().title, "Fetch example/basic");
    assert_eq!(e.st.jobs.get(id).unwrap().state, JobState::Done);
    // no origin → refused synchronously
    let local = add(&e, &testkit::releases(&e.t.path().join("noorigin.git")), Source::Imported);
    assert_eq!(code(jobs::fetch_start_impl(&e.st, local, None)), "refused");
    let all = jobs::fetch_all_start_impl(&e.st).unwrap();
    e.st.jobs.join();
    assert_eq!(e.st.jobs.get(all).unwrap().kind, JobKind::FetchAll);
    // finished jobs: cancel → false, retry of a done job refused, dismiss removes
    assert!(!jobs::jobs_cancel_impl(&e.st, id));
    assert!(!jobs::jobs_cancel_impl(&e.st, 777));
    assert_eq!(code(jobs::jobs_retry_impl(&e.st, id)), "refused");
    assert_eq!(code(jobs::jobs_retry_impl(&e.st, 777)), "notFound");
    jobs::jobs_dismiss_impl(&e.st, id);
    assert!(e.st.jobs.get(id).is_none());
    // fast-forward is managed-only
    assert_eq!(code(jobs::fast_forward_impl(&e.st, e.id)), "refused");
}

#[test]
fn waiting_job_can_be_cancelled_and_retried() {
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let r2 = release.clone();
    let worker: Worker = Arc::new(move |_k: &JobKind, ctx: &JobCtx| {
        while !r2.load(std::sync::atomic::Ordering::SeqCst) {
            if ctx.cancelled() {
                return Err(Error::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Ok(JobResult::Fetched(FetchOutcome::default()))
    });
    let e = env_with(Some(worker), false);
    let ids: Vec<_> = (0..4).map(|_| jobs::fetch_all_start_impl(&e.st).unwrap()).collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while e.st.jobs.running() < crate::state::MAX_JOBS {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let waiting = ids.iter().copied().find(|i| e.st.jobs.get(*i).unwrap().state == JobState::Waiting).unwrap();
    assert!(jobs::jobs_cancel_impl(&e.st, waiting));
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    e.st.jobs.join();
    assert_eq!(e.st.jobs.get(waiting).unwrap().state, JobState::Cancelled);
    let again = jobs::jobs_retry_impl(&e.st, waiting).unwrap();
    e.st.jobs.join();
    assert_ne!(again, waiting);
    assert_eq!(e.st.jobs.get(again).unwrap().state, JobState::Done);
    assert!(e.st.jobs.get(waiting).is_none());
}

#[test]
fn job_errors_get_friendly_text() {
    let base: Worker = Arc::new(|kind: &JobKind, _ctx: &JobCtx| match kind {
        JobKind::Clone { .. } => Err(Error::Git(git2::Error::new(git2::ErrorCode::GenericError, git2::ErrorClass::Http, "request failed with status code: 401"))),
        JobKind::Fetch { .. } => Err(Error::Git(git2::Error::new(git2::ErrorCode::GenericError, git2::ErrorClass::Os, "failed to connect to host"))),
        JobKind::FetchAll => Err(Error::Cancelled),
        JobKind::GithubSync { .. } | JobKind::ReleaseDownload { .. } => Err(Error::Network("unused".into())),
    });
    let t = tempfile::tempdir().unwrap();
    let shelf = Arc::new(testkit::shelf(t.path()));
    let basic = testkit::basic(&t.path().join("b"));
    let id = rs_core::library::import(&shelf, &basic.path).unwrap().id;
    let worker = crate::state::friendly_worker(shelf.clone(), base);
    let st = AppState::with_worker(shelf, false, worker, Arc::new(FakePicker::default()), Arc::new(FakeTrash::default()), Arc::new(FakeOpener::default()));
    let c = jobs::clone_start_impl(&st, "https://codeberg.org/o/r", None).unwrap();
    let f = jobs::fetch_start_impl(&st, id, None).unwrap();
    let a = jobs::fetch_all_start_impl(&st).unwrap();
    st.jobs.join();
    assert_eq!(st.jobs.get(c).unwrap().error.as_deref(), Some(CREDENTIALS));
    assert_eq!(st.jobs.get(f).unwrap().error.as_deref(), Some("Could not reach github.com: failed to connect to host"));
    assert_eq!(st.jobs.get(a).unwrap().state, JobState::Cancelled);
}

/// Shipped builds: the production worker refuses a file:// clone even if the flag were set.
#[cfg(not(feature = "e2e"))]
#[test]
fn file_clone_seam_absent_without_e2e() {
    let e = env_with(None, true);
    let (_up, url) = testkit::upstream_downstream(&e.t.path().join("up").join("demo"));
    assert_eq!(code(jobs::clone_preview_impl(&e.st, &url)), "invalidUrl");
    assert_eq!(code(jobs::clone_start_impl(&e.st, &url, None)), "invalidUrl");
}

/// e2e build with the flag: file:// clone and fetch through the command layer (D-14, D-20).
#[cfg(feature = "e2e")]
#[test]
fn file_clone_seam_in_e2e_build() {
    let t = tempfile::tempdir().unwrap();
    let shelf = Arc::new(testkit::shelf(t.path()));
    let worker = crate::state::make_worker(shelf.clone(), true);
    let st = AppState::with_worker(shelf.clone(), true, worker, Arc::new(FakePicker::default()), Arc::new(FakeTrash::default()), Arc::new(FakeOpener::default()));
    let (mut up, url) = testkit::upstream_downstream(&t.path().join("up").join("demo"));
    let p = jobs::clone_preview_impl(&st, &url).unwrap();
    assert_eq!(p.name, "demo");
    let id = jobs::clone_start_impl(&st, &url, None).unwrap();
    st.jobs.join();
    let info = st.jobs.get(id).unwrap();
    assert_eq!(info.state, JobState::Done, "{:?}", info.error);
    let Some(JobResult::Cloned { repo_id, .. }) = info.result else { panic!("{info:?}") };
    // the UI offers Fetch on the file:// clone only through the seam (D-90)
    assert!(library::repo_detail_impl(&st, repo_id).unwrap().fetchable);
    up.file("more", b"1").commit("more", testkit::T0 + 999);
    let f = jobs::fetch_start_impl(&st, repo_id, None).unwrap();
    st.jobs.join();
    let info = st.jobs.get(f).unwrap();
    assert_eq!(info.state, JobState::Done, "{:?}", info.error);
    // without the flag the same build refuses file://
    let off = AppState::with_worker(shelf.clone(), false, crate::state::make_worker(shelf, false), Arc::new(FakePicker::default()), Arc::new(FakeTrash::default()), Arc::new(FakeOpener::default()));
    assert_eq!(code(jobs::clone_start_impl(&off, &url, None)), "invalidUrl");
    assert!(!library::repo_detail_impl(&off, repo_id).unwrap().fetchable, "file:// is not fetchable without the seam");
}

// ---------------------------------------------------------------------------------------------------------------
// Navigation guard and registration

#[test]
fn navigation_guard_allows_only_the_app() {
    let u = |s: &str| tauri::Url::parse(s).unwrap();
    for ok in ["http://tauri.localhost/", "http://tauri.localhost/index.html#/o/r", "https://tauri.localhost/", "tauri://localhost/"] {
        assert!(crate::is_app_url(&u(ok), false), "{ok}");
    }
    assert!(crate::is_app_url(&u("http://localhost:1450/#/settings"), true));
    for bad in [
        "http://localhost:1450/",
        "https://github.com/",
        "http://tauri.localhost.evil.com/",
        "http://tauri.localhost:8080/",
        "file:///C:/Windows/win.ini",
        "javascript:alert(1)",
        "data:text/html,x",
        "about:blank",
        "tauri://evil/",
    ] {
        assert!(!crate::is_app_url(&u(bad), false), "{bad}");
    }
    assert!(!crate::is_app_url(&u("http://localhost:1451/"), true));
    assert!(!crate::is_app_url(&u("https://localhost:1450/"), true));
}

// ---------------------------------------------------------------------------------------------------------------
// v0.2.0 M11: remote images, release downloads, GitHub follow-up jobs (all offline: canned HTTP)

/// An env whose shelf talks to `canned` (or refuses everything with `Net::offline()`).
fn env_net(canned: Option<Arc<rs_core::net::Canned>>) -> (tempfile::TempDir, AppState, Arc<FakeOpener>, Arc<Mutex<Vec<JobKind>>>) {
    let t = tempfile::tempdir().unwrap();
    let mut s = testkit::shelf(t.path());
    if let Some(c) = canned {
        s = s.with_net(rs_core::net::Net::new(c));
    }
    let shelf = Arc::new(s);
    std::fs::create_dir_all(&shelf.dirs.archive).unwrap();
    let opener = Arc::new(FakeOpener::default());
    let worked = Arc::new(Mutex::new(Vec::new()));
    let st = AppState::with_worker(
        shelf,
        false,
        fake_worker(worked.clone()),
        Arc::new(FakePicker::default()),
        Arc::new(FakeTrash::default()),
        opener.clone(),
    );
    (t, st, opener, worked)
}

/// A work-tree repo with a github.com origin.
fn github_repo(dir: &Path) -> PathBuf {
    let mut b = testkit::repo(dir);
    b.file("a", b"1").commit("one", testkit::T0).tag("v1.0.0", false).set_head().origin("https://github.com/o/r.git");
    dir.to_path_buf()
}

#[test]
fn image_commands_fetch_cache_and_refuse() {
    let c = rs_core::net::Canned::new();
    c.on("https://example.com/a.png", 200, &[("content-type", "image/png")], b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec());
    let (_t, st, _, _) = env_net(Some(c.clone()));
    let img = images::image_fetch_impl(&st, "https://example.com/a.png").unwrap();
    assert_eq!(img.mime, "image/png");
    assert_eq!(images::image_bytes_impl(&st, &img.key).unwrap().len() as u64, img.size);
    assert!(app::app_info_impl(&st).unwrap().cache_bytes >= img.size);
    for bad in ["", "..", "../x", "C:\\x", &"g".repeat(64), &"A".repeat(64), &"a".repeat(63)] {
        assert_eq!(code(images::image_bytes_impl(&st, bad)), "invalidInput", "{bad:?}");
    }
    for bad in ["http://example.com/a.png", "https://127.0.0.1/a.png", "https://localhost/a.png", "file:///C:/x.png", "javascript:alert(1)", "rsrepo:/a.png"] {
        assert_eq!(code(images::image_fetch_impl(&st, bad)), "refused", "{bad:?}");
    }
    assert_eq!(code(images::image_fetch_impl(&st, "nope")), "invalidUrl");
    assert_eq!(c.seen().len(), 1, "refusals never reach the transport");
    // Clear cache empties the image cache too
    app::cache_clear_impl(&st).unwrap();
    assert_eq!(code(images::image_bytes_impl(&st, &img.key)), "notFound");
    // the setting turns it off (enforced in Rust)
    let off = rs_core::settings::Settings { load_remote_images: false, ..Default::default() };
    app::settings_set_impl(&st, off).unwrap();
    assert_eq!(code(images::image_fetch_impl(&st, "https://example.com/a.png")), "refused");
    assert_eq!(c.seen().len(), 1);
}

#[test]
fn release_download_and_reveal_use_names_from_the_cached_answer() {
    let (t, st, opener, worked) = env_net(None);
    let id = testkit::register_raw(&st.shelf, &github_repo(&t.path().join("r")), Source::Imported).unwrap();
    assert_eq!(code(jobs::release_download_impl(&st, id, "v1.0.0", "a.zip")), "notFound", "no cached list yet");
    let json = r#"[{"tag_name":"v1.0.0","assets":[{"name":"a.zip","size":1,"browser_download_url":"https://github.com/o/r/releases/download/v1.0.0/a.zip"}]}]"#;
    rs_core::ghreleases::store(&st.shelf, id, json).unwrap();
    for (tag, asset) in [("", "a.zip"), ("v1.0.0", ""), ("v1\n", "a.zip"), ("v1.0.0", &"x".repeat(600))] {
        assert_eq!(code(jobs::release_download_impl(&st, id, tag, asset)), "invalidInput", "{tag:?} {asset:?}");
    }
    for (tag, asset) in [("v2", "a.zip"), ("v1.0.0", "b.zip"), ("v1.0.0", "..\\..\\x.exe"), ("v1.0.0", "https://evil.example/a.zip")] {
        assert_eq!(code(jobs::release_download_impl(&st, id, tag, asset)), "notFound", "{tag:?} {asset:?}");
    }
    let job = jobs::release_download_impl(&st, id, "v1.0.0", "a.zip").unwrap();
    st.jobs.join();
    assert!(st.jobs.get(job).unwrap().title.contains("a.zip"));
    assert_eq!(*worked.lock().unwrap(), vec![JobKind::ReleaseDownload { repo_id: id, tag: "v1.0.0".into(), asset: "a.zip".into() }]);
    // reveal: only an existing file in the store, path computed in Rust
    assert_eq!(code(releases::release_reveal_impl(&st, id, "v1.0.0", "a.zip")), "notFound");
    let file = st.shelf.dirs.archive.join("_releases").join("github.com").join("o").join("r").join("v1.0.0").join("a.zip");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"x").unwrap();
    releases::release_reveal_impl(&st, id, "v1.0.0", "a.zip").unwrap();
    assert_eq!(opener.reveals.lock().unwrap().as_slice(), std::slice::from_ref(&file));
    let list = releases::releases_list_impl(&st, id).unwrap();
    assert_eq!(list[0].assets[0].downloaded.as_deref(), Some(file.display().to_string().as_str()));
    assert_eq!(list[0].source_zip_url.as_deref(), Some("https://github.com/o/r/archive/refs/tags/v1.0.0.zip"));
}

#[test]
fn import_queues_github_sync_and_actions_pass_their_download_option() {
    let (t, st, _, worked) = env_net(None);
    let gh = github_repo(&t.path().join("gh"));
    let p = dialogs::import_preview_for(&st, &gh).unwrap();
    let sum = dialogs::import_confirm_impl(&st, &p.token, Some(false)).unwrap();
    // a non-GitHub import queues nothing
    let mut b = testkit::repo(&t.path().join("lab"));
    b.file("a", b"1").commit("one", testkit::T0).set_head().origin("https://gitlab.com/o/lab.git");
    let p2 = dialogs::import_preview_for(&st, &t.path().join("lab")).unwrap();
    dialogs::import_confirm_impl(&st, &p2.token, None).unwrap();
    jobs::clone_start_impl(&st, "o/r2", Some(true)).unwrap();
    jobs::fetch_start_impl(&st, sum.id, Some(false)).unwrap();
    st.jobs.join();
    let got = worked.lock().unwrap().clone();
    assert!(got.contains(&JobKind::GithubSync { repo_id: sum.id, auto_download: Some(false) }), "{got:?}");
    assert!(got.contains(&JobKind::Clone { url: "https://github.com/o/r2.git".into(), auto_download: Some(true) }), "{got:?}");
    assert!(got.contains(&JobKind::Fetch { repo_id: sum.id, auto_download: Some(false) }), "{got:?}");
    assert_eq!(got.iter().filter(|k| matches!(k, JobKind::GithubSync { .. })).count(), 1, "{got:?}");
    // every GitHub setting off → no follow-up job either
    let off = rs_core::settings::Settings { fetch_release_notes: false, fetch_repo_info: false, auto_download_latest_release: false, ..Default::default() };
    app::settings_set_impl(&st, off).unwrap();
    let gh2 = github_repo(&t.path().join("gh2"));
    let p3 = dialogs::import_preview_for(&st, &gh2).unwrap();
    dialogs::import_confirm_impl(&st, &p3.token, None).unwrap();
    st.jobs.join();
    assert_eq!(worked.lock().unwrap().iter().filter(|k| matches!(k, JobKind::GithubSync { .. })).count(), 1);
}

/// Every command of ARCH §5.1 is in `generate_handler!` and nothing else is.
#[test]
fn every_arch_command_is_registered() {
    let arch = include_str!("../../docs/ARCHITECTURE.md");
    let table = arch.split("### 5.1 Commands").nth(1).unwrap().split("### 5.2").next().unwrap();
    let mut want: Vec<String> = Vec::new();
    for line in table.lines().filter(|l| l.starts_with("| `")) {
        let cell = line.split('|').nth(1).unwrap();
        for part in cell.split('/') {
            let name = part.trim().trim_matches('`');
            if !name.is_empty() {
                want.push(name.to_string());
            }
        }
    }
    let lib = include_str!("lib.rs");
    let list = lib.split("generate_handler![").nth(1).unwrap().split("])").next().unwrap();
    let mut got: Vec<String> = list
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.rsplit("::").next().unwrap().to_string())
        .collect();
    want.sort();
    got.sort();
    assert_eq!(want.len(), 73, "{want:?}");
    assert_eq!(got, want);
}

/// `RS_TEST_FILE_CLONE` may appear only in cfg(e2e) code (D-14): in this crate's sources, every line naming it
/// sits right after a `#[cfg(feature = "e2e")]` item header.
#[test]
fn file_clone_env_literal_only_in_e2e_code() {
    let lib = include_str!("lib.rs");
    let needle = concat!("RS_TEST_", "FILE_CLONE");
    let lines: Vec<&str> = lib.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.contains(needle) && !l.trim_start().starts_with("//") {
            let cfg = lines[..i].iter().rev().take(4).any(|x| x.contains("#[cfg(feature = \"e2e\")]"));
            assert!(cfg, "line {} uses the literal outside cfg(e2e): {l}", i + 1);
        }
    }
    for src in [include_str!("state.rs"), include_str!("cmd/jobs.rs"), include_str!("cmd/mod.rs")] {
        assert!(!src.lines().any(|l| l.contains(needle) && !l.trim_start().starts_with("//")), "literal outside lib.rs");
    }
}

/// The e2e seam env names (D-84) live only in `e2e.rs`, which `lib.rs` declares under cfg(e2e).
#[test]
fn e2e_seam_env_names_only_in_the_e2e_module() {
    let lib = include_str!("lib.rs");
    let decl = lib.lines().position(|l| l.trim() == "pub mod e2e;").expect("e2e module declared");
    assert!(lib.lines().nth(decl - 1).unwrap().contains("#[cfg(feature = \"e2e\")]"));
    let needle = concat!("RS_", "E2E_");
    for src in [lib, include_str!("state.rs"), include_str!("cmd/jobs.rs"), include_str!("cmd/mod.rs"), include_str!("cmd/dialogs.rs"), include_str!("cmd/app.rs")] {
        assert!(!src.contains(needle), "e2e env name outside e2e.rs");
    }
}

#[cfg(feature = "e2e")]
mod e2e_seams {
    use super::*;
    use crate::e2e::{E2eSeams, parse_pick_queue};

    fn seams(pick: Option<&str>, log: Option<PathBuf>) -> (E2eSeams, Arc<FakePicker>, Arc<FakeOpener>) {
        let p = Arc::new(FakePicker::default());
        let o = Arc::new(FakeOpener::default());
        (E2eSeams::new(pick, log, p.clone(), o.clone()), p, o)
    }

    #[test]
    fn pick_queue_parses() {
        let q = parse_pick_queue(" C:\\a ;; C:\\b c;");
        assert_eq!(q, VecDeque::from([PathBuf::from("C:\\a"), PathBuf::from("C:\\b c")]));
        assert!(parse_pick_queue("").is_empty());
    }

    #[test]
    fn queued_picks_then_cancel_and_no_dialog() {
        let (s, p, _) = seams(Some("C:\\one;C:\\two"), None);
        assert_eq!(s.pick_folder("t"), Some(PathBuf::from("C:\\one")));
        assert_eq!(s.pick_folder("t"), Some(PathBuf::from("C:\\two")));
        assert_eq!(s.pick_folder("t"), None);
        assert_eq!(s.save_file("t", "x.txt"), None);
        assert!(p.calls.lock().unwrap().is_empty(), "the real dialog must not open");
    }

    #[test]
    fn without_env_the_real_seams_are_used() {
        let (s, p, o) = seams(None, None);
        p.folders.lock().unwrap().push_back(Some(PathBuf::from("C:\\real")));
        assert_eq!(s.pick_folder("t"), Some(PathBuf::from("C:\\real")));
        s.open_url("https://example.com/").unwrap();
        assert_eq!(o.urls.lock().unwrap().as_slice(), ["https://example.com/"]);
    }

    #[test]
    fn opens_are_logged_not_opened() {
        let t = tempfile::tempdir().unwrap();
        let log = t.path().join("opens.log");
        let (s, _, o) = seams(None, Some(log.clone()));
        s.open_url("https://example.com/x").unwrap();
        s.open_folder(Path::new("C:\\data")).unwrap();
        s.reveal(Path::new("C:\\arc\\r.git")).unwrap();
        assert!(o.urls.lock().unwrap().is_empty() && o.folders.lock().unwrap().is_empty());
        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(text, "url\thttps://example.com/x\nfolder\tC:\\data\nreveal\tC:\\arc\\r.git\n");
    }

    /// `app_open_dir` through the seam: the data folder lands in the log.
    #[test]
    fn app_open_dir_through_the_recording_opener() {
        let e = env();
        let log = e.t.path().join("opens.log");
        let seams = Arc::new(E2eSeams::new(Some(""), Some(log.clone()), e.picker.clone(), e.opener.clone()));
        let shelf = e.st.shelf.clone();
        let st = AppState::with_worker(shelf.clone(), false, fake_worker(Arc::default()), seams.clone(), Arc::new(FakeTrash::default()), seams);
        app::app_open_dir_impl(&st, "data").unwrap();
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.starts_with("folder\t") && text.contains("data"), "{text}");
    }

    /// D-91: with the log set, the folder is deleted (read-only files included) and logged; the real trasher is never
    /// called; folders outside the archive (and the archive itself) are refused.
    #[test]
    fn trash_seam_deletes_inside_the_archive_only() {
        use crate::e2e::E2eTrash;
        let t = tempfile::tempdir().unwrap();
        let archive = t.path().join("archive");
        let clone = archive.join("o").join("r.git");
        std::fs::create_dir_all(clone.join("objects")).unwrap();
        let pack = clone.join("objects").join("pack.idx");
        std::fs::write(&pack, b"x").unwrap();
        let mut perm = std::fs::metadata(&pack).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&pack, perm).unwrap();
        let outside = t.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let log = t.path().join("trash.log");
        let real = Arc::new(FakeTrash::default());
        let s = E2eTrash::new(Some(log.clone()), archive.clone(), real.clone());
        assert!(s.delete(&outside).is_err() && outside.exists());
        assert!(s.delete(&archive).is_err() && archive.exists());
        s.delete(&clone).unwrap();
        assert!(!clone.exists());
        assert!(real.got.lock().unwrap().is_empty(), "the real Recycle Bin must not be used");
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.starts_with("trash\t") && text.trim_end().ends_with("r.git") && text.lines().count() == 1, "{text}");
    }

    #[test]
    fn trash_seam_without_env_uses_the_real_trasher() {
        use crate::e2e::E2eTrash;
        let real = Arc::new(FakeTrash::default());
        let s = E2eTrash::new(None, PathBuf::from("C:\\nowhere"), real.clone());
        s.delete(Path::new("C:\\nowhere\\x.git")).unwrap();
        assert_eq!(real.got.lock().unwrap().as_slice(), [PathBuf::from("C:\\nowhere\\x.git")]);
    }

    /// M13 network seam: routes answer through the production Net rules (redirect hops re-checked), unrouted URLs
    /// fail as `network`, every request is logged.
    #[test]
    fn canned_net_answers_routes_logs_and_fails_the_rest() {
        use crate::e2e::E2eNet;
        use rs_core::net::{Net, Profile};
        let t = tempfile::tempdir().unwrap();
        let log = t.path().join("net.log");
        let json = r#"{ "routes": {
            "https://api.github.com/repos/e2e/demo": { "body": "{\"description\":\"d\"}", "headers": { "Content-Type": "application/json" } },
            "https://github.com/e2e/demo/releases/download/v1/a.zip": { "status": 302, "headers": { "location": "https://objects.githubusercontent.com/a" } },
            "https://objects.githubusercontent.com/a": { "bodyHex": "504b0506" },
            "https://images.example.com/tracker.png": { "status": 404 }
        } }"#;
        let n = Arc::new(E2eNet::parse(json, Some(log.clone())).unwrap());
        let net = Net::new(n.clone());
        let r = net.get("https://api.github.com/repos/e2e/demo", &[], Profile::API).unwrap();
        assert_eq!((r.status, r.mime()), (200, "application/json".to_string()));
        assert_eq!(r.bytes().unwrap(), br#"{"description":"d"}"#);
        let zip = net.get("https://github.com/e2e/demo/releases/download/v1/a.zip", &[], Profile::asset(100)).unwrap();
        assert_eq!(zip.bytes().unwrap(), [0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(net.get("https://images.example.com/tracker.png", &[], Profile::IMAGE).unwrap().status, 404);
        assert_eq!(net.get("https://example.org/x.png", &[], Profile::IMAGE).err().unwrap().code(), "network");
        // refused by the production rules before the transport is asked (not logged)
        assert_eq!(net.get("http://example.org/x.png", &[], Profile::IMAGE).err().unwrap().code(), "refused");
        let text = std::fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5, "{text}");
        assert_eq!(lines[0], "get\thttps://api.github.com/repos/e2e/demo\t200");
        assert_eq!(lines[2], "get\thttps://objects.githubusercontent.com/a\t200");
        assert_eq!(lines[4], "get\thttps://example.org/x.png\tunrouted");
        assert_eq!(n.seen().len(), 5);
        assert!(E2eNet::parse(r#"{ "routes": { "https://a.example.com/": { "body": "x", "bodyHex": "00" } } }"#, None).is_err());
        assert!(E2eNet::parse(r#"{ "routes": {}, "extra": 1 }"#, None).is_err());
    }

    #[test]
    fn git_map_parses_and_normalises() {
        use crate::e2e::parse_git_map;
        let (m, live) = parse_git_map("e2e/demo=file:///C:/up/demo; https://github.com/o/r.git=file:///C:/up/r ;").unwrap();
        assert!(!live);
        let key = rs_core::validate::clone_url("https://github.com/e2e/demo").unwrap().url;
        assert_eq!(m.get(&key).map(String::as_str), Some("file:///C:/up/demo"));
        assert_eq!(m.len(), 2);
        assert!(parse_git_map("!live").unwrap().1);
        assert!(parse_git_map("e2e/demo=https://elsewhere.example.com/x").is_err());
        assert!(parse_git_map("no-equals-sign").is_err());
    }
}
