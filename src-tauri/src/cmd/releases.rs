//! Release commands (M4.8, v0.2.0 M11): `releases_list` (cached GitHub notes + assets + download state merged
//! when present, no network), `releases_fetch_github` (network; `unsupported` when the `ghreleases` feature is
//! off), `release_reveal` (Explorer on a downloaded asset). FR-31, F4, D-22, D-49, D-104.

use super::{St, blocking};
use crate::ipc_error::{CmdResult, IpcError};
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::types::Release;

pub const GITHUB_API_HOST: &str = "api.github.com";

pub fn releases_list_impl(st: &AppState, repo_id: i64) -> Result<Vec<Release>> {
    rs_core::ghreleases::releases_with_cached(&st.shelf, repo_id)
}

/// One conditional GET to the GitHub API (github.com origins only), cached, merged.
pub fn releases_fetch_github_impl(st: &AppState, repo_id: i64) -> Result<Vec<Release>> {
    #[cfg(feature = "ghreleases")]
    {
        rs_core::ghreleases::fetch_github_notes(&st.shelf, repo_id)
    }
    #[cfg(not(feature = "ghreleases"))]
    {
        rs_core::repo::summary(&st.shelf, repo_id)?;
        Err(rs_core::Error::Unsupported("GitHub release notes are not included in this build.".into()))
    }
}

/// "Show in folder" for a downloaded asset: the path is computed in Rust from (repo, tag, name) and must exist.
pub fn release_reveal_impl(st: &AppState, repo_id: i64, tag: &str, asset: &str) -> Result<()> {
    let origin = rs_core::ghreleases::origin_of(&st.shelf, repo_id)?;
    let file = rs_core::assets::store_file(&st.shelf, origin.as_deref(), tag, asset)?;
    if !rs_core::paths::long(&file).is_file() {
        return Err(Error::not_found("downloaded file"));
    }
    st.opener.reveal(&file).map_err(|e| Error::Refused(format!("Could not open the folder: {e}")))
}

#[tauri::command]
pub async fn releases_list(state: St<'_>, repo_id: i64) -> CmdResult<Vec<Release>> {
    blocking(&state, move |st| releases_list_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn releases_fetch_github(state: St<'_>, repo_id: i64) -> CmdResult<Vec<Release>> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || releases_fetch_github_impl(&st, repo_id))
        .await
        .map_err(|e| IpcError::new("internal", format!("Internal error: {e}")))?
        .map_err(|e| IpcError::from_core(&e, Some(GITHUB_API_HOST)))
}

#[tauri::command]
pub async fn release_reveal(state: St<'_>, repo_id: i64, tag: String, asset: String) -> CmdResult<()> {
    blocking(&state, move |st| release_reveal_impl(st, repo_id, &tag, &asset)).await
}
