-- v0.3.0 (M10a, D-118): Explore mode. Additive only, so a v0.2.0 DB keeps every row.
-- One row per GitHub API URL answered (the key never contains the token): the body, its ETag (If-None-Match on the
-- next request; a 304 only bumps `fetched_at`), the `Link` header (page counts) and LRU bookkeeping (100 MB cap).
CREATE TABLE cache_gh_api (
  url          TEXT PRIMARY KEY,
  etag         TEXT,
  link         TEXT,
  body         BLOB NOT NULL,
  size         INTEGER NOT NULL,
  fetched_at   INTEGER NOT NULL,
  last_used_at INTEGER NOT NULL
);
CREATE INDEX cache_gh_api_lru ON cache_gh_api (last_used_at);

-- Last rate-limit headers seen per GitHub bucket (`core`, `search`), so the quota shows right after a restart.
CREATE TABLE gh_quota (
  resource  TEXT PRIMARY KEY,
  quota     INTEGER NOT NULL,
  remaining INTEGER NOT NULL,
  reset_at  INTEGER NOT NULL,
  seen_at   INTEGER NOT NULL
);
