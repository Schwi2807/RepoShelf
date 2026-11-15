//! Explore commands (v0.3.0 M10a, D-117..D-125): browse GitHub repositories online through `api.github.com`,
//! read-only. Every command is addressed by `owner` + `name` (never a filesystem path, never a repoId), validated
//! with `validate::github_owner` / `github_repo`; `rev` / `path` / `oid` go through the same checks as the local
//! commands. The HTTP, the cache, the rate limit and the token live in `rs_core::explore`; the commands here only
//! validate, call and map errors (`api.github.com` is the host named in network messages).
//!
//! With the setting `exploreEnabled` off every command except `explore_status`, the token commands and
//! `explore_cache_clear` refuses before any request.

use super::{MAX_FILE_INDEX, MAX_SKIP, St, path_arg, rev_arg};
use crate::ipc_error::{CmdResult, IpcError};
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::explore::cred::Token;
use rs_core::explore::github::{self, GithubSource};
use rs_core::explore::source::RepoSource;
use rs_core::jobs::{JobId, JobKind, JobState};
use rs_core::types::{
    BlobMeta, BlobText, CommitDetail, CommitsPage, ExploreRefs, ExploreRepo, ExploreSearch, ExploreStatus, FileDiff, Languages, LatestCommit, Release,
    Rendered, ResolvedRev, TreeListing,
};
use rs_core::validate;

pub const GITHUB_API_HOST: &str = "api.github.com";
/// `explore_blob_bytes` hands over at most this much (GitHub's contents API stops at 1 MB anyway).
pub const BLOB_BYTES_MAX: u64 = 5 * 1024 * 1024;

/// Runs `f` on the blocking pool; errors are mapped with `api.github.com` as the host of network messages.
async fn blocking_gh<T, F>(st: &St<'_>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> Result<T> + Send + 'static,
{
    let st = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(&st))
        .await
        .map_err(|e| IpcError::new("internal", format!("Internal error: {e}")))?
        .map_err(|e| IpcError::from_core(&e, Some(GITHUB_API_HOST)))
}

fn src<'a>(st: &'a AppState, owner: &str, name: &str) -> Result<GithubSource<'a>> {
    GithubSource::new(&st.shelf, owner, name)
}

/// `rev` → commit (one saved answer per name, none for a sha already known).
fn resolve(s: &GithubSource, rev: &str) -> Result<ResolvedRev> {
    s.resolve(&rev_arg(rev)?)
}

// -- status / token / cache --------------------------------------------------------------------------------------

pub fn explore_status_impl(st: &AppState) -> Result<ExploreStatus> {
    st.shelf.explore.status(&st.shelf)
}

/// Saves the token in Windows Credential Manager (never in the DB), then checks it with one free request. A token
/// GitHub rejects (401) is removed again and reported; the token itself is never repeated.
pub fn explore_token_set_impl(st: &AppState, token: &str) -> Result<ExploreStatus> {
    let t = Token::parse(token)?;
    st.shelf.explore.set_token(&st.shelf, &t)?;
    if let Err(e) = rs_core::explore::check_token(&st.shelf) {
        st.shelf.explore.remove_token(&st.shelf)?;
        return Err(e);
    }
    explore_status_impl(st)
}

pub fn explore_token_remove_impl(st: &AppState) -> Result<ExploreStatus> {
    st.shelf.explore.remove_token(&st.shelf)?;
    explore_status_impl(st)
}

/// "Clear Explore cache" (Settings): the saved GitHub answers only.
pub fn explore_cache_clear_impl(st: &AppState) -> Result<ExploreStatus> {
    rs_core::explore::cache::clear(&st.shelf.db)?;
    explore_status_impl(st)
}

// -- browse --------------------------------------------------------------------------------------------------------

pub fn explore_repo_impl(st: &AppState, owner: &str, name: &str) -> Result<ExploreRepo> {
    src(st, owner, name)?.repo()
}

pub fn explore_refs_impl(st: &AppState, owner: &str, name: &str) -> Result<ExploreRefs> {
    src(st, owner, name)?.refs_ex()
}

pub fn explore_languages_impl(st: &AppState, owner: &str, name: &str) -> Result<Languages> {
    src(st, owner, name)?.languages()
}

pub fn explore_resolve_impl(st: &AppState, owner: &str, name: &str, rev: &str) -> Result<ResolvedRev> {
    resolve(&src(st, owner, name)?, rev)
}

/// Latest commit of `rev` plus the commit count (from GitHub's page count; one request).
pub fn explore_commit_latest_impl(st: &AppState, owner: &str, name: &str, rev: &str) -> Result<LatestCommit> {
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.latest_commit(&r)
}

pub fn explore_tree_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: &str) -> Result<TreeListing> {
    let gp = path_arg(path)?;
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.tree(&r, &gp)
}

pub fn explore_readme_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: &str) -> Result<Option<Rendered>> {
    let gp = path_arg(path)?;
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.readme(&r, &gp)
}

