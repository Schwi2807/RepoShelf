//! e2e-only OS seams (M8.2/M8.3, D-84). Compiled only with `--features e2e` (the separate `target\e2e` build); a
//! shipped build contains none of these env names.
//!
//! - `RS_E2E_PICK` = folders separated by `;`. Each native folder dialog takes the next one instead of opening; an
//!   empty queue answers "cancelled". "Save as" answers "cancelled".
//! - `RS_E2E_OPEN_LOG` = a file. Browser / Explorer opens are appended to it (`url\t…`, `folder\t…`, `reveal\t…`)
//!   instead of popping a window on the user's screen.
//! - `RS_E2E_OFFSCREEN=1` = the main window is created off-screen (it keeps rendering; WebDriver's minimize does not
//!   reach a WebView2 window).
//! - `RS_E2E_TRASH_LOG` = a file. "Move to Recycle Bin" deletes the folder outright (only inside the archive dir) and
//!   appends `trash\t<path>` instead of filling the user's real Recycle Bin (D-91).
//! - `RS_E2E_NET` = a JSON routes file (M13, D-113). The shelf's HTTP client gets a canned transport: every request
//!   (GitHub info, release lists, asset downloads, README images) is answered from the file or fails as
//!   "no canned answer" — the e2e exe then has no path to the internet at all. `RS_E2E_NET_LOG` = a file that gets
//!   one line per request (`get\t<url>\t<status>` or `get\t<url>\tunrouted`; `\tauth` is appended when the request
//!   carried an `Authorization` header — the header's value is never logged, M10c).
//! - `RS_E2E_GIT_MAP` = `<https clone URL>=<file:// URL>` pairs separated by `;` (M13): a Clone / Fetch of a mapped
//!   github.com repo downloads from the local upstream and keeps the https origin; every other non-`file:` git
//!   remote is refused unless the list contains the item `!live` (the opt-in `-Network` run). Needs
//!   `RS_TEST_FILE_CLONE=1` (the e2e worker).

use crate::state::{Opener, Picker, Trasher, lock};
use rs_core::net::{Net, RawResponse, Transport};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const ENV_PICK: &str = "RS_E2E_PICK";
pub const ENV_OPEN_LOG: &str = "RS_E2E_OPEN_LOG";
pub const ENV_OFFSCREEN: &str = "RS_E2E_OFFSCREEN";
pub const ENV_TRASH_LOG: &str = "RS_E2E_TRASH_LOG";
pub const ENV_NET: &str = "RS_E2E_NET";
pub const ENV_NET_LOG: &str = "RS_E2E_NET_LOG";
pub const ENV_GIT_MAP: &str = "RS_E2E_GIT_MAP";
/// Where the off-screen window goes (same spot PM's e2e watcher uses).
pub const OFFSCREEN_POS: (f64, f64) = (-32000.0, -32000.0);

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub fn offscreen() -> bool {
    env(ENV_OFFSCREEN).is_some_and(|v| v == "1")
}

/// `a;b;;c` → `[a, b, c]` (blank items dropped, surrounding spaces trimmed).
pub fn parse_pick_queue(v: &str) -> VecDeque<PathBuf> {
    v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from).collect()
}

/// Wraps the real picker/opener; each half is replaced only when its env var is set.
pub struct E2eSeams {
    picks: Option<Mutex<VecDeque<PathBuf>>>,
    open_log: Option<PathBuf>,
    picker: Arc<dyn Picker>,
    opener: Arc<dyn Opener>,
}

impl E2eSeams {
    pub fn new(pick: Option<&str>, open_log: Option<PathBuf>, picker: Arc<dyn Picker>, opener: Arc<dyn Opener>) -> E2eSeams {
        E2eSeams { picks: pick.map(|v| Mutex::new(parse_pick_queue(v))), open_log, picker, opener }
    }

    pub fn from_env(picker: Arc<dyn Picker>, opener: Arc<dyn Opener>) -> E2eSeams {
        E2eSeams::new(env(ENV_PICK).as_deref(), env(ENV_OPEN_LOG).map(PathBuf::from), picker, opener)
    }

