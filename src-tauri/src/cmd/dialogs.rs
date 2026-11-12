//! Commands that open a native dialog or touch the Recycle Bin (M4.6): import (D-18 token), locate, remove
//! (D-30 Trasher, D-50 archive-only trash), save-as. FR-4, FR-5, FR-23, FR-36, NFR-S4, D-10. Filesystem paths
//! never arrive from the UI.

use super::{St, blocking, open, path_arg, resolve, rev_arg};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::library;
use rs_core::types::{ImportPreview, RepoSummary, Source};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// "Save a copy" limit (the in-app view stops at 5 MB; saving is allowed up to this).
pub const SAVE_MAX: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveResult {
    /// `true` when the managed clone folder went to the Recycle Bin.
    pub trashed: bool,
}

/// D-18: folder dialog → preview + one-shot token. `None` when the dialog was cancelled.
pub fn import_pick_impl(st: &AppState) -> Result<Option<ImportPreview>> {
    let Some(dir) = st.picker.pick_folder("Import a repository folder") else { return Ok(None) };
    import_preview_for(st, &dir).map(Some)
}

/// The part of `import_pick` after the dialog (tests inject the folder here or through the fake picker).
pub fn import_preview_for(st: &AppState, dir: &Path) -> Result<ImportPreview> {
    let mut p = library::preview_import(&st.shelf, dir)?;
    p.token = st.issue_import_token(PathBuf::from(&p.path));
    Ok(p)
}

/// Imports, then (github.com origins, settings on) queues the GitHub info / auto-download follow-up job —
/// never waits for the network (D-103). `auto_download`: the dialog's checkbox (`None` = setting).
pub fn import_confirm_impl(st: &AppState, token: &str, auto_download: Option<bool>) -> Result<RepoSummary> {
    if token.is_empty() || token.len() > 64 {
        return Err(Error::InvalidInput("Unknown folder selection. Choose the folder again.".into()));
    }
    let path = st.take_import_token(token)?;
    let sum = library::import(&st.shelf, &path)?;
    super::jobs::queue_github_sync(st, sum.id, auto_download);
    Ok(sum)
}

/// FR-4 "Locate…": folder dialog → relocate. `None` when the dialog was cancelled.
pub fn repo_locate_impl(st: &AppState, repo_id: i64) -> Result<Option<RepoSummary>> {
    rs_core::repo::summary(&st.shelf, repo_id)?;
    let Some(dir) = st.picker.pick_folder("Locate the repository folder") else { return Ok(None) };
    library::relocate(&st.shelf, repo_id, &dir).map(Some)
}

fn fold(p: &Path) -> PathBuf {
    PathBuf::from(p.to_string_lossy().to_lowercase())
}

/// D-50: only a folder inside the archive dir is ever trashed (a relocated "cloned" entry may point anywhere).
pub fn inside_archive(archive: &Path, path: &Path) -> bool {
    let (Ok(a), Ok(p)) = (rs_core::paths::canonical(archive), rs_core::paths::canonical(path)) else { return false };
    let (a, p) = (fold(&a), fold(&p));
    p != a && p.starts_with(&a)
}

/// FR-36: cloned → folder to the Recycle Bin first (entry kept if that fails), then the entry is removed;
/// imported → only the entry (nothing on disk).
pub fn repo_remove_impl(st: &AppState, repo_id: i64) -> Result<RemoveResult> {
    let sum = rs_core::repo::summary(&st.shelf, repo_id)?;
    let path = PathBuf::from(&sum.path);
    let mut trashed = false;
    if sum.source == Source::Cloned && path.exists() && inside_archive(&st.shelf.dirs.archive, &path) {
        st.trasher
            .delete(&path)
            .map_err(|e| Error::Refused(format!("Could not move the folder to the Recycle Bin: {e}")))?;
        trashed = true;
    }
    library::remove(&st.shelf, repo_id)?;
    Ok(RemoveResult { trashed })
}

/// FR-23 "Save a copy": save dialog → write the blob. `None` when cancelled; else the written path.
pub fn blob_save_as_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<Option<String>> {
    let spec = rev_arg(rev)?;
    let gp = path_arg(path)?;
    if gp.is_root() {
        return Err(Error::InvalidPath("a file path is required".into()));
    }
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    // Fail on a missing path / submodule / oversize file before showing the dialog.
    let meta = rs_core::blob::meta(&repo, &r, &gp)?;
    if meta.size > SAVE_MAX {
        return Err(Error::Refused("File is too large to save from here.".into()));
    }
    let name = gp.as_str().rsplit('/').next().unwrap_or("file").to_string();
    let Some(target) = st.picker.save_file("Save a copy", &name) else { return Ok(None) };
    let bytes = rs_core::blob::bytes(&repo, &r, &gp, SAVE_MAX)?;
    drop(repo);
    std::fs::write(rs_core::paths::long(&target), bytes)?;
    Ok(Some(target.display().to_string()))
}

#[tauri::command]
pub async fn import_pick(state: St<'_>) -> CmdResult<Option<ImportPreview>> {
    blocking(&state, import_pick_impl).await
}

#[tauri::command]
pub async fn import_confirm(state: St<'_>, token: String, auto_download: Option<bool>) -> CmdResult<RepoSummary> {
    blocking(&state, move |st| import_confirm_impl(st, &token, auto_download)).await
}

#[tauri::command]
pub async fn repo_locate(state: St<'_>, repo_id: i64) -> CmdResult<Option<RepoSummary>> {
    blocking(&state, move |st| repo_locate_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn repo_remove(state: St<'_>, repo_id: i64) -> CmdResult<RemoveResult> {
    blocking(&state, move |st| repo_remove_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn blob_save_as(state: St<'_>, repo_id: i64, rev: String, path: String) -> CmdResult<Option<String>> {
    blocking(&state, move |st| blob_save_as_impl(st, repo_id, &rev, &path)).await
}