pub fn explore_render_file_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: &str) -> Result<Rendered> {
    let gp = path_arg(path)?;
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.render_file(&r, &gp)
}

pub fn explore_blob_meta_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: &str) -> Result<BlobMeta> {
    let gp = path_arg(path)?;
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.blob_meta(&r, &gp)
}

pub fn explore_blob_text_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: &str) -> Result<BlobText> {
    let gp = path_arg(path)?;
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.blob_text(&r, &gp)
}

pub fn explore_blob_bytes_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: &str) -> Result<Vec<u8>> {
    let gp = path_arg(path)?;
    let s = src(st, owner, name)?;
    let r = resolve(&s, rev)?;
    s.blob_bytes(&r, &gp, BLOB_BYTES_MAX)
}

/// One page of 35. `tip` (40 hex, from the first page) pins the list across pages; `skip` is the previous page's
/// returned `skip` and must be a multiple of 35 online.
pub fn explore_commits_impl(st: &AppState, owner: &str, name: &str, rev: &str, path: Option<&str>, skip: u32, tip: Option<&str>) -> Result<CommitsPage> {
    let spec = rev_arg(rev)?;
    let gp = match path {
        Some(p) => Some(path_arg(p)?).filter(|g| !g.is_root()),
        None => None,
    };
    let tip = tip.map(validate::oid).transpose()?;
    if skip > MAX_SKIP {
        return Err(Error::InvalidInput(format!("skip must be at most {MAX_SKIP}")));
    }
    let s = src(st, owner, name)?;
    let r = match tip {
        Some(t) => ResolvedRev { input: t.to_string(), kind: rs_core::types::RefKind::Commit, full_name: None, commit: t.to_string(), tree: String::new() },
        None => s.resolve(&spec)?,
    };
    s.commits_page(&r, gp.as_ref(), skip)
}

pub fn explore_commit_impl(st: &AppState, owner: &str, name: &str, oid: &str) -> Result<CommitDetail> {
    let oid = validate::oid(oid)?.to_string();
    src(st, owner, name)?.commit_detail(&oid)
}

pub fn explore_commit_file_diff_impl(st: &AppState, owner: &str, name: &str, oid: &str, index: u32, force: bool) -> Result<FileDiff> {
    let oid = validate::oid(oid)?.to_string();
    if index > MAX_FILE_INDEX {
        return Err(Error::InvalidInput("file index out of range".into()));
    }
    src(st, owner, name)?.commit_file_diff(&oid, index, force)
}

pub fn explore_releases_impl(st: &AppState, owner: &str, name: &str) -> Result<Vec<Release>> {
    src(st, owner, name)?.releases()
}

/// `sort`: `null` (best match), `stars`, `forks`, `updated` or `help-wanted-issues`. `page` 1–34. Counts against
/// the search bucket (10 / min anonymous, 30 / min with a token).
pub fn explore_search_impl(st: &AppState, query: &str, sort: Option<&str>, page: u32) -> Result<ExploreSearch> {
    github::search(&st.shelf, query, sort, page)
}

/// Save to shelf: the existing bare-clone job for `https://github.com/<owner>/<name>.git` (Windows-safe folder
/// names, background progress, cancellable). Already saved → `duplicate`; a clone of the same address already
/// queued or running → its job id (a double click starts one job).
pub fn explore_save_to_shelf_impl(st: &AppState, owner: &str, name: &str, auto_download: Option<bool>) -> Result<JobId> {
    validate::github_owner(owner)?;
    validate::github_repo(name)?;
    if !rs_core::settings::load(&st.shelf.db)?.explore_enabled {
        return Err(Error::Refused("Explore is turned off (Settings → Network → Explore).".into()));
    }
    let url = format!("https://github.com/{owner}/{name}.git");
    let preview = super::jobs::clone_preview_impl(st, &url)?;
    if let Ok(existing) = rs_core::library::by_slug(&st.shelf, &preview.owner, &preview.name)
        && existing.origin_host.as_deref().is_some_and(|h| h.eq_ignore_ascii_case("github.com"))
    {
        return Err(Error::Duplicate(format!("{}/{}", existing.owner, existing.name)));
    }
    let active = st.jobs.list().into_iter().find(|j| {
        matches!(j.state, JobState::Waiting | JobState::Running) && matches!(&j.kind, JobKind::Clone { url: u, .. } if u.eq_ignore_ascii_case(&preview.url))
    });
    if let Some(j) = active {
        return Ok(j.id);
    }
    super::jobs::clone_start_impl(st, &url, auto_download)
}

// -- Tauri commands ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn explore_status(state: St<'_>) -> CmdResult<ExploreStatus> {
    blocking_gh(&state, explore_status_impl).await
}

#[tauri::command]
pub async fn explore_token_set(state: St<'_>, token: String) -> CmdResult<ExploreStatus> {
    blocking_gh(&state, move |st| explore_token_set_impl(st, &token)).await
}

