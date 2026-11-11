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

## M4 — IPC commands + tests (2026-11-11)

Steps: M4.1–M4.15 done. New decisions D-50..D-58. Tauri APIs checked against the 2.12.5 registry sources
(`WebviewWindowBuilder::{data_directory, on_navigation, on_new_window}`, `NewWindowResponse::Deny`, `ipc::Channel`,
`ipc::Response`, `tauri::is_dev()`), dialog 2.8.4 (`blocking_pick_folder`, `blocking_save_file`), opener 2.7.3
(`open_url`, `open_path`, `reveal_item_in_dir`, `Builder::open_js_links_on_click`), trash 5.3.0.

- `state.rs`: `AppState { shelf, jobs, requests: CancelRegistry, compute, subscribers, import_tokens, trasher, picker,
  opener, test_file_clone, startup_refresh }` managed as `Arc<AppState>`; `AppState::new` (production worker) and
  `with_worker` (tests). `CancelRegistry` (guard unregisters on drop; early cancels remembered 30 s), `Subscribers`
  (one channel per webview label, D-53), `friendly_worker` (D-52), `make_worker` (file:// seam only under cfg(e2e)),
  `Trasher`/`RealTrash`, `Picker`, `Opener`, `TauriShell` (dialog + opener), import tokens (128-bit hex, 10 min, one
  use, ≤ 32 kept).
- `ipc_error.rs`: `IpcError { code, message }`; `friendly()` — auth / 401 / 403 / 407 / auth-replay loop → credentials
  text, 404 → "Repository not found on <host>", other HTTP status, Net/Http/Ssl/Os/Callback → "Could not reach
  <host>: …", `Network` → same, insteadOf refusal → plain wording (strings from vendored libgit2 `winhttp.c`: without
  a credentials callback a 401 ends as "request failed with status code: 401").
- `cmd/{app,library,dialogs,tree,blob,commits,refs,releases,jobs}.rs`: all 47 commands of ARCH §5.1, each a
  `#[tauri::command] async fn` over a plain `*_impl(&AppState, …)` (D-51). Compute gate on last commits, counts,
  languages, file last commit, ahead/behind, library refresh. `blob_bytes` returns `tauri::ipc::Response`.
- `lib.rs`: startup (dirs created, `cleanup_partials`, `Shelf::open`, `load_persisted`, state, window in code with the
  navigation guard + `data_directory`, background `prune_caches` (head tips) + `refresh_summaries` → event
  `library-refreshed`); on exit the startup refresh and all jobs are cancelled. `RS_TEST_FILE_CLONE` read once, only
  in cfg(e2e) code (a test asserts it).
- Config: `tauri.conf.json` `windows: []`, CSP/devCsp unchanged (= ARCH §8, no `assetProtocol`); capability
  `core:event:default` only (D-54). `Cargo.toml`: features `default = ["ghreleases"]`, `e2e`; dev-deps rs-core
  (testkit) + tempfile; `rs_core::types::samples` public under testkit (D-57).
- Tests (`src-tauri/src/tests.rs`, 41 in the lib incl. state/ipc_error units): hostile table for all 16 rev-taking
  and 10 path-taking argument positions (also with an unknown repoId), bad oids for 3, unknown repoId → `notFound` for
  every repo command, requestId validation, `open_external` refusals (javascript:, file:, ftp:, userinfo, mailto:,
  data:, bare host, UNC), `blob_bytes` 5 MB cap (6 MB refused, 1.5 MB ok), import token (cancel, bad, reuse,
  expired, duplicate, non-repo), remove (imported → no trash; cloned in archive → fake trasher got the path; cloned
  outside → not trashed), save-as, every browse/commit/branch/tag/release command on fixtures, jobs (subscribe
  snapshot + channel events, waiting cancel + retry, dismiss, friendly job errors), `releases_fetch_github` with the
  feature (canned cache merged, non-GitHub origin refused with 0 network calls) and `unsupported` without
  (`--no-default-features`, run once), e2e build: file:// clone + fetch through the commands, refused without the
  flag. Registration test: `generate_handler!` list == ARCH §5.1 names (47). Navigation guard unit test.
- M4.12: `src/ipc/types.ts` (every IPC type + `IpcError`, `JobInfo`, `Settings`, `RemoveResult`,
  `AheadBehindEntry`); golden `src/ipc/__samples__/ipc-samples.json` (91 samples, keys `Type` / `Type.variant`);
  `ipc_samples_match_the_golden` compares parsed JSON (`UPDATE_SAMPLES=1` rewrites); `src/ipc/types.test.ts` checks
  every sample against a shape typed as `Shape<T>` (exact keys enforced by tsc) and that every enum value / union
  variant is sampled.
- M4.14 live check (scratch `target\scratch\m4-dev` / `m4-exe`, CDP via `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=
  --remote-debugging-port`): `npx tauri dev` rendered the M0 page on the main display; `app_info` showed the scratch
  dirs, `tree_list` with `../x` → `invalidPath`, `open_external('javascript:…')` → `invalidUrl`, `jobs_subscribe`
  Channel received waiting → running → done for a no-network `fetch_all_start`, event listen allowed,
  `plugin:window|set_title` denied, `window.open` and a foreign `location.href` blocked, `<RS_DATA_DIR>\webview`
  created. Console: no CSP messages; only Vite's dev `favicon.ico` 404. Positive control showed `devCsp` is not
  enforced in dev (D-56), so the same probe ran on a `npx tauri build --no-bundle --debug` exe: zero console
  messages before the control, the injected inline script blocked (`script-src-elem`). Both runs killed by
  `taskkill /T /F`; leftovers 0 (processes matched by command line); scratch dirs removed.

Gate M4: `cargo test -p rs-core --features ghreleases` 137 passed (1 ignored live smoke) · `cargo test -p reposhelf`
41 passed · `--features e2e` 41 passed · `--no-default-features` 41 passed · clippy `-D warnings` clean for the
workspace, `-p reposhelf --features e2e` and `--no-default-features` · `npm run typecheck` ok · `npm test` 95 passed
(types.test.ts against the committed golden) · `npm run build` ok · `cargo test --workspace` 178 passed ·
`cargo build -p reposhelf` and `--features e2e` ok; the plain debug exe does not contain `RS_TEST_FILE_CLONE`.
No `src/components` / `src/screens`. **Backend done.**

Review follow-up (second commit): the insteadOf rewording now matches only the single-repo refusal (a fetch-all
failure list containing it keeps every line; test added). Checks: `cargo tree -e normal` shows rs-core with
`default,ghreleases` only (no testkit in the app graph, D-26); a throwaway field added to `RemoveResult` in
`types.ts` failed `npm run typecheck` in `types.test.ts` (reverted); `%LOCALAPPDATA%\dev.schwi.reposhelf` and
`Documents\RepoShelf` do not exist (existence check only). Not exercised live: the real dialogs, opener and Recycle
Bin (`TauriShell`, `RealTrash`; fakes in tests) and repo-backed commands in the window (empty scratch shelf) — M8.

Open items:
- `fetch_all` failures list rs-core's raw per-repo text (`git: …`) inside one message (D-52).
- Dev-mode favicon 404 (Vite serves no `favicon.ico`); add an icon link in M5.
- D-45 / D-49 limits from M3 still apply.