    fn log(&self, file: &Path, kind: &str, what: &str) -> Result<(), String> {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file).map_err(|e| e.to_string())?;
        f.write_all(format!("{kind}\t{what}\n").as_bytes()).map_err(|e| e.to_string())
    }
}

impl Picker for E2eSeams {
    fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        match &self.picks {
            Some(q) => lock(q).pop_front(),
            None => self.picker.pick_folder(title),
        }
    }
    fn save_file(&self, title: &str, file_name: &str) -> Option<PathBuf> {
        match &self.picks {
            Some(_) => None,
            None => self.picker.save_file(title, file_name),
        }
    }
}

impl Opener for E2eSeams {
    fn open_url(&self, url: &str) -> Result<(), String> {
        match &self.open_log {
            Some(f) => self.log(f, "url", url),
            None => self.opener.open_url(url),
        }
    }
    fn open_folder(&self, path: &Path) -> Result<(), String> {
        match &self.open_log {
            Some(f) => self.log(f, "folder", &path.display().to_string()),
            None => self.opener.open_folder(path),
        }
    }
    fn reveal(&self, path: &Path) -> Result<(), String> {
        match &self.open_log {
            Some(f) => self.log(f, "reveal", &path.display().to_string()),
            None => self.opener.reveal(path),
        }
    }
}

/// Trash seam (D-91): with a log file, the folder is deleted outright and logged — never sent to the user's real
/// Recycle Bin — and only when it sits strictly inside `archive` (the scratch archive of the e2e run).
pub struct E2eTrash {
    log: Option<PathBuf>,
    archive: PathBuf,
    real: Arc<dyn Trasher>,
}

impl E2eTrash {
    pub fn new(log: Option<PathBuf>, archive: PathBuf, real: Arc<dyn Trasher>) -> E2eTrash {
        E2eTrash { log, archive, real }
    }

    pub fn from_env(archive: PathBuf, real: Arc<dyn Trasher>) -> E2eTrash {
        E2eTrash::new(env(ENV_TRASH_LOG).map(PathBuf::from), archive, real)
    }
}

impl Trasher for E2eTrash {
    fn delete(&self, path: &Path) -> Result<(), String> {
        let Some(log) = &self.log else { return self.real.delete(path) };
        let canon = |p: &Path| rs_core::paths::canonical(p).map_err(|e| format!("{}: {e}", p.display()));
        let (dir, archive) = (canon(path)?, canon(&self.archive)?);
        if dir == archive || !dir.starts_with(&archive) {
            return Err(format!("e2e trash refused outside the archive: {}", dir.display()));
        }
        std::fs::remove_dir_all(rs_core::paths::long(&dir)).map_err(|e| e.to_string())?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(log).map_err(|e| e.to_string())?;
        writeln!(f, "trash\t{}", dir.display()).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Network seam (M13, D-113)

/// One canned answer of the routes file.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Route {
    #[serde(default = "ok_status")]
    status: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    /// UTF-8 body.
    #[serde(default)]
    body: Option<String>,
    /// Binary body as hex (zip/png payloads).
    #[serde(default)]
    body_hex: Option<String>,
}

fn ok_status() -> u16 {
    200
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutesFile {
    routes: HashMap<String, Route>,
}

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !s.len().is_multiple_of(2) {
        return Err("odd hex length".into());
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string())).collect()
}

/// Status, lower-case headers, body.
type Canned = (u16, Vec<(String, String)>, Vec<u8>);

/// Canned transport for the e2e build: exact-URL routes, everything else fails; every request is logged.
pub struct E2eNet {
    routes: HashMap<String, Canned>,
    log: Option<PathBuf>,
    seen: Mutex<Vec<String>>,
}

