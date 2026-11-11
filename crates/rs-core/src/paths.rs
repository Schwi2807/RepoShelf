//! Data/archive directories and Windows path helpers (ARCH §3.2, D-13, FR-35, NFR-W1).

use crate::error::Result;
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "dev.schwi.reposhelf";
pub const ENV_DATA_DIR: &str = "RS_DATA_DIR";
pub const ENV_ARCHIVE_DIR: &str = "RS_ARCHIVE_DIR";
pub const DB_FILE_NAME: &str = "reposhelf.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    pub data: PathBuf,
    pub archive: PathBuf,
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `RS_DATA_DIR` / `RS_ARCHIVE_DIR` win; else `%LOCALAPPDATA%\dev.schwi.reposhelf` and `Documents\RepoShelf`.
/// Pure: nothing is created here.
pub fn resolve_dirs() -> Dirs {
    let data = env_dir(ENV_DATA_DIR).unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(APP_ID)
    });
    let archive = env_dir(ENV_ARCHIVE_DIR).unwrap_or_else(|| {
        dirs::document_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join("Documents")))
            .unwrap_or_else(std::env::temp_dir)
            .join("RepoShelf")
    });
    Dirs { data, archive }
}

pub fn db_file(d: &Dirs) -> PathBuf {
    d.data.join(DB_FILE_NAME)
}

/// `\\?\` prefix for filesystem operations on deep paths (Windows only; idempotent; relative paths unchanged).
#[cfg(windows)]
pub fn long(p: &Path) -> PathBuf {
    let s = p.as_os_str().to_string_lossy();
    if s.starts_with(r"\\?\") || !p.is_absolute() {
        return p.to_path_buf();
    }
    let s = s.replace('/', "\\");
    if let Some(rest) = s.strip_prefix(r"\\") {
        PathBuf::from(format!(r"\\?\UNC\{rest}"))
    } else {
        PathBuf::from(format!(r"\\?\{s}"))
    }
}

#[cfg(not(windows))]
pub fn long(p: &Path) -> PathBuf {
    p.to_path_buf()
}

/// Strips a verbatim prefix for display / duplicate detection.
pub fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

/// Canonical display form: `canonicalize` (resolves 8.3 names, casing, `..`) then strip `\\?\`.
pub fn canonical(p: &Path) -> Result<PathBuf> {
    Ok(strip_verbatim(&std::fs::canonicalize(p)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Env is process-global: the only test that touches it holds this lock.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn env_overrides_honoured() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_d = std::env::var_os(ENV_DATA_DIR);
        let old_a = std::env::var_os(ENV_ARCHIVE_DIR);
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("my data");
        let archive = t.path().join("my archive");
        // SAFETY: guarded by ENV_LOCK; no other test reads these variables.
        unsafe {
            std::env::set_var(ENV_DATA_DIR, &data);
            std::env::set_var(ENV_ARCHIVE_DIR, &archive);
        }
        let d = resolve_dirs();
        assert_eq!(d.data, data);
        assert_eq!(d.archive, archive);
        assert_eq!(db_file(&d), data.join("reposhelf.db"));

        // Defaults when unset (computed only, never created).
        unsafe {
            std::env::remove_var(ENV_DATA_DIR);
            std::env::set_var(ENV_ARCHIVE_DIR, "");
        }
        let d = resolve_dirs();
        assert!(d.data.ends_with(APP_ID), "{:?}", d.data);
        assert!(d.archive.ends_with("RepoShelf"), "{:?}", d.archive);

        unsafe {
            match old_d {
                Some(v) => std::env::set_var(ENV_DATA_DIR, v),
                None => std::env::remove_var(ENV_DATA_DIR),
            }
            match old_a {
                Some(v) => std::env::set_var(ENV_ARCHIVE_DIR, v),
                None => std::env::remove_var(ENV_ARCHIVE_DIR),
            }
        }
    }

    #[test]
    fn canonical_handles_spaces_and_strips_verbatim() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("with space").join("deeper dir");
        std::fs::create_dir_all(&dir).unwrap();
        let c = canonical(&dir).unwrap();
        assert!(!c.to_string_lossy().starts_with(r"\\?\"), "{c:?}");
        assert!(c.ends_with("deeper dir"));
        // Canonical is stable: canonical(canonical(x)) == canonical(x), and `..` resolves.
        assert_eq!(canonical(&c).unwrap(), c);
        assert_eq!(canonical(&dir.join("..").join("deeper dir")).unwrap(), c);
    }

    #[test]
    fn long_is_idempotent() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("a b").join("c");
        let once = long(&p);
        assert_eq!(long(&once), once);
        #[cfg(windows)]
        {
            assert!(once.to_string_lossy().starts_with(r"\\?\"));
            assert_eq!(strip_verbatim(&once), PathBuf::from(p.to_string_lossy().replace('/', "\\")));
            assert_eq!(long(Path::new("rel/x")), PathBuf::from("rel/x"));
            assert_eq!(long(Path::new(r"\\srv\share\x")), PathBuf::from(r"\\?\UNC\srv\share\x"));
        }
    }

    #[test]
    fn long_path_roundtrip_on_disk() {
        let t = tempfile::tempdir().unwrap();
        let mut p = t.path().to_path_buf();
        for i in 0..30 {
            p.push(format!("segment-number-{i:02}"));
        }
        assert!(p.as_os_str().len() > 260);
        std::fs::create_dir_all(long(&p)).unwrap();
        std::fs::write(long(&p.join("f.txt")), b"x").unwrap();
        assert_eq!(std::fs::read(long(&p.join("f.txt"))).unwrap(), b"x");
    }
}
