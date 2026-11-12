-- v0.2.0 (M11, D-99): GitHub metadata. Additive only, so a v0.1.0 DB keeps every row.
-- ETag of the last releases answer (If-None-Match on the next fetch; 304 keeps `json`).
ALTER TABLE cache_gh_releases ADD COLUMN etag TEXT;

-- Fetched About values (GET /repos/{o}/{r}), kept apart from the user's own `repo_meta` / `repo_topics`.
-- Not a `cache_*` table: "Clear cache" keeps it (it is shown on the dashboard).
CREATE TABLE gh_repo_info (
  repo_id     INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
  etag        TEXT,
  fetched_at  INTEGER NOT NULL,
  description TEXT,
  homepage    TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  html_url    TEXT
);
