//! Response cache and quota table of Explore (D-118): `cache_gh_api` (URL → body + ETag + `Link`, LRU, 100 MB) and
//! `gh_quota` (last rate-limit headers per bucket). Nothing here ever sees the token: the key is the URL, the
//! value is GitHub's public answer.

use crate::db::Db;
use crate::error::Result;
use crate::types::Quota;
use rusqlite::{OptionalExtension, params};

/// Cache cap (lead decision): the least recently used rows go first.
pub const CACHE_CAP_BYTES: u64 = 100 << 20;

#[derive(Debug, Clone)]
pub struct Entry {
    pub body: Vec<u8>,
    pub etag: Option<String>,
    pub link: Option<String>,
    pub fetched_at: i64,
}

/// The cached answer for `url` (marks it used).
pub fn get(db: &Db, url: &str) -> Result<Option<Entry>> {
    db.with(|c| {
        let e = c
            .query_row("SELECT body, etag, link, fetched_at FROM cache_gh_api WHERE url = ?1", [url], |r| {
                Ok(Entry { body: r.get(0)?, etag: r.get(1)?, link: r.get(2)?, fetched_at: r.get(3)? })
            })
            .optional()?;
        if e.is_some() {
            // best effort (a read-only library cannot be written)
            let _ = c.execute("UPDATE cache_gh_api SET last_used_at = ?2 WHERE url = ?1", params![url, crate::now_secs()]);
        }
        Ok(e)
    })
}

/// Stores / replaces an answer, then trims the table to `cap` bytes (never the row just written).
pub fn put(db: &Db, url: &str, e: &Entry, cap: u64) -> Result<()> {
    let now = crate::now_secs();
    db.tx(|tx| {
        tx.execute(
            "INSERT INTO cache_gh_api (url, etag, link, body, size, fetched_at, last_used_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(url) DO UPDATE SET etag = excluded.etag, link = excluded.link, body = excluded.body, size = excluded.size,
               fetched_at = excluded.fetched_at, last_used_at = excluded.last_used_at",
            params![url, e.etag, e.link, e.body, e.body.len() as i64, now],
        )?;
        trim(tx, cap, url)
    })
}

fn trim(tx: &rusqlite::Transaction, cap: u64, keep: &str) -> Result<()> {
    let total: i64 = tx.query_row("SELECT COALESCE(SUM(size), 0) FROM cache_gh_api", [], |r| r.get(0))?;
    let mut over = total - cap as i64;
    if over <= 0 {
        return Ok(());
    }
    let mut victims: Vec<String> = Vec::new();
    {
        let mut st = tx.prepare("SELECT url, size FROM cache_gh_api WHERE url <> ?1 ORDER BY last_used_at ASC, rowid ASC")?;
        let mut rows = st.query([keep])?;
        while over > 0 {
            let Some(r) = rows.next()? else { break };
            victims.push(r.get(0)?);
            over -= r.get::<_, i64>(1)?;
        }
    }
    for u in victims {
        tx.execute("DELETE FROM cache_gh_api WHERE url = ?1", [u])?;
    }
    Ok(())
}

/// A 304: the saved answer is still current.
pub fn touch(db: &Db, url: &str) -> Result<()> {
    db.with(|c| {
        let now = crate::now_secs();
        c.execute("UPDATE cache_gh_api SET fetched_at = ?2, last_used_at = ?2 WHERE url = ?1", params![url, now])?;
        Ok(())
    })
}

/// "Clear Explore cache": every saved answer. Returns the rows removed.
pub fn clear(db: &Db) -> Result<usize> {
    db.with(|c| Ok(c.execute("DELETE FROM cache_gh_api", [])?))
}

