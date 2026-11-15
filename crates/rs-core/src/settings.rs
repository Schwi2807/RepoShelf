//! Typed view over the `settings` table (key → JSON value). FR-3, FR-33, D-16, D-28 (no `recent`).
//! v0.2.0 (D-100): the "Network" keys, with the lead's defaults.

use crate::db::Db;
use crate::error::Result;
use crate::types::{SortSpec, SourceFilter, Theme};
use serde::{Deserialize, Serialize};

/// Download size cap bounds (MB).
pub const MIN_CAP_MB: u32 = 1;
pub const MAX_CAP_MB: u32 = 16 * 1024;
pub const DEFAULT_CAP_MB: u32 = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    pub sort: SortSpec,
    pub source_filter: SourceFilter,
    /// D-16: off by default.
    pub show_dirty_note: bool,
    /// F2: README images from other hosts are fetched by Rust and cached (on).
    pub load_remote_images: bool,
    /// F6: GitHub release notes + assets on Clone / Import / Fetch (on).
    pub fetch_release_notes: bool,
    /// F6: GitHub description / topics / homepage on Clone / Import / Fetch (on).
    pub fetch_repo_info: bool,
    /// F4: download the latest release asset for this PC after Clone / Import / Fetch (on).
    pub auto_download_latest_release: bool,
    /// F4: … also during "Fetch all" (off).
    pub auto_download_on_fetch_all: bool,
    /// F4: largest release asset downloaded, in MB (1 GB).
    pub download_size_cap_mb: u32,
    /// v0.3.0 (D-119): the Explore entry and every Explore call (on).
    pub explore_enabled: bool,
    /// v0.3.0 (D-120): OUTPUT ONLY — whether a GitHub token is saved in Windows Credential Manager. Never stored
    /// in the DB (`load` always yields `false`, `save` ignores it); the command layer fills it from the credential
    /// store, so it cannot drift from the real state. The token itself never crosses IPC.
    pub github_token_saved: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::default(),
            sort: SortSpec::default(),
            source_filter: SourceFilter::default(),
            show_dirty_note: false,
            load_remote_images: true,
            fetch_release_notes: true,
            fetch_repo_info: true,
            auto_download_latest_release: true,
            auto_download_on_fetch_all: false,
            download_size_cap_mb: DEFAULT_CAP_MB,
            explore_enabled: true,
            github_token_saved: false,
        }
    }
}

impl Settings {
    pub fn download_cap_bytes(&self) -> u64 {
        u64::from(self.download_size_cap_mb.clamp(MIN_CAP_MB, MAX_CAP_MB)) << 20
    }
}

const K_THEME: &str = "theme";
const K_SORT: &str = "sort";
const K_SOURCE_FILTER: &str = "sourceFilter";
const K_SHOW_DIRTY_NOTE: &str = "showDirtyNote";
const K_LOAD_REMOTE_IMAGES: &str = "loadRemoteImages";
const K_FETCH_RELEASE_NOTES: &str = "fetchReleaseNotes";
const K_FETCH_REPO_INFO: &str = "fetchRepoInfo";
const K_AUTO_DOWNLOAD: &str = "autoDownloadLatestRelease";
const K_AUTO_DOWNLOAD_FETCH_ALL: &str = "autoDownloadOnFetchAll";
const K_CAP_MB: &str = "downloadSizeCapMb";
const K_EXPLORE: &str = "exploreEnabled";

fn parse_or<T: serde::de::DeserializeOwned>(v: &str, default: T) -> T {
    serde_json::from_str(v).unwrap_or(default)
}

/// Defaults for absent keys (and for values this build cannot parse); unknown keys are ignored.
pub fn load(db: &Db) -> Result<Settings> {
    let rows: Vec<(String, String)> = db.with(|c| {
        let mut st = c.prepare("SELECT key, value FROM settings")?;
        Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?)
    })?;
    let mut s = Settings::default();
    for (k, v) in rows {
        match k.as_str() {
            K_THEME => s.theme = parse_or(&v, s.theme),
            K_SORT => s.sort = parse_or(&v, s.sort),
            K_SOURCE_FILTER => s.source_filter = parse_or(&v, s.source_filter),
            K_SHOW_DIRTY_NOTE => s.show_dirty_note = parse_or(&v, s.show_dirty_note),
            K_LOAD_REMOTE_IMAGES => s.load_remote_images = parse_or(&v, s.load_remote_images),
            K_FETCH_RELEASE_NOTES => s.fetch_release_notes = parse_or(&v, s.fetch_release_notes),
            K_FETCH_REPO_INFO => s.fetch_repo_info = parse_or(&v, s.fetch_repo_info),
            K_AUTO_DOWNLOAD => s.auto_download_latest_release = parse_or(&v, s.auto_download_latest_release),
            K_AUTO_DOWNLOAD_FETCH_ALL => s.auto_download_on_fetch_all = parse_or(&v, s.auto_download_on_fetch_all),
            K_CAP_MB => s.download_size_cap_mb = parse_or::<u32>(&v, s.download_size_cap_mb).clamp(MIN_CAP_MB, MAX_CAP_MB),
            K_EXPLORE => s.explore_enabled = parse_or(&v, s.explore_enabled),
            _ => {}
        }
    }
    Ok(s)
}

