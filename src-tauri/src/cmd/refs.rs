//! Branch and tag commands (M4.8): `branches_list`, `branches_ahead_behind` (cancellable), `tags_list`.
//! FR-29, FR-30.

use super::{St, blocking, open, rev_arg};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::types::{AheadBehind, BranchRow, TagRow};

/// Ahead/behind counts stop at this many commits per side (`capped: true`).
pub const AHEAD_BEHIND_CAP: u32 = 10_000;
pub const MAX_NAMES: usize = 1_000;

pub fn branches_list_impl(st: &AppState, repo_id: i64) -> Result<Vec<BranchRow>> {
    let (_, repo) = open(st, repo_id)?;
    rs_core::refs::branches(&repo)
}

/// Each name relative to the default branch, as `[name, AheadBehind]` pairs.
pub fn branches_ahead_behind_impl(st: &AppState, repo_id: i64, names: &[String], request_id: &str) -> Result<Vec<(String, AheadBehind)>> {
    if names.len() > MAX_NAMES {
        return Err(Error::InvalidInput(format!("at most {MAX_NAMES} branch names per call")));
    }
    for n in names {
        rev_arg(n)?;
    }
    let req = st.requests.register(request_id)?;
    let (_, repo) = open(st, repo_id)?;
    let base = rs_core::refs::head_branch(&repo).ok_or_else(|| Error::Refused("This repository has no default branch.".into()))?;
    let _slot = st.compute.acquire(req.token())?;
    rs_core::refs::ahead_behind(&repo, &base, names, AHEAD_BEHIND_CAP, req.token())
}

pub fn tags_list_impl(st: &AppState, repo_id: i64) -> Result<Vec<TagRow>> {
    let (_, repo) = open(st, repo_id)?;
    rs_core::refs::tags(&repo)
}

#[tauri::command]
pub async fn branches_list(state: St<'_>, repo_id: i64) -> CmdResult<Vec<BranchRow>> {
    blocking(&state, move |st| branches_list_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn branches_ahead_behind(
    state: St<'_>,
    repo_id: i64,
    names: Vec<String>,
    request_id: String,
) -> CmdResult<Vec<(String, AheadBehind)>> {
    blocking(&state, move |st| branches_ahead_behind_impl(st, repo_id, &names, &request_id)).await
}

#[tauri::command]
pub async fn tags_list(state: St<'_>, repo_id: i64) -> CmdResult<Vec<TagRow>> {
    blocking(&state, move |st| tags_list_impl(st, repo_id)).await
}