/// (bytes, rows) of the saved answers.
pub fn stats(db: &Db) -> Result<(u64, u32)> {
    db.with(|c| {
        let (b, n): (i64, i64) = c.query_row("SELECT COALESCE(SUM(size), 0), COUNT(*) FROM cache_gh_api", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok((b.max(0) as u64, n.max(0) as u32))
    })
}

pub fn save_quota(db: &Db, q: &Quota) -> Result<()> {
    db.with(|c| {
        c.execute(
            "INSERT INTO gh_quota (resource, quota, remaining, reset_at, seen_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(resource) DO UPDATE SET quota = excluded.quota, remaining = excluded.remaining,
               reset_at = excluded.reset_at, seen_at = excluded.seen_at",
            params![q.resource, q.limit, q.remaining, q.reset_at, q.seen_at],
        )?;
        Ok(())
    })
}

pub fn load_quota(db: &Db, resource: &str) -> Result<Option<Quota>> {
    db.with(|c| {
        Ok(c.query_row("SELECT quota, remaining, reset_at, seen_at FROM gh_quota WHERE resource = ?1", [resource], |r| {
            Ok(Quota { resource: resource.to_string(), limit: r.get(0)?, remaining: r.get(1)?, reset_at: r.get(2)?, seen_at: r.get(3)? })
        })
        .optional()?)
    })
}

/// A token change starts a new quota bucket: the old numbers belong to the other identity.
pub fn clear_quota(db: &Db) -> Result<()> {
    db.with(|c| Ok(c.execute("DELETE FROM gh_quota", []).map(|_| ())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: usize) -> Entry {
        Entry { body: vec![b'x'; n], etag: Some("\"e\"".into()), link: None, fetched_at: 0 }
    }

    #[test]
    fn put_get_touch_clear_and_stats() {
        let db = Db::open_in_memory().unwrap();
        assert!(get(&db, "u").unwrap().is_none());
        put(&db, "u", &Entry { link: Some("<x>; rel=\"next\"".into()), ..entry(5) }, CACHE_CAP_BYTES).unwrap();
        let e = get(&db, "u").unwrap().unwrap();
        assert_eq!((e.body.len(), e.etag.as_deref(), e.link.as_deref()), (5, Some("\"e\""), Some("<x>; rel=\"next\"")));
        assert!(e.fetched_at > 0);
        put(&db, "u", &entry(7), CACHE_CAP_BYTES).unwrap();
        assert_eq!(stats(&db).unwrap(), (7, 1), "replace, not add");
        db.with(|c| Ok(c.execute("UPDATE cache_gh_api SET fetched_at = 1", [])?)).unwrap();
        touch(&db, "u").unwrap();
        assert!(get(&db, "u").unwrap().unwrap().fetched_at > 1);
        assert_eq!(clear(&db).unwrap(), 1);
        assert_eq!(stats(&db).unwrap(), (0, 0));
    }

    #[test]
    fn lru_trim_keeps_the_newest_row_and_the_recently_used_ones() {
        let db = Db::open_in_memory().unwrap();
        for (i, u) in ["a", "b", "c"].iter().enumerate() {
            put(&db, u, &entry(40), 1_000).unwrap();
            db.with(|c| Ok(c.execute("UPDATE cache_gh_api SET last_used_at = ?2 WHERE url = ?1", params![u, 100 + i as i64])?)).unwrap();
        }
        // "a" is the oldest, but reading it makes it the newest
        assert!(get(&db, "a").unwrap().is_some());
        db.with(|c| Ok(c.execute("UPDATE cache_gh_api SET last_used_at = 500 WHERE url = 'a'", [])?)).unwrap();
        put(&db, "d", &entry(40), 100).unwrap(); // cap 100 → room for two 40-byte rows (+ the new one kept)
        let urls: Vec<String> = db
            .with(|c| {
                let mut st = c.prepare("SELECT url FROM cache_gh_api ORDER BY url")?;
                Ok(st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
            })
            .unwrap();
        assert_eq!(urls, vec!["a", "d"], "b and c (the least recently used) went");
        // a single answer bigger than the cap is kept (it is the one just fetched)
        put(&db, "big", &entry(500), 100).unwrap();
        assert!(get(&db, "big").unwrap().is_some());
        assert_eq!(stats(&db).unwrap().1, 1);
    }

    #[test]
    fn quota_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert!(load_quota(&db, "core").unwrap().is_none());
        let q = Quota { resource: "core".into(), limit: 60, remaining: 3, reset_at: 99, seen_at: 5 };
        save_quota(&db, &q).unwrap();
        save_quota(&db, &Quota { remaining: 2, ..q.clone() }).unwrap();
        assert_eq!(load_quota(&db, "core").unwrap().unwrap().remaining, 2);
        clear_quota(&db).unwrap();
        assert!(load_quota(&db, "core").unwrap().is_none());
    }
}
