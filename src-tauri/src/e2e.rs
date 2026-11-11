//! e2e-only OS seams (M8.2/M8.3, D-84). Compiled only with `--features e2e` (the separate `target\e2e` build); a
//! shipped build contains none of these env names.
//!
//! - `RS_E2E_PICK` = folders separated by `;`. Each native folder dialog takes the next one instead of opening; an
//!   empty queue answers "cancelled". "Save as" answers "cancelled".
//! - `RS_E2E_OPEN_LOG` = a file. Browser / Explorer opens are appended to it (`url\t…`, `folder\t…`, `reveal\t…`)
//!   instead of popping a window on the user's screen.
//! - `RS_E2E_OFFSCREEN=1` = the main window is created off-screen (it keeps rendering; WebDriver's minimize does not
//!   reach a WebView2 window).

use crate::state::{Opener, Picker, lock};
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const ENV_PICK: &str = "RS_E2E_PICK";
pub const ENV_OPEN_LOG: &str = "RS_E2E_OPEN_LOG";
pub const ENV_OFFSCREEN: &str = "RS_E2E_OFFSCREEN";
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
        writeln!(f, "{kind}\t{what}").map_err(|e| e.to_string())
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
