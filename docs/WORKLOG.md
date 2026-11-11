# RepoShelf — worklog

One entry per milestone: date, steps done, test counts, gate output summary, open items, handoff notes.

## M0 — Project layout (2026-11-11)

Steps: M0.1–M0.9 done; M0.10 (optional `npx tauri dev` smoke) skipped — mandatory in M4.14.

- Workspace (`resolver = "3"`, edition 2024, version 0.0.1) with `crates/rs-core`, `crates/rs-cli` (bin `rs`, usage
  only), `src-tauri` (package `reposhelf`, lib `reposhelf_lib`, features `e2e = ["rs-core/testkit"]`). All registry
  deps resolved and compiled on Rust 1.99.0 (per `Cargo.lock`): git2 0.21.0 (vendored, https), rusqlite 0.40.2,
  comrak 0.59.0, ammonia 4.2.4, ureq 3.5.1 (via `ghreleases`), tauri 2.12.5, tauri-plugin-dialog 2.8.4,
  tauri-plugin-opener 2.7.3, trash 5.3.0. Note: newer than the "PM-lock" versions in ARCH §1.1 (fresh resolve, same
  semver ranges) — M4 must verify Tauri APIs against 2.12.5.
- comrak 0.59 option names verified in the registry source (`parser/options.rs`): `extension.table`,
  `extension.tasklist`, `render.r#unsafe` (not `unsafe_`), and header ids are `extension.header_id_prefix:
  Option<String>` (there is no `header_ids` field). M2.10 must use these names.
- `tauri.conf.json`: CSP + devCsp exactly as ARCH §8, no `assetProtocol`; window `main` 1320x840 min 900x560 still in
  config (M4.3 moves it to code). Capability = `core:default` only.
- Icon: own SVG `docs/icon/reposhelf.svg` (three spines on a shelf, teal/indigo/amber on navy); `npx tauri icon`
  accepted the SVG directly; android/ios outputs deleted (desktop only).
- npm: highlight.js 11, @tanstack/react-virtual 3, vite 8.3.8, vitest 5.1.0, TS 7 (`tsc --noEmit -p .`).

Gate M0: `npm install` ok · `cargo build --workspace` ok (51 s first build) · `cargo test --workspace` 1 passed
(`smoke_markdown_strips_script`) · `npm run typecheck` ok · `npm test` 1 passed (hljs core smoke) · `npm run build`
ok · `npm run dev` served `/` with status 200 on 1450 (started hidden, killed by tree, no leftovers).
Note: the preview tool's launch config lookup uses the parent session's `C:\Dev\projects\.claude\launch.json`, so the
dev-server check ran via a hidden `npm run dev` instead; `repo-shelf\.claude\launch.json` exists for sessions rooted here.

Open items: `npm audit` reports happy-dom ≤ 21.0.3 (dev-only test DOM, GHSA-834v-8h66-3h79 / GHSA-c8p4-j97h-3vg8);
fix needs happy-dom 21 (outside the ARCH range `^20.14.5`) — decide before M5 tests parse untrusted HTML in it.

## M1 — Framework layer (2026-11-11)

Steps: M1.1–M1.12 done. New decisions D-32..D-35 (DECISIONS.md).

- `lib.rs` declares every ARCH §2 module (M2/M3 ones are one-line stubs; `ghreleases` behind its feature, `testkit`
  behind `cfg(any(test, feature="testkit"))`). `Shelf { db, dirs }`, `Shelf::open` (creates data dir only),
  `Shelf::in_memory`, `rs_core::now_secs()`. `render_smoke` kept until `markdown::render` replaces it in M2.10.
