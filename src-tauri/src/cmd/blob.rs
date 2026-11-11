//! File commands (M4.7, part 2): `blob_meta`, `blob_text`, `blob_bytes` (raw `ArrayBuffer`, ≤ 5 MB),
//! `file_last_commit`. FR-21..25, NFR-P4, NFR-S7, D-11.

use super::{St, blocking, open, path_arg, resolve, rev_arg};
use crate::ipc_error::{CmdResult, IpcError};
use crate::state::AppState;
use rs_core::blob::DISPLAY_MAX;
use rs_core::error::{Error, Result};
use rs_core::types::{BlobMeta, BlobText, CommitSummary};

fn file_arg(path: &str) -> Result<rs_core::validate::GitPath> {
    let gp = path_arg(path)?;
    if gp.is_root() {
        return Err(Error::InvalidPath("a file path is required".into()));
    }
    Ok(gp)
}

pub fn blob_meta_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<BlobMeta> {
    let spec = rev_arg(rev)?;
    let gp = file_arg(path)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::blob::meta(&repo, &r, &gp)
}

/// Text (lossy, BOM stripped); `text: null` above 5 MB or for non-text kinds (NFR-P4).
pub fn blob_text_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<BlobText> {
    let spec = rev_arg(rev)?;
    let gp = file_arg(path)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::blob::text(&repo, &r, &gp)
}

/// Raw bytes for images (`blob:` URLs, D-11) and the raw view; `Refused` above 5 MB. The image MIME type comes
/// from `blob_meta` (`BlobKind::Image.mime`, extension allow-list incl. `image/svg+xml`, NFR-S7), not from here.
pub fn blob_bytes_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<Vec<u8>> {
    let spec = rev_arg(rev)?;
    let gp = file_arg(path)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::blob::bytes(&repo, &r, &gp, DISPLAY_MAX)
}

/// File view header: the last commit that changed `path` at `rev` (via the parent dir's cache). Cancellable.
pub fn file_last_commit_impl(st: &AppState, repo_id: i64, rev: &str, path: &str, request_id: &str) -> Result<Option<CommitSummary>> {
    let spec = rev_arg(rev)?;
    let gp = file_arg(path)?;
    let req = st.requests.register(request_id)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    let _slot = st.compute.acquire(req.token())?;
    rs_core::commits::file_last_commit(&st.shelf, repo_id, &repo, &r, &gp, req.token())
}

#[tauri::command]
pub async fn blob_meta(state: St<'_>, repo_id: i64, rev: String, path: String) -> CmdResult<BlobMeta> {
    blocking(&state, move |st| blob_meta_impl(st, repo_id, &rev, &path)).await
}

#[tauri::command]
pub async fn blob_text(state: St<'_>, repo_id: i64, rev: String, path: String) -> CmdResult<BlobText> {
    blocking(&state, move |st| blob_text_impl(st, repo_id, &rev, &path)).await
}

#[tauri::command]
pub async fn blob_bytes(state: St<'_>, repo_id: i64, rev: String, path: String) -> std::result::Result<tauri::ipc::Response, IpcError> {
    let bytes = blocking(&state, move |st| blob_bytes_impl(st, repo_id, &rev, &path)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn file_last_commit(
    state: St<'_>,
    repo_id: i64,
    rev: String,
    path: String,
    request_id: String,
) -> CmdResult<Option<CommitSummary>> {
    blocking(&state, move |st| file_last_commit_impl(st, repo_id, &rev, &path, &request_id)).await
}
