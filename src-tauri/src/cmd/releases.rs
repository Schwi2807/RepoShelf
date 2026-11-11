//! Release commands (M4.8): `releases_list` (cached GitHub notes merged when present), `releases_fetch_github`
//! (feature `ghreleases`, network; `unsupported` when the feature is off). FR-31, D-22, D-49.

use super::{St, blocking};
use crate::ipc_error::{CmdResult, IpcError};
use crate::state::AppState;
use rs_core::error::Result;
use rs_core::types::Release;

pub const GITHUB_API_HOST: &str = "api.github.com";

pub fn releases_list_impl(st: &AppState, repo_id: i64) -> Result<Vec<Release>> {
    #[cfg(feature = "ghreleases")]
    {
        rs_core::ghreleases::releases_with_cached(&st.shelf, repo_id)
    }
    #[cfg(not(feature = "ghreleases"))]
    {
        let (_, repo) = super::open(st, repo_id)?;
        rs_core::releases::list(&repo)
    }
}

/// One GET to the GitHub API (github.com origins only), cached, merged.
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
