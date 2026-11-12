//! App-level commands (M4.4): `app_info`, settings, `cache_clear`, `request_cancel`, `open_external`.
//! FR-33, FR-35, S10, NFR-S3.

use super::{St, blocking};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::settings::{self, Settings};
use rs_core::types::AppInfo;
use std::path::Path;

/// Archive size walk stops after this many entries (the size is then unknown: `archiveBytes: null`).
pub const ARCHIVE_SCAN_CAP: u32 = 200_000;
const CACHE_TABLES: [&str; 3] = ["cache_last_commit", "cache_languages", "cache_gh_releases"];

fn cache_bytes(st: &AppState) -> Result<u64> {
    st.shelf.db.with(|c| {
        let mut total: i64 = 0;
        for t in CACHE_TABLES {
            total += c.query_row(&format!("SELECT COALESCE(SUM(LENGTH(json)), 0) FROM {t}"), [], |r| r.get::<_, i64>(0))?;
        }
        // count rows: oid + number
        total += c.query_row("SELECT COUNT(*) * 56 FROM cache_commit_count", [], |r| r.get::<_, i64>(0))?;
        Ok(total.max(0) as u64)
    })
    .map(|db| db + rs_core::imgcache::size(&st.shelf))
}

/// Bytes under `root` (iterative, no symlink following); `None` past `cap` entries or on an unreadable root.
pub fn dir_bytes(root: &Path, cap: u32) -> Option<u64> {
    let root = rs_core::paths::long(root);
    if !root.exists() {
        return Some(0);
    }
    let mut total = 0u64;
    let mut seen = 0u32;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(items) = std::fs::read_dir(&dir) else { continue };
        for it in items.flatten() {
            seen += 1;
            if seen > cap {
                return None;
            }
            let Ok(ft) = it.file_type() else { continue };
            if ft.is_dir() {
                stack.push(it.path());
            } else if ft.is_file() {
                total += it.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    Some(total)
}

pub fn app_info_impl(st: &AppState) -> Result<AppInfo> {
    let s = &st.shelf;
    let repo_count: i64 = s.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM repos", [], |r| r.get(0))?))?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: s.dirs.data.display().to_string(),
        archive_dir: s.dirs.archive.display().to_string(),
        cache_bytes: cache_bytes(st)?,
        repo_count: repo_count.max(0) as u32,
        archive_bytes: dir_bytes(&s.dirs.archive, ARCHIVE_SCAN_CAP),
    })
}

pub fn settings_get_impl(st: &AppState) -> Result<Settings> {
    settings::load(&st.shelf.db)
}

pub fn settings_set_impl(st: &AppState, s: Settings) -> Result<Settings> {
    if st.shelf.db.read_only() {
        return Err(Error::Refused("The library was written by a newer RepoShelf and is read-only here.".into()));
    }
    settings::save(&st.shelf.db, &s)?;
    settings::load(&st.shelf.db)
}

/// S10 "Clear caches": every cache table (counts, last commits, languages, GitHub notes) and the README image
/// cache (v0.2.0). Repos, settings, fetched About values (`gh_repo_info`) and downloaded releases stay.
pub fn cache_clear_impl(st: &AppState) -> Result<AppInfo> {
    st.shelf.db.tx(|tx| {
        for t in CACHE_TABLES.iter().chain(["cache_commit_count"].iter()) {
            tx.execute(&format!("DELETE FROM {t}"), [])?;
        }
        Ok(())
    })?;
    rs_core::imgcache::clear(&st.shelf)?;
    app_info_impl(st)
}

pub fn request_cancel_impl(st: &AppState, request_id: &str) -> Result<bool> {
    st.requests.cancel(request_id)
}

/// NFR-S3: http/https without userinfo only (re-validated here; the UI confirmed first).
pub fn open_external_impl(st: &AppState, url: &str) -> Result<()> {
    let url = rs_core::validate::external_url(url)?;
    st.opener.open_url(&url).map_err(|e| Error::Refused(format!("Could not open the browser: {e}")))
}

/// S10 "Open" next to the data / archive folder (D-76): a keyword, never a path from the webview.
pub fn app_open_dir_impl(st: &AppState, which: &str) -> Result<()> {
    let dir = match which {
        "data" => &st.shelf.dirs.data,
        "archive" => &st.shelf.dirs.archive,
        _ => return Err(Error::InvalidInput("unknown folder: expected \"data\" or \"archive\"".into())),
    };
    std::fs::create_dir_all(rs_core::paths::long(dir))?;
    st.opener.open_folder(dir).map_err(|e| Error::Refused(format!("Could not open the folder: {e}")))
}

#[tauri::command]
pub async fn app_open_dir(state: St<'_>, which: String) -> CmdResult<()> {
    blocking(&state, move |st| app_open_dir_impl(st, &which)).await
}

#[tauri::command]
pub async fn app_info(state: St<'_>) -> CmdResult<AppInfo> {
    blocking(&state, app_info_impl).await
}

#[tauri::command]
pub async fn settings_get(state: St<'_>) -> CmdResult<Settings> {
    blocking(&state, settings_get_impl).await
}

#[tauri::command]
pub async fn settings_set(state: St<'_>, settings: Settings) -> CmdResult<Settings> {
    blocking(&state, move |st| settings_set_impl(st, settings)).await
}

#[tauri::command]
pub async fn cache_clear(state: St<'_>) -> CmdResult<AppInfo> {
    blocking(&state, cache_clear_impl).await
}

#[tauri::command]
pub async fn request_cancel(state: St<'_>, request_id: String) -> CmdResult<bool> {
    blocking(&state, move |st| request_cancel_impl(st, &request_id)).await
}

#[tauri::command]
pub async fn open_external(state: St<'_>, url: String) -> CmdResult<()> {
    blocking(&state, move |st| open_external_impl(st, &url)).await
}
