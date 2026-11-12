//! RepoShelf Tauri shell (M4): startup, the `main` window (created in code so a scratch `RS_DATA_DIR` gets its
//! own WebView2 profile), the navigation guard and the IPC command table (ARCH §5.1).

pub mod cmd;
#[cfg(feature = "e2e")]
pub mod e2e;
pub mod ipc_error;
pub mod state;

use rs_core::Shelf;
use rs_core::jobs::CancelToken;
use state::{AppState, RealTrash, TauriShell};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::webview::NewWindowResponse;
use tauri::{Emitter, Manager, RunEvent, Url, WebviewUrl, WebviewWindowBuilder};

pub const MAIN_WINDOW: &str = "main";
/// Emitted once the startup summary refresh finished; payload `RepoSummary[]`.
pub const EVENT_LIBRARY_REFRESHED: &str = "library-refreshed";
/// Vite dev server (`vite.config.ts`, `tauri.conf.json` devUrl).
pub const DEV_PORT: u16 = 1450;

/// Navigation guard (NFR-S3, defence in depth): only the app's own origin, plus the Vite dev server in a
/// `tauri dev` build. Everything else (http(s) links, `file:`, `javascript:`, `data:`) is refused.
pub fn is_app_url(u: &Url, dev: bool) -> bool {
    match u.scheme() {
        "tauri" => u.host_str() == Some("localhost"),
        "http" | "https" => {
            let host = u.host_str().unwrap_or("");
            (host == "tauri.localhost" && u.port().is_none())
                || (dev && u.scheme() == "http" && host == "localhost" && u.port() == Some(DEV_PORT))
        }
        _ => false,
    }
}

/// `RS_TEST_FILE_CLONE=1` is honoured only by the separate e2e build (D-14).
#[cfg(feature = "e2e")]
fn test_file_clone_env() -> bool {
    std::env::var("RS_TEST_FILE_CLONE").is_ok_and(|v| v == "1")
}

#[cfg(not(feature = "e2e"))]
fn test_file_clone_env() -> bool {
    false
}

/// Dirs, DB, interrupted jobs, partial-clone cleanup, state, window, background refresh (M4.3).
fn startup<R: tauri::Runtime>(app: &mut tauri::App<R>, test_file_clone: bool) -> Result<(), Box<dyn std::error::Error>> {
    let dirs = rs_core::paths::resolve_dirs();
    std::fs::create_dir_all(rs_core::paths::long(&dirs.data))?;
    std::fs::create_dir_all(rs_core::paths::long(&dirs.archive))?;
    // FR-8: leftovers of a killed clone (before any job can start).
    if let Err(e) = rs_core::clone::cleanup_partials(&dirs.archive) {
        eprintln!("RepoShelf: partial clone cleanup failed: {e}");
    }
    // v0.2.0: leftovers of a killed release download
    if let Err(e) = rs_core::assets::cleanup_partials(&dirs.archive) {
        eprintln!("RepoShelf: partial download cleanup failed: {e}");
    }
    let shelf = Arc::new(Shelf::open(dirs.clone())?);
    let shell = Arc::new(TauriShell { app: app.handle().clone() });
    let (picker, opener): (Arc<dyn state::Picker>, Arc<dyn state::Opener>) = (shell.clone(), shell);
    // e2e build only (D-84): env-driven folder picks and a recording opener.
    #[cfg(feature = "e2e")]
    let (picker, opener): (Arc<dyn state::Picker>, Arc<dyn state::Opener>) = {
        let seams = Arc::new(e2e::E2eSeams::from_env(picker, opener));
        (seams.clone(), seams)
    };
    let trasher: Arc<dyn state::Trasher> = Arc::new(RealTrash);
    // e2e build only (D-91): a scratch run deletes the removed clone instead of filling the real Recycle Bin.
    #[cfg(feature = "e2e")]
    let trasher: Arc<dyn state::Trasher> = Arc::new(e2e::E2eTrash::from_env(dirs.archive.clone(), trasher));
    let st = Arc::new(AppState::new(shelf.clone(), test_file_clone, picker, trasher, opener));
    st.jobs.load_persisted()?;
    app.manage(st.clone());

    let mut win = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("RepoShelf")
        .inner_size(1320.0, 840.0)
        .min_inner_size(900.0, 560.0)
        .on_navigation(|u| is_app_url(u, tauri::is_dev()))
        .on_new_window(|_, _| NewWindowResponse::Deny);
    #[cfg(feature = "e2e")]
    {
        win = if e2e::offscreen() { win.position(e2e::OFFSCREEN_POS.0, e2e::OFFSCREEN_POS.1).focused(false) } else { win.center() };
    }
    #[cfg(not(feature = "e2e"))]
    {
        win = win.center();
    }
    if let Some(d) = std::env::var_os(rs_core::paths::ENV_DATA_DIR).filter(|d| !d.is_empty()) {
        win = win.data_directory(PathBuf::from(d).join("webview"));
    }
    win.build()?;

    let handle = app.handle().clone();
    let cancel: CancelToken = st.startup_refresh.clone();
    std::thread::Builder::new().name("reposhelf-startup".into()).spawn(move || {
        let tips: Vec<String> = shelf
            .db
            .with(|c| {
                let mut s = c.prepare("SELECT head_tip FROM repos WHERE head_tip IS NOT NULL")?;
                let v = s.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
                Ok(v)
            })
            .unwrap_or_default();
        if let Err(e) = rs_core::db::prune_caches(&shelf.db, &tips) {
            eprintln!("RepoShelf: cache pruning failed: {e}");
        }
        match rs_core::library::refresh_summaries(&shelf, &cancel) {
            Ok(list) => {
                let _ = handle.emit(EVENT_LIBRARY_REFRESHED, list);
            }
            Err(e) => eprintln!("RepoShelf: startup refresh stopped: {e}"),
        }
    })?;
    Ok(())
}