/// Saves every key; the size cap is clamped to 1 MB … 16 GB.
pub fn save(db: &Db, s: &Settings) -> Result<()> {
    let j = |v: &dyn erased::Ser| v.json();
    let cap = s.download_size_cap_mb.clamp(MIN_CAP_MB, MAX_CAP_MB);
    let pairs = [
        (K_THEME, j(&s.theme)),
        (K_SORT, j(&s.sort)),
        (K_SOURCE_FILTER, j(&s.source_filter)),
        (K_SHOW_DIRTY_NOTE, j(&s.show_dirty_note)),
        (K_LOAD_REMOTE_IMAGES, j(&s.load_remote_images)),
        (K_FETCH_RELEASE_NOTES, j(&s.fetch_release_notes)),
        (K_FETCH_REPO_INFO, j(&s.fetch_repo_info)),
        (K_AUTO_DOWNLOAD, j(&s.auto_download_latest_release)),
        (K_AUTO_DOWNLOAD_FETCH_ALL, j(&s.auto_download_on_fetch_all)),
        (K_CAP_MB, j(&cap)),
        (K_EXPLORE, j(&s.explore_enabled)),
    ];
    db.tx(|tx| {
        let mut st = tx.prepare("INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?;
        for (k, v) in pairs {
            let v = v.map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
            st.execute([k, v.as_str()])?;
        }
        Ok(())
    })
}

mod erased {
    /// One JSON encoder for the differently typed values above.
    pub trait Ser {
        fn json(&self) -> serde_json::Result<String>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn json(&self) -> serde_json::Result<String> {
            serde_json::to_string(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SortKey;

    #[test]
    fn defaults_when_absent() {
        let db = Db::open_in_memory().unwrap();
        let s = load(&db).unwrap();
        assert_eq!(s, Settings::default());
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.sort, SortSpec { key: SortKey::Updated, desc: true });
        assert!(!s.show_dirty_note);
        // v0.2.0 network defaults (lead decisions)
        assert!(s.load_remote_images && s.fetch_release_notes && s.fetch_repo_info && s.auto_download_latest_release);
        assert!(!s.auto_download_on_fetch_all);
        assert_eq!(s.download_size_cap_mb, 1024);
        assert_eq!(s.download_cap_bytes(), 1 << 30);
        assert!(s.explore_enabled && !s.github_token_saved);
    }

    #[test]
    fn round_trip_across_reopen() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("reposhelf.db");
        let want = Settings {
            theme: Theme::Dark,
            sort: SortSpec { key: SortKey::Name, desc: false },
            source_filter: SourceFilter::Cloned,
            show_dirty_note: true,
            load_remote_images: false,
            fetch_release_notes: false,
            fetch_repo_info: false,
            auto_download_latest_release: false,
            auto_download_on_fetch_all: true,
            download_size_cap_mb: 50,
            explore_enabled: false,
            github_token_saved: false,
        };
        save(&Db::open(&f).unwrap(), &want).unwrap();
        assert_eq!(load(&Db::open(&f).unwrap()).unwrap(), want);
        // saving again updates in place
        let again = Settings { theme: Theme::Light, ..want };
        let db = Db::open(&f).unwrap();
        save(&db, &again).unwrap();
        assert_eq!(load(&db).unwrap(), again);
        // cap clamped
        save(&db, &Settings { download_size_cap_mb: 0, ..Settings::default() }).unwrap();
        assert_eq!(load(&db).unwrap().download_size_cap_mb, MIN_CAP_MB);
        save(&db, &Settings { download_size_cap_mb: u32::MAX, ..Settings::default() }).unwrap();
        assert_eq!(load(&db).unwrap().download_size_cap_mb, MAX_CAP_MB);
    }

    /// D-120: the token-presence flag is output only; nothing about the token reaches the `settings` table.
    #[test]
    fn token_saved_flag_is_never_persisted() {
        let db = Db::open_in_memory().unwrap();
        save(&db, &Settings { github_token_saved: true, ..Settings::default() }).unwrap();
        assert!(!load(&db).unwrap().github_token_saved);
        let keys: Vec<String> = db
            .with(|c| {
                let mut st = c.prepare("SELECT key FROM settings ORDER BY key")?;
                Ok(st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
            })
            .unwrap();
        assert!(keys.iter().all(|k| !k.to_lowercase().contains("token")), "{keys:?}");
        assert!(keys.contains(&"exploreEnabled".to_string()));
        // a planted row is ignored too
        db.with(|c| Ok(c.execute("INSERT INTO settings (key, value) VALUES ('githubTokenSaved', 'true')", [])?)).unwrap();
        assert!(!load(&db).unwrap().github_token_saved);
    }

    #[test]
    fn unknown_keys_and_bad_values_ignored() {
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO settings (key, value) VALUES ('recent', '[1,2,3]'), ('future', '{\"x\":1}'), ('theme', '\"dark\"'), ('sort', 'not json'), ('loadRemoteImages', '\"yes\"')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let s = load(&db).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.sort, SortSpec::default());
        assert!(s.load_remote_images);
    }

    #[test]
    fn wire_shape_is_camel_case_and_old_payloads_parse() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "theme": "system", "sort": {"key": "updated", "desc": true}, "sourceFilter": "all", "showDirtyNote": false,
                "loadRemoteImages": true, "fetchReleaseNotes": true, "fetchRepoInfo": true,
                "autoDownloadLatestRelease": true, "autoDownloadOnFetchAll": false, "downloadSizeCapMb": 1024,
                "exploreEnabled": true, "githubTokenSaved": false
            })
        );
        // a v0.1.0 frontend sends only the old four keys: the rest take the defaults
        let old: Settings =
            serde_json::from_value(serde_json::json!({"theme": "dark", "sort": {"key": "name", "desc": false}, "sourceFilter": "all", "showDirtyNote": true}))
                .unwrap();
        assert_eq!(old, Settings { theme: Theme::Dark, sort: SortSpec { key: SortKey::Name, desc: false }, show_dirty_note: true, ..Settings::default() });
    }
}
