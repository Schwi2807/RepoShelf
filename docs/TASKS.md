# RepoShelf — task plan (M0..M9)

Inputs: `docs/REQUIREMENTS.md` (FR-1..37, NFR-*, S0..S10, R-1..12) and `docs/ARCHITECTURE.md` (§0 decisions D-1..17,
§10 milestones). This file is the implementer's checklist. Tick a box (`- [x]`) when the step is done and its tests
pass. Step IDs (`M2.3`) are referenced from WORKLOG entries and commit bodies.

## Rules for implementers

### Hard constraints for every subagent (verbatim from REQUIREMENTS.md)

- Work only inside `C:\Dev\projects\repo-shelf`. Project Manager is read-only reference: no edits, no builds in its
  `target\`, no branch switches.
- Own app id `dev.schwi.reposhelf`, own data dir and managed dir. Never read or write
  `%LOCALAPPDATA%\dev.schwi.projectmanager*` or `Documents\ProjectManager*`.
- Tests use scratch dirs only (env overrides `RS_DATA_DIR`, `RS_ARCHIVE_DIR`). Never install/uninstall anything,
  never touch PATH, never change AV/system settings.
- Kill whole process trees after tests (`taskkill /T /F`). Test windows offscreen or on the main display.
- Commit locally per milestone with the repo-local identity (already configured). Never push.
- Untrusted content: READMEs/files of cloned repos are untrusted. Sanitise rendered HTML, strict CSP, no scripts.

### Additional rules

- Before any `cargo` / `npm` / `npx` command, refresh the **process** PATH (process-scoped only; never
  `SetEnvironmentVariable`, never edit the user/machine PATH):
  `$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')`
