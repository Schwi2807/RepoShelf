//! Browse commands (M4.7, part 1): refs, rev resolution, tree listing, last commits, latest commit + count,
//! README / markdown, languages. FR-13..19, NFR-P3, NFR-S4.

use super::{St, blocking, oid_of, open, path_arg, resolve, rev_arg};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::error::Result;
use rs_core::types::{LastCommits, Languages, LatestCommit, RefsList, Rendered, ResolvedRev, TreeListing};

pub fn refs_list_impl(st: &AppState, repo_id: i64) -> Result<RefsList> {
    let (_, repo) = open(st, repo_id)?;
    rs_core::refs::list(&repo)
}

pub fn rev_resolve_impl(st: &AppState, repo_id: i64, rev: &str) -> Result<ResolvedRev> {
    let spec = rev_arg(rev)?;
    let (_, repo) = open(st, repo_id)?;
    resolve(&repo, &spec)
}

pub fn tree_list_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<TreeListing> {
    let spec = rev_arg(rev)?;
    let gp = path_arg(path)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::tree::list_dir(&repo, &r, &gp)
}

/// FR-16: last commit per entry of the directory, cached per (tip, dir); cancellable, compute gate (2).
pub fn tree_last_commits_impl(st: &AppState, repo_id: i64, rev: &str, path: &str, request_id: &str) -> Result<LastCommits> {
    let spec = rev_arg(rev)?;
    let gp = path_arg(path)?;
    let req = st.requests.register(request_id)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    let _slot = st.compute.acquire(req.token())?;
    rs_core::lastcommit::for_dir(&st.shelf, repo_id, &repo, oid_of(&r.commit)?, &gp, rs_core::lastcommit::CAP, req.token())
}

pub fn commit_latest_impl(st: &AppState, repo_id: i64, rev: &str) -> Result<LatestCommit> {
    let spec = rev_arg(rev)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::commits::latest(&st.shelf, repo_id, &repo, &r)
}

/// FR-14: commits reachable from `rev` (cached per tip); cancellable, compute gate.
pub fn commit_count_impl(st: &AppState, repo_id: i64, rev: &str, request_id: &str) -> Result<u64> {
    let spec = rev_arg(rev)?;
    let req = st.requests.register(request_id)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    let _slot = st.compute.acquire(req.token())?;
    rs_core::commits::count(&st.shelf, repo_id, &repo, oid_of(&r.commit)?, req.token())
}

/// FR-17: README of the directory `path` (root fallbacks), sanitised; `None` if there is none.
pub fn readme_render_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<Option<Rendered>> {
    let spec = rev_arg(rev)?;
    let gp = path_arg(path)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::readme::render_readme(&repo, &r, &gp)
}

/// S4 preview of a markdown file, sanitised.
pub fn markdown_render_file_impl(st: &AppState, repo_id: i64, rev: &str, path: &str) -> Result<Rendered> {
    let spec = rev_arg(rev)?;
    let gp = path_arg(path)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    rs_core::readme::render_file(&repo, &r, &gp)
}

/// FR-19: language bar for the tree at `rev` (cached per root tree); cancellable, compute gate.
pub fn languages_impl(st: &AppState, repo_id: i64, rev: &str, request_id: &str) -> Result<Languages> {
    let spec = rev_arg(rev)?;
    let req = st.requests.register(request_id)?;
    let (_, repo) = open(st, repo_id)?;
    let r = resolve(&repo, &spec)?;
    let _slot = st.compute.acquire(req.token())?;
    rs_core::langs::for_tree(&st.shelf, repo_id, &repo, oid_of(&r.tree)?, req.token())
}

#[tauri::command]
pub async fn refs_list(state: St<'_>, repo_id: i64) -> CmdResult<RefsList> {
    blocking(&state, move |st| refs_list_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn rev_resolve(state: St<'_>, repo_id: i64, rev: String) -> CmdResult<ResolvedRev> {
    blocking(&state, move |st| rev_resolve_impl(st, repo_id, &rev)).await
}

#[tauri::command]
pub async fn tree_list(state: St<'_>, repo_id: i64, rev: String, path: String) -> CmdResult<TreeListing> {
    blocking(&state, move |st| tree_list_impl(st, repo_id, &rev, &path)).await
}

#[tauri::command]
pub async fn tree_last_commits(state: St<'_>, repo_id: i64, rev: String, path: String, request_id: String) -> CmdResult<LastCommits> {
    blocking(&state, move |st| tree_last_commits_impl(st, repo_id, &rev, &path, &request_id)).await
}

#[tauri::command]
pub async fn commit_latest(state: St<'_>, repo_id: i64, rev: String) -> CmdResult<LatestCommit> {
    blocking(&state, move |st| commit_latest_impl(st, repo_id, &rev)).await
}

#[tauri::command]
pub async fn commit_count(state: St<'_>, repo_id: i64, rev: String, request_id: String) -> CmdResult<u64> {
    blocking(&state, move |st| commit_count_impl(st, repo_id, &rev, &request_id)).await
}

#[tauri::command]
pub async fn readme_render(state: St<'_>, repo_id: i64, rev: String, path: String) -> CmdResult<Option<Rendered>> {
    blocking(&state, move |st| readme_render_impl(st, repo_id, &rev, &path)).await
}

#[tauri::command]
pub async fn markdown_render_file(state: St<'_>, repo_id: i64, rev: String, path: String) -> CmdResult<Rendered> {
    blocking(&state, move |st| markdown_render_file_impl(st, repo_id, &rev, &path)).await
}

#[tauri::command]
pub async fn languages(state: St<'_>, repo_id: i64, rev: String, request_id: String) -> CmdResult<Languages> {
    blocking(&state, move |st| languages_impl(st, repo_id, &rev, &request_id)).await
}
