# RepoShelf — decisions

One line per decision: `D-n decision — why — rejected alternative`. D-1..D-17 from ARCHITECTURE §0, D-18..D-31 from
the gap fixes in TASKS.md, D-32.. made while implementing.

- D-1 Clone hosts = any public `https://` host; `owner/repo` = github.com, `host.tld/owner/repo` = https; no credentials callback ever — public archive only, no secret handling — ssh/credential prompts, github-only.
- D-2 Managed clones are bare (`<archive>/<host>/<owner>/<name>.git`), never checked out — no Windows file-name/long-path failures, browsing reads objects anyway — non-bare clones with checkout.
- D-3 Fetch allowed for managed clones and imported repos with an https origin; writes only `refs/remotes/origin/*`, new free tags (via staging) and `FETCH_HEAD` — the single, bounded write exception to NFR-O2 — fetch for managed clones only / full `git fetch` semantics.
- D-4 Markdown rendered in Rust: comrak (GFM) → ammonia allow-list, one pipeline for READMEs, `.md`, tag notes, GH release bodies — one sanitiser on trusted side — DOMPurify/markdown-it in the webview.
- D-5 Syntax highlighting in TS: highlight.js core + lazy languages in a Web Worker, class tokens only — no inline styles so CSP needs no `unsafe-inline` — Shiki (inline styles, WASM), syntect (no TSX, CPU on command thread).
- D-6 Hand-written hash router (~200 lines) — needs are small and react-router 8 is unverified on this machine — react-router.
- D-7 Refs in routes are one percent-encoded segment, resolved with a fixed precedence (§3.5) — no ambiguous longest-prefix guessing — GitHub-style multi-segment refs.
- D-8 Line anchors in the route query (`?L=10-20`); "Copy link" shows GitHub form `#L10-L20` — hash is taken by the router — `#L` fragments inside a hash route.
- D-9 Commit paging = offset paging over one pinned tip (`tip` + `skip`) — restarting a time-sorted walk at a cursor drops interleaved merge-side commits — cursor-oid paging.
- D-10 Native dialogs and external links opened from Rust; webview has no dialog/opener/fs/shell permissions — smallest webview attack surface — plugin JS APIs in the webview.
- D-11 No asset protocol; images fetched as bytes over IPC and shown via `blob:` URLs — no fs scope to get wrong — `asset:` protocol with scopes.
- D-12 UI repo identity = slug `owner/name` (origin owner or `local`), numeric `repoId` over IPC, collisions `-2`, `-3` — readable URLs, ids never trusted as paths — paths or hashes in URLs.
- D-13 Default archive dir `Documents\RepoShelf`, data dir `%LOCALAPPDATA%\dev.schwi.reposhelf`, env `RS_DATA_DIR`/`RS_ARCHIVE_DIR` win — lead's decision, visible archive for the user — archive under the app id dir.
- D-14 File-clone test seam `clone_from_unchecked` only under `cfg(any(test, feature="testkit"))`, enabled in src-tauri only via non-default feature `e2e` (separate build); M9 checks `RS_TEST_FILE_CLONE` is absent from shipped exe — tests need local sources without weakening the shipped URL policy — runtime flag in the shipped app.
- D-15 `crates/rs-cli` dev tool (fixtures, bench, inspect), not shipped — PoC criterion 3 needs a bench — benches inside the app.
- D-16 Optional "uncommitted changes" note for imported non-bare repos, off by default, `statuses()` without `UPDATE_INDEX` — never write the index of imported repos — always-on status.
- D-17 No diff syntax highlighting in the PoC (plain lines coloured add/del) — scope — highlighted diffs.
- D-18 `ImportPreview` gets `token: String`; src-tauri keeps `token → (path, expiry 10 min)` — `import_confirm` needs a token the preview never carried — passing the path back from the UI.
- D-19 `RepoSummary` gets `cloned_at: Option<i64>` — FR-18 shows "Cloned <date>" — a separate detail call.
- D-20 `fetch::fetch` = URL policy + `fetch_from(source)`; `fetch_unchecked` under the testkit cfg; e2e build routes `file://` origins there with `RS_TEST_FILE_CLONE=1` — fetch tests and the e2e smoke need `file://` upstreams — relaxing the https rule.
- D-21 `testkit::register_raw(&Shelf, path, source) -> i64` inserts a `repos` row by SQL (M1) — M2 caches need a repos row before `library::import` exists (M3) — reordering milestones.
- D-22 `ghreleases.rs` implemented in M3 behind feature `ghreleases` (on in src-tauri), tested with canned JSON only — module had no milestone — dropping optional FR-31 notes.
- D-23 Extra testkit fixtures `big-tree` (50k), `huge-commit` (400 files), `corrupt`, `missing` — bench and NFR-O3/FR-4 had no inputs — ad-hoc fixtures in tests.
- D-24 IPC samples golden committed at `src/ipc/__samples__/ipc-samples.json`; Rust test compares (`UPDATE_SAMPLES=1` rewrites), vitest reads the golden — `npm test` must not depend on a prior `cargo test` — reading `target/ipc-samples.json`.
- D-25 e2e harness runs only `target\e2e\release\reposhelf.exe`, built with `npx tauri build --no-bundle --features e2e` and `CARGO_TARGET_DIR=target\e2e` — plain cargo build leaves the app on the dev URL — e2e against the portable exe.
- D-26 Shipped builds only via the tauri CLI / `-p reposhelf`, never `--workspace` release — feature unification would compile testkit into the app — workspace release builds.
- D-27 Search/filter/sort implemented once in TS (`filterRepos`, `sortRepos`); Rust `library::search` dropped — the list already lives in the TS store — two implementations.
- D-28 `Settings.recent` dropped; "Recent" = top 5 by `repos.last_opened_at` — duplicated state — a settings list.
- D-29 Measurement steps added: `rs fixtures --register 50` + e2e start-time (NFR-P1), light/dark screenshot pass (M8), NFR-W3 badge (M6.6), NFR-A1 (M5.10) — requirements had no step — leaving them unverified.
- D-30 Trash behind a `Trasher` trait (`RealTrash` = `trash::delete`, tests use a recording fake) — tests must not fill the real Recycle Bin — calling `trash` directly.
- D-31 Virtualised lists use `useWindowVirtualizer` (window scroll) — the router's `scrollY` restore must apply — inner scroll containers.
- D-32 IPC wire shape: unit-only enums serialise as camelCase strings (`"imported"`); enums with data are internally tagged `{"type": "<camelVariant>", …camelCase fields}` (unit variants `{"type":"ok"}`, newtype `Fetched(FetchOutcome)` flattens) — TS gets discriminated unions, `jobs.kind_json` uses the same shape — serde's default external tagging (mixed string/object shapes).
- D-33 `validate::clone_url` refuses URLs whose archive dir components (host dir, owner, folded name dir) fail `winpath::path_ok_here` (e.g. owner `aux`, `x.`); port and IPv6 hosts map to storable dirs (`host_8443`, `[::1]` → `__1`) — the bare clone must be creatable on Windows — failing later inside the clone job.
- D-34 `validate::git_path` drive-prefix rule is `X:` alone or followed by `/` or `\` (refuses `c:/x`, `C:\x`, `Z:`) — `a:b.txt` is a legal git file name that NFR-W3 requires to stay browsable; paths are never joined onto the filesystem — the literal `^[A-Za-z]:` rule.
- D-35 `JobCtx::progress` always emits on a phase change and on `Finalizing`, otherwise ≥ 80 ms apart; `JobRunner::retry` inserts the new row before deleting the old one — the UI must see every phase switch, and SQLite reuses the rowid of a deleted max row — pure time throttle / delete-then-insert.