- "Never install" means: no `npm install -g`, no `cargo install`, no winget/choco/msi, no running the NSIS installer.
  Repo-local `npm install` (into `node_modules\`) and cargo dependency downloads/build caches are in scope.
- Run long first compiles (first `cargo build` of the tauri crate, first `cargo test -p reposhelf`, release builds) in
  the background or with long timeouts (10 min); do not abort them as hung.
- Use the default `target\` in this repo. A separate `CARGO_TARGET_DIR` only where this plan says so (the e2e build,
  `target\e2e`) or if a lock clash forces it.
- Delete with `Remove-Item -LiteralPath <path> -Recurse -Force` in its own short command (never chained with other
  work, never with wildcards on paths outside this repo or a scratch dir).
- Write files that contain Windows paths (docs, scripts, JSON, TS/Rust sources) with the Write/Edit tools, not bash
  heredocs or PowerShell double-quoted here-strings (they corrupt backslashes, `$` and TS template literals).
- PowerShell 5.1: no `&&` / `||`. Chain as `cmd1; if ($LASTEXITCODE -ne 0) { throw 'cmd1 failed' }; cmd2`.
- Keep `docs/WORKLOG.md` (append one entry per milestone: date, steps done, test counts, gate output summary, open
  items, handoff notes) and `docs/DECISIONS.md` (one line per decision: `D-n decision — why — rejected alternative`;
  D-1..D-17 from ARCHITECTURE §0 plus D-18.. from the gap fixes below, plus any new ones made while implementing).
- Per milestone: run the full Gate (including all earlier gates' commands), then commit and tag:
  `git add -A; git commit -m "<area>: M<N> <summary>" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"; git tag m<N>-done`
  (the second `-m` produces the required blank line before the trailer). Never push. Never amend a tagged commit.
- Anything launched (tauri dev, exe, vite, tauri-driver, msedgedriver): kill by tree (`taskkill /T /F /PID <pid>`),
  then count leftovers (`Get-Process reposhelf,tauri-driver,msedgedriver,node -ErrorAction SilentlyContinue` filtered
  to PIDs you started / children of them) and make it 0. Never kill processes you did not start.
- Never inspect a real install or real data dir; tests and manual runs use scratch dirs under the session scratchpad
  or `target\scratch\` with `RS_DATA_DIR` / `RS_ARCHIVE_DIR` set.
- Backend first: no file under `src/components/` or `src/screens/` before M5 starts (M4 may write only
  `src/ipc/types.ts` and its test).

## Gaps and contradictions found in ARCHITECTURE.md (fixed in this plan)

Each fix becomes a DECISIONS.md line (D-18..D-31) in step M0.9.

| # | Gap / contradiction | Fix (where) |
|---|---|---|
| D-18 | `import_confirm` takes a token "from the preview" (§5.1) but `ImportPreview` (§3.3) has no token field. | Add `token: String` to `ImportPreview`; `src-tauri` keeps `token → (path, expiry 10 min)` in `AppState`. (M1.10, M4.6) |
| D-19 | FR-18 shows "Cloned <date>" and the schema has `cloned_at`, but `RepoSummary` lacks it. | Add `cloned_at: Option<i64>` to `RepoSummary`. (M1.10) |
| D-20 | Fetch re-validates the origin as https (§5.4), so neither M3 fetch tests nor the M8 clone→fetch smoke (file-clone seam) can fetch from a `file://` upstream. D-14 only covers clone. | Split `fetch::fetch` into URL policy + `fetch_from(source: FetchSource)`; add `fetch_unchecked` under the same `cfg(any(test, feature="testkit"))` as `clone_from_unchecked`; src-tauri `e2e` feature + `RS_TEST_FILE_CLONE=1` routes `file://` origins there. (M3.6, M4.9) |
| D-21 | M2 functions with caches (`commits::count`, `lastcommit::for_dir`, `langs::for_tree`) and `repo::open/detail` need a `repos` row (FK), but `library::import` is M3. | `testkit::register_raw(&Shelf, path, source) -> i64` inserts a `repos` row by SQL in M1; M2 tests use it. `repo::open/detail` land in M2 using only the DB lookup. (M1.11, M2.1) |
| D-22 | `ghreleases.rs` (optional FR-31 notes) is in the module list and the command table but in no milestone scope. | Implement in M3 behind feature `ghreleases` (enabled by src-tauri by default), tested with canned JSON only (no network in tests). (M3.8) |
| D-23 | The bench (§9) times a 50k-file tree and a 400-file commit; neither fixture exists in §3.4/M2. No corrupt/missing-repo fixture for NFR-O3/FR-4. | Add testkit fixtures `big-tree` (50k files), `huge-commit` (400 files, > 20k lines), `corrupt` (deleted object), `missing` (registered then renamed). (M2.2) |
| D-24 | M4's type check has the vitest read `target/ipc-samples.json` produced by `cargo test`, so `npm test` fails on a clean checkout and depends on order. | Golden file `src/ipc/__samples__/ipc-samples.json` is committed; the Rust test regenerates into `target/` and fails if it differs from the golden (`UPDATE_SAMPLES=1` rewrites it). Vitest reads the committed golden. (M4.12) |
| D-25 | §9 says the e2e harness "starts the portable exe"; M8 and D-14 say a separate `--features e2e` build. | Harness runs `target\e2e\release\reposhelf.exe` only. Built with `npx tauri build --no-bundle --features e2e` and `$env:CARGO_TARGET_DIR='target\e2e'` (PM's portable script uses `tauri build --no-bundle`, which embeds `dist`; plain `cargo build` would leave the app on the dev URL). Gate: renders with nothing listening on 1450. (M8.1) |
| D-26 | Workspace feature unification: `rs-cli` enables `rs-core/testkit`; a `cargo build --workspace --release` compiles rs-core with testkit for the app too. | Shipped builds only through the tauri CLI / `-p reposhelf`; never `--workspace` for release artifacts. M9 checks the marker string. (M9.3) |
| D-27 | `library::search` (Rust) and as-you-type filtering (FR-2) would be two implementations; the list is already in the TS store. | Search/filter/sort implemented once in TS (`src/state/library.ts` `filterRepos`, `sortRepos`); Rust `library::search` dropped. (M6.2) |
| D-28 | `Settings.recent: Vec<i64>` duplicates `repos.last_opened_at` (written by `touch_opened`). | Drop `Settings.recent`; "Recent" = top 5 by `last_opened_at`. (M1.6) |
| D-29 | NFR-P1 (cold start < 1.5 s with 50 repos) and success criterion 2 (every screen, real app, light **and** dark) have no measurement step; NFR-W3 badge and NFR-A1 (`/`, focus rings) have no UI step. | `rs fixtures --register 50` + e2e start-time measurement and a light/dark screenshot pass in M8; NFR-W3 badge in M6.6; NFR-A1 in M5.10. |
| D-30 | FR-36 tests would send scratch folders into the user's real Recycle Bin. | `src-tauri` puts trash behind a `Trasher` trait (`RealTrash` uses `trash::delete`; tests use a recording fake). Real trashing only in the M8 e2e on a scratch clone. (M4.6) |
| D-31 | FR-34 scroll restore stores `window.scrollY`, but a virtualiser with an inner scroll container would not restore. | Virtualised lists use `useWindowVirtualizer` (window scrolling), so the router's scroll memory applies. (M5.2, M6.5) |

Smaller clarifications (no decision line needed): the e2e and dev windows are created on the main display or moved
offscreen in-process (M4.14, M8.2); `tauri dev` in M4 is killed by tree; the bundle-size check (NFR-P8) is repeated in
M7 and M9, not only M5; PowerShell gates avoid `&&` (ARCHITECTURE M8 line used it).

## Dependency overview

```
M0 layout ─► M1 framework ─► M2 git read model ─► M3 library/clone/fetch ─► M4 IPC (+types.ts) ─► M5 shell+mock
                                                                                      └────────► M6 screens A ─► M7 screens B ─► M8 real app ─► M9 release
```
Strict order between milestones. Inside a milestone, steps marked **∥** can run in parallel (different files);
everything else follows its listed "after" dependencies. When running steps in parallel, the step that owns `lib.rs`
/ `mod` declarations / `App.tsx` routes goes first so parallel steps never edit the same file.

---

## M0 — Project layout

Goal: workspace and empty crates compile with every registry dependency pulled in; Tauri shell shows "RepoShelf";
docs scaffolding. ARCH §2, §1.1.

- [x] M0.1 Root `Cargo.toml`: `[workspace] members = ["crates/rs-core","crates/rs-cli","src-tauri"]`, `resolver = "3"`,
  `[workspace.package] edition = "2024", version = "0.0.1"`, `[workspace.dependencies]` with the exact versions from
  ARCH §1.1, `[profile.release] codegen-units = 1, lto = "thin", strip = true`. `.gitignore`: `target/`,
  `node_modules/`, `dist/`, `src-tauri/gen/`, `*.db`, `target/scratch/`.
- [x] M0.2 `crates/rs-core/Cargo.toml` (deps: git2 `default-features=false, features=["vendored-libgit2","https"]`,
  rusqlite `bundled`, serde, serde_json, thiserror, dirs, url, comrak `default-features=false`, ammonia; optional
  `tempfile`, `ureq`; features `testkit = ["dep:tempfile"]`, `ghreleases = ["dep:ureq"]`; dev-dep tempfile) and
  `src/lib.rs` with `pub fn render_smoke(md: &str) -> String` (comrak → ammonia) + test `smoke_markdown_strips_script`.
  Verify comrak 0.59 option names (`extension.table`, `render.unsafe_`, `header_ids`) against its docs.rs/source in
  the cargo registry; if comrak fails to compile, fall back to pulldown-cmark 0.13 and record a decision. (FR-17 prep)
- [x] M0.3 `crates/rs-cli/{Cargo.toml,src/main.rs}`: bin `rs`, depends on rs-core with `features=["testkit"]`; prints
  usage. Not shipped (D-15).
- [x] M0.4 `src-tauri/`: `Cargo.toml` (package **name `reposhelf`**, lib name `reposhelf_lib`, deps tauri 2,
  tauri-plugin-dialog, tauri-plugin-opener, trash, rs-core with `features=["ghreleases"]`, serde; features
  `e2e = ["rs-core/testkit"]`, default = []), `build.rs`, `src/main.rs` (calls `reposhelf_lib::run()`), `src/lib.rs`
  (Builder + plugins, one window `main` 1320x840 min 900x560), `tauri.conf.json` (`productName "RepoShelf"`,
  `identifier "dev.schwi.reposhelf"`, `build.devUrl http://localhost:1450`, `frontendDist ../dist`,
  `beforeDevCommand npm run dev`, `beforeBuildCommand npm run build`, CSP + devCsp exactly as ARCH §8, no
  `assetProtocol`), `capabilities/default.json` (`["core:default"]`, window `main`).
- [x] M0.5 Icon: write `docs/icon/reposhelf.svg` (own design: shelf with three book spines, own palette, no GitHub
  marks, R-11); `npx tauri icon docs/icon/reposhelf.svg` → `src-tauri/icons/*` (if the CLI refuses SVG, render a
  1024 px PNG with a small node script using no new deps, or hand-author the PNG; record how).
- [x] M0.6 npm: `package.json` (name `reposhelf`, private, `"type": "module"`, scripts `dev` = `vite`, `build` =
  `vite build`, `typecheck` (copy PM's form for TS 7), `test` = `vitest run`, `tauri` = `tauri`, `build:installer` =
  `tauri build`, `build:portable` = `node tools/build-portable.mjs`), deps/devDeps with ARCH §1.1 ranges;
  `npm install` (repo-local). `vite.config.ts` (port 1450, `strictPort`, watch ignores `src-tauri/target`, `target`,
  `crates`; `worker.format = "es"`), `vitest.config.ts` (happy-dom, `src/**/*.test.{ts,tsx}`), `tsconfig.json`
  (strict, ES2022, bundler resolution), `index.html` (no inline script/style), `src/main.tsx`, `src/App.tsx` (plain
  "RepoShelf" heading), `src/smoke.test.ts` (imports `highlight.js/lib/core`, registers one language, highlights).
- [x] M0.7 `.claude/launch.json` with config `reposhelf-mock` (`npm`, `["run","dev"]`, port 1450).
- [x] M0.8 Stub `tools/build-portable.mjs` (filled in M9) so the script exists.
- [x] M0.9 `docs/DECISIONS.md`: D-1..D-17 from ARCH §0 and D-18..D-31 from the table above, one line each.
  `docs/WORKLOG.md`: header + M0 entry.
- [ ] M0.10 Smoke-run `npx tauri dev` once (window on main display; confirm "RepoShelf" page; then kill the tree).
  Optional here, mandatory in M4.

Order: M0.1 → (M0.2 ∥ M0.3 ∥ M0.4 ∥ M0.6) → M0.5 (needs CLI from M0.6) → M0.7, M0.8, M0.9 → gate.

**Gate M0**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
npm install; if ($LASTEXITCODE -ne 0) { throw 'npm install' }
cargo build --workspace; if ($LASTEXITCODE -ne 0) { throw 'cargo build' }        # first run: background / 10 min timeout
cargo test --workspace; if ($LASTEXITCODE -ne 0) { throw 'cargo test' }
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }
npm run build; if ($LASTEXITCODE -ne 0) { throw 'npm run build' }
```
Must be true: `npm run dev` serves on 1450 (start via launch config, fetch `/`, stop); `src-tauri/icons/` populated;
DECISIONS.md has D-1..D-31; `Cargo.lock` and `package-lock.json` committed; tag `m0-done`.

---

## M1 — Framework layer (rs-core)

Goal: errors, dirs, DB + migrations, settings, validation, Windows path rules, jobs, shared types, testkit basics.
ARCH §3.1–3.3, §4, §5.2–5.3 (validator only). No git read logic yet.

- [x] M1.1 `lib.rs`: declare **all** modules of ARCH §2 now (`pub mod error; … pub mod fetch;` with empty bodies for
  later ones; `#[cfg(feature="ghreleases")] pub mod ghreleases; #[cfg(any(test, feature="testkit"))] pub mod testkit;`)
  so later parallel steps never touch `lib.rs`. `pub struct Shelf { pub db: Db, pub dirs: Dirs }` +
  `Shelf::open(dirs) -> Result<Shelf>` and `Shelf::in_memory(dirs)`.
- [x] M1.2 `error.rs`: `Error` enum exactly as ARCH §3.1, `Result<T>`, `Error::code()`; plus `Unsupported(String)`
  (code `unsupported`, used when a feature such as `ghreleases` is compiled out). Tests: every variant's `code()`
  is unique camelCase.
- [x] M1.3 `paths.rs`: `Dirs`, `resolve_dirs()` (env `RS_DATA_DIR`/`RS_ARCHIVE_DIR` win; else
  `%LOCALAPPDATA%\dev.schwi.reposhelf` and `Documents\RepoShelf`, D-13), `db_file`, `long` (`\\?\` on Windows only,
  `cfg(windows)`), `canonical` (canonicalize + strip `\\?\`). Tests: env overrides honoured (run under a mutex since
  env is process-global), path with spaces, `long` idempotent. (FR-35, NFR-W1, NFR-O4)
- [x] M1.4 `migrations/0001_init.sql` exactly as ARCH §4 (incl. `cloned_at`); `db.rs`: `Db::open` (WAL,
  `busy_timeout` 5 s, `foreign_keys=ON`, migrations via `include_str!` + `PRAGMA user_version`, each in one tx;
  higher `user_version` → open read-only and expose `Db::read_only() -> bool`), `open_in_memory`, `with`, `tx`,
  `SCHEMA_VERSION`. `prune_caches(&Db, keep_tips: &[String])` (≤ 2,000 `cache_last_commit` rows, 30-day rule).
  Tests: fresh open, reopen (no re-run), future version → read-only, prune keeps newest 2,000.
- [x] M1.5 `validate.rs`: `git_path`, `rev`, `clone_url` (full §5.3 rules), `topics`, `homepage`, `oid` (40 hex),
  `external_url` (http/https, no userinfo), `MetadataInput::validate` (description ≤ 350 chars); newtypes `GitPath`,
  `RevSpec`, `CloneUrl`. Tests: hostile table from §5.2 (`..`, `a/../b`, `/etc`, `C:\x`, `c:/x`, `a\b`, `a\0b`,
  `refs/../x`, `HEAD~1`, `a..b`, `@{-1}`), hex-looking names stay names, every rejected clone form (`git@…`, `ssh://`,
  `git://`, `file://`, `http://`, userinfo, `\`, drive letter, leading `/` or `.`, 3 segments without dot host) with
  user-readable messages, shorthand `owner/repo` → github.com, `gitlab.com/a/b/c` folding into `rel_dir`,
  `.git`/trailing `/` normalised; topics (case, charset, ≤ 20, dedup); homepage. (FR-6, FR-20, NFR-S4, NFR-S5, D-1)
- [x] M1.6 `settings.rs`: `Settings { theme, sort, source_filter, show_dirty_note }` (no `recent`, D-28), `load`
  (defaults when keys absent), `save`. Test: round-trip, unknown keys ignored. (FR-3, FR-33, D-16)
- [x] M1.7 `winpath.rs`: `path_ok_here`, `ref_name_ok_here`, `storable_refs`, `refspecs` (reimplemented from the PM
  ideas, not copied). Tests: `a:b.txt`, `CON.txt`, `con.txt` / `CON.tar.gz` (follow libgit2's NT rules for device names with extensions), trailing dot/space,
  `.git`, `git~1`, `llm` + `LLM/x` clash with and without `prefer`, wildcard refspec when nothing skipped.
  (FR-9, NFR-W2, NFR-W3)
- [x] M1.8 `jobs.rs`: `JobId`, `JobKind`, `JobState`, `JobInfo`, `Progress`, `Worker`, `JobRunner` (`new`,
  `load_persisted`, `submit`, `cancel`, `retry`, `list`, `dismiss`), `JobCtx` (`progress` throttled ≥ 80 ms except
  terminal, `cancelled`), `Gate` (Mutex + Condvar, 100 ms wait slices), `CancelToken`. `jobs` table is written only
  here. Tests (fake worker sleeping/looping on cancel): never > 3 running with 6 submitted; Waiting job cancel →
  Cancelled without running; throttle drops events < 80 ms; retry creates new row and deletes old; restart
  (`load_persisted`) turns running/waiting rows into Failed "Interrupted (app closed)". (FR-7, NFR-P7, L-6)
- [x] M1.9 Also in `jobs.rs` (or `compute.rs` if cleaner; declare in M1.1): `ComputeGate` of 2 for request-scoped
  walks (ARCH §5.5); test cap.
- [x] M1.10 `types.rs`: every type of ARCH §3.3 with `#[serde(rename_all = "camelCase")]` and
  `rename_all_fields = "camelCase"` on struct-variant enums; **add** `ImportPreview.token: String` (D-18) and
  `RepoSummary.cloned_at: Option<i64>` (D-19). Test: serialising one sample of each enum shape yields camelCase field
  names (guards L-10).
- [x] M1.11 `testkit.rs` basics: `repo(dir) -> TestRepo` builder (`file`, `commit(msg, time)`, `branch`, `tag(annotated)`,
  `merge`, `bare()`), and `register_raw(&Shelf, path, Source) -> i64` (D-21). Test: builder produces expected commits.
- [x] M1.12 Clippy clean.

Order: M1.1 → M1.2 → (M1.3 ∥ M1.5 ∥ M1.7 ∥ M1.10) → M1.4 (needs error, paths) → M1.6 (needs db, types) → M1.8, M1.9
(need db) → M1.11 (needs db, types) → M1.12.

**Gate M1**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
cargo test -p rs-core; if ($LASTEXITCODE -ne 0) { throw 'rs-core tests' }
cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy' }
cargo test --workspace; if ($LASTEXITCODE -ne 0) { throw 'workspace tests' }
npm run typecheck; npm test; npm run build    # M0 still green (check each exit code)
```
Must be true: all M1 tests above exist and pass; no test touches a real data dir; tag `m1-done`.

---

## M2 — Data layer: git read model (rs-core)

Goal: every read function behind the Code/File/Commits/Commit/Branches/Tags/Releases screens, fixtures, bench.
ARCH §3.4, §3.5, §6.1–6.8, §9. All functions take an opened `&Repository`; caches use a `repos` row created with
`testkit::register_raw` in tests (D-21).

- [x] M2.1 `repo.rs`: `open(&Shelf, id)` (DB lookup → `Repository::open_ext(NO_SEARCH)`; missing path →
  `RepoMissing`, open failure → `RepoCorrupt`), `detail` (head state, counts, license, `fetchable` = https origin,
  `can_fast_forward` for managed clones, `dirty` only if `show_dirty_note`), `dirty` (`statuses()` without
  `UPDATE_INDEX`, D-16). Tests: missing, corrupt, bare, unborn HEAD, detached. (FR-4, FR-12, FR-18, NFR-O3)
- [x] M2.2 `testkit::fixtures(root) -> Fixtures`: basic (README with table/code/relative image/link, `.rs` + `.ts`
  files, LICENSE MIT), wide-dir (1,000 entries), long-history (10k linear commits), merges (ordinary merge, merge
  dropping a side change, side revert), binaries (png, svg-with-script, LFS pointer, symlink, submodule gitlink,
  > 1 MB and > 5 MB files, non-UTF-8 text, BOM, non-UTF-8 file name), security-readme (from
  `tests/fixtures/security/README.md`), releases (3 annotated version tags + 1 lightweight + 1 `-rc1`), **big-tree**
  (50k files), **huge-commit** (one commit touching 400 files, > 20k lines), **corrupt** (one object deleted),
  **missing** (D-23). Write `tests/fixtures/security/{README.md,svg-with-script.svg}` (script, `onerror`,
  `javascript:` link, remote image, iframe, `style=`, `data:text/html` link, raw `<form>`). Builders must be fast
  (write objects with git2 directly, no checkout).
- [x] M2.3 `refs.rs` ∥: `resolve` (§3.5 precedence 1–6; ambiguous prefix → `InvalidRev`; not found →
  `NotFound{what:"ref"}`), `list`, `branches` (local + `refs/remotes/origin/*`, last commit), `ahead_behind` (capped,
  cancellable), `tags` (newest first by tagger/commit date, annotated flag). Tests: branch beats same-name tag,
  `refs/tags/x` reaches the tag, `origin/main`, hex-looking branch name `dead` resolves as branch, counts vs `git
  rev-list --left-right` when git on PATH. (FR-13, FR-29, FR-30)
- [x] M2.4 `tree.rs` ∥: `list_dir` (dirs first, then case-insensitive name; kinds Dir/File/Exec/Symlink/Submodule;
  sizes via `odb.read_header`; `storable_here` = `winpath::path_ok_here`; `path_raw` when lossy; `readme` name via
  `readme::find_readme`), `entry_kind`. Tests: 1,000-entry order, kinds, non-UTF-8 name sets `path_raw`, NUL-free.
  (FR-15, NFR-P2, NFR-W3, R-7)
- [x] M2.5 `blob.rs` ∥: `sniff`, `meta` (lines, sloc, `highlight` ≤ 1 MB, `displayable` ≤ 5 MB, `lang_hint` from
  extension/shebang), `text` (BOM stripped, lossy flag, `None` > 5 MB), `bytes(max)`. Tests: one per placeholder kind
  of FR-25, byte-for-byte equality with the blob (FR-23). (FR-22..25, NFR-P4, R-7)
- [x] M2.6 `commits.rs` ∥: `latest`, `count` (cached in `cache_commit_count`, cancellable every 256), `page`
  (pinned tip + skip, take 36, path filter with first-parent rule, `scan_cap` 20k, `hide` for since),
  `file_last_commit`. Tests: concatenated pages == one full walk == `git log --format=%H` on the merges fixture;
  path filter; since-tag; count cache hit; cancel returns `Cancelled`. (FR-14, FR-26, FR-31 compare, NFR-P5, D-9)
- [x] M2.7 `diff.rs` ∥: `commit_detail(oid, DiffLimits)`, `file_diff(index, force)`, generated heuristic, root vs
  empty tree, merge vs first parent + note, renames (threshold 50), CRLF stripped, `NoNewline`. Tests: stats vs
  expected numbers (and `git show --stat` when available), rename `old → new`, binary, > 2,000-line file collapsed,
  huge-commit → `diffs_deferred`. (FR-27, FR-28, R-8)
- [x] M2.8 `lastcommit.rs` ∥: `for_dir` per ARCH §6.1 (memoised subtree lookups, extra `oid_C == pending[e]` rule,
  cap 10k, cancel every 256, cache in `cache_last_commit`; walk runs without holding the DB mutex). Tests vs `git log
  -1 -- <path>` on linear, ordinary merge, dropped-side merge; documented divergence (side revert) asserted as such;
  cap → `capped=true`; cancel; second call served from cache. (FR-16, NFR-P3, R-1)
- [x] M2.9 `langs.rs` + `langs_table.rs` ∥: `classify` (filename then extension, ≈ 80 languages, data formats
  excluded except HTML/CSS), exclusion list §6.2, `for_tree` (sizes via `read_header`, top 6 + Other, cached by tree,
  cancellable every 1,024 entries). Tests: `.rs`/`.ts` percentages, `vendor/` excluded, cache hit. (FR-19, NFR-P6)
- [x] M2.10 `markdown.rs` + `readme.rs` + `license.rs` ∥: `render(md, base_dir)` per §6.6 steps 1–4 (comrak GFM →
  ammonia allow-list with `url_relative` custom → `rsrepo:/<path>`, `id_prefix("user-content-")`,
  `rel="noopener noreferrer"`, `code[class=language-*]` only from a fixed list, `data:image/*` only in img), headings
  outline; `find_readme`, `render_readme` (plain files → escaped `<pre class="readme-plain">`), `render_file`;
  `detect_license`. Tests: security fixture — script, `onerror`, `javascript:`, iframe, `style=`, form, `data:text/html`
  all stripped; remote img kept as `https` URL (blocked in UI); relative link/img rewritten, escape above root dropped;
  tables, task lists, fenced code class kept; README priority order; MIT/Apache detection. (FR-17, FR-18, NFR-S1, R-2)
- [x] M2.11 `releases.rs` ∥: `is_version_tag`, `Version` ordering, `list` (title/notes split, prerelease, latest,
  `previous_tag`). Tests: releases fixture gives 4 version cards in order, `-rc1` prerelease, latest = newest stable,
  lightweight tag gets no notes. (FR-31)
- [x] M2.12 Immutability test (`tests/immutability.rs` in rs-core): hash every file under `.git` and the work tree of
  an imported non-bare fixture, call every read function of M2.1–M2.11 (with `show_dirty_note` off and on), re-hash →
  equal. (NFR-O2, FR-13)
- [x] M2.13 `rs-cli`: `src/fixtures.rs` (`rs fixtures <dir>` builds every fixture), `src/bench.rs` (`rs bench <dir>`:
  `tree.list_dir` 1,000 entries < 300 ms, `lastcommit` second visit < 50 ms, `commits.page` skip 0 and 9,965 < 200 ms,
  `langs.for_tree` 50k files < 3 s, `diff.commit_detail` 400 files < 1 s; prints a table + JSON; exit 1 on any miss),
  `rs inspect <repo-path> <cmd>` (prints JSON of one read function, for debugging). (NFR-P2, P3, P5, P6, FR-28)
- [x] M2.14 Clippy clean.

Order: M2.1 → M2.2 → M2.3 ∥ M2.4 ∥ M2.5 ∥ M2.6 ∥ M2.7 ∥ M2.9 ∥ M2.10 ∥ M2.11 (all need fixtures + refs::resolve —
write `refs::resolve` first inside M2.3 and land it before the others start) → M2.8 (uses tree + commits helpers)
→ M2.12 → M2.13 → M2.14.

**Gate M2**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
cargo test -p rs-core; if ($LASTEXITCODE -ne 0) { throw 'rs-core tests' }
cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy' }
$s = 'target\scratch\m2-bench'; New-Item -ItemType Directory -Force $s | Out-Null
cargo run -p rs-cli --release -- fixtures $s; if ($LASTEXITCODE -ne 0) { throw 'fixtures' }
cargo run -p rs-cli --release -- bench $s; if ($LASTEXITCODE -ne 0) { throw 'bench targets missed' }
npm run typecheck; npm test; npm run build    # earlier gates still green
```
Must be true: bench exits 0 with all five numbers under target; immutability test green; comparison tests against
`git` ran (git is on PATH on this laptop; note in WORKLOG if skipped); tag `m2-done`. Remove `target\scratch\m2-bench`
afterwards (`Remove-Item -LiteralPath` in its own command).

---

## M3 — Data layer: library, clone, fetch (rs-core)

Goal: shelf management and the only network code. ARCH §3.4 (library, clone, fetch, ghreleases), §5.3, §5.4, §0.2.

- [x] M3.1 `library.rs` ∥: `list` (DB only, joins `repo_meta` + `repo_topics`), `by_slug`, `preview_import`
  (canonical path, `is_bare`, origin, duplicate check, slug `owner/name` with `-2` suffix, D-12; token is added by
  src-tauri), `import` (no writes to the repo), `register_clone`, `set_metadata` (validated, FR-20),
  `relocate`, `remove` (returns `Removed`, DB rows cascade), `touch_opened`, `recent(n)` (D-28),
  `refresh_summaries` (exists? open? tip changed → `last_commit_at`, `default_branch`, `primary_language` via
  `langs::for_tree`, status ok/missing/corrupt/empty; cancellable). No `search` (D-27). Tests: import non-bare, bare,
  duplicate (canonical, different casing/trailing slash), non-repo refused with message; repo files + `.git` mtimes
  unchanged; slug collision `-2`; metadata persists across `Shelf` reopen; invalid topics/homepage refused; missing
  fixture → status Missing without error; corrupt → Corrupt with message; relocate. (FR-1, FR-3 data, FR-4, FR-5,
  FR-20, FR-36 record, L-12, NFR-O3)
- [x] M3.2 `testkit` additions: `winnames` upstream (`a:b.txt`, `CON.txt`, trailing-dot file, long path), `refclash`
  upstream (`llm` + `LLM/x` branches, invalid-on-Windows tag name), `upstream_downstream()` pair helper, and a
  network-call counter (`testkit::net_calls()`; incremented in the single place that connects a git2 remote and in
  `ghreleases`). (FR-9, NFR-O1)
- [x] M3.3 `clone.rs`: `preview` (CloneUrl → target dir preview), `clone(&Shelf, &CloneUrl, &JobCtx)` per §5.4 steps
  1–7 (bare `init_bare(long(partial))`, `core.longpaths`, no credentials callback, `follow_redirects(Initial)`,
  advertised default, `storable_refs`, `AutotagOption::None`, progress via ctx, cancel via callback, `refs/heads/
  <default>`, HEAD, `origin/HEAD`, standard fetch refspec, FINALIZE mutex + unique `<name>.git`/`<name>-2.git`,
  `remove_dir_all` on error/cancel), `cleanup_partials(archive)`, and `clone_from_unchecked` under
  `cfg(any(test, feature="testkit"))` sharing the same inner function (D-14). Tests (file:// upstream via
  unchecked): result is bare with expected refs; refclash/invalid refs skipped and listed; winnames fixture clones
  fine (bare); cancel mid-transfer → no folder and no partial; leftover `.x.git.partial-*` removed by
  `cleanup_partials`; second clone of same URL → `-2`; default branch unstorable → `Refused`. (FR-6..9, NFR-W1, R-4,
  R-6)
- [x] M3.4 Validator purity test: for every rejected URL form, `clone::preview`/the job entry returns `InvalidUrl` and
  `testkit::net_calls()` stays 0; `clone()` signature accepts only `&CloneUrl` (type-level). (FR-6, NFR-S5)
- [x] M3.5 Redirect check (*verify per ARCH §5.4 step 2*): test against a local `http://127.0.0.1` TCP listener that
  answers a redirect to `http://` is not feasible over https without certs — instead confirm from libgit2/WinHTTP
  source/docs that `RemoteRedirect::Initial` refuses https→http downgrade; if not guaranteed, add a
  `certificate_check`/transport URL check that fails the job. Record the finding as a decision. (NFR-S5, R-10)
- [x] M3.6 `fetch.rs`: `fetch(&Shelf, repo_id, &JobCtx)` = policy (origin re-validated with `clone_url`, else
  `Refused("Fetch needs an https origin")`) + `fetch_from(source, ...)` implementing §5.4 fetch steps 1–6 (snapshot,
  advertised, `storable_refs`, staging namespace `refs/reposhelf-staging/tags/*` cleared before and after — also on
  error, adopt free tag names, report `kept_local_tags`, `new_commits` capped 100k, `last_fetched_at`, summary
  refresh); `fetch_unchecked` under `cfg(any(test, feature="testkit"))` (D-20). Tests: upstream adds 2 commits + 1
  tag → `new_commits=2`, `new_tags=[tag]`; a local tag with the same name but different target untouched and listed;
  imported non-bare fixture: hash index, HEAD, `refs/heads/*`, existing tags, config, work tree before/after → equal
  (only allowed refs changed, §0.2); staging refs gone after an injected error; http origin refused. (FR-10, R-5,
  NFR-O2 exception)
- [x] M3.7 `fetch::fast_forward_default` (managed only): FF when ancestor, no-op when equal, `Refused("Branch has
  diverged from origin")` when diverged; imported → `Refused`. Tests for all three. (FR-11)
- [x] M3.8 `ghreleases.rs` (feature `ghreleases`, D-22): `fetch_github_notes` (github.com origins only, one GET
  `/repos/<o>/<r>/releases?per_page=100`, 10 s timeout, generic User-Agent `RepoShelf`, body through
  `markdown::render`, cached in `cache_gh_releases`), split into `parse_releases(json) -> Vec<GhRelease>` and
  `merge_into(releases, gh)`. Tests with canned JSON only (no network): parse, merge by tag name, non-github origin →
  `Refused`. (FR-31 optional, NFR-O1)
- [x] M3.9 Network-boundary test: run every M2 read function and every library function on fixtures → `net_calls()
  == 0`. (NFR-O1)
- [x] M3.10 Clippy clean; `cargo test -p rs-core --features ghreleases` and without.

Order: M3.1 ∥ M3.2 → M3.3 → M3.4 ∥ M3.5 → M3.6 → M3.7 → M3.8 (∥ with M3.3–M3.7 if staffed; separate file) → M3.9 →
M3.10.

**Gate M3**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
cargo test -p rs-core; if ($LASTEXITCODE -ne 0) { throw 'rs-core' }
cargo test -p rs-core --features ghreleases; if ($LASTEXITCODE -ne 0) { throw 'rs-core ghreleases' }
cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy' }
cargo test --workspace; if ($LASTEXITCODE -ne 0) { throw 'workspace' }
npm run typecheck; npm test; npm run build
```
Must be true: no test reaches the network (all clones/fetches use file:// via the unchecked seam); every FR-5..11
test listed above exists; tag `m3-done`.

---

## M4 — IPC commands + tests (src-tauri)

Goal: thin command layer over rs-core, security config, TS type mirror. ARCH §5.1, §5.2, §5.5, §8. After this the
backend is complete and frozen except for bug fixes.

- [x] M4.1 `src-tauri/src/state.rs`: `AppState { shelf: Arc<Shelf>, jobs: JobRunner, requests: CancelRegistry,
  compute: ComputeGate, subscribers: Mutex<Vec<Channel<JobInfo>>>, import_tokens: Mutex<HashMap<String,(PathBuf,
  Instant)>>, trasher: Arc<dyn Trasher> }`; `CancelRegistry` (`register(requestId) -> Guard` that unregisters on
  drop, `cancel(requestId)`). Worker closure dispatching `JobKind::Clone` → `clone::clone`, `Fetch` → `fetch::fetch`,
  `FetchAll` → sequential fetch over fetchable repos (cancellable).
- [x] M4.2 `ipc_error.rs`: `IpcError { code, message }` from `rs_core::Error`; friendly git messages (auth/401/403 →
  credentials text; 404; Net/Http/Ssl → "Could not reach <host>"). Tests per mapping.
- [x] M4.3 Startup in `lib.rs`: `resolve_dirs`, create dirs, `Shelf::open`, `clone::cleanup_partials`,
  `jobs.load_persisted`, `db::prune_caches`, background `library::refresh_summaries`; create window `main` in code
  (so `data_directory(<RS_DATA_DIR>/webview)` applies when `RS_DATA_DIR` is set — *verify* the Tauri 2.11
  `WebviewWindowBuilder::data_directory` API) and register `on_navigation` returning false for non-app URLs (and deny
  new windows). Remove the window from `tauri.conf.json` if created in code. (FR-35, FR-8, NFR-S3)
- [x] M4.4 `cmd/app.rs` ∥: `app_info` (sizes: cache = sum of cache-table JSON lengths; archive bytes computed
  lazily/capped), `settings_get`, `settings_set`, `cache_clear`, `request_cancel`, `open_external` (`validate::
  external_url`, opener from Rust). (FR-33, FR-35, S10, NFR-S3)
- [x] M4.5 `cmd/library.rs` ∥: `library_list`, `library_refresh(requestId)`, `repo_by_slug`, `repo_set_metadata`,
  `repo_detail` (+ `touch_opened`), `repo_open_folder` (bare → reveal `<name>.git`, else open work tree, from Rust).
  (FR-1, FR-4, FR-12, FR-18, FR-20)
- [x] M4.6 `cmd/dialogs.rs` ∥: `import_pick` (Rust folder dialog → `preview_import` → token, D-18), `import_confirm(
  token)` (expired/unknown token → `InvalidInput`), `repo_locate` (folder dialog → `relocate`), `repo_remove`
  (`library::remove`; cloned → `trasher.delete(path)`; imported → nothing on disk), `blob_save_as` (save dialog →
  write bytes). `Trasher` trait + `RealTrash` (D-30). Dialog calls go through a `Picker` trait so tests inject
  paths. (FR-5, FR-4, FR-36, FR-23, NFR-S4, D-10)
- [x] M4.7 `cmd/tree.rs` + `cmd/blob.rs` ∥: `refs_list`, `rev_resolve`, `tree_list`, `tree_last_commits`
  (requestId, compute gate), `commit_latest`, `commit_count` (requestId), `readme_render`, `markdown_render_file`,
  `languages` (requestId), `blob_meta`, `blob_text`, `blob_bytes` (`tauri::ipc::Response`, > 5 MB → `Refused`,
  mime from extension allow-list), `file_last_commit`. Every arg validated per §5.2 before rs-core. (FR-13..19,
  FR-21..25, NFR-P4, NFR-S4, NFR-S7, D-11)
- [x] M4.8 `cmd/commits.rs` + `cmd/refs.rs` + `cmd/releases.rs` ∥: `commits_page` (limit fixed 35, skip ≤ 10M,
  optional `tip`, `since`, `path`), `commit_detail` (oid 40 hex), `commit_file_diff`, `branches_list`,
  `branches_ahead_behind(names, requestId)`, `tags_list`, `releases_list` (merges cached GH notes),
  `releases_fetch_github` (returns `unsupported` if feature off). (FR-26..31)
- [x] M4.9 `cmd/jobs.rs`: `clone_preview`, `clone_start`, `fetch_start` (imported repos allowed when fetchable; the
  confirm is the UI's job), `fetch_all_start`, `fast_forward`, `jobs_subscribe(channel)` (store channel, return
  snapshot), `jobs_cancel`, `jobs_retry`, `jobs_dismiss`. Under `#[cfg(feature = "e2e")]` only: when env
  `RS_TEST_FILE_CLONE=1`, `clone_start` with a `file://` input uses `clone_from_unchecked` and fetch of a `file://`
  origin uses `fetch_unchecked` (D-14, D-20). The literal `RS_TEST_FILE_CLONE` must appear only inside cfg(e2e)
  code. Sink pushes every `JobInfo` change to all live channels, dropping closed ones. (FR-6..11, NFR-P7)
- [x] M4.10 `tauri.conf.json` CSP/devCsp re-checked against ARCH §8, no `assetProtocol`; `capabilities/default.json`
  = `core:default`; try narrowing to `core:event:default` + `core:app:default` (+ what `Channel` needs) and keep the
  narrowest set that runs. (NFR-S2, NFR-S6)
- [x] M4.11 Command-layer tests (`src-tauri/tests/` or `#[cfg(test)]` modules calling the inner `fn`s with a test
  `AppState` over scratch dirs, no window): hostile table (`..`, `a/../b`, `/etc`, `C:\x`, `c:/x`, `a\b`, `a\0b`,
  `refs/../x`, `HEAD~1`, `a..b`, `@{-1}`) refused with `invalidPath`/`invalidRev` for **every** command taking `path`
  or `rev`; non-hex/short oid refused; unknown `repoId` → `notFound`; `open_external` refuses `javascript:`,
  `file:`, `ftp:`, userinfo; `blob_bytes` cap; `import_confirm` with bad token; remove cloned → fake trasher got the
  path, imported → not called; job subscribe snapshot contains submitted job; `releases_fetch_github` works with the
  feature on (canned) and `unsupported` off. Commands are written as `#[tauri::command] async fn x(state, …)` wrappers
  over plain `fn x_impl(&AppState, …)` so tests need no runtime. (PoC criterion 4, NFR-S4)
- [x] M4.12 `src/ipc/types.ts`: TS mirror of every type in `types.rs` (+ `IpcError`, `JobInfo`, `Settings`).
  Rust test `ipc_samples` serialises one sample of every type (every enum variant) to
  `target/ipc-samples.json` and compares with committed `src/ipc/__samples__/ipc-samples.json` (`UPDATE_SAMPLES=1`
  rewrites the golden). `src/ipc/types.test.ts` loads the golden and checks each sample against a hand-written shape
  validator per TS type (keys and primitive kinds), so a camelCase drift fails one side (D-24).
- [x] M4.13 `cargo build -p reposhelf` and `cargo build -p reposhelf --features e2e` both compile (long first build:
  background / 10 min).
- [x] M4.14 Live check: with `RS_DATA_DIR`/`RS_ARCHIVE_DIR` set to `target\scratch\m4`, run `npx tauri dev`
  (window on the main display), confirm the M0 page renders and the devtools console has no CSP errors, then
  `taskkill /T /F` the tree and verify leftovers 0.
- [x] M4.15 Clippy clean for the whole workspace incl. `--features e2e`.

Order: M4.1 → M4.2 → M4.3 → (M4.4 ∥ M4.5 ∥ M4.6 ∥ M4.7 ∥ M4.8 ∥ M4.9 — each its own `cmd/*.rs`; one person
registers all commands in `lib.rs`'s `generate_handler!` after they land) → M4.10 → M4.11 → M4.12 → M4.13 → M4.14 →
M4.15.

**Gate M4**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
cargo test -p rs-core --features ghreleases; if ($LASTEXITCODE -ne 0) { throw 'rs-core' }
cargo test -p reposhelf; if ($LASTEXITCODE -ne 0) { throw 'reposhelf tests' }      # long first run
cargo test -p reposhelf --features e2e; if ($LASTEXITCODE -ne 0) { throw 'reposhelf e2e-feature tests' }
cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy' }
cargo clippy -p reposhelf --all-targets --features e2e -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy e2e' }
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }      # includes types.test.ts against the committed golden
npm run build; if ($LASTEXITCODE -ne 0) { throw 'build' }
```
Must be true: every command of ARCH §5.1 is registered and has at least one test; the hostile table passes for every
path/rev command; `npx tauri dev` ran once with zero CSP errors and was killed (leftovers 0); no file under
`src/components` or `src/screens` exists yet; tag `m4-done`. **Backend done.**

---

## M5 — Frontend shell + mock

Goal: router, typed IPC + mock backend, stores, CSS tokens, global header, repo layout with tabs, highlighter worker,
markdown mount pipeline, dialog primitives. ARCH §7.1, §7.3–7.6. Screens are placeholders here.

- [x] M5.1 `src/router/` ∥: `router.ts` (`parse(hash)`, `href(name, params, query)`, `navigate(to,{replace})`,
  `hashchange` listener, history-state keys + `Map<key, scrollY>` restore, mouse buttons 3/4 and Alt+Left/Right →
  `history.back/forward`), `routes.ts` (patterns of ARCH §7.1, one encoded `:ref` segment D-7, `?L=` D-8),
  `Link.tsx`, `useRoute.ts`, `copyLocation()` (decoded GitHub-style `owner/repo/tree/<ref>/<path>`). Tests
  `router.test.ts`: parse/href round-trips for every pattern, `feature%2Fx`, `?L=10-20`, unknown → NotFound,
  `#/settings` vs `#/:owner/:repo`. (FR-13, FR-21, FR-22, FR-34)
- [x] M5.2 Scroll memory: virtualised lists use `useWindowVirtualizer` (window scroll) so the router's restore applies
  (D-31); test that restore runs after the route's first render. (FR-34)
- [x] M5.3 `src/ipc/` ∥: `bridge.ts` (only file importing `@tauri-apps/api`; `isTauri`; `invoke`, `Channel`; errors →
  `IpcError`), `errors.ts`, `api.ts` (one typed function per §5.1 command; `requestId` generation for cancellable
  ones), `query.ts` (`useQuery(key, fn, {cancelable})`, LRU 200, oid-keyed entries never stale, ref-keyed entries
  invalidated on window focus and job completion, `request_cancel` on unmount/route change). Test `query.test.ts`:
  cache hit, cancel on unmount calls `request_cancel`, invalidation. (FR-16, FR-34, NFR-S4)
- [x] M5.4 `src/ipc/mock/` (after M5.3): `index.ts` (implements every `api.ts` function), `switches.ts` (`?empty`,
  `?slow` 600 ms, `?clonefail`, `?fetchfail`, `?huge`), `fixtures/*.ts` (imported Rust repo with README/table/code/
  relative image, cloned TS repo with releases, empty repo, missing repo; security README; `?huge`: 1,000-entry dir,
  10k-commit paging, 400-file commit), `mockJobs.ts` (timer progress, cancel, retry). Fixtures typed with `types.ts`
  so drift fails typecheck. Test `mock.test.ts`: each switch changes behaviour. (FR-37, L-7)
- [x] M5.5 `src/state/` ∥: `theme.ts` (light/dark/system via `matchMedia`, `data-theme` on `<html>`, persisted via
  `settings_set`), `settings.ts`, `library.ts` (list + background `library_refresh` + `filterRepos`/`sortRepos`, D-27),
  `jobs.ts` (`jobs_subscribe` once, again after reload; done rows auto-hide after 5 s; failures stay), `recent.ts`
  (top 5 by `lastOpenedAt`, D-28). All `useSyncExternalStore`. Tests: `filterRepos` ("pars" matches name/description/
  topics, case-insensitive), `sortRepos` (3 keys × direction), jobs auto-hide timer. (FR-2, FR-3, FR-7, FR-33)
- [x] M5.6 `src/styles/` ∥: `tokens.css` (all token groups of ARCH §7.5, own teal-indigo accent, light + dark),
  `base.css`, `layout.css` (1280 px max, 1000 px breakpoint), `components.css`, `code.css` (`.hljs-*` → tokens),
  `diff.css` (filled in M7), `markdown.css`; fonts from `@fontsource-variable/*`. Test `contrast.test.ts`: parse
  `tokens.css`, every text/background pair ≥ 4.5:1 in both themes. (FR-33, R-11)
- [x] M5.7 `src/hl/` ∥: `languages.ts` (extension/filename/shebang → hljs id, ≈ 45), `splitLines.ts` (close/reopen
  spans per line), `hl.worker.ts` (core + lazy `import()` per language, 2,000-line chunks, > 1 MB or > 20k lines →
  plain), `highlighter.ts` (worker client, request ids, cancel). Tests: `splitLines` nested spans, `languages` map,
  shebang. (FR-22, NFR-P4, NFR-P8, D-5)
- [x] M5.8 `src/markdown/` (after M5.1, M5.7): `linkRewrite.ts` (on an inert `<template>`: `rsrepo:` anchors → `blob`
  routes, `rsrepo:` images → `blob_bytes` → `blob:` URLs revoked on unmount, remote images → "Remote image: <host>"
  link chip, any href not http/https/mailto/rsrepo/`#fragment` removed), `MarkdownBody.tsx` (mount, intercept all
  clicks: in-app → router, external → `ConfirmExternalLink` → `open_external`; `code.language-*` → highlighter).
  Test `linkRewrite.test.ts`: rsrepo link, rsrepo img, remote img replaced, `javascript:` dropped, fragment kept.
  (FR-17, NFR-S1, NFR-S2, NFR-S3)
- [x] M5.9 `src/components/ui/` ∥: Button, Dropdown, Dialog (focus trap, Esc), Popover, Chip, Badge, Shimmer,
  RelativeTime, CopyButton, Avatar (initials), Empty, ConfirmExternalLink; `src/util/format.ts` (bytes, relative
  time, day groups) + `format.test.ts`.
- [x] M5.10 `src/components/layout/`: `GlobalHeader` (S0: logo + name → home; `RepoJump` search with dropdown and `/`
  shortcut; `AddMenu` "+" with Import folder… / Clone URL… opening dialog skeletons; `JobsButton` + `JobsPopover`
  with count badge, progress rows, Cancel, Retry; `ThemeToggle`; settings link), `RepoHeader` (S2: book icon,
  `owner / name`, source badge, Fetch / Open folder / "…" placeholders), `TabBar` (Code, Commits, Branches, Tags,
  Releases + counts), `RepoBanner`, `Sidebar`. Visible focus rings, sane tab order, `/` focuses search (NFR-A1).
  (FR-12, FR-32)
- [x] M5.11 `src/App.tsx` + `RouteView`: every route of ARCH §7.1 mapped to placeholder screens
  `src/screens/<Name>/index.tsx` (all screen folders created now, so M6/M7 never edit `App.tsx` in parallel);
  `RepoLayout` resolving `slug → repoId` (`repo_by_slug`) and `rev → ResolvedRev`; NotFound.
- [x] M5.12 `tools/shots/mock-shots.md`: table of route × theme × switch to screenshot in M5–M7.
- [x] M5.13 Browser check: start `reposhelf-mock` with the preview tools, visit `#/`, `#/settings`, a repo route,
  each tab, `?empty`, `?slow`, light + dark; screenshot the shell; console has no errors; stop the server.

Order: M5.1 ∥ M5.3 ∥ M5.5 ∥ M5.6 ∥ M5.7 ∥ M5.9 → M5.4, M5.8 → M5.2 → M5.10 → M5.11 → M5.12 → M5.13.

**Gate M5**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }
npm run build; if ($LASTEXITCODE -ne 0) { throw 'build' }
# NFR-P8: initial JS = the .js files referenced by dist\index.html (entry + modulepreload), sum < 1.5 MB
$entry = Select-String -LiteralPath dist\index.html -Pattern 'assets/[^"]+\.js' -AllMatches | ForEach-Object { $_.Matches } | ForEach-Object { $_.Value } | Sort-Object -Unique
$bytes = ($entry | ForEach-Object { (Get-Item -LiteralPath (Join-Path 'dist' $_)).Length } | Measure-Object -Sum).Sum
if ($bytes -ge 1.5MB) { throw "initial JS $bytes bytes" }
cargo test --workspace; if ($LASTEXITCODE -ne 0) { throw 'cargo' }     # backend still green
```
Must be true: highlight.js languages are separate chunks in `dist\assets`; mock shell screenshots in light + dark
taken and reviewed; no console errors; tag `m5-done`.

---

## M6 — Screens A: Dashboard, Code tab, File view, dialogs

Goal: S1, S2 complete, S3, S4, S10 dialogs (Import, Clone, Edit About, Remove, Fetch confirm). ARCH §7.2.

- [ ] M6.1 `src/util/paths.ts` (breadcrumb segments, parent dir, git path join) + `src/util/lang-colors.ts` (own
  palette, theme-aware), with tests.
- [ ] M6.2 `src/screens/Dashboard/` + `src/components/dashboard/` ∥: `RepoToolbar` (filter bound to `?q=`, `SortMenu`
  persisted, `SourceFilter`, New → AddMenu), `RepoList` (virtual > 500), `RepoRow` (name link, `SourceBadge`, 2-line
  description, `TopicChips`, `LangDot` + name, origin host/owner, "Updated …"), `MissingRow` (Locate… →
  `repo_locate`, Remove from shelf), `Sidebar` (Recent 5, archive size + count from `app_info`, optional Fetch all),
  `EmptyState` (Import / Clone). Test `RepoList.test.tsx`: filter "pars", sort order, missing-row actions.
  (FR-1, FR-2, FR-3, FR-4, S1)
- [ ] M6.3 `src/components/dialogs/` ∥: `ImportDialog` (`import_pick` → summary incl. duplicate/non-repo errors →
  `import_confirm(token)`), `CloneDialog` (URL input, debounced `clone_preview` live validation, host shown for
  confirmation, target folder preview, Clone → `clone_start`; progress lives in the jobs popover), `EditAboutDialog`
  (description, homepage, topics chip input with client rules mirroring `validate::topics`; server errors shown),
  `RemoveDialog` (imported: "unregister only, folder untouched"; cloned: "moves <path> to the Recycle Bin"),
  `FetchConfirmDialog` for imported repos (ARCH §0.2 wording). Test `CloneDialog.test.tsx`: each rejected form shows
  the backend message; a valid URL shows host + target. (FR-5, FR-6, FR-10, FR-20, FR-36, S10)
- [ ] M6.4 RepoLayout completion (S2, after M6.3): Fetch button (clones, and fetchable imported repos via confirm) →
  `fetch_start`, result "N new commits"; Fast-forward button when `canFastForward`; Open folder; "…" → RemoveDialog;
  tab counts from `repo_detail` + cached commit count; missing/corrupt banner; origin link → ConfirmExternalLink.
  (FR-10, FR-11, FR-12, NFR-O3)
- [ ] M6.5 `src/screens/RepoCode/` + `src/components/code/` ∥: `RefSwitcher` (Branches|Tags tabs, filter, default
  marked, tags linked as `refs/tags/<name>`, detached shows short oid), `CountsLinks`, `CodeMenu` ("Copy clone URL",
  "Open folder", "Archived as a bare repository"), `Breadcrumbs` + Copy path, `LatestCommitBox` (initials, author,
  summary link, short oid, relative date, `commit_count` spinner → "N commits"), `FileTable` (`useWindowVirtualizer`
  > 500, `..` row, `EntryIcon` per kind, `LastCommitCell` shimmer until `tree_last_commits` resolves, cancelled on
  navigation), `ReadmeCard` (outline button from headings, `MarkdownBody`), `AboutSidebar` (gear → EditAbout,
  description, homepage, topics, origin, license, "Imported from" / "Cloned <date> · last fetched", counts,
  `ReleasesBox`, `LanguagesBox` with `LangBar` + `LangList`), edge states (empty repo, detached, ref not found with
  link to default). Tests: `FileTable.test.tsx` (order, `..` row, shimmer then messages), `RefSwitcher.test.tsx`
  (filter, default marker, tag href). (FR-13..19, FR-21, S3, NFR-P2)
- [ ] M6.6 NFR-W3 badge in `FileTable`/FileView: entries with `storableHere=false` in imported non-bare repos show
  "not checked out on this PC"; non-UTF-8 names show a marker and browse via `pathRaw`. (NFR-W3, R-7)
- [ ] M6.7 `src/screens/FileView/` + `src/components/file/` ∥: `FileCommitHeader` (`file_last_commit`, History →
  commits `?path=`), `FileToolbar` (lines/sloc/size, Raw `?plain=1`, Copy, Save a copy → `blob_save_as`, History),
  `CodeViewer` (virtualised gutter + lines, raw first then highlighted chunks, click/shift-click selects `?L=a-b`,
  anchor scrolls + highlights, "Copy link" in `#L10-L20` form), `ImagePreview` (blob URL on a checkerboard; svg only
  via `<img>`), `Placeholder` (binary, > 1 MB unhighlighted, > 5 MB Raw/Save only, LFS, submodule, symlink, lossy
  notice, not checked out), `MarkdownTabs` (Preview via `markdown_render_file` / Code). Directory path on a blob
  route → redirect to tree. Test `CodeViewer.test.tsx`: `?L=10-20` selects range, shift-click extends.
  (FR-21..25 incl. FR-24 images, NFR-P4, NFR-S7, D-8)
- [ ] M6.8 Mock fixtures extended (every placeholder kind, security README, `?huge` dir); `mock-shots.md` updated.
- [ ] M6.9 Browser check (mock): S1 (normal, `?empty`, missing row), S3 (root, subdir, tag, empty repo, detached, ref
  not found, security README: no remote image loaded, no script), S4 (code with `?L=`, image, svg, each placeholder,
  markdown preview), `?huge` 1,000-entry dir, all dialogs; light + dark; console clean.

Order: M6.1 → M6.2 ∥ M6.3 ∥ M6.5 ∥ M6.7 → M6.4 → M6.6 → M6.8 → M6.9.

**Gate M6**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }
npm run build; if ($LASTEXITCODE -ne 0) { throw 'build' }
cargo test --workspace; if ($LASTEXITCODE -ne 0) { throw 'cargo' }
```
Must be true: component tests above green; S1/S3/S4/dialog screenshots in light + dark reviewed (listed in WORKLOG);
no console errors; tag `m6-done`.

---

## M7 — Screens B: Commits, Commit, Branches, Tags, Releases, Settings, jobs final

- [ ] M7.1 `src/screens/Commits/` + `src/components/commits/` ∥: RefSwitcher, `PathChip` ("History for <path> ×"),
  `DayGroup` ("Commits on Nov 11, 2026", local date), `CommitRow` (summary link, expandable body, author + relative
  date, `CopyOid`, browse-tree icon), `Pager` (Newer/Older with pinned `?tip=` + `?page=`), "Search older commits"
  when `capped`, `?since=` banner. Tests: day grouping across midnight/time-zone offsets, pager hrefs keep `tip`.
  (FR-26, FR-31 compare, NFR-P5, D-9)
- [ ] M7.2 `src/screens/Commit/` + `src/components/diff/` ∥: `CommitHeader` (summary, pre-wrapped body,
  author/committer, parent links, full oid copy, merge note), `DiffSummary` ("Showing N changed files with A
  additions and D deletions"), `FileDiffCard` (chevron, `MiniBar`, path or `old → new`, View file), `DiffTable` (old
  no | new no | marker | text; hunk rows tinted; add/del from tokens; no highlighting, D-17), collapsed cards
  (large / binary / generated / deferred → "Load diff" → `commit_file_diff(force)`), virtualised card list, `?file=N`
  scroll; `src/styles/diff.css`. Tests: DiffTable renders a hunk with NoNewline; collapsed card loads on click.
  (FR-27, FR-28, S6)
- [ ] M7.3 `src/screens/Branches/` + `src/components/refs/` ∥: filter, sections Default / Your branches / Remote
  branches, `BranchRow` (tree link, "Updated … by X", commit link), `AheadBehindBar` filled lazily by
  `branches_ahead_behind` (visible rows only, cancellable, capped marker). Test: bar widths for ahead/behind/capped.
  (FR-29, S7)
- [ ] M7.4 `src/screens/Tags/` ∥: Releases | Tags toggle, `TagRow` (name, date, short oid, annotated/lightweight
  badge, Browse files + commit links). (FR-30, S8)
- [ ] M7.5 `src/screens/Releases/` ∥: `ReleaseCard` (left: tag, short oid, date; right: title, Latest / Pre-release
  badge, `MarkdownBody` notes, "Commits since <prev>" → `commits/<tag>?since=<prev>`); `GhNotesButton` only for
  github.com origins and when the command is not `unsupported`, with a confirm ("one request to api.github.com"),
  fetched date shown. (FR-31, S9, NFR-O1)
- [ ] M7.6 `src/screens/Settings/` ∥: theme radio (light/dark/system), data dir + archive dir (read-only, open via
  backend), cache size + Clear cache, dirty-note toggle (D-16), app version. (FR-33, FR-35, S10)
- [ ] M7.7 Jobs popover final ∥: per-kind titles, progress from `Progress` variants, Cancel (waiting + running),
  Retry on failed, Dismiss, done rows auto-hide after 5 s, clone result lists skipped refs (FR-9 report), fetch result
  "N new commits, new tags …, kept local tags …"; `?clonefail` / `?fetchfail` states. Test: popover rows for each
  state. (FR-7, FR-9, FR-10)
- [ ] M7.8 Mock: `?huge` 400-file commit, branches with ahead/behind, releases incl. prerelease; `mock-shots.md`
  updated.
- [ ] M7.9 Browser check (mock): S5 (paging, path chip, since), S6 (normal, merge, root, `?huge`), S7, S8, S9, S10,
  jobs popover with `?clonefail` / `?fetchfail`; light + dark; console clean.

Order: M7.1 ∥ M7.2 ∥ M7.3 ∥ M7.4 ∥ M7.5 ∥ M7.6 ∥ M7.7 (separate folders; routes exist since M5.11) → M7.8 → M7.9.

**Gate M7**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }
npm run build; if ($LASTEXITCODE -ne 0) { throw 'build' }
# repeat the M5 initial-JS < 1.5 MB check (NFR-P8)
cargo test --workspace; if ($LASTEXITCODE -ne 0) { throw 'cargo' }
```
Must be true: every screen S0–S10 renders in the mock in light and dark (screenshots listed in WORKLOG); initial JS
still < 1.5 MB; no console errors; tag `m7-done`.

---

## M8 — Integration / real-app run

Goal: the real app against scratch dirs, e2e smoke, security / perf / offline passes, TESTING.md. ARCH §9, §10 M8.

- [ ] M8.1 e2e build (D-25): `npm run build`, then
  `$env:CARGO_TARGET_DIR = 'target\e2e'; npx tauri build --no-bundle --features e2e; Remove-Item Env:CARGO_TARGET_DIR`
  → `target\e2e\release\reposhelf.exe`. Check it renders the dashboard with nothing listening on port 1450 (it embeds
  `dist`, not the dev URL).
- [ ] M8.2 `tools/e2e/run-e2e.ps1` (modelled on PM's `tools/e2e`, read-only reference): scratch `RS_DATA_DIR` /
  `RS_ARCHIVE_DIR`; `rs fixtures` (3 imported fixtures + an upstream for the file clone); hash the imported fixtures'
  `.git` + work tree; start tauri-driver + msedgedriver (port 4444 may be busy from other sessions → pick a free
  port); launch the e2e exe with `RS_TEST_FILE_CLONE=1`, window moved offscreen in-process; run `wd-e2e.mjs`;
  `taskkill /T /F` the tree; count leftovers (must be 0); re-hash (equal for the browse-only phase); print JSON
  results; `-Network` switch for the public-clone run. (R-12, L-8, NFR-O2)
- [ ] M8.3 `tools/e2e/wd-e2e.mjs`: smoke dashboard → import (the `Picker` returns a scratch fixture path from an env
  var, honoured only under `e2e`) → repo → file → commits → commit → branches → tags → releases → clone (file seam) →
  fetch (file seam, D-20) → remove cloned (real trash of the scratch clone only). Collects console messages and fails
  on any CSP violation or error; screenshots every screen S0–S10 in **light and dark** into `target\e2e\shots\`
  (success criterion 2, D-29); invokes the hostile IPC table from the webview → all refused (criterion 4).
- [ ] M8.4 `tools/e2e/check-e2e.ps1`: asserts the result JSON, shot count (S0–S10 × 2 themes), leftovers 0.
- [ ] M8.5 Timings (D-29): new rs-cli flag `rs fixtures --register 50 <dir>` registers 50 fixture copies in a scratch DB;
  wd-e2e measures exe start → dashboard rows visible (< 1.5 s warm, NFR-P1) and click into the 1,000-entry dir →
  table rendered (< 300 ms, NFR-P2) via `performance.now()`.
- [ ] M8.6 Security pass in the real app: security README (no script ran, remote image shown as a chip, no
  navigation), svg-with-script image, external link → confirm dialog appears (cancel it; do not open the browser),
  zero CSP violations across all screens incl. worker highlighting and virtual lists. (NFR-S1..S7)
- [ ] M8.7 Perf pass: `rs fixtures` + `rs bench` (release) green again. (NFR-P2, P3, P5, P6)
- [ ] M8.8 `-Network` run (opt-in): clone 2 small public repos (one with Windows-invalid names), then fetch; the
  skipped-refs report is shown. (Success criterion 1, FR-6..10)
- [ ] M8.9 Offline: the network-adapter-off browse run needs admin → write it as a hand step in TESTING.md (never
  change adapter settings). Automated proxies: `net_calls()` test (M3.9) and wd-e2e asserting no requests other than
  IPC/app origins during the browse-only phase. (NFR-O1)
- [ ] M8.10 `docs/TESTING.md`: every gate command, e2e usage, mock preview usage, hand-test checklist (theme toggle +
  restart, mouse back/forward, network-off run, clone of a big repo, Recycle Bin check).
- [ ] M8.11 Fix everything found (anywhere, including backend), re-run affected gates.

Order: M8.1 → M8.2 ∥ M8.3 ∥ M8.4 → M8.5 → M8.6 → M8.7 → M8.8 → M8.9 → M8.10 → M8.11 (loop until green).

**Gate M8**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
cargo test --workspace --all-features; if ($LASTEXITCODE -ne 0) { throw 'cargo test' }
cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy' }
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }
npm run build; if ($LASTEXITCODE -ne 0) { throw 'ui build' }
$env:CARGO_TARGET_DIR = 'target\e2e'; npx tauri build --no-bundle --features e2e; $rc = $LASTEXITCODE; Remove-Item Env:CARGO_TARGET_DIR; if ($rc -ne 0) { throw 'e2e build' }
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1; if ($LASTEXITCODE -ne 0) { throw 'e2e' }
powershell -ExecutionPolicy Bypass -File tools\e2e\check-e2e.ps1; if ($LASTEXITCODE -ne 0) { throw 'e2e check' }
$s = 'target\scratch\m8-bench'; New-Item -ItemType Directory -Force $s | Out-Null
cargo run -p rs-cli --release -- fixtures $s; if ($LASTEXITCODE -ne 0) { throw 'fixtures' }
cargo run -p rs-cli --release -- bench $s; if ($LASTEXITCODE -ne 0) { throw 'bench' }
```
Must be true: e2e smoke passes with JSON results; 0 CSP violations; S0–S10 shots in light and dark; hostile IPC
refused in the real app; imported `.git` hash unchanged after the browse-only phase; NFR-P1/P2 UI timings met;
`-Network` run done (or recorded as pending for the user with the reason); leftovers 0; tag `m8-done`.

---

## M9 — Docs + release build

- [ ] M9.1 Version `0.1.0` in `package.json`, root `Cargo.toml` (`workspace.package.version`),
  `src-tauri/tauri.conf.json`.
- [ ] M9.2 `tauri.conf.json` bundle: `targets: ["nsis"]`, `nsis.installMode: "currentUser"`, icons.
  `tools/build-portable.mjs` (as PM: `npx tauri build --no-bundle`, then copy `target\release\reposhelf.exe` to
  `target\release\portable\RepoShelf_<ver>_x64-portable.exe`, version read from `tauri.conf.json`; no `e2e` feature,
  no `--workspace`, D-26).
- [ ] M9.3 Build: `npm run build:installer` → `target\release\bundle\nsis\RepoShelf_0.1.0_x64-setup.exe` (built,
  **never run**); `npm run build:portable` → portable exe. Marker check: `RS_TEST_FILE_CLONE` absent from both (D-14,
  D-26).
- [ ] M9.4 Portable smoke: start the portable exe with scratch `RS_DATA_DIR`/`RS_ARCHIVE_DIR` (main display or
  offscreen), confirm the dashboard, kill the tree, leftovers 0.
- [ ] M9.5 `README.md` ∥: what RepoShelf is, screens, data locations (D-13) + env overrides, build/test commands, mock
  preview, e2e, known limits (deferred list from REQUIREMENTS §5).
- [ ] M9.6 Final `docs/DECISIONS.md`, `docs/WORKLOG.md` (handoff: hand tests pending), `docs/TESTING.md` ∥.
- [ ] M9.7 Copy installer + portable into `_REVIVE/releases/` only if `_REVIVE/` exists in this repo.
- [ ] M9.8 Re-run gates M0–M8 (bundle-size check included), commit, tag `m9-done` and `v0.1.0`.

Order: M9.1 → M9.2 → M9.3 → M9.4 → M9.5 ∥ M9.6 → M9.7 → M9.8.

**Gate M9**
```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
cargo test --workspace --all-features; if ($LASTEXITCODE -ne 0) { throw 'cargo test' }
cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw 'clippy' }
npm run typecheck; if ($LASTEXITCODE -ne 0) { throw 'typecheck' }
npm test; if ($LASTEXITCODE -ne 0) { throw 'npm test' }
npm run build:installer; if ($LASTEXITCODE -ne 0) { throw 'installer' }
npm run build:portable; if ($LASTEXITCODE -ne 0) { throw 'portable' }
$setup = 'target\release\bundle\nsis\RepoShelf_0.1.0_x64-setup.exe'; $port = 'target\release\portable\RepoShelf_0.1.0_x64-portable.exe'
if (-not (Test-Path -LiteralPath $setup) -or -not (Test-Path -LiteralPath $port)) { throw 'artifact missing' }
if (Select-String -SimpleMatch -Pattern 'RS_TEST_FILE_CLONE' -LiteralPath $setup, $port -Quiet) { throw 'test seam in shipped exe' }
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1; if ($LASTEXITCODE -ne 0) { throw 'e2e' }
```
Must be true: both artifacts exist; the installer was never executed; marker absent; portable smoke ok (leftovers 0);
docs final; tags `m9-done` and `v0.1.0` (local only, never pushed).

---

## M10 — Explore mode (PLAN ONLY — do not implement until the user says so)

Added 2026-11-11 at the user's request as plan-only. Not part of v0.1.0; implementers of M5–M9 must not build it.
Keep M6/M7 screens reading through the swappable data-source layer so this fits without redoing them.

- Explore mode: type `owner/repo` and the same GitHub-style screens fill from GitHub's public API online instead of
  from disk (Code tab, file view, README, commits, branches, tags, releases). Read-only.
- "Save to shelf" button: starts the existing bare clone job (background progress, Windows-safe names); the repo then
  appears in the local list.
- Optional search page: search GitHub repos, then open or save one.
- Constraints to resolve first: unauthenticated rate limit (~60 requests/hour) → cache responses, and optional token
  sign-in would need OS-keychain token storage (sign-in was out of scope for v0.1); CSP `connect-src` and Rust HTTP
  allow-list widened to `api.github.com` only; the app stays offline unless Explore is used.
- Effort: about 1–2 milestones (~1–1.5 h).

---

## Traceability (FR/NFR → steps)

| IDs | Backend steps | Frontend / integration steps |
|---|---|---|
| FR-1..4 | M2.1, M3.1, M4.5 | M6.2, M8.3 |
| FR-5, FR-36 | M3.1, M4.6 | M6.3, M6.4, M8.3 |
| FR-6..9 | M1.5, M1.7, M3.2–M3.5, M4.9 | M6.3, M7.7, M8.3, M8.8 |
| FR-10, FR-11 | M3.6, M3.7, M4.9 | M6.4, M7.7 |
| FR-12..19 | M2.1–M2.10, M4.5, M4.7 | M6.4, M6.5 |
| FR-20 | M1.5, M3.1 | M6.3 |
| FR-21..25 | M2.4, M2.5, M4.7 | M6.6, M6.7 |
| FR-26..28 | M2.6, M2.7, M4.8 | M7.1, M7.2 |
| FR-29..31 | M2.3, M2.11, M3.8, M4.8 | M7.3–M7.5 |
| FR-32..35 | M1.3, M1.6, M4.4 | M5.1, M5.5, M5.6, M5.10, M7.6 |
| FR-37 | — | M5.4, M6.8, M7.8 |
| NFR-P1..P8 | M1.8, M2.4–M2.9, M2.13 | M5.7, M5 gate, M6.5, M8.5, M8.7 |
| NFR-S1..S7 | M1.5, M2.10, M4.7, M4.10, M4.11 | M5.8, M6.7, M8.6 |
| NFR-W1..W3 | M1.3, M1.7, M2.4, M3.3 | M6.6 |
| NFR-O1..O4, NFR-A1 | M1.3, M2.1, M2.12, M3.6, M3.9 | M5.10, M8.2, M8.9 |