impl E2eNet {
    pub fn parse(json: &str, log: Option<PathBuf>) -> Result<E2eNet, String> {
        let f: RoutesFile = serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|e| format!("e2e routes: {e}"))?;
        let mut routes = HashMap::new();
        for (url, r) in f.routes {
            let body = match (r.body, r.body_hex) {
                (Some(_), Some(_)) => return Err(format!("e2e routes: {url} has both body and bodyHex")),
                (Some(b), None) => b.into_bytes(),
                (None, Some(h)) => unhex(&h).map_err(|e| format!("e2e routes: {url}: {e}"))?,
                (None, None) => Vec::new(),
            };
            let mut headers: Vec<(String, String)> = r.headers.into_iter().map(|(k, v)| (k.to_ascii_lowercase(), v)).collect();
            if !headers.iter().any(|(k, _)| k == "content-length") {
                headers.push(("content-length".into(), body.len().to_string()));
            }
            routes.insert(url, (r.status, headers, body));
        }
        Ok(E2eNet { routes, log, seen: Mutex::new(Vec::new()) })
    }

    pub fn from_env() -> Option<Result<E2eNet, String>> {
        let file = env(ENV_NET)?;
        Some(
            std::fs::read_to_string(&file)
                .map_err(|e| format!("e2e routes file {file}: {e}"))
                .and_then(|j| E2eNet::parse(&j, env(ENV_NET_LOG).map(PathBuf::from))),
        )
    }

    pub fn seen(&self) -> Vec<String> {
        lock(&self.seen).clone()
    }

    fn note(&self, line: String) {
        lock(&self.seen).push(line.clone());
        if let Some(f) = &self.log
            && let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(f)
        {
            // one write per line: two threads (the UI and the job worker) must not interleave the text and the newline
            let _ = f.write_all(format!("{line}\n").as_bytes());
        }
    }
}

impl Transport for E2eNet {
    /// Every name "resolves" to one public address, so the production host rules still run; nothing is looked up.
    fn resolve(&self, _host: &str) -> std::io::Result<Vec<IpAddr>> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(140, 82, 112, 3))])
    }

    fn get(&self, url: &tauri::Url, h: &[(&str, String)], _t: Duration) -> rs_core::Result<RawResponse> {
        match self.routes.get(url.as_str()) {
            Some((status, headers, body)) => {
                // M10c: a fourth field says that the request carried an Authorization header (never its value).
                let auth = if h.iter().any(|(k, _)| k.eq_ignore_ascii_case("authorization")) { "\tauth" } else { "" };
                self.note(format!("get\t{url}\t{status}{auth}"));
                Ok(RawResponse { status: *status, headers: headers.clone(), body: Box::new(std::io::Cursor::new(body.clone())) })
            }
            None => {
                self.note(format!("get\t{url}\tunrouted"));
                Err(rs_core::Error::Network(format!("e2e: no canned answer for {url}")))
            }
        }
    }
}

/// The shelf's client for this run: canned when `RS_E2E_NET` is set (an unreadable file stops the start-up).
pub fn net_from_env() -> Option<Result<Net, String>> {
    E2eNet::from_env().map(|r| r.map(|n| Net::new(Arc::new(n))))
}

// ---------------------------------------------------------------------------------------------------------------
// Git map (M13)

/// `https://github.com/o/r=file:///C:/up/r;…;!live` → (normalised https URL → file URL, live).
pub fn parse_git_map(v: &str) -> Result<(HashMap<String, String>, bool), String> {
    let mut map = HashMap::new();
    let mut live = false;
    for item in v.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if item == "!live" {
            live = true;
            continue;
        }
        let (from, to) = item.split_once('=').ok_or_else(|| format!("e2e git map item without '=': {item}"))?;
        let from = rs_core::validate::clone_url(from).map_err(|e| format!("e2e git map {from}: {e}"))?.url;
        if !to.starts_with("file:") {
            return Err(format!("e2e git map target must be a file:// URL: {to}"));
        }
        map.insert(from, to.to_string());
    }
    Ok((map, live))
}

/// `Some((map, live))` when `RS_E2E_GIT_MAP` is set; a malformed value maps nothing and stays strict.
pub fn git_map_from_env() -> Option<(rs_core::jobs::GitMap, bool)> {
    let v = env(ENV_GIT_MAP)?;
    let (map, live) = parse_git_map(&v).unwrap_or_else(|e| {
        eprintln!("RepoShelf e2e: {e}");
        (HashMap::new(), false)
    });
    Some((Arc::new(move |u: &str| map.get(u).cloned()), live))
}
