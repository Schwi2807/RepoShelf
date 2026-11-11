//! Library commands (M4.5): list, refresh, slug lookup, metadata, detail, open folder. FR-1, FR-4, FR-12, FR-18, FR-20.

use super::{St, blocking};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::library;
use rs_core::types::{RepoDetail, RepoSummary, Source};
use rs_core::validate::MetadataInput;
use std::path::PathBuf;

const MAX_SLUG_PART: usize = 200;

pub fn library_list_impl(st: &AppState) -> Result<Vec<RepoSummary>> {
    library::list(&st.shelf)
}

/// Re-checks every repo (exists? tip changed? → summary fields, status). Cancellable via `request_cancel`.
pub fn library_refresh_impl(st: &AppState, request_id: &str) -> Result<Vec<RepoSummary>> {
    let req = st.requests.register(request_id)?;
    let _slot = st.compute.acquire(req.token())?;
    library::refresh_summaries(&st.shelf, req.token())
}

pub fn repo_by_slug_impl(st: &AppState, owner: &str, name: &str) -> Result<RepoSummary> {
    for p in [owner, name] {
        if p.is_empty() || p.len() > MAX_SLUG_PART || p.chars().any(char::is_control) {
            return Err(Error::InvalidInput("invalid repository name".into()));
        }
    }
    library::by_slug(&st.shelf, owner, name)
}

pub fn repo_set_metadata_impl(
    st: &AppState,
    repo_id: i64,
    description: Option<String>,
    homepage: Option<String>,
    topics: Vec<String>,
) -> Result<RepoSummary> {
    library::set_metadata(&st.shelf, repo_id, MetadataInput { description, homepage, topics })
}

/// S2 header + About; marks the repo as opened (Recent, D-28).
pub fn repo_detail_impl(st: &AppState, repo_id: i64) -> Result<RepoDetail> {
    library::touch_opened(&st.shelf, repo_id)?;
    #[allow(unused_mut)]
    let mut d = rs_core::repo::detail(&st.shelf, repo_id)?;
    // e2e build with the file:// seam (D-14, D-20): a clone of a file:// upstream offers Fetch like an https one,
    // so the real UI can drive clone → fetch → fast-forward (found by the M8 e2e, D-90).
    #[cfg(feature = "e2e")]
    if st.test_file_clone && d.summary.source == Source::Cloned && d.summary.origin_url.as_deref().is_some_and(|u| u.starts_with("file:")) {
        d.fetchable = true;
    }
    Ok(d)
}

/// Bare repos: select `<name>.git` in Explorer; work trees: open the folder.
pub fn repo_open_folder_impl(st: &AppState, repo_id: i64) -> Result<()> {
    let sum = rs_core::repo::summary(&st.shelf, repo_id)?;
    let path = PathBuf::from(&sum.path);
    let repo = rs_core::repo::open_path(&path)?;
    let bare = repo.is_bare() || sum.source == Source::Cloned;
    drop(repo);
    let r = if bare { st.opener.reveal(&path) } else { st.opener.open_folder(&path) };
    r.map_err(|e| Error::Refused(format!("Could not open the folder: {e}")))
}

#[tauri::command]
pub async fn library_list(state: St<'_>) -> CmdResult<Vec<RepoSummary>> {
    blocking(&state, library_list_impl).await
}

#[tauri::command]
pub async fn library_refresh(state: St<'_>, request_id: String) -> CmdResult<Vec<RepoSummary>> {
    blocking(&state, move |st| library_refresh_impl(st, &request_id)).await
}

#[tauri::command]
pub async fn repo_by_slug(state: St<'_>, owner: String, name: String) -> CmdResult<RepoSummary> {
    blocking(&state, move |st| repo_by_slug_impl(st, &owner, &name)).await
}

#[tauri::command]
pub async fn repo_set_metadata(
    state: St<'_>,
    repo_id: i64,
    description: Option<String>,
    homepage: Option<String>,
    topics: Vec<String>,
) -> CmdResult<RepoSummary> {
    blocking(&state, move |st| repo_set_metadata_impl(st, repo_id, description, homepage, topics)).await
}

#[tauri::command]
pub async fn repo_detail(state: St<'_>, repo_id: i64) -> CmdResult<RepoDetail> {
    blocking(&state, move |st| repo_detail_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn repo_open_folder(state: St<'_>, repo_id: i64) -> CmdResult<()> {
    blocking(&state, move |st| repo_open_folder_impl(st, repo_id)).await
}