Handoff for M5 (frontend needs):
- Invoke names are snake_case, args camelCase (`repoId`, `requestId`, `jobId`); errors reject with `IpcError
  { code, message }` (`code` per `types.ts` `IpcErrorCode`, plus `internal` for a crashed blocking task).
- Cancellable commands take `requestId` (1–64 chars `[A-Za-z0-9_-]`): `library_refresh`, `tree_last_commits`,
  `commit_count`, `languages`, `file_last_commit`, `branches_ahead_behind`. `request_cancel` may arrive before the
  request starts (remembered 30 s). A cancelled request rejects with code `cancelled`.
- `jobs_subscribe({ channel })`: use `@tauri-apps/api/core` `Channel` (raw messages arrive wrapped as
  `{ message, index }`; the API class unwraps). One channel per webview; resubscribe after reload; snapshot returned.
- `blob_bytes` resolves to an `ArrayBuffer`; build the `Blob` with the MIME from `blob_meta` (`kind.type ===
  'image'` → `kind.mime`, incl. `image/svg+xml`). `blob_text.text` is null above 5 MB → "Save a copy" only.
- `commits_page({ repoId, rev, path?, since?, skip, tip? })`: pass the first page's `tip` and the returned `skip` for
  "Older"; `path: ''` means no filter; `since` = previous tag for "commits since".
