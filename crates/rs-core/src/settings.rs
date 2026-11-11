//! Typed view over the `settings` table (key → JSON value). FR-3, FR-33, D-16, D-28 (no `recent`).

use crate::db::Db;
use crate::error::Result;
use crate::types::{SortSpec, SourceFilter, Theme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub sort: SortSpec,
    pub source_filter: SourceFilter,
    /// D-16: off by default.
    pub show_dirty_note: bool,
}

const K_THEME: &str = "theme";
const K_SORT: &str = "sort";
const K_SOURCE_FILTER: &str = "sourceFilter";
const K_SHOW_DIRTY_NOTE: &str = "showDirtyNote";

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
            _ => {}
        }
    }
    Ok(s)
}

pub fn save(db: &Db, s: &Settings) -> Result<()> {
    let pairs = [
        (K_THEME, serde_json::to_string(&s.theme)),
        (K_SORT, serde_json::to_string(&s.sort)),
        (K_SOURCE_FILTER, serde_json::to_string(&s.source_filter)),
        (K_SHOW_DIRTY_NOTE, serde_json::to_string(&s.show_dirty_note)),
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
        };
        save(&Db::open(&f).unwrap(), &want).unwrap();
        assert_eq!(load(&Db::open(&f).unwrap()).unwrap(), want);
        // saving again updates in place
        let again = Settings { theme: Theme::Light, ..want };
        let db = Db::open(&f).unwrap();
        save(&db, &again).unwrap();
        assert_eq!(load(&db).unwrap(), again);
    }

    #[test]
    fn unknown_keys_and_bad_values_ignored() {
        let db = Db::open_in_memory().unwrap();
        db.with(|c| {
            c.execute("INSERT INTO settings (key, value) VALUES ('recent', '[1,2,3]'), ('future', '{\"x\":1}'), ('theme', '\"dark\"'), ('sort', 'not json')", [])?;
            Ok(())
        })
        .unwrap();
        let s = load(&db).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.sort, SortSpec::default());
    }

    #[test]
    fn wire_shape_is_camel_case() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(v, serde_json::json!({"theme": "system", "sort": {"key": "updated", "desc": true}, "sourceFilter": "all", "showDirtyNote": false}));
    }
}
