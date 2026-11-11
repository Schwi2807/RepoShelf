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
