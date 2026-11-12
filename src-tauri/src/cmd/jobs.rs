//! Job commands (M4.9): clone/fetch/fetch-all/fast-forward and the progress channel. FR-6..11, NFR-P7.
//! The file:// test seam (D-14, D-20) exists only in the `e2e` build and only with `test_file_clone` set.

use super::{St, blocking};
use crate::ipc_error::CmdResult;
use crate::state::AppState;
use rs_core::error::{Error, Result};
use rs_core::jobs::{JobId, JobInfo, JobKind};
use rs_core::types::ClonePreview;
use tauri::ipc::Channel;

/// e2e build with `test_file_clone`: `file://` inputs bypass the https rules (never in a shipped build).
#[cfg(feature = "e2e")]
fn test_file_input(st: &AppState, input: &str) -> bool {
    st.test_file_clone && input.trim().starts_with("file:")
}

#[cfg(not(feature = "e2e"))]
fn test_file_input(_st: &AppState, _input: &str) -> bool {
    false
}

#[cfg(feature = "e2e")]
fn test_file_preview(st: &AppState, input: &str) -> Result<ClonePreview> {
    let url = tauri::Url::parse(input.trim()).map_err(|e| Error::InvalidUrl(e.to_string()))?;
    let path = url.to_file_path().map_err(|_| Error::InvalidUrl("not a local path".into()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().trim_end_matches(".git").to_string()).unwrap_or_default();
    let owner = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let target = st.shelf.dirs.archive.join("file").join(&owner).join(format!("{name}.git"));
    Ok(ClonePreview { url: input.trim().to_string(), host: "file".into(), owner, name, target_dir: target.display().to_string() })
}

#[cfg(not(feature = "e2e"))]
fn test_file_preview(_st: &AppState, _input: &str) -> Result<ClonePreview> {
    Err(Error::InvalidUrl("unreachable".into()))
}

/// FR-6: validates (pure, no network) and shows the target folder.
pub fn clone_preview_impl(st: &AppState, input: &str) -> Result<ClonePreview> {
    if test_file_input(st, input) {
        return test_file_preview(st, input);
    }
    rs_core::clone::preview(&st.shelf, input)
}

/// Validates synchronously (bad input → `invalidUrl` before any job exists), then queues the clone.
/// `auto_download`: the dialog's checkbox (`None` = setting `autoDownloadLatestRelease`).
pub fn clone_start_impl(st: &AppState, input: &str, auto_download: Option<bool>) -> Result<JobId> {
    let p = clone_preview_impl(st, input)?;
    st.jobs.submit(JobKind::Clone { url: p.url, auto_download }, format!("Clone {}/{}", p.owner, p.name))
}

/// FR-10: managed clones and imported repos with an https origin (the UI asks before fetching an imported repo).
/// `auto_download`: the Fetch menu option (`None` = setting).
pub fn fetch_start_impl(st: &AppState, repo_id: i64, auto_download: Option<bool>) -> Result<JobId> {
    let sum = rs_core::repo::summary(&st.shelf, repo_id)?;
    let origin = sum.origin_url.as_deref();
    let ok = rs_core::repo::is_fetchable(origin) || origin.is_some_and(|o| test_file_input(st, o));
    if !ok {
        return Err(Error::Refused(rs_core::fetch::NEEDS_HTTPS.into()));
    }
    st.jobs.submit(JobKind::Fetch { repo_id, auto_download }, format!("Fetch {}/{}", sum.owner, sum.name))
}

/// Upper bound for tag / asset names from the UI (GitHub's own limits are far lower).
const MAX_NAME: usize = 512;

/// F4 "Download": one asset of the cached GitHub release list (looked up by tag + name in Rust, never a URL or
/// a path from the webview) into the release store, as a cancellable job. Feature off → `unsupported`.
pub fn release_download_impl(st: &AppState, repo_id: i64, tag: &str, asset: &str) -> Result<JobId> {
    for v in [tag, asset] {
        if v.is_empty() || v.len() > MAX_NAME || v.chars().any(char::is_control) {
            return Err(Error::InvalidInput("invalid release or file name".into()));
        }
    }
    rs_core::repo::summary(&st.shelf, repo_id)?;
    if !st.shelf.net.available() {
        return Err(Error::Unsupported("Release downloads are not included in this build.".into()));
    }
    let gh = rs_core::ghreleases::cached(&st.shelf, repo_id)?.ok_or_else(|| Error::not_found("GitHub release list (load the release notes first)"))?;
    let known = gh.iter().any(|r| r.tag_name == tag && r.assets.iter().any(|a| a.name == asset));
    if !known {
        return Err(Error::not_found("release file"));
    }
    st.jobs.submit(
        JobKind::ReleaseDownload { repo_id, tag: tag.to_string(), asset: asset.to_string() },
        format!("Download {asset} ({tag})"),
    )
}

/// After an import (D-103): GitHub info / notes (+ auto-download) as a follow-up job, only for github.com
/// origins and when any of the settings asks for it. `None` when nothing is queued.
pub fn queue_github_sync(st: &AppState, repo_id: i64, auto_download: Option<bool>) -> Option<JobId> {
    let opts = rs_core::github::SyncOpts::from_settings(&st.shelf, auto_download);
    if !opts.any() || !st.shelf.net.available() || !rs_core::github::applies(&st.shelf, repo_id) {
        return None;
    }
    let sum = rs_core::repo::summary(&st.shelf, repo_id).ok()?;
    st.jobs.submit(JobKind::GithubSync { repo_id, auto_download }, format!("GitHub info {}/{}", sum.owner, sum.name)).ok()
}

/// S1 "Fetch all": one sequential, cancellable job over every fetchable repo.
pub fn fetch_all_start_impl(st: &AppState) -> Result<JobId> {
    st.jobs.submit(JobKind::FetchAll, "Fetch all".into())
}

/// FR-11 (managed clones only): returns the new tip.
pub fn fast_forward_impl(st: &AppState, repo_id: i64) -> Result<String> {
    rs_core::fetch::fast_forward_default(&st.shelf, repo_id)
}

/// Registers the channel first (a duplicate event is harmless, a gap is not), then returns the snapshot.
pub fn jobs_subscribe_impl(st: &AppState, key: &str, channel: Channel<JobInfo>) -> Vec<JobInfo> {
    st.subscribers.add(key, channel);
    st.jobs.list()
}

pub fn jobs_cancel_impl(st: &AppState, job_id: JobId) -> bool {
    st.jobs.cancel(job_id)
}

pub fn jobs_retry_impl(st: &AppState, job_id: JobId) -> Result<JobId> {
    st.jobs.retry(job_id)
}

pub fn jobs_dismiss_impl(st: &AppState, job_id: JobId) {
    st.jobs.dismiss(job_id)
}

#[tauri::command]
pub async fn clone_preview(state: St<'_>, input: String) -> CmdResult<ClonePreview> {
    blocking(&state, move |st| clone_preview_impl(st, &input)).await
}

#[tauri::command]
pub async fn clone_start(state: St<'_>, input: String, auto_download: Option<bool>) -> CmdResult<JobId> {
    blocking(&state, move |st| clone_start_impl(st, &input, auto_download)).await
}

#[tauri::command]
pub async fn fetch_start(state: St<'_>, repo_id: i64, auto_download: Option<bool>) -> CmdResult<JobId> {
    blocking(&state, move |st| fetch_start_impl(st, repo_id, auto_download)).await
}

#[tauri::command]
pub async fn release_download(state: St<'_>, repo_id: i64, tag: String, asset: String) -> CmdResult<JobId> {
    blocking(&state, move |st| release_download_impl(st, repo_id, &tag, &asset)).await
}

#[tauri::command]
pub async fn fetch_all_start(state: St<'_>) -> CmdResult<JobId> {
    blocking(&state, fetch_all_start_impl).await
}

#[tauri::command]
pub async fn fast_forward(state: St<'_>, repo_id: i64) -> CmdResult<String> {
    blocking(&state, move |st| fast_forward_impl(st, repo_id)).await
}

#[tauri::command]
pub async fn jobs_subscribe<R: tauri::Runtime>(
    state: St<'_>,
    webview: tauri::Webview<R>,
    channel: Channel<JobInfo>,
) -> CmdResult<Vec<JobInfo>> {
    Ok(jobs_subscribe_impl(&state, webview.label(), channel))
}

#[tauri::command]
pub async fn jobs_cancel(state: St<'_>, job_id: JobId) -> CmdResult<bool> {
    Ok(jobs_cancel_impl(&state, job_id))
}

#[tauri::command]
pub async fn jobs_retry(state: St<'_>, job_id: JobId) -> CmdResult<JobId> {
    blocking(&state, move |st| jobs_retry_impl(st, job_id)).await
}

#[tauri::command]
pub async fn jobs_dismiss(state: St<'_>, job_id: JobId) -> CmdResult<()> {
    blocking(&state, move |st| {
        jobs_dismiss_impl(st, job_id);
        Ok(())
    })
    .await
}