- `import_pick()` → `ImportPreview | null` → `import_confirm({ token })`; `repo_locate` → `RepoSummary | null`;
  `blob_save_as` → written path or null; `repo_remove` → `{ trashed }` (confirm dialog is the UI's job).
- `open_external` accepts only http/https without userinfo (show `mailto:` as text). `clone_preview` validates as the
  user types (pure, no network); `clone_start` returns the JobId or rejects `invalidUrl`.
- Event `library-refreshed` (payload `RepoSummary[]`) fires once after startup; the library store may also call
  `library_refresh` itself.
- Capability is `core:event:default` only: no `@tauri-apps/api/window|webview|app` calls from the UI.

## M5 — Frontend shell + mock (2026-11-11)

Steps: M5.1–M5.13 done. happy-dom → `^21` first (D-59; `npm audit` 0, 95 earlier tests green). New decisions
D-59..D-67.

- `src/router/`: `routes.ts` (pattern table, one encoded `:ref`, `*path`), `router.ts` (`parse`, `href`, `withQuery`,
  `navigate` via `pushState({key})` keeping `location.search`, popstate + hashchange, scroll memory with
  `markRendered()` retry loop, mouse 3/4 + Alt+←/→, `locationText`/`copyLocation`), `Link.tsx`, `useRoute.ts` (D-64).
- `src/ipc/`: `commands.ts` (47-command table, D-60), `bridge.ts` (only `@tauri-apps/api` importer: `core` invoke +
  Channel, `event` listen; mock via dynamic import, D-61; `setBackendForTests`), `errors.ts` (`toIpcError`,
  `isCancelled`, `errorText`), `api.ts` (one function per command, `newRequestId`, `jobsSubscribe`), `query.ts`
  (`useQuery`, `qk`, LRU 200, `stable` oid-keyed entries, `invalidate` on focus/job done, cancel on unmount, D-65).
- `src/ipc/mock/`: `switches.ts`, `fixtures/` (gen = deterministic oids/people/history; `parsekit` imported Rust with
  README table/code/relative svg image/remote badge + branches/tags/LFS/submodule/symlink/binary/png/exec/generated
  file, `lumen` cloned TS with releases + GitHub notes + fast-forward, `quill` cloned codeberg.org, `beacon` imported
  Go detached, `scratchpad` empty, `telemetry` missing, `hostile-readme` security README, `harbor-cli`; `?huge`:
  1,000-entry `generated/`, 10k commits, 400-file commit with diffs deferred), `repoModel.ts` (refs/rev resolution,
  tree, blobs, tip+skip paging, path filter, since, diffs, ahead/behind, tags, releases; errors as rs-core),
  `cloneUrl.ts` (port of `validate::clone_url` messages), `mockJobs.ts`, `index.ts` (typed handler table, `?slow`
  600 ms, 180 ms on compute commands, requestId validation + cancel (also early), `library-refreshed` once).
- `src/state/`: `store.ts`, `settings.ts`, `theme.ts` (resolved `data-theme`, system via matchMedia, localStorage
  hint against a flash), `library.ts` (list → background refresh → event; `filterRepos`, `sortRepos`, upsert),
  `jobs.ts` (subscribe once, merge, done/cancelled hide after 5 s + `jobs_dismiss`, done → `invalidate` + library
  refresh), `recent.ts`, `ui.ts` (app dialog store).
- `src/styles/`: `tokens.css` (all ARCH §7.5 groups + header tokens, D-63), `base.css` (focus-visible rings),
  `layout.css` (1280 container, header, repo header, tabs, columns stack < 1000 px), `components.css`, `code.css`,
  `diff.css` (stub), `markdown.css`; Inter / JetBrains Mono from `@fontsource-variable`.
- `src/hl/`: `languages.ts` (48 ids, ext/filename/shebang/fence aliases), `loaders.ts`, `splitLines.ts`, `engine.ts`,
  `hl.worker.ts`, `highlighter.ts` (`highlight`, `highlightBlock`, plain fallback without Worker) (D-62).
- `src/markdown/`: `linkRewrite.ts` (inert template; rsrepo links → blob routes, rsrepo images → blob_bytes,
  remote images → "Remote image: host" chip, only http/https/mailto/rsrepo/#fragment survive, handlers/styles/frames
  stripped), `MarkdownBody.tsx` (blob: URLs revoked on unmount, code fences → worker, all clicks intercepted).
- `src/components/ui/`: Button, Dropdown (menu keyboard), Dialog (portal, focus trap, Esc, focus return), Popover +
  `useDismiss`, `bits.tsx` (Badge, Chip, Counter, Shimmer, Spinner, Avatar initials, Empty), RelativeTime (one
  shared minute timer), CopyButton, ConfirmExternalLink (+ `confirmExternal` store, mailto shown as text),
  WindowList (`useWindowVirtualizer` > 500 rows, D-31), `index.ts`. `src/util/format.ts`, `hash.ts`.
- `src/components/layout/`: AppMark (own icon), GlobalHeader (brand, RepoJump with Recent + `/`, AddMenu, JobsButton
  + popover with progress/Cancel/Retry/Dismiss, ThemeToggle cycling System → Light → Dark, settings link),
  RepoLayout + `repoContext` (slug → repoId → detail → `rev_resolve`; commit count for the tab), RepoHeader
  (SourceBadge, Fetch for clones, Open folder, "…" with Copy location), TabBar (icons + counts), RepoBanner
  (missing + Locate…, corrupt), Sidebar primitives (Columns, Sidebar, SidebarSection, Box).
  `src/components/dialogs/`: Import / Clone skeletons + DialogHost (D-66).
- `src/App.tsx`: RouteView (every ARCH §7.1 route; repo routes inside RepoLayout keyed by slug; `markRendered` in a
  layout effect). `src/screens/{Dashboard,RepoCode,FileView,Commits,Commit,Branches,Tags,Releases,Settings,NotFound}`
  placeholders (+ `screens/placeholder.tsx`). `index.html` icon link → `public/favicon.svg` (copy of the app icon;
  fixes M4's dev favicon 404). `tools/shots/mock-shots.md` (24 rows, M5–M7).

Tests (430 TS, 11 files): router 32 (round-trips for every pattern, `feature%2Fx`, `?L=`, settings vs repo,
malformed → notFound, history/scroll restore only after `markRendered`, Alt+Left / mouse 4), query 6 (cache hit,
cancel on unmount → `request_cancel` and never an error, no cancel for plain queries, invalidate ref- vs oid-keyed,
errors kept, LRU), mock 14 (each switch, error shapes, refs, early cancel, paging tip+skip/path/since, blob bytes,
README shape, jobs snapshot/cancel/retry/dismiss, event), state 12 (filter "pars", sort 3 keys × direction, recent,
theme, jobs auto-hide 5 s with fake timers), contrast 202 (99 pairs × 2 themes + groups + focus ring), hl 34
(splitLines nested, languages, shebang, engine chunks/provisional first chunk/too large/cancel), linkRewrite 9,
format 19, App 7 (header, `?empty`, repo tabs with counts, not found, missing banner + empty repo, scroll restore on
back after render, `/` focus), types 94 + smoke 1 (M4).

Gate M5: `npm run typecheck` ok · `npm test` 430 passed · `npm run build` ok, initial JS = `index-*.js` 286,101 bytes
(< 1.5 MB), 48 language chunks + `hl.worker-*.js` + `mock-*.js` separate, none referenced by `dist\index.html` ·
`cargo test --workspace` 178 passed. Earlier gates re-run (Rust unchanged): `cargo build --workspace`,
`cargo test -p rs-core` (+ `--features ghreleases` 135 + 1 ignored), `cargo test -p reposhelf` 41 (+ `--features e2e`
41), clippy `-D warnings` for the workspace and `-p reposhelf --features e2e` — all exit 0. M2 bench re-run
(`rs fixtures` + `rs bench` on `target\scratch\m2-bench`, exit 0, all six under target: list_dir 3.2 ms, lastcommit
2nd visit 0.8 ms, page skip 0 / 9,965 44 ms, langs 50k 56 ms, 400-file diff 1.9 ms; scratch dir removed).

Review fixes (second commit, after cd7c698; the lead's 16ba3ff (M10 plan only) sits between — not implemented):
- Mock checked against the Rust sources and fixed: `tree_list.readme` is the entry name (not a path); `blob_*` on a
  directory → `notFound` "file not found", nothing there → "path not found"; `blob_bytes` > 5 MB → `refused`;
  refs sorted like `refs::list` (default branch first, then byte order; remotes/tags by name); `branches_list` =
  local + `origin/*`, default first, newest commit, name; ambiguous short oid → `invalidRev`; `commit_detail` /
  `commit_file_diff` / `commits_page.tip` need a full 40-hex oid (`invalidRev`, as `validate::oid`); `langHint`
  filled (hljs id from path/shebang); releases: prerelease = numeric version followed by `-`, title = first
  annotation line unless empty/equal to the tag; jobs: cancel of an unknown/finished job → `false`, cancelled jobs
  carry no error text, retry of a non-failed job → `refused`, retry replaces the old row, dismiss leaves
  waiting/running jobs alone. Verified by name in a new mock test. Not verified against Rust: annotated-tag notes
  come from the fixture's `notesHtml` (real: rendered annotation body); `lastCommits`/path-filter choices are
  synthetic.
- Light-theme header focus ring was ~2.4:1 on the navy header: new `--header-focus-ring` (both themes) for
  `.gh :focus-visible` and the search field (popovers keep `--focus-ring`); contrast test adds ≥ 3:1 checks for
  both rings (canvas, subtle, overlay, header, header input) and the banner text pairs (fg-default / fg-muted /
  danger on attention-subtle and danger-subtle).
- Jobs store: the `jobs_subscribe` snapshot no longer overwrites channel updates that arrived first (test).
- Built `dist` served by a hidden `vite preview` on 4173: README code highlighted by `hl.worker-*.js`, logo via
  blob: URL; server killed by tree, 4173 and 1450 free, 0 leftover node processes.
- Final gate: typecheck ok · `npm test` 452 passed · build ok, initial JS 287,300 bytes · `cargo test --workspace`
  178 passed.

M5.13 browser check (hidden `npm run dev`, Browser pane, D-67): `#/` (dark, system), `#/northwind/parsekit` light +
dark (header, tab counts 148/4/4/3, README: table, highlighted toml/rust via the worker, svg logo via blob: URL,
remote badge chip), README link → blob route, external link → confirm dialog, Alt+Left back restored scrollY 596,
`#/sandbox/hostile-readme` (only blob:/data:png images, remote image chip, `javascript:`/`data:`/`//host` hrefs
removed, 0 scripts, 0 inline handlers), `?clonefail` clone: http URL rejected with the backend wording, valid
shorthand → jobs popover progress → "Could not reach github.com…" + Retry, `?slow` shimmer then content, `?empty`
empty shelf, `#/settings`, encoded `feature%2Fstreaming` ref, missing repo banner. Console: no errors or warnings.
Dev server killed by tree (5 processes), port 1450 free, no leftover node processes.

Open items:
- Clone dialog error stays until the next submit (live validation is M6.3).
- The mock ships in `dist` as an unreachable chunk (D-61).
- The mock's settings (theme Dark) and the theme hint stay in the `localhost:1450` localStorage of the Browser pane
  from the M5.13 check; the header toggle changes them back.
- Not exercised by hand: mouse buttons 3/4 (unit test only). `sed -i` was used once to tick TASKS boxes / fix counts
  (diff checked: line-level only).
- Pane screenshots are the only visual record (no files saved); M8 takes the real-app light/dark pass.

Handoff for M6/M7:
- Screens import `api.ts` + `useQuery(qk(cmd, …args), fn, { stable, cancelable })`; pass `resolved.commit` from
  `useRepo()` for oid-keyed (stable) queries; cancellable ones take the `requestId` argument of `fn`.
- `useRepo()` gives summary, detail, `refName` (decoded), `refFromRoute`, `resolved`, `revError`; build links with
  `href(name, params, query)` and `<Link to>`; `withQuery(route, q)` for same-route query changes (`?L=`, `?page=`).
- Replace the placeholders in `src/screens/*/index.tsx` (default export, `{ route }` prop where needed); remove
  `screens/placeholder.tsx` when the last one is gone. RepoCode currently mounts MarkdownBody directly — M6.5 moves
  it into ReadmeCard.
- Long lists: `WindowList` (threshold 500). Highlighting: `highlight(text, languageFor(path, firstLine), onChunk)`
  for the file view (chunks may replace earlier lines), `highlightBlock` for small blocks.
- External links only via `confirmExternal(url)`; app dialogs via `openDialog('import' | 'clone')`; new dialogs go
  in `src/components/dialogs/`. Use existing tokens/classes (`box`, `box__header`, `box__row`, `btn`, `badge`,
  `chip`, `counter`, `banner`, `menu`, `popover`) and avoid `style={{}}` except computed widths.
- Mock fixtures: extend `src/ipc/mock/fixtures/*` (M6.8); `RepoDef.files` already has every placeholder kind.

## M6 — Screens A: Dashboard, Code tab, File view, dialogs (2026-11-11)

Steps M6.1–M6.9 done (one subagent, resumed once after a usage-limit stop; no work lost). Visual target: github.com's
repository page arrangement with our own tokens.

- M6.1 `src/util/paths.ts` (segments, parent, join, extension, markdown), `src/util/lang-colors.ts` (12 own slots,
  D-70), `src/util/topics.ts` (mirror of `validate::topics`); tokens `--lang-*`, `--code-line-selected(-edge)`.
- M6.2 `src/components/dashboard/` (RepoToolbar: `?q=` filter with replace + keepScroll, Type + Sort menus persisted,
  New → Import/Clone; RepoList via WindowList; RepoRow: name, source badge, 2-line description, topic chips →
  `?q=`, language dot, origin host/owner, "Updated …"; MissingRow: Locate… / Remove from shelf; DashSidebar: Recent 5,
  archive count + size + dir, "Fetch all N clones"; EmptyState) + `screens/Dashboard`.
- M6.3 dialogs: Import (pick → summary with bare/work tree, origin, duplicate link, backend refusal → confirm →
  opens the repo), Clone (250 ms debounced `clone_preview`, stale answers dropped, host + repo + URL + target),
  Edit About (description counter, website, topics chip input with client rules), Remove (imported: unregister only;
  cloned: Recycle Bin), Fetch confirm (ARCH §0.2 wording, git dir path).
- M6.4 RepoHeader: origin chip (confirmed external link), Fetch for clones and fetchable imported repos (confirm),
  job result "N new commits[, N new tags]" / failure text, Fast-forward when `canFastForward`, "…" → Remove (then
  home). RepoBanner: Locate… + Remove from shelf. `state/repoCache.ts` keeps store + slug/detail cache in step (D-69).
- M6.5 `src/components/code/`: RefSwitcher (Branches | Tags tabs, filter, default badge, check on current, tags as
  `refs/tags/<name>`, detached short oid, keeps the path), CountsLinks, CodeMenu (Copy clone URL, Open folder, bare
  note, path), Breadcrumbs + Copy path, LatestCommitBox (root: `commit_latest` + shared `commit_count`; subdir:
  `file_last_commit` + History, D-74), FileTable (`..`, EntryIcon, lazy LastCommitCell shimmer, virtual > 500),
  ReadmeCard (outline popover, sticky header, MarkdownBody), AboutSidebar (gear → Edit About, homepage, topics,
  origin (ssh origins as text + copy), license, imported/cloned lines, counts, ReleasesBox, LanguagesBox with
  LangBar + LangList); edge states empty repo / ref not found / path not found; "Filter files…" box for the folder.
- M6.6 FileTable badges "not checked out on this PC" (imported work tree, `storableHere=false`) and "non-UTF-8 name";
  file view notices for both (D-71).
- M6.7 `src/components/file/`: CodeViewer (fixed rows, worker chunks swapped in, click / shift-click `?L=`, two-frame
  anchor scroll only on fresh visits, virtual > 500 with the width from the longest line, D-72/D-73), `lines.ts`,
  FileCommitHeader, FileToolbar (Preview / Code / Raw segmented, stats, Copy link `#L10-L20`, Copy raw, Save a copy),
  ImagePreview (blob: URL on a checkerboard, revoked, natural size), Placeholder (binary, > 5 MB, LFS, submodule,
  symlink with in-repo link); `screens/FileView` (meta first, directory → tree redirect, lossy / large notices).
- M6.8 mock: FileSpec `bigText` (declared size, lazy text), `hugeText` (never materialised), `lossy`; RepoDef
  `unstorable`, `rawNames`; parsekit gains `data/readings.csv` (1.25 MB, 26k lines), `data/export.log` (6.4 MB),
  `docs/legacy-notes.txt`, `odd/a:b.txt`, `odd/CON.txt`, `odd/caf�.txt`; `import_pick` cycles new → not a repo →
  duplicate. `tools/shots/mock-shots.md` rows 18–19a.
- Router: `navigate(…, { keepScroll })` (D-68). Library store generation guard (D-75, fixed a flaky App test).
- CSS: `src/styles/screens.css` (imported in main.tsx).

Tests: 522 TS in 19 files (+70): mutations 5 (Edit About save → sidebar + dashboard row, invalid topic refused,
header Remove → dashboard without the repo, imported Fetch confirm → "3 new commits", Fast-forward), plus paths/lang-colors 6, topics 3, RepoList 5 (filter "pars", replace history, sort
3 orders, Locate, Remove dialog), CloneDialog 10 (8 rejected forms show the backend message, valid URL shows host +
target, stale answers dropped), FileTable 5 (order, `..`, shimmer then messages, NFR-W3/R-7 badges, folder link),
RefSwitcher 3 (filter + default + current, tag href `refs%2Ftags%2Fv0.3.0` keeps path, display names), CodeViewer 5
(`?L=10-20`, click + shift-click, every placeholder + lossy + not-checked-out + dir redirect, raw + markdown tabs),
contrast +28 (12 hl + 2 text pairs on the selected-line background, both themes), App test updated for the real Code tab. Shared harness `src/test/appHarness.tsx`.

Gate M6 (final, after the review commit): `npm run typecheck` ok · `npm test` 522 passed · `npm run build` ok, initial
JS `index-*.js` 388,476 bytes (< 1.5 MB) · `cargo test --workspace` 178 passed (41 + 135 + 1 ignored + 2). Earlier
gates re-run (Rust unchanged): `cargo build --workspace`, `cargo test -p rs-core --features ghreleases` (135 + 1
ignored + 2), `cargo test -p reposhelf` 41, `--features e2e` 41, clippy `-D warnings` for the workspace and
`-p reposhelf --features e2e` — all exit 0; M2 bench (`rs fixtures` + `rs bench` on `target\scratch\m2-bench`, exit 0:
list_dir 3.1 ms, lastcommit 2nd visit 0.8 ms, page skip 0 / 9,965 40.5 / 40.8 ms, langs 50k 51.5 ms, 400-file diff
2.0 ms; scratch dir removed).

M6.9 browser check (hidden `npm run dev`, PID 30704, Browser pane; killed by tree, 5 processes, port 1450 free, no
leftover node/headless Edge): dark — `#/`, `#/northwind/parsekit` (file table, latest commit, README, About,
releases, languages), `blob/main/src/lexer.rs?L=30-36` (highlighted, scrolled to the range), `?huge` `tree/main/generated`
(1,000 rows virtualised, rows follow scroll), `data/readings.csv?L=20000` (26k-line virtual plain viewer, large-file
notice, scrolled to line 20000 after the fix in D-72), `docs/logo.svg` (img on checkerboard); light — `#/`, Clone
dialog (codeberg.org preview), `#/lumen-ui/lumen` (Fast-forward, cloned/last fetched, releases), Edit About dialog;
`#/local/scratchpad` (empty), `tree/nope/` (ref not found + default link), `#/local/beacon` (detached `f10accc`),
`#/sandbox/hostile-readme` (only blob:/data:png images, 0 scripts). Second pass (hidden `npm run dev`, PID 24588,
killed by tree, 5 processes): light — `?empty` empty card, dashboard missing row (Locate… / Remove from shelf),
tag ref `tree/refs%2Ftags%2Fv0.2.0/src/stream` (tag switcher label, breadcrumbs, `..` row, dir History), file view
`docs/screenshot.png`, `assets/sample.bin`, `data/corpus.json`, `data/export.log`, `vendor/ucd`, `latest`,
`odd/a:b.txt` (not-checked-out notice), `docs/legacy-notes.txt` (lossy notice), `docs/architecture.md` Preview /
Raw; dark — Fetch confirm → "3 new commits", Import dialog picks new → "not a git repository" → duplicate (link to
northwind/parsekit), Remove dialog for a clone (Recycle Bin wording) → removed, landed on `#/` with no "not found"
flash, Fast-forward on lumen ("Fast-forwarded to d4b9dba", button gone), Edit About save (sidebar + dashboard row
updated). Console: no errors or warnings. Mock settings in
the pane's localStorage left at theme Light, sort updated/desc, source all. Saved light PNGs (headless Edge, scratch
profile, `preferredColorScheme=1`, 1280×1000, no window shown): `docs/screenshots/m6-dashboard.png`,
`docs/screenshots/m6-code.png` (reviewed: fonts loaded, no shimmers).

Open items / deviations:
- One source edit (RepoToolbar check-column labels) was made with a scripted replace instead of Edit/Write
  (content checked; no Windows paths involved). Everything else via Edit/Write.
- `pathRaw` is not used for browsing (D-71). Remove for an imported repo was checked in the harness test (dashboard
  missing row) and as wording only in the pane.
- Text files > 5 MB offer Save a copy only (FR-25 says Raw/Save): `blob_text` returns `text: null` above 5 MB, so
  there is nothing to show raw.
- In the hidden Browser pane scroll events are throttled, so a virtual list may lag one scroll behind until the
  next frame; not seen with the pane visible / in screenshots.
- Edit About's first focus is the description field (Dialog default), not topics.

Handoff for M7:
- Reuse `RefSwitcher` (`hrefFor(routeRef)` decides the target route; tags already `refs/tags/<name>`),
  `CommitLine` (avatar + author + summary + oid + date) from `code/LatestCommitBox`, `ExternalText` from
  `code/AboutSidebar` for any URL, `Placeholder`, `checked()` menu labels from `dashboard/RepoToolbar`.
- History links already point at `commits/<ref>?path=<path>`; commit links at `commit/<oid>`.
- After mutations use `state/repoCache.ts`; same-page query changes use `navigate(…, { replace: true, keepScroll: true })`.
- Component tests: `src/test/appHarness.tsx` (`boot(search, hash)`, `go`, `click`, `typeInto`, `installHarness()`).
- `screens/placeholder.tsx` is still used by Commits, Commit, Branches, Tags, Releases, Settings.

## M7 — Screens B: Commits, Commit, Branches, Tags, Releases, Settings, jobs final (2026-11-12)

Steps M7.1–M7.9 done (one subagent). Same visual language as M6 (boxes, muted meta, mono oids, own tokens);
`screens/placeholder.tsx` is gone (every screen is real now).

- M7.1 `src/components/commits/`: `paging.ts` (`?page=` + pinned `?tip=`, `path`, `since`; unfiltered pages start at
  (N-1)·35, path-filtered pages are chained through the returned `skip` via a module cache, D-77), `CommitRow`
  (summary link, "…" loads the description from `commit_detail`, D-78; author + "committed …" / "authored and X
  committed"; `CopyOid` = short-oid link + copy; browse-tree button), `DayGroup` (local day of the committer time,
  timeline dot), `PathChip` ("History for <path>" → file, × drops the filter), `Pager` (Newer / Older links keep the
  tip; capped → "Search older commits"); screen with ref switcher (keeps the path, drops tip/page), since banner,
  "pinned at <oid>" notice when the ref moved, empty / past-the-end / capped-empty states.
- M7.2 `src/components/diff/`: `CommitHeader` (summary, pre-wrapped body, author / committer with dates, "N parents"
  links, full oid + copy, Browse files), `DiffSummary` ("Showing N changed files with A additions and D deletions"
  + mini bar, merge / root / large-commit notes, collapsible file list → `?file=N`), `FileDiffCard` (sticky header:
  chevron, `MiniBar`, change badge, `old → new`, copy path, View file — deleted files at the parent), collapsed
  bodies (binary, > 1 MB, generated, deferred, large → Load diff = `commit_file_diff(force)`, D-82), `DiffTable`
  (old | new | marker | text, hunk rows tinted, NoNewline row, React text only), `diffs.ts`; screen with short-oid
  resolve, cards virtualised past 40 through `WindowList` (now with `scrollTo`, D-83); `src/styles/diff.css`.
- M7.3 `src/components/refs/`: `AheadBehindBar` (behind left / ahead right, scaled to the page max, "1,000+" when
  capped), `BranchRow` + `BranchChunk` (chunks of 20, requested once on screen, cancellable, D-79); Branches screen
  with `?q=` filter, sections Default / Your branches / Remote branches, HEAD badge, "compared with <HEAD branch>".
- M7.4 Tags: `ReleasesTagsToggle`, `TagRow` (name → tree `refs/tags/…`, annotated / lightweight badge, tagger, date,
  short oid → commit, Browse files).
- M7.5 Releases: `ReleaseCard` (side: date, tag, oid; title, Latest / Pre-release / GitHub-notes badges,
  `MarkdownBody` notes, "Commits since <prev>" with full tag refs, D-81), `GhNotesButton` (github.com only, confirm,
  primes `releases_list`, hides on `unsupported`, session date, D-80).
- M7.6 Settings: theme radio cards, data + archive folders (Copy, Open via the new `app_open_dir`, D-76), repo count /
  archive size / cache size + Clear cache, "Note uncommitted changes" (evicts `repo_detail`; `DirtyNote` under the
  repo header when `detail.dirty`), version. Rust: `cmd::app::app_open_dir` + test; ARCH §5.1 row; 48 commands.
- M7.7 Jobs popover: per-kind chip (Clone / Fetch / Fetch all clones) + target, `util/jobText.ts` (progress text and
  fraction, clone report with skipped refs, fetch report with new tags / kept local tags / skipped refs, header
  short text shared with RepoHeader), Cancel on waiting + running, Retry on failed only (errors shown in the row),
  Dismiss, "Open repository" after a clone, red badge when only failures remain.
- M7.8 Mock: `RepoDef.merges` (lumen #28, parsekit #40 with side commits in `byOid`), `special` (parsekit #3: rename
  `docs/design.md → docs/architecture.md`, deletion, no-newline change on `rustfmt.toml`), ahead/behind cap 1,000
  (`?huge` `docs/tutorial` at 6,000 → capped), generated files inlined like rs-core, binary `Load diff` refused,
  cloned-repo fetch reports a new tag / kept tag / skipped ref, parsekit `dirty: true`, `app_open_dir`. All opt-in
  after generation, so M6 data is unchanged. `mock-shots.md` rows 20–25.

Tests: 573 TS in 24 files (+51; 572 at a7bd174, +1 detached-HEAD test in the review fixes): Commits 10 (midnight split, author offset ignored, interrupted day, pager hrefs keep
tip/path/since, last page / capped, query parsing, Older → Newer over one tip, path-filtered page 2 with cold caches,
stubbed capped scan → "Search older commits", description + since banner), DiffTable/commit page 7 (hunk with
NoNewline, no-content / truncated notes, collapse rules, labels + mini bar, `?huge` collapsed card → Load diff,
rename/delete/no-newline commit with parent + full oid + View file at parent + chevron, merge note + second parent
opens + root note), refs 10 (bar widths ahead/behind/capped, Branches sections + lazy bars + `?q=`, `?huge` capped
"1,000+", detached HEAD sends no request, tags order/badges/links, release cards + compare href, GitHub notes confirm flow, other hosts /
unsupported hide the button), jobs 5 (rows for waiting / receiving / resolving / done clone / done fetch / failed /
cancelled / fetch-all with their actions, `?clonefail`, clone → Open repository + fetch report, `?fetchfail` Retry),
Settings 3 (theme radio, folders + Open + Clear cache + version, dirty toggle → note), mock +2 (M7.8 fixtures,
`app_open_dir`), contrast +7 pairs per theme. Rust: +1 (`app_open_dir_takes_a_keyword_only`), command count 48.

Gate M7: `npm run typecheck` ok · `npm test` 572 passed · `npm run build` ok, initial JS `index-*.js` 435,638 bytes
(< 1.5 MB) · `cargo test --workspace` 179 passed (42 + 135 + 1 ignored + 2). Earlier gates re-run: `cargo build
--workspace`, `cargo test -p rs-core --features ghreleases` (135 + 1 ignored + 2), `cargo test -p reposhelf` 42,
`--features e2e` 42, clippy `-D warnings` workspace and `-p reposhelf --features e2e` — all exit 0; M2 bench exit 0
(list_dir 3.1 ms, lastcommit 2nd visit 0.9 ms, page skip 0 / 9,965 43.9 / 44.0 ms, langs 50k 53.5 ms, 400-file diff
2.1 ms; scratch dir removed).

M7.9 browser check (hidden `npm run dev`, PID 14344 then 12816, Browser pane at 1280×1000; both killed by tree, 5
processes each, port 1450 free, 0 node / msedge / reposhelf left): light — S5 parsekit commits (groups, description
expand, Older → page 2 with the merge, pinned tip in the URL), S6 merge commit (2 parents, first-parent note), the
rename/delete/no-newline commit (`?file=4` reload scrolls to the end), `?huge` 400-file commit (16 cards rendered,
deferred note, Load diff), quill root commit (root note, all Added), S7 branches (lazy bars, HEAD badge), S8 tags,
S9 lumen releases + GitHub notes confirm → "Lumen 1.1 — theming", compare link → since banner (3 commits), S10
settings, `?huge` path-filtered page 2 reload (same commits), edge states (`local/scratchpad` commits / branches /
tags / releases "This repository is empty.", `commits/nope`, unknown commit), jobs popover: clone done/fetch report,
`?fetchfail` failure → Retry, `?clonefail` failure with Retry; dark — Settings (radio cards), S6 cards with
NoNewline rows and sticky headers, S5, S7, S8, S9, dirty note on parsekit, jobs popover. Fixed during the check: card
`overflow: hidden` broke the sticky header (D-83), `?file=` on a reload (fresh-visit test as FileView), "new tags"
singular. Console: no errors or warnings. Pane localStorage left at theme Light, dirty note off. Saved light PNGs
(headless Edge, scratch profile, `preferredColorScheme=1`, 1280×1000, no window): `docs/screenshots/m7-commits.png`,
`docs/screenshots/m7-commit.png` (reviewed: fonts loaded, no shimmers).

Review fixes (second commit, after a7bd174):
- Virtual card list: the list's page offset was measured only at mount, so after opening the File list a jump landed
  ~12,800 px off; `WindowList` now re-measures before `scrollTo` and on body resizes (D-83). Verified in the pane:
  plain list `?file=3` on parsekit's root commit → card top at 76 px; `?huge` `?file=250` reload → 76 px; File list
  → entry 390 → 77 px.
- `?file=` under StrictMode: the fresh-visit flag is cleared inside the second frame, so a cancelled first mount
  retries.
- Branches with a detached HEAD (`local/beacon`): src-tauri refuses `branches_ahead_behind` there; the page no longer
  falls back to the default branch — no request, dashes, a note (mock refuses like the backend; test added).
- `.oidbtn` no longer clips focus rings (`overflow` removed, halves round their own corners); first / last commit,
  branch and tag rows round their hover background inside the box; empty-repo copy for the commits tab.
- Pass 3 (hidden `npm run dev`, PID 3836, killed by tree, 5 processes, port 1450 free, 0 node / msedge): oid focus
  ring, expanded description ("No description."), open File list, GitHub confirm dialog, `?huge` "1,000+" bar,
  Clear-cache status line, dirty note in light, successful clone → "Open repository" (`#/example-org/new-thing`),
  beacon branches (detached note); console clean; pane settings back to Light, dirty note off.
- `npm test` run 3× after the fixes: 573 passed each time (no flakes).
- Final gate (Rust unchanged by the fixes, re-run anyway): typecheck ok · `npm test` 573 · build ok, initial JS
  436,510 bytes · `cargo test --workspace` 179 · build / ghreleases / reposhelf 42 / e2e 42 / both clippy runs exit 0
  · M2 bench exit 0 (3.0 / 0.8 / 43.8 / 40.7 / 51.8 / 2.0 ms; scratch dir removed).

Open items / deviations:
- One source edit (BranchRow: `RefObject` import + failed-count dash) was made with a scripted replace instead of
  Edit/Write (no Windows paths; the whole file was read back in the review pass). Everything else via Edit/Write.
- New IPC command `app_open_dir` (D-76) — touches src-tauri, ARCH §5.1 and the command count (48).
- The GitHub-notes "fetched date" is session-only (D-80); release links use full tag refs (D-81).
- Mock side commits of merges are not in the commits list walk (the list stays linear); the merge's second parent
  opens on the commit page only.
- The real rs-core walk cap (`SCAN_CAP` 20,000) is never reached by the mock; "Search older commits" is covered by a
  stubbed test only.

Handoff for M8:
- All screens S0–S10 are real; M8's light/dark real-app pass can follow `tools/shots/mock-shots.md` rows 14–25.
- Check against the real backend: `commits_page` returned `skip` with a path filter (chaining, D-77), the
  `branches_ahead_behind` base (HEAD's branch), `releases_fetch_github` with the feature on/off, `app_open_dir` in the
  built exe, generated-file inlining (D-82), and `?file=N` with real large commits.
- `WindowList` now takes `scrollTo`; `util/jobText.ts` holds all job wording.

## M8 — Integration / real-app run (2026-11-12)

Steps M8.1–M8.7, M8.9–M8.11 done (one subagent); M8.8 (`-Network` run) implemented, run pending for the user.

- M8.1 e2e build: `npm run build` + `npx tauri build --no-bundle --features e2e` with an **absolute**
  `CARGO_TARGET_DIR` (relative `target\e2e` landed in `src-tauri\target\e2e`, D-85; TASKS M8.1 + Gate M8 updated) →
  `target\e2e\release\reposhelf.exe`. Renders the dashboard from the embedded `dist` (origin `http://tauri.localhost`,
  nothing listening on 1450 — recorded per run in `run.json`, checked by check-e2e).
- e2e seams (D-84), `src-tauri/src/e2e.rs`, compiled only with `--features e2e`: `RS_E2E_PICK` folder queue for the
  Rust-side dialogs, `RS_E2E_OPEN_LOG` recording opener (Explorer/browser never opened), `RS_E2E_OFFSCREEN=1` window
  at −32000,−32000. Tests: 5 e2e-gated (queue parsing, picks then cancel without a dialog, pass-through without env,
  log lines, `app_open_dir` through the seam) + 1 source test keeping the `RS_E2E_` names out of every other file.
  Binary check: `RS_E2E_` occurs 0× in `target\debug\reposhelf.exe` (no e2e feature), present in the e2e exe.
- M8.2–M8.4 `tools/e2e/run-e2e.ps1` (scratch `target\scratch\e2e-<time>`: data, archive, all fixtures, 50 registered
  copies, a git-CLI upstream with isolated config; hashes the 8 UI-imported fixtures; tauri-driver on free
  `--port`/`--native-port`; records every descendant PID while running; `taskkill /T /F`; leftovers by seen PIDs,
  exe path and scratch path; re-hash; `run.json`; scratch removed unless `-Keep`), `wd-e2e.mjs` (75 steps, sessions
  A/B/(C), D-89) and `check-e2e.ps1` (failed/missing steps, 12 screens × 2 themes, leftovers, hashes, port 1450,
  opened URLs).
- M8.5 `rs fixtures --register <n> <dir>` (needs `RS_DATA_DIR` + `RS_ARCHIVE_DIR`; copies of `basic` without origin →
  `local/basic-NN`; test in rs-cli). `performance.mark('rs:dashboard-rows')` once per page load (`util/perfMark.ts`).
- M8.6 security pass, M8.9 network-free proxy, M8.10 `docs/TESTING.md` (gate commands, mock preview, e2e build/run,
  hand-test checklist incl. the adapter-off run).

Real-app results (gate run): NFR-P1 process start → dashboard rows with 50 repos **410 ms** warm (A: 439 ms first
launch; 80 ms of it after navigation start) < 1.5 s · NFR-P2 dashboard click → 1,000-entry table rows **42 ms** (7 ms
second visit) < 300 ms · 8 imports through the real Import dialog · 24 screenshots (dashboard, code tab, file view
with worker highlighting, SVG image, commits, commit, merge, treesame merge, branches, tags, releases, security README,
settings, Import + Clone dialogs; light and dark), reviewed: fonts loaded, no shimmers · path-filtered commits page 2
(`src/f01.rs`, 35 + 35 rows, no overlap, chained skip) · `?file=250` on the 400-file commit → card at 76 px ·
ahead/behind "0 | 1" for `feature` "compared with main" · GitHub-notes button only on the github.com origin (not
clicked) · `app_open_dir` + `repo_open_folder` reach the opener (logged) · security README: 0 script/iframe/form/on*/
style, no `javascript:`/`data:` hrefs, remote image as a chip, 0 alerts; external link → confirm dialog, cancelled,
no URL opened; SVG with script only as `<img>` · 25 hostile IPC probes refused with the expected code (invalidPath /
invalidRev / notFound / invalidUrl / invalidInput; 6 `plugin:*` commands "not allowed by ACL") · no resource request
outside the app origin / IPC · file-seam clone → upstream commit + tag → Fetch "1 new commit, 1 new tag" →
Fast-forward to the new upstream tip → Remove → folder in the Recycle Bin, entry gone · CSP collector via CDP,
positive control seen (inline script blocked) → **0 CSP violations, 0 errors, 0 alerts** (one Tauri "callback id"
warning from the harness's own reload) · imported fixtures' `.git` + work tree unchanged · 19 processes seen,
leftovers 0.

Bugs found and fixed (each with a test):
1. CSP `font-src` violation in the real app: Vite inlined a < 4 KB woff2 subset as `data:font/woff2` (D-86) →
   `build.assetsInlineLimit: 0`; `src/csp.test.ts` (config + any built CSS has no `url(data:`).
2. Dashboard "Recent" never filled in the real app (`repo_detail` stamps `lastOpenedAt`, but only the mock mutated
   the shared summary in place) → `noteSummary` from `RepoLayout`, mock returns a copy (D-87);
   `RepoLayout.test.tsx` (red before the fix, green after); e2e asserts 5 Recent items.
3. Option-looking revs (`--upload-pack=x`) passed `validate::rev` (answered `notFound`) → refused (D-88); validate
   test + the command-layer hostile table (+1 value; legal as a path).
4. e2e build: Fetch was hidden on a `file://` clone (`fetchable` = https only), so the real UI could not drive the
   seam's fetch → `repo_detail` reports `fetchable` for cloned `file://` origins only with the seam on (D-90; test in
   `file_clone_seam_in_e2e_build` both ways).
5. Harness-only: relative `CARGO_TARGET_DIR` (D-85).

Tests: Rust 179 → 181 in `cargo test --workspace` (rs-core 135 + 1 ignored, reposhelf 43 (+1), rs-cli 1 (new), the
2 integration tests) and reposhelf `--features e2e` 48 (+6); TS 573 → 579 in 27 files (+perfMark 2, csp 3,
RepoLayout 1).

Gate M8 (`target\m8-gate-*.log`): `cargo test --workspace --all-features` ok · clippy `-D warnings` ok · typecheck ok
· `npm test` 579 · `npm run build` ok, initial JS 436,870 bytes < 1.5 MB · e2e build ok (1 m 13 s) · `run-e2e.ps1`
75/75, check-e2e "all 75 steps passed; 24 shots; NFR-P1 410 ms; NFR-P2 42 ms; leftovers 0; imported fixtures
unchanged" · `rs fixtures` + `rs bench` (release) exit 0: list_dir 3.0 ms, lastcommit 2nd visit 0.8 ms, page skip 0 /
9,965 41.7 / 40.4 ms, langs 50k 51.2 ms, 400-file diff 1.9 ms (scratch removed). Earlier gates: `cargo build
--workspace`, `cargo test --workspace` 181, `-p rs-core --features ghreleases`, `-p reposhelf` 43 / `--features e2e`
48 / `--no-default-features` 43, clippy `-p reposhelf --features e2e` and `--no-default-features` — all exit 0.

Open items / deviations:
- M8.8 `-Network`: not run by the agent (network-free run; no verified small public repo with Windows-invalid ref
  names). Hand step in `docs/TESTING.md`; skipped refs are covered by the M3 `file://` tests.
- The network-adapter-off browse run (NFR-O1) needs admin → hand step in `docs/TESTING.md`. Automated proxies: M3.9
  `net_calls()` + the e2e "no foreign requests" step.
- Each e2e run sends one scratch `demo-upstream.git` folder to the real Recycle Bin (D-30); five are there from
  today's runs (empty them at will).
- Not covered by fixtures in the real app: generated-file inlining (no generated files in the fixtures; D-82 is
  covered by rs-core + mock tests), the 20,000-commit scan cap ("Search older commits", stub test only), GitHub notes
  with the feature off (`--no-default-features` tests).
- S3 has no size column (REQUIREMENTS S3); NFR-P2's "sizes" is the backend `tree_list` (bench 3.0 ms).
- Scripted edits (not Edit/Write): one test block in `src-tauri/src/tests.rs` was first appended with a bash heredoc,
  which dropped backslashes in Windows-path literals (caught by reading it back, fixed with Edit before any build);
  `wd-e2e.mjs` got a `sed -i` selector rename and two small node string replacements (expected codes in the hostile
  table, the collector sanity check); `TASKS.md` checkboxes were ticked by a PowerShell string replace. All re-read.
- `-Network` / session C is implemented but has never been executed; note that run-e2e still sets
  `RS_TEST_FILE_CLONE=1` there (https clones still go through the production URL rules, `file:` only is re-routed).

Follow-up (second commit, after e65d3c3): the console/CSP and "no foreign requests" checks only covered the
document after the NFR-P2 reload. The harness now drains `window.__e2e` and the resource-timing list into node-side
accumulators before the hostile-IPC probes ("browse": 159 requests), after them, and at the end, with a 10,000-entry
resource buffer, and asserts on the union (check-e2e requires the three drain steps). Re-run: 78/78 steps, 0 CSP
violations / errors / alerts over the whole session (2 Tauri "callback id" warnings, both from the harness's
reloads), no foreign requests, NFR-P1 422 ms, NFR-P2 44 ms, leftovers 0, imported fixtures unchanged. The bench was
re-run directly to record its exit code: `rs fixtures` rc 0, `rs bench` rc 0 (2.9 / 0.8 / 41.5 / 41.8 / 53.9 /
2.2 ms); the gate summary had lost the bench line (output formatting), the rows were all ok.

Notes for M9:
- Ship builds only via the tauri CLI without `--features e2e` (D-26); check the shipped exe contains neither
  `RS_TEST_FILE_CLONE` nor `RS_E2E_` (D-84) nor the testkit marker.
- `vite.config.ts` `assetsInlineLimit: 0` must stay (CSP, D-86).
- The e2e harness can run against any exe built with `--features e2e` via `run-e2e.ps1 -App <exe>`.
- Hand-test list for the user: `docs/TESTING.md` §4.

## M9 — Docs + release build (2026-11-12)

Steps M9.1–M9.8 done (one subagent). Decisions D-91..D-95.

- **Recycle Bin fix first (D-91).** M8's e2e sent the removed scratch clone to the user's real Recycle Bin (five
  `demo-upstream.git` folders are still there from M8 runs; not emptied by the agent). New e2e-only trash seam
  `RS_E2E_TRASH_LOG` (`E2eTrash` in `src-tauri/src/e2e.rs`, wired in `lib.rs` under `cfg(feature = "e2e")`): deletes
  the folder outright, only strictly inside the scratch archive, and logs `trash\t<path>`. `run-e2e.ps1` sets it and
  refuses an e2e exe without it; `wd-e2e.mjs` (step renamed "remove cloned (trash seam)") and `check-e2e.ps1` require
  exactly one logged deletion inside the scratch archive. 2 new e2e-gated tests (deletes inside the archive incl. a
  read-only file, refuses outside / the archive itself, never calls the real trasher; pass-through without env).
  Every e2e run in M9 used the seam: nothing reached the Recycle Bin.
- M9.1 version 0.1.0 in `package.json` (+ `package-lock.json`), `Cargo.toml` workspace (+ `Cargo.lock`),
  `tauri.conf.json`; mock `app_info` and the Settings test follow (0.1.0). The IPC golden / `types.rs` sample keeps
  "0.0.1" (sample data, not the app version).
- M9.2 `tauri.conf.json` bundle was already `nsis` / `currentUser` / icons. `tools/build-portable.mjs` implemented
  (D-92): name + version from `tauri.conf.json`, `npx tauri build --no-bundle`, refuses a set `CARGO_TARGET_DIR` and
  any `RS_TEST_FILE_CLONE` / `RS_E2E_` bytes, copies to `target\release\portable\`.
- M9.3 builds: `npm run build:installer` → `RepoShelf_0.1.0_x64-setup.exe` (never run). The tauri bundler
  re-downloaded NSIS 3.13 + `nsis_tauri_utils` 0.5.3 into `%LOCALAPPDATA%\tauri` (hash-validated build-tool cache; no
  Bitdefender interference seen; the setup exe still existed after the build). Markers (D-93): `RS_TEST_FILE_CLONE`,
  `RS_E2E_` absent from `target\release\reposhelf.exe` (installer payload), the portable and the setup exe; present in
  the e2e exe (positive control). The testkit has no string surviving LTO + strip (absent even from the e2e exe), so
  D-26 rests on the build path, not a byte search.
- M9.4 portable smoke (D-94): scratch `RS_DATA_DIR` / `RS_ARCHIVE_DIR`, WebView2 remote debugging on a free port,
  window moved to −32000,−32000 by `SetWindowPos` ~0.1–0.3 s after start (visible centred on the main display for that
  moment — a shipped exe has no off-screen seam), page over CDP: `http://tauri.localhost/`, "Repositories" + "Your
  shelf is empty"; `reposhelf.db` created in the scratch data dir; tree killed, leftovers 0, scratch removed. Run on
  the final portable (after the gate rebuild) as well.
- M9.5 `README.md` (what it is, screenshots from `docs/screenshots`, screens, download/run, data locations + env
  overrides, develop + mock preview, tests + e2e, release build, fixed assumptions, out of scope, planned M10 as a
  plan only, known limits). M9.6 `docs/TESTING.md` (trash seam, §4 release build + marker check + portable smoke, §5
  hand tests incl. the installer), DECISIONS D-91..D-95.
- M9.7 `_REVIVE\` kit created at the user's request (TASKS said "only if it exists"; D-95): `REVIVE.md`,
  `memory\reposhelf-project.md` (copy), `releases\v0.1.0\` with both exes + `SHA256SUMS.txt` (copies verified against
  `target\`). `/_REVIVE/` added to `.git\info\exclude` (as Project Manager), so no exe is committed. Row added to
  `C:\Dev\projects\_REVIVE_INDEX.md`.

Artifacts (final, from the gate run): `RepoShelf_0.1.0_x64-setup.exe` 5,475,361 bytes (sha256 ddfde27c…7f4e),
`RepoShelf_0.1.0_x64-portable.exe` 18,681,856 bytes (sha256 641353e2…b32d). Builds are not byte-reproducible (each
rebuild changed both hashes), so the copies were refreshed after the gate.

Tests: Rust `cargo test --workspace` 181 (unchanged), reposhelf `--features e2e` 48 → 50; TS 579.

Gate M9 (`target\m9-gate-summary.log`, `target\m9g-*.log`): `cargo test --workspace --all-features` ok · clippy
`-D warnings` (workspace and `-p reposhelf --features e2e`) ok · typecheck ok · `npm test` 579 · `npm run build` ok,
initial JS 436,870 bytes < 1.5 MB · build:installer ok · build:portable ok · both artifacts present, markers absent ·
e2e build ok · `run-e2e.ps1` + check: 78/78 steps, 24 shots, NFR-P1 426 ms, NFR-P2 44 ms, leftovers 0, imported
fixtures unchanged, trash seam 1 deletion · `rs fixtures` + `rs bench` (release) rc 0: 2.9 / 0.7 / 41.5 / 42.0 /
53.6 / 2.0 ms. Earlier: `cargo test --workspace` 181, `-p reposhelf --no-default-features` 43, clippy
`--no-default-features` ok.

Open items / handoff (v0.1.0 done):
- Hand tests for the user: `docs/TESTING.md` §5 — installer install + uninstall (agents never run it), network adapter
  off, theme + restart, mouse back/forward, real dialogs, Open folder, external link, big clone, real Recycle Bin.
- `run-e2e.ps1 -Network` (M8.8) still never executed.
- Five scratch `demo-upstream.git` folders from M8 in the Recycle Bin: empty at will.
- M10 Explore mode stays plan only until the user says so.
- Scripted edits (not Edit/Write): TASKS M9 checkboxes ticked by a PowerShell string replace; `.git\info\exclude`
  appended with `Add-Content` (no Windows paths). The smoke helpers (`smoke-portable.ps1`, `cdp-eval.mjs`) lived in the
  session scratchpad and are not in the repo; the procedure is in TESTING §4.