#[tauri::command]
pub async fn explore_token_remove(state: St<'_>) -> CmdResult<ExploreStatus> {
    blocking_gh(&state, explore_token_remove_impl).await
}

#[tauri::command]
pub async fn explore_cache_clear(state: St<'_>) -> CmdResult<ExploreStatus> {
    blocking_gh(&state, explore_cache_clear_impl).await
}

#[tauri::command]
pub async fn explore_repo(state: St<'_>, owner: String, name: String) -> CmdResult<ExploreRepo> {
    blocking_gh(&state, move |st| explore_repo_impl(st, &owner, &name)).await
}

#[tauri::command]
pub async fn explore_refs(state: St<'_>, owner: String, name: String) -> CmdResult<ExploreRefs> {
    blocking_gh(&state, move |st| explore_refs_impl(st, &owner, &name)).await
}

#[tauri::command]
pub async fn explore_languages(state: St<'_>, owner: String, name: String) -> CmdResult<Languages> {
    blocking_gh(&state, move |st| explore_languages_impl(st, &owner, &name)).await
}

#[tauri::command]
pub async fn explore_resolve(state: St<'_>, owner: String, name: String, rev: String) -> CmdResult<ResolvedRev> {
    blocking_gh(&state, move |st| explore_resolve_impl(st, &owner, &name, &rev)).await
}

#[tauri::command]
pub async fn explore_commit_latest(state: St<'_>, owner: String, name: String, rev: String) -> CmdResult<LatestCommit> {
    blocking_gh(&state, move |st| explore_commit_latest_impl(st, &owner, &name, &rev)).await
}

#[tauri::command]
pub async fn explore_tree(state: St<'_>, owner: String, name: String, rev: String, path: String) -> CmdResult<TreeListing> {
    blocking_gh(&state, move |st| explore_tree_impl(st, &owner, &name, &rev, &path)).await
}

#[tauri::command]
pub async fn explore_readme(state: St<'_>, owner: String, name: String, rev: String, path: String) -> CmdResult<Option<Rendered>> {
    blocking_gh(&state, move |st| explore_readme_impl(st, &owner, &name, &rev, &path)).await
}

#[tauri::command]
pub async fn explore_render_file(state: St<'_>, owner: String, name: String, rev: String, path: String) -> CmdResult<Rendered> {
    blocking_gh(&state, move |st| explore_render_file_impl(st, &owner, &name, &rev, &path)).await
}

#[tauri::command]
pub async fn explore_blob_meta(state: St<'_>, owner: String, name: String, rev: String, path: String) -> CmdResult<BlobMeta> {
    blocking_gh(&state, move |st| explore_blob_meta_impl(st, &owner, &name, &rev, &path)).await
}

#[tauri::command]
pub async fn explore_blob_text(state: St<'_>, owner: String, name: String, rev: String, path: String) -> CmdResult<BlobText> {
    blocking_gh(&state, move |st| explore_blob_text_impl(st, &owner, &name, &rev, &path)).await
}

#[tauri::command]
pub async fn explore_blob_bytes(state: St<'_>, owner: String, name: String, rev: String, path: String) -> std::result::Result<tauri::ipc::Response, IpcError> {
    let bytes = blocking_gh(&state, move |st| explore_blob_bytes_impl(st, &owner, &name, &rev, &path)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn explore_commits(
    state: St<'_>,
    owner: String,
    name: String,
    rev: String,
    path: Option<String>,
    skip: u32,
    tip: Option<String>,
) -> CmdResult<CommitsPage> {
    blocking_gh(&state, move |st| explore_commits_impl(st, &owner, &name, &rev, path.as_deref(), skip, tip.as_deref())).await
}

#[tauri::command]
pub async fn explore_commit(state: St<'_>, owner: String, name: String, oid: String) -> CmdResult<CommitDetail> {
    blocking_gh(&state, move |st| explore_commit_impl(st, &owner, &name, &oid)).await
}

#[tauri::command]
pub async fn explore_commit_file_diff(state: St<'_>, owner: String, name: String, oid: String, index: u32, force: bool) -> CmdResult<FileDiff> {
    blocking_gh(&state, move |st| explore_commit_file_diff_impl(st, &owner, &name, &oid, index, force)).await
}

#[tauri::command]
pub async fn explore_releases(state: St<'_>, owner: String, name: String) -> CmdResult<Vec<Release>> {
    blocking_gh(&state, move |st| explore_releases_impl(st, &owner, &name)).await
}

#[tauri::command]
pub async fn explore_search(state: St<'_>, query: String, sort: Option<String>, page: u32) -> CmdResult<ExploreSearch> {
    blocking_gh(&state, move |st| explore_search_impl(st, &query, sort.as_deref(), page)).await
}

#[tauri::command]
pub async fn explore_save_to_shelf(state: St<'_>, owner: String, name: String, auto_download: Option<bool>) -> CmdResult<JobId> {
    blocking_gh(&state, move |st| explore_save_to_shelf_impl(st, &owner, &name, auto_download)).await
}