- `error.rs` ARCH §3.1 + `Unsupported` (`unsupported`) + helper `Error::not_found(what)`.
- `paths.rs` `Dirs`, `resolve_dirs` (pure), `db_file`, `long` (`\\?\` / `\\?\UNC\`, Windows only), `canonical`,
  plus `strip_verbatim`. Only `env_overrides_honoured` touches the process env (mutex-guarded).
- `db.rs` `Db` is `Clone` (shared `Arc<Mutex<Connection>>`) so `JobRunner` and `Shelf` share one connection;
  `user_version` read before any write pragma; future version → read-only handle; `prune_caches` returns rows deleted
  and is a no-op on a read-only DB.
- `validate.rs` per §5.2/§5.3 plus `oid` → `git2::Oid`, `external_url` → `InvalidUrl`; topics/homepage/metadata →
  `InvalidInput`. Raw-input checks for `..`, `%`, empty segments, userinfo and empty host run before `Url::parse`.
- `winpath.rs` mirrors libgit2 (`validate_dospath` incl. `CON.txt`/`con.tar.gz`, NT chars, trailing dot/space,
  `.git`/`git~1` with trailing dots/spaces). `storable_refs`: existing names win (identical name = update),
  then `prefer`, then input order; both outputs keep input order.
- `settings.rs` one JSON value per key (`theme`, `sort`, `sourceFilter`, `showDirtyNote`); unknown keys and
  unparsable values ignored.
- `jobs.rs` `JobKind/JobState/Progress/JobInfo` live here (not types.rs); `Gate`/`GatePermit`, `ComputeGate` (2),
  `CancelToken`, `JobCtx::new(cancel, emit)` + `JobCtx::noop()` (public, for M3 tests), `JobRunner::{get, join,
  running}` extras. Worker panics → Failed; an error after cancel → Cancelled.
- `types.rs` all §3.3 types + `Time`; `ImportPreview.token` (D-18, empty from rs-core), `RepoSummary.cloned_at`
  (D-19). `types::samples::all()` (cfg(test)) = one sample per type/variant — seed for the M4 golden.
- `testkit.rs` builder `repo(dir)` (lazy init, `.bare()` first, initial branch `main`, fixed signature
  `Fixture Author <fixture@example.com>`): `file`, `exec`, `symlink`, `gitlink`, `remove`, `commit(msg, time)`,
  `branch` (create + switch), `checkout`, `tag(name, annotated)`, `merge(other, msg, time)` (tree = staged state),
  `origin(url)`, `head()`. Writes objects/refs only — no work tree files. `register_raw` stores the canonical path,
  slug `local/<dir>` with `-2` on collision.

Gate M1: `cargo test -p rs-core` 56 passed (4 repeated runs, no flakes) · `cargo clippy --workspace --all-targets
-- -D warnings` clean (also `-p rs-core --all-targets` and `-p rs-core --features testkit`) · `cargo test
--workspace` ok · `npm run typecheck` ok · `npm test` 1 passed · `npm run build` ok. All tests use tempdirs or
in-memory DBs; nothing touches a real data dir.

Handoff for M2+:
- git2 0.21 accessors return `Result` (`shorthand()`, `name()`, `summary()` → `Result<Option<&str>>`): use lossy
  fallbacks, never `unwrap` in non-test code.
- `%TEMP%` here is an 8.3 path (`JOHNSM~1`); compare canonical paths only (`paths::canonical`).
- Non-bare testkit repos have no work tree files; the M2.12 immutability test needs a work tree → add a
  `write_worktree()` helper (or check out with git2) in M2.2.
- Wire shape D-32 applies to the M4 TS mirror and `jobs.kind_json`. Git converts LF → CRLF on checkout here
  (`core.autocrlf`): the M4 golden compare must normalise line endings or set `.gitattributes`.
- Comrak field names: see M0 entry.
- `types::samples` is `pub(crate)` + `cfg(test)` and lacks job types, `Settings` and some unit variants: M4.12 needs
  its own complete sample set in src-tauri (or a `testkit`-gated public copy).
- Non-bare testkit repos have no index either (only objects + refs): M2.1 dirty tests and M2.12 need index + work tree.
- D-34 relaxes ARCH's literal `^[A-Za-z]:` rule (`c:x` now passes `git_path`); safe only because git paths are never
  joined onto a filesystem path. Reverting is one line in `validate::is_drive_prefix`.
- M3: D-35 emits on every phase change — a transfer callback alternating Receiving/Resolving bypasses the throttle;
  emit Resolving only once receiving is complete. `winpath::refspecs` returns an empty list when every name was
  skipped — do not let libgit2 fall back to the configured wildcard.
- M4: on a read-only DB (`db.read_only()`) `JobRunner::load_persisted/submit` and `settings::save` fail — branch at
  startup. `resolve_dirs` passes relative env values through and `long()` ignores relative paths: make
  `target\scratch\m4` absolute.

## M2 — Data layer: git read model (2026-11-11)

Steps: M2.1–M2.14 done. New decisions D-36..D-43 (DECISIONS.md). D-34 confirmed by the lead.

- `repo.rs`: `summary(s, id)` (repos + repo_meta + topics → `RepoSummary`), `open_path`, `open` (missing →
  `RepoMissing`, not a repo → `RepoCorrupt`), `head_state`, `origin_url`, `is_fetchable` (https + `clone_url`),
  `detail` (HEAD commit/tree unreadable → `RepoCorrupt`; release count = version-like tag names; FF possible only for
  cloned), `dirty` (no `UPDATE_INDEX`, untracked counted).
- `refs.rs`: `resolve` per §3.5 (exact `refs/…`, heads, tags, remotes, `HEAD`, hex 4–40 via `find_object_by_prefix`,
  ambiguous → `InvalidRev`), `head_branch`, `list`, `branches`, `ahead_behind` (own capped/cancellable counting
  walks), `tags`.
- `tree.rs`: `lookup` walks tree entries by segment (D-39; lossy-name fallback), `subtree`, `list_dir` (sizes via
  `odb.read_header`, `path_raw` for non-UTF-8 names, `storable_here`), `sort_entries`, `entry_kind`.
- `blob.rs`: `sniff`, `image_mime`, `lang_hint` (table, then shebang), `decode`, `meta`, `text`, `bytes`; submodule
  URL from `.gitmodules`. `commits.rs`: `signature_of`, `summary_of`, `latest`, `cached_count`, `count`, `PageQuery`,
  `page` (D-37), `file_last_commit`. `diff.rs`: `DiffLimits`, `commit_detail`, `file_diff`, `is_generated` (D-40).
  `lastcommit.rs`: §6.1 walk with memoised subtrees/entries, `for_dir` (cached, D-43), `for_path`.
  `langs.rs` + `langs_table.rs` (≈ 100 languages/data formats, hljs ids, shebang table): `classify`, `compute`,
  `for_tree`. `markdown.rs` (D-38), `readme.rs` (`find_readme`, `render_readme` with root fallbacks, `render_file`,
  `render_plain`), `license.rs`, `releases.rs` (`is_version_tag`, `Ord for Version`, `list`).
- testkit: builder now keys raw byte paths (`file_raw`), `tag_msg`, `set_head`, `write_worktree` (checkout + index;
  `core.autocrlf=false` pinned per fixture), `packed` (mempack → one pack, D-41), `shelf`, `open`, `git_cli`
  (isolated from user/system git config). Fixtures: basic, wide_dir, long_history, merges (`MergesFx` named oids),
  binaries, security_readme, releases, big_tree, huge_commit, corrupt, missing; `fixtures(root)`.
  `tests/fixtures/security/{README.md,svg-with-script.svg}` written.
- `tests/immutability.rs` (D-42): basic fixture with work tree + index + uncommitted change; > 100 read calls over
  every rev and directory with the dirty note off and on; every file under the folder (incl. `.git/index`) has the
  same bytes and mtime, no file added or removed.
- rs-cli: `rs fixtures <dir>` (marker `.rs-fixtures`, rebuilds known fixture folders, refuses foreign non-empty
  dirs), `rs bench <dir>`, `rs inspect <repo-path> <cmd>`.

Gate M2: `cargo test -p rs-core` 101 unit + 1 integration passed (git CLI comparisons ran: `rev-list --left-right`,
`log --format=%H`, `log -1 -- <path>`, `show --numstat`, `diff --numstat`, `rev-list --count`, `cat-file`) ·
`cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` ok · fixtures built in
11.8 s (release) and rebuilt over the same dir · bench (release) all under target:

| measurement | ms | target |
|---|---|---|
| tree.list_dir 1,000 entries | 3.1 | 300 |
| lastcommit.for_dir 2nd visit (1st: 7.0) | 0.8 | 50 |
| commits.page skip 0 | 45.4 | 200 |
| commits.page skip 9,965 | 44.7 | 200 |
| langs.for_tree 50k files | 53.6 | 3,000 |
| diff.commit_detail 400 files | 2.0 | 1,000 |

Review fixes after the first M2 commit: the `data:` attribute filter now classifies the scheme with `Url::parse`
like ammonia does (tab/newline/control-char obfuscated `data:text/html` and `data:image/svg+xml` were otherwise
possible) — test `obfuscated_data_urls_are_dropped`; `langs::compute` descends trees explicitly because git2's
`Tree::walk` callback fails on non-UTF-8 directory names — test with `d\xe9/x.rs`.

`npm run typecheck` ok · `npm test` 1 passed · `npm run build` ok. `target\scratch\m2-bench` removed afterwards.

Open items: none for M2. `render_smoke` stays in `lib.rs` (lib.rs untouched per M1.1); `markdown::render` is the real
pipeline — drop the smoke fn whenever lib.rs is next edited.

Handoff for M3+:
- Reuse `repo::summary` (row loader), `repo::open_path`, `repo::origin_url`, `repo::is_fetchable`,
  `refs::head_branch`, `langs::compute`/`for_tree` (primary language), `commits::summary_of`.
- Signatures that differ from ARCH §3.4: `commits::latest(s, repo_id, repo, rev)` (D-36); `PageQuery.skip` /
  `CommitsPage.skip` semantics (D-37) — the M4 `commits_page` command should pass `skip` through and the UI uses the
  returned `skip` for "Older"/"Search older commits"; `lastcommit::for_path` backs `commits::file_last_commit`.
- `tree::lookup` accepts the lossy display path of a non-UTF-8 entry (first match), so M4 can pass `path` for
  browsing; `pathRaw` is informational.
- `testkit::git_cli(repo, args)` returns `None` when git is missing (tests then skip comparisons, message on stderr).
- PowerShell 5.1 gotcha hit in this milestone: `Get-Content -Raw` + `WriteAllText` re-encodes UTF-8 sources as
  cp1252 mojibake (`→` → `â†’`). Edit sources only with the Edit/Write tools (TASKS rule); repaired before commit.
- `testkit::fixtures(root)` asserts every target folder is absent; `rs fixtures` handles cleanup.
- Paging cost scales with history length, not offset (TIME sort walks everything first): 10k commits ≈ 45 ms for
  any page. Fine for NFR-P5; a 100k+ history would need a topo/first-parent mode later.
- Do not use git2 `Tree::walk` on repo content (fails on non-UTF-8 dir names); descend with `tree.iter()`.
- The rs-core self dev-dependency (D-42) enables `testkit` only for rs-core's own test targets; M9's marker check
  (D-26) still confirms shipped builds carry no testkit.

## M3 — Data layer: library, clone, fetch (2026-11-11)

Steps: M3.1–M3.10 done. New decisions D-44..D-49. Built over two sessions (the first stopped at a usage limit; its
work was parked on branch `wip-m3`, finished there and squash-merged onto `main`).

- `library.rs`: `list`, `by_slug` (exact, then case-insensitive), `recent(n)`, `touch_opened`, `preview_import`,
  `import`, `register_clone`, `relocate`, `set_metadata`, `remove`, `refresh_one`, `refresh_summaries`, `origin_slug`
  (lenient: https, ssh://, scp-like). Broken repos never error on refresh: status `missing`/`corrupt` (message)/`empty`.
- `clone.rs`: `preview`, `clone(&CloneUrl)`, `job(input)` (validate → clone → `register_clone`), `cleanup_partials`,
  `clone_from_unchecked` / `job_unchecked` (testkit cfg, D-14). The only git2 connect sites: `advertised` and
  `download` (both count `note_net_call`), `check_effective_url` (D-44), `remove_tree` (read-only packs, `\\?\`).
- `fetch.rs`: `fetch` (policy) → `fetch_from(FetchSource)`; `fetch_unchecked`, `fetch_all`, `fetch_all_unchecked`,
  `fast_forward_default`, `job`. Staging namespace cleared by a drop guard; `#[cfg(test)]` failure hook after download.
- `ghreleases.rs`: `parse_releases`, `merge_into`, `api_url`, `store`, `cached`, `releases_with_cached` (for M4
  `releases_list`), `fetch_github_notes` (one GET; the only network test is `#[ignore]`d `live_github_smoke`).
- `jobs.rs`: `shelf_worker(Arc<Shelf>)` and testkit-gated `shelf_worker_unchecked` (file:// routing) + a JobRunner
  test (clone → fetch through the runner; production worker refuses a file:// clone).
- testkit: `snapshot`, `snapshot_diff`, `read_everything` (moved from tests/immutability.rs), `net_calls`,
  `file_url`, `add_packed_refs`, `set_head_file`, fixtures `winnames`, `refclash`, `upstream_downstream`.
- `tests/network_boundary.rs` (M3.9): every M2 read function + every library function → `net_calls()` unchanged.

Review follow-up (second commit): `drop_long_refs` restored after confirming the DWIM lookup is local-transport only;
`clear_staging` also prunes the emptied `refs/reposhelf-staging` / `logs/refs/reposhelf-staging` directories (the
imported-repo fetch test asserts none is left).

Gate M3: `cargo test -p rs-core` 132 unit + 2 integration passed · `--features ghreleases` 135 + 2 passed, 1 ignored
(live smoke) · `cargo clippy --workspace --all-targets -- -D warnings` clean, also `-p rs-core --all-targets` (no
features) and `--features ghreleases` · `cargo test --workspace` ok · `npm run typecheck` / `npm test` / `npm run
build` ok. No test reaches the network: all clones/fetches use file:// upstreams through the unchecked seam; the
cancel test cancels during a real transfer (asserted); Recycle Bin untouched (trash is M4's `Trasher`, D-30).

M3.5 finding (D-44): the https→http downgrade is refused inside libgit2 (source-verified, WinHTTP redirects
disabled, non-https `Location` never applied); the reachable bypass is `url.*.insteadOf`, now refused in code.

Open items:
- D-45: libgit2's MAX_PATH checks ignore `core.longpaths`. Archive folders deeper than ~128 chars are refused
  before cloning (readable message); refs too long for a loose file are skipped and listed (FR-9). The default archive
  (`Documents\RepoShelf`) is far below the limit. Over `file://` (tests/e2e only) a very long advertised name fails
  the whole clone because the local transport looks every name up (verified in local.c).
- Renamed GitHub repos (API 301) fail release-note fetching by design (D-49).

Handoff for M4:
- Worker for `JobRunner::new`: `rs_core::jobs::shelf_worker(shelf.clone())`; under `cfg(feature = "e2e")` +
  `RS_TEST_FILE_CLONE=1` use `shelf_worker_unchecked` (keep the env literal inside cfg(e2e) code only).
- `clone::preview` = `clone_preview`; `fetch::fast_forward_default` = `fast_forward`; `library::recent` exists but
  the UI computes Recent from the list (D-28); `ghreleases::releases_with_cached` backs `releases_list` when the feature
  is on, `releases::list` otherwise; `releases_fetch_github` = `ghreleases::fetch_github_notes`.
- `library::remove` returns `Removed { path, source }`; trash only `Cloned` paths via the `Trasher`.
- Error mapping for M4.2: clone/fetch network failures arrive as `Error::Git` (class Net/Http/Ssl/Os) or
  `Error::Refused` (policy, empty repo, unstorable default, insteadOf, archive path too long); `fetch_all` returns one
  `Refused` listing failed repos; ghreleases HTTP errors are `Error::Network`.
- `startup`: `clone::cleanup_partials(&dirs.archive)` before the runner starts; `library::refresh_summaries` in the
  background with a `CancelToken`.
