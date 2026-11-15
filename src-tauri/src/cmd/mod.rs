//! IPC commands (ARCH §5.1). Each command is a thin `#[tauri::command] async fn` that moves a plain
//! `*_impl(&AppState, …)` function onto a blocking thread; tests call the `_impl` functions directly (no runtime,
//! no window). Arguments are validated per §5.2 before rs-core touches anything.

pub mod app;
pub mod blob;
pub mod commits;
pub mod dialogs;
pub mod explore;
pub mod images;
pub mod jobs;
pub mod library;
pub mod refs;
pub mod releases;
pub mod tree;

use crate::ipc_error::{CmdResult, IpcError};
use crate::state::AppState;
use git2::Repository;
use rs_core::error::{Error, Result};
use rs_core::types::{RepoSummary, ResolvedRev};
use rs_core::validate::{self, GitPath};
use std::sync::Arc;

pub type St<'a> = tauri::State<'a, Arc<AppState>>;

/// Runs `f` on the blocking pool and maps its error to `IpcError`.
pub async fn blocking<T, F>(st: &St<'_>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> Result<T> + Send + 'static,
{
    let st: Arc<AppState> = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(&st))
        .await
        .map_err(|e| IpcError::new("internal", format!("Internal error: {e}")))?
        .map_err(IpcError::from)
}

/// Upper bound for `skip` (§5.2).
pub const MAX_SKIP: u32 = 10_000_000;
/// Upper bound for a diff file index.
pub const MAX_FILE_INDEX: u32 = 1_000_000;

/// DB lookup → open (the path never comes from the UI).
pub fn open(st: &AppState, repo_id: i64) -> Result<(RepoSummary, Repository)> {
    rs_core::repo::open(&st.shelf, repo_id)
}

/// `rev` syntax check (§5.2) — call before opening anything.
pub fn rev_arg(rev: &str) -> Result<validate::RevSpec> {
    validate::rev(rev)
}

/// `path` check (§5.2) — call before opening anything.
pub fn path_arg(path: &str) -> Result<GitPath> {
    validate::git_path(path)
}

pub fn resolve(repo: &Repository, rev: &validate::RevSpec) -> Result<ResolvedRev> {
    rs_core::refs::resolve(repo, rev)
}

pub fn oid_of(hex: &str) -> Result<git2::Oid> {
    git2::Oid::from_str(hex).map_err(|e| Error::InvalidRev(e.message().to_string()))
}
