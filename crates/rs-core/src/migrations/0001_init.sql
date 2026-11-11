-- 0001_init.sql (ARCH §4)
CREATE TABLE repos (
  id              INTEGER PRIMARY KEY,
  slug_owner      TEXT NOT NULL,                 -- origin owner or 'local'
  slug_name       TEXT NOT NULL,
  source          TEXT NOT NULL CHECK (source IN ('imported','cloned')),
  path            TEXT NOT NULL UNIQUE,          -- canonical, display form (no \\?\)
  is_bare         INTEGER NOT NULL,
  origin_url      TEXT, origin_host TEXT,
  default_branch  TEXT, head_tip TEXT,           -- tip oid at last summary refresh
  last_commit_at  INTEGER, primary_language TEXT,
  status          TEXT NOT NULL DEFAULT 'ok',    -- ok|missing|corrupt|empty
  status_message  TEXT,
  added_at        INTEGER NOT NULL, cloned_at INTEGER, last_opened_at INTEGER, last_fetched_at INTEGER,
  UNIQUE (slug_owner, slug_name)
);
CREATE TABLE repo_meta (                          -- FR-20, user-edited only
  repo_id     INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
  description TEXT, homepage TEXT, updated_at INTEGER NOT NULL
);
CREATE TABLE repo_topics (
  repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
  topic   TEXT NOT NULL, pos INTEGER NOT NULL,
  PRIMARY KEY (repo_id, topic)
);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);   -- JSON values
CREATE TABLE cache_commit_count (
  repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
  tip TEXT NOT NULL, count INTEGER NOT NULL, PRIMARY KEY (repo_id, tip)
);
CREATE TABLE cache_last_commit (                  -- FR-16 / NFR-P3
  repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
  tip TEXT NOT NULL, dir TEXT NOT NULL,          -- dir '' = root
  json TEXT NOT NULL,                            -- LastCommits (entries + embedded CommitSummary)
  created_at INTEGER NOT NULL, PRIMARY KEY (repo_id, tip, dir)
);
CREATE TABLE cache_languages (                    -- FR-19 / NFR-P6, keyed by root tree oid
  repo_id INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
  tree TEXT NOT NULL, json TEXT NOT NULL, PRIMARY KEY (repo_id, tree)
);
CREATE TABLE cache_gh_releases (                  -- optional FR-31
  repo_id INTEGER PRIMARY KEY REFERENCES repos(id) ON DELETE CASCADE,
  fetched_at INTEGER NOT NULL, json TEXT NOT NULL
);
CREATE TABLE jobs (                               -- FR-7: clone/fetch jobs; failures survive restart. Written only by jobs.rs
  id INTEGER PRIMARY KEY,                        -- = JobId
  kind_json TEXT NOT NULL,                       -- serialized JobKind (Clone{url} | Fetch{repoId} | FetchAll)
  title TEXT NOT NULL, state TEXT NOT NULL,      -- waiting|running|done|failed|cancelled
  error TEXT, result_json TEXT,                  -- JobResult (repoId, skipped refs, fetch outcome)
  created_at INTEGER NOT NULL, finished_at INTEGER
);
CREATE INDEX cache_last_commit_age ON cache_last_commit(created_at);
