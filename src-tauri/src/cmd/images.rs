//! Remote README images (v0.2.0 F2, D-105). The webview stays offline: `image_fetch` makes Rust fetch and cache
//! the image (or answer from the disk cache), `image_bytes` hands the cached bytes over as raw IPC bytes, which
//! the UI turns into a `blob:` URL (CSP `img-src 'self' data: blob:` unchanged).

use super::{St, blocking};
use crate::ipc_error::{CmdResult, IpcError};
use crate::state::AppState;
use rs_core::error::Result;
use rs_core::types::RemoteImage;

pub fn image_fetch_impl(st: &AppState, url: &str) -> Result<RemoteImage> {
    rs_core::imgcache::fetch(&st.shelf, url)
}

pub fn image_bytes_impl(st: &AppState, key: &str) -> Result<Vec<u8>> {
    rs_core::imgcache::bytes(&st.shelf, key)
}

#[tauri::command]
pub async fn image_fetch(state: St<'_>, url: String) -> CmdResult<RemoteImage> {
    let host = tauri::Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_string));
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || image_fetch_impl(&st, &url))
        .await
        .map_err(|e| IpcError::new("internal", format!("Internal error: {e}")))?
        .map_err(|e| IpcError::from_core(&e, host.as_deref()))
}

#[tauri::command]
pub async fn image_bytes(state: St<'_>, key: String) -> std::result::Result<tauri::ipc::Response, IpcError> {
    let bytes = blocking(&state, move |st| image_bytes_impl(st, &key)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}