pub fn run() {
    let test_file_clone = test_file_clone_env();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Rust-side only (D-10): no click interception script in the webview.
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build())
        .setup(move |app| startup(app, test_file_clone))
        .invoke_handler(tauri::generate_handler![
            cmd::app::app_info,
            cmd::app::settings_get,
            cmd::app::settings_set,
            cmd::app::cache_clear,
            cmd::app::request_cancel,
            cmd::app::open_external,
            cmd::app::app_open_dir,
            cmd::library::library_list,
            cmd::library::library_refresh,
            cmd::library::repo_by_slug,
            cmd::library::repo_set_metadata,
            cmd::library::repo_detail,
            cmd::library::repo_open_folder,
            cmd::dialogs::import_pick,
            cmd::dialogs::import_confirm,
            cmd::dialogs::repo_locate,
            cmd::dialogs::repo_remove,
            cmd::dialogs::blob_save_as,
            cmd::tree::refs_list,
            cmd::tree::rev_resolve,
            cmd::tree::tree_list,
            cmd::tree::tree_last_commits,
            cmd::tree::commit_latest,
            cmd::tree::commit_count,
            cmd::tree::readme_render,
            cmd::tree::markdown_render_file,
            cmd::tree::languages,
            cmd::blob::blob_meta,
            cmd::blob::blob_text,
            cmd::blob::blob_bytes,
            cmd::blob::file_last_commit,
            cmd::commits::commits_page,
            cmd::commits::commit_detail,
            cmd::commits::commit_file_diff,
            cmd::refs::branches_list,
            cmd::refs::branches_ahead_behind,
            cmd::refs::tags_list,
            cmd::releases::releases_list,
            cmd::releases::releases_fetch_github,
            cmd::releases::release_reveal,
            cmd::images::image_fetch,
            cmd::images::image_bytes,
            cmd::jobs::clone_preview,
            cmd::jobs::clone_start,
            cmd::jobs::fetch_start,
            cmd::jobs::release_download,
            cmd::jobs::fetch_all_start,
            cmd::jobs::fast_forward,
            cmd::jobs::jobs_subscribe,
            cmd::jobs::jobs_cancel,
            cmd::jobs::jobs_retry,
            cmd::jobs::jobs_dismiss,
        ])
        .build(tauri::generate_context!())
        .expect("error while starting RepoShelf");
    app.run(|app, ev| {
        if let RunEvent::Exit = ev
            && let Some(st) = app.try_state::<Arc<AppState>>()
        {
            st.startup_refresh.cancel();
            // Running clones remove their partial folder on cancel; anything left is cleaned up next start.
            for j in st.jobs.list() {
                st.jobs.cancel(j.id);
            }
        }
    });
}

#[cfg(test)]
mod tests;
