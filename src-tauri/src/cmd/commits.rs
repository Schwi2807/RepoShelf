//! Commit commands (M4.8): `commits_page` (D-9 pinned tip + offset, limit fixed at 35), `commit_detail`,
//! `commit_file_diff`. FR-26..28, FR-31 compare.

use super::{MAX_FILE_INDEX, MAX_SKIP, St, blocking, oid_of, open, path_arg, resolve, rev_arg};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::commits::PageQuery;
use rs_core::diff::DiffLimits;
use rs_core::error::{Error, Result};
use rs_core::jobs::CancelToken;
use rs_core::types::{CommitDetail, CommitsPage, FileDiff};
use rs_core::validate;

/// One page of 35. `tip` (40 hex) pins the walk across pages (the first page returns it); `since` hides
/// commits reachable from that rev ("since previous tag"); `path` filters (empty = whole tree); `skip` is the
/// previous page's returned `skip` (D-37).
pub fn commits_page_impl(
    st: &AppState,
    repo_id: i64,
    rev: &str,
    path: Option<&str>,
    since: Option<&str>,
    skip: u32,
    tip: Option<&str>,
) -> Result<CommitsPage> {
    let spec = rev_arg(rev)?;
    let gp = match path {
        Some(p) => Some(path_arg(p)?).filter(|g| !g.is_root()),
        None => None,
    };
    let since = since.map(validate::rev).transpose()?;
    let tip = tip.map(validate::oid).transpose()?;
    if skip > MAX_SKIP {
        return Err(Error::InvalidInput(format!("skip must be at most {MAX_SKIP}")));
    }
    let (_, repo) = open(st, repo_id)?;
    let tip = match tip {
        Some(t) => {
            repo.find_commit(t).map_err(|_| Error::not_found("commit"))?;
            t
        }
        None => oid_of(&resolve(&repo, &spec)?.commit)?,
    };
    let mut q = PageQuery::new(tip);
    q.path = gp;
    q.skip = skip;
    q.hide = match since {
        Some(s) => Some(oid_of(&resolve(&repo, &s)?.commit)?),
        None => None,
    };
    rs_core::commits::page(&repo, &q, &CancelToken::new())
}

pub fn commit_detail_impl(st: &AppState, repo_id: i64, oid: &str) -> Result<CommitDetail> {
    let oid = validate::oid(oid)?;
    let (_, repo) = open(st, repo_id)?;
    rs_core::diff::commit_detail(&repo, oid, &DiffLimits::default())
}

/// "Load diff" for one file of a commit (`force` = past the per-file line cap).
pub fn commit_file_diff_impl(st: &AppState, repo_id: i64, oid: &str, index: u32, force: bool) -> Result<FileDiff> {
    let oid = validate::oid(oid)?;
    if index > MAX_FILE_INDEX {
        return Err(Error::InvalidInput("file index out of range".into()));
    }
    let (_, repo) = open(st, repo_id)?;
    rs_core::diff::file_diff(&repo, oid, index, force)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn commits_page(
    state: St<'_>,
    repo_id: i64,
    rev: String,
    path: Option<String>,
    since: Option<String>,
    skip: u32,
    tip: Option<String>,
) -> CmdResult<CommitsPage> {
    blocking(&state, move |st| commits_page_impl(st, repo_id, &rev, path.as_deref(), since.as_deref(), skip, tip.as_deref())).await
}

#[tauri::command]
pub async fn commit_detail(state: St<'_>, repo_id: i64, oid: String) -> CmdResult<CommitDetail> {
    blocking(&state, move |st| commit_detail_impl(st, repo_id, &oid)).await
}

#[tauri::command]
pub async fn commit_file_diff(state: St<'_>, repo_id: i64, oid: String, index: u32, force: bool) -> CmdResult<FileDiff> {
    blocking(&state, move |st| commit_file_diff_impl(st, repo_id, &oid, index, force)).await
}
