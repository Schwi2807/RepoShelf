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
