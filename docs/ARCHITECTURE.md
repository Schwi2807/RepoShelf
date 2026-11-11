# RepoShelf — architecture

Status: design for the PoC (2026-11-11). Input: `docs/REQUIREMENTS.md` (FR-1..37, NFR-*, S0..S10, R-1..12) plus the
lead's decisions listed in §0. Project Manager (`C:\Dev\projects\project-manager`, "PM") was read as a reference
only; nothing here depends on it. Every decision that deviates from or sharpens REQUIREMENTS.md is marked **[D-n]**
and must be copied into `docs/DECISIONS.md` in M0 (one line: decision — why — rejected alternative).

## 0. Decisions baked in

| # | Decision | Effect on requirements |
|---|---|---|
| D-1 | Clone hosts: any public `https://` host. `owner/repo` and `github.com/owner/repo` shorthand = github.com. No credentials ever (no credential callback registered at all). | FR-6, NFR-S5, R-10 |
| D-2 | Managed clones are **bare**: `<archive>/<host>/<owner>/<name>.git`. No checkout ever. | See §0.1 |
| D-3 | Fetch is offered for managed clones **and** imported repos with an `https` origin. It writes only `refs/remotes/origin/*`, new tags (via staging namespace) and `FETCH_HEAD`. | FR-10; the one write exception to NFR-O2, §0.2 |
| D-4 | Markdown rendered in Rust: `comrak` (GFM) → `ammonia` allow-list. One pipeline for READMEs, `.md` previews, tag annotations and (optional) GitHub release bodies. | FR-17, FR-31, NFR-S1, R-2 |
| D-5 | Syntax highlighting in TS: `highlight.js` **core** + per-language dynamic imports, run in a Web Worker, class-based tokens (`hljs-*`) styled by our own CSS. No inline styles, so CSP has no `style-src 'unsafe-inline'`. | FR-22, NFR-P4, NFR-P8, NFR-S2, R-3 |
| D-6 | Router is **hand-written** (≈200 lines, `src/router/`), hash-based. react-router 8.5.1 is registry-latest but not installed or compiled anywhere on this machine and its v8 API is unverified; our needs are small (pattern match, scroll restoration, mouse back/forward). | FR-34 |
| D-7 | Refs in routes are **one URL segment, percent-encoded** (`feature%2Fx`). The backend resolves the decoded string with a fixed precedence (§3.5). No longest-prefix guessing. | FR-13, FR-34 |
| D-8 | Line anchors live in the route query: `#/o/r/blob/main/src/a.rs?L=10-20`. The "Copy link" text shows the GitHub-style form `…/blob/main/src/a.rs#L10-L20`. | FR-22 |
| D-9 | Commit paging uses **offset paging over one pinned tip** (`tip` oid + `skip`), not "restart a walk at a cursor oid" — restarting a time-sorted walk mid-history drops interleaved side-branch commits on merge histories. | FR-26 (deviation from "cursor oid") |
| D-10 | Native dialogs and external links are opened **from Rust**. The webview gets **no** dialog/opener/fs/shell permissions (capability = `core:default` minus nothing else). | NFR-S4, NFR-S6 |
| D-11 | No asset protocol. Images (README + file view) are fetched as bytes over IPC (`tauri::ipc::Response`) and shown via `blob:` URLs. So `img-src` drops `asset:` from NFR-S2's string (nothing to serve, no fs scope to get wrong). | NFR-S2 |
| D-12 | Repo identity in the UI = slug `owner/name` (origin owner, or `local`); numeric `repoId` over IPC. Collisions get `-2`, `-3`… | FR-12, NFR-S4 |
| D-13 | Default archive dir `Documents\RepoShelf` (lead's decision; overrides FR-35's "under the app id" for the archive dir). Data dir = `%LOCALAPPDATA%\dev.schwi.reposhelf`. Env `RS_DATA_DIR`, `RS_ARCHIVE_DIR` win. Archive dir is shown in Settings, not editable in the PoC. | FR-35 |
| D-14 | Test seam for local clone sources: `rs_core::clone::clone_from_unchecked` exists only under `cfg(any(test, feature = "testkit"))`; `testkit` is never enabled by `src-tauri`. Release builds cannot reach it. | FR-6 tests |
| D-15 | `crates/rs-cli` is included (dev tool, not shipped): builds fixture repos and runs the perf benchmark (PoC success criterion 3). | §9 |
| D-16 | Optional "uncommitted changes" note for imported non-bare repos is **off by default** (setting). It uses `statuses()` without `UPDATE_INDEX`; the `.git` hash test runs with it on and off. | NFR-O2 |
| D-17 | Diff highlighting is out of the PoC (plain monospace diff lines, coloured by add/del). | FR-27 (GitHub has it; ours later) |

### 0.1 Consequences of bare managed clones (D-2)

- FR-9's skip report lists **refs only** (case-clashing / Windows-invalid branch and tag names). No files are ever
  written, so invalid file names and long paths cannot fail a clone.
- NFR-W3's "not checked out on this PC" badge is moot for managed clones. It still applies to **imported non-bare**
  repos: entries whose git path fails `winpath::path_ok_here` get the badge (pure string check, no fs access).
- Loose refs are still files under `refs/`, so case-folding and file/dir clashes (`llm` vs `LLM/x`) still apply →
  the PM `storable_refs` rule is reimplemented (`rs_core::winpath`).
- FR-11 (fast-forward default branch) is a pure ref update: `refs/heads/<default>` set to the origin tip when the old
  tip is an ancestor; no checkout, no index.
- "Open folder" on a managed clone reveals the `<name>.git` directory in Explorer (`reveal_item_in_dir`); on an
  imported repo it opens the work tree folder. The S3 "Code" dropdown says "Archived as a bare repository".
- `core.longpaths=true` is still set (harmless, helps if the user later runs `git worktree add` from it).
- Managed clone layout: `refs/remotes/origin/*` (all storable branches), `refs/heads/<default>` (created once at
  clone), `refs/tags/*`, `HEAD → refs/heads/<default>`, `refs/remotes/origin/HEAD` symbolic. Config has `origin`
  with the standard fetch refspec so command-line git works too.

### 0.2 The one write exception (D-3)

Fetch is user-initiated (button + confirm dialog for imported repos: "Fetch writes `refs/remotes/origin/*`, new tags
and `FETCH_HEAD` into `<path>\.git`. Branches, the index and your files are not touched."). It writes: remote-tracking
refs, `refs/reposhelf-staging/tags/*` (deleted again in the same call), new `refs/tags/*` whose names are free,
`FETCH_HEAD`, pack files/objects, and reflog entries for remote-tracking refs. NFR-O2's `.git` hash test covers
**browse-only** runs; a separate test asserts that fetch on an imported fixture changes nothing outside that list
(index, HEAD, `refs/heads`, existing tags, config, work tree byte-identical).

---

## 1. Overview

```
 ┌──────────────────────────── WebView2 (React 19, strict CSP) ─────────────────────────────┐
 │ router (hash) ─ screens S0..S10 ─ components ─ CSS tokens (light/dark)                    │
 │        │                                   ▲                                               │
 │        ▼                                   │ class-token HTML                              │
 │ src/ipc/api.ts (typed) ── bridge.ts ──┐    hl.worker.ts (highlight.js core + lazy langs)   │
 │                                       │                                                   │
 │                     mock/ (fixtures, ?empty ?slow ?clonefail ?fetchfail ?huge)            │
 └───────────────────────────────────────┼───────────────────────────────────────────────────┘
                     invoke / Channel     │  (only file importing @tauri-apps/api)
 ┌───────────────────────────────────────▼───────────────────────────────────────────────────┐
 │ src-tauri (thin): commands/*.rs → spawn_blocking → rs-core; dialogs + opener + trash here │
 │ AppState { core: rs_core::Shelf, jobs: JobRunner, requests: CancelRegistry, subscribers } │
 └───────────────────────────────────────┬───────────────────────────────────────────────────┘
 ┌───────────────────────────────────────▼───────────────────────────────────────────────────┐
 │ crates/rs-core (no Tauri)                                                                 │
 │  framework: error · paths · db(+migrations) · settings · jobs · validate · winpath        │
 │  data:      library · repo · refs · tree · blob · commits · diff · lastcommit · langs ·   │
 │             readme · markdown · license · releases · clone · fetch · ghreleases(opt)      │
 │  testkit (feature): fixture repo builders                                                  │
 └──────────────┬──────────────────────────────┬─────────────────────────────────────────────┘
        git2 0.21 (vendored libgit2,      rusqlite 0.40 (bundled SQLite)
        https via WinHTTP, no ssh)        <data>/reposhelf.db (WAL)
              │                                   
   imported repos (read-only)   <archive>/<host>/<owner>/<name>.git (bare, managed)
 crates/rs-cli (dev only): fixtures · bench · inspect   ── uses rs-core with feature testkit
```

### 1.1 Tech stack (exact versions)

"PM-lock" = the version resolved in PM's `Cargo.lock` / `node_modules` and compiled on this laptop (Rust 1.99.0,
Node 24.19.0, npm 11). "Registry" = latest on crates.io/npm today, **not yet compiled here** — M0 must compile them.

| Layer | Package | Version (manifest → resolved) | Provenance | Use |
|---|---|---|---|---|
| Rust | edition 2024, resolver 3 | rustc 1.99.0 | PM | workspace |
| Rust | `git2` (`default-features=false`, `vendored-libgit2`, `https`) | `0.21` → 0.21.0 | PM-lock | all git; WinHTTP on Windows |
| Rust | `rusqlite` (`bundled`) | `0.40` → 0.40.2 | PM-lock | DB |
| Rust | `serde` (`derive`) / `serde_json` | `1.0.229` / `1.0.151` | PM-lock | IPC types |
| Rust | `thiserror` | `2` → 2.0.20 | PM-lock | error enum |
| Rust | `dirs` | `6.0.0` | PM-lock | default dirs |
| Rust | `tempfile` | `3.27.0` | PM-lock | tests, testkit |
| Rust | `url` | `2.5` → 2.5.8 | PM-lock (transitive) | clone URL parse/rebuild |
| Rust | `comrak` (`default-features=false`) | `0.59` → 0.59.0 | Registry | GFM → HTML |
| Rust | `ammonia` | `4.2` → 4.2.4 | Registry | HTML allow-list |
| Rust | `ureq` (optional feature `ghreleases`) | `3.4` → 3.4.2 | PM-lock | optional release-notes fetch |
| Tauri | `tauri` (no extra features) | `2` → 2.11.5 | PM-lock | shell |
| Tauri | `tauri-build` | `2` | PM-lock | build.rs |
| Tauri | `tauri-plugin-dialog` | `2` → 2.7.3 | PM-lock | dialogs, called from Rust only |
| Tauri | `tauri-plugin-opener` | `2` → 2.5.5 | PM-lock | open URL / reveal folder, from Rust only |
| Tauri | `trash` | `5.2` → 5.2.9 | PM-lock | FR-36 Recycle Bin |
| npm | `react`, `react-dom` | `^19.3.0` | PM-lock | UI |
| npm | `@tauri-apps/api` | `^2.11.1` | PM-lock | bridge.ts only |
| npm | `lucide-react` | `^1.46.0` | PM-lock | icons |
| npm | `@fontsource-variable/inter`, `@fontsource-variable/jetbrains-mono` | `^5.3.0` | PM-lock | bundled fonts (font-src 'self') |
| npm | `highlight.js` | `^11.12.0` | Registry | highlighter (core + lazy langs) |
| npm | `@tanstack/react-virtual` | `^3.14.26` | Registry | virtualised lists > 500 rows, long files/diffs |
| npm dev | `@tauri-apps/cli` | `^2.11.4` | PM-lock | build/dev |
| npm dev | `typescript` | `^7.0.2` | PM-lock | typecheck |
| npm dev | `vite` / `@vitejs/plugin-react` | `^8.3.0` / `^6.1.1` | PM-lock | bundler |
| npm dev | `vitest` / `happy-dom` | `^5.0.1` / `^20.14.5` | PM-lock | TS tests |
| npm dev | `@types/react`, `@types/react-dom` | `^19.3.0` | PM-lock | types |
| tool | tauri-driver + msedgedriver | as installed for PM (`C:\Dev\tools\cargo\bin`, user env `MSEDGEDRIVER`) | PM | e2e (M8) |
| tool | NSIS | `C:\Dev` (see memory "new laptop toolchain") | PM | installer (M9) |

Not used (and why): DOMPurify/markdown-it (would duplicate D-4's pipeline), Shiki (inline styles by default, large
WASM), syntect (no TS/TSX grammar in default set, CPU on the command thread), react-router (D-6), `similar`
(libgit2 diff is enough), chrono (timestamps cross IPC as epoch seconds + offset minutes; formatted in TS).

---

## 2. Repo layout

```
repo-shelf/
├─ Cargo.toml                 workspace: members = ["crates/rs-core", "crates/rs-cli", "src-tauri"]; release profile (codegen-units=1, lto="thin", strip)
├─ Cargo.lock                 committed
├─ package.json / package-lock.json   scripts: dev, build, typecheck, test, tauri, build:installer, build:portable
├─ vite.config.ts             port 1450 strictPort; watch ignores src-tauri/target/crates; worker format "es"
├─ vitest.config.ts           happy-dom, include src/**/*.test.{ts,tsx}
├─ tsconfig.json              strict, ES2022, bundler resolution (as PM)
├─ index.html                 no inline script/style; <div id="root">
├─ .gitignore                 target/, node_modules/, dist/, src-tauri/gen/, *.db
├─ .claude/launch.json        { name: "reposhelf-mock", runtimeExecutable: "npm", runtimeArgs: ["run","dev"], port: 1450 }
├─ docs/
│  ├─ REQUIREMENTS.md  ARCHITECTURE.md  DECISIONS.md  WORKLOG.md
│  ├─ TESTING.md              (M8) how to run every check; hand-test checklist
│  └─ icon/reposhelf.svg      own app icon source (shelf of three spines; no GitHub marks)
├─ crates/
│  ├─ rs-core/
│  │  ├─ Cargo.toml           features: testkit = ["dep:tempfile"], ghreleases = ["dep:ureq"]
│  │  └─ src/ lib.rs error.rs paths.rs db.rs migrations/ (0001_init.sql, …) settings.rs jobs.rs validate.rs
│  │           winpath.rs library.rs repo.rs refs.rs tree.rs blob.rs commits.rs diff.rs lastcommit.rs
│  │           langs.rs langs_table.rs readme.rs markdown.rs license.rs releases.rs clone.rs fetch.rs
│  │           ghreleases.rs (feature) types.rs testkit.rs (feature/test)
│  └─ rs-cli/                 bin `rs`: `rs fixtures <dir>`, `rs bench <dir>`, `rs inspect <repo-path> <cmd>`
├─ src-tauri/
│  ├─ Cargo.toml  build.rs  tauri.conf.json  tauri.windows.conf.json (if needed)
│  ├─ capabilities/default.json
│  ├─ icons/                  generated by `npx tauri icon docs/icon/reposhelf.svg` (M0)
│  └─ src/ main.rs lib.rs state.rs ipc_error.rs
│          cmd/ app.rs library.rs repo.rs tree.rs blob.rs commits.rs refs.rs releases.rs jobs.rs dialogs.rs
├─ src/
│  ├─ main.tsx  App.tsx
│  ├─ router/  router.ts (match/navigate/scroll memory) routes.ts (patterns + href builders) Link.tsx useRoute.ts
│  ├─ ipc/     bridge.ts (only @tauri-apps import) api.ts types.ts errors.ts query.ts (cache + useQuery)
│  │           mock/ index.ts switches.ts fixtures/*.ts mockJobs.ts
│  ├─ state/   theme.ts jobs.ts library.ts recent.ts settings.ts (useSyncExternalStore stores)
│  ├─ hl/      highlighter.ts (worker client) hl.worker.ts languages.ts (ext/shebang → hljs id) splitLines.ts
│  ├─ markdown/ MarkdownBody.tsx (inert template → rewrite links/images → mount) linkRewrite.ts
│  ├─ components/ layout/ (GlobalHeader, RepoHeader, TabBar, Sidebar) ui/ (Button, Dropdown, Dialog, Popover,
│  │           Chip, Badge, Shimmer, RelativeTime, CopyButton, Avatar(initials), Empty) …per-screen folders
│  ├─ screens/ Dashboard/ RepoCode/ FileView/ Commits/ Commit/ Branches/ Tags/ Releases/ Settings/ NotFound/
│  ├─ styles/  tokens.css base.css layout.css components.css code.css (hljs classes) diff.css markdown.css
│  └─ util/    format.ts (bytes, relative time, day groups) paths.ts (breadcrumbs) lang-colors.ts
├─ tests/fixtures/  security/ (malicious README.md, svg-with-script.svg) — inputs to testkit + mock
└─ tools/
   ├─ build-portable.mjs      tauri build --no-bundle → target/release/portable/RepoShelf_<ver>_x64-portable.exe
   ├─ e2e/ run-e2e.ps1 wd-e2e.mjs check-e2e.ps1   (M8; PM's tauri-driver approach, scratch dirs, offscreen)
   └─ shots/ mock-shots.md    (M5–M7) list of mock routes × themes to screenshot
```

---

## 3. rs-core

All public types are `#[derive(Serialize, Deserialize)] #[serde(rename_all = "camelCase")]`; enums with struct
variants also get `rename_all_fields = "camelCase"` (PM lesson L-10). Oids cross IPC as 40-hex strings; git paths as
`String` (lossy UTF-8) plus `pathRaw: Option<Vec<u8>>` only when lossy conversion changed bytes (R-7; the UI then
shows a "non-UTF-8 name" marker and browsing that entry uses `pathRaw`). Timestamps: `Time { secs: i64, offset_min: i32 }`.

git2 0.21 accessors returning `Result` (names, messages, emails) are handled with lossy fallbacks, never `unwrap`.
`Repository` is opened per call (`Repository::open_ext(path, RepositoryOpenFlags::NO_SEARCH, [] as [&OsStr; 0])`),
cheap; never shared across threads. **No long git walk ever holds the DB mutex**: read cache → release → compute → write cache.

### 3.1 Error type (`error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{what} not found")]            NotFound { what: String },
    #[error("invalid path: {0}")]           InvalidPath(String),        // NFR-S4
    #[error("invalid ref: {0}")]            InvalidRev(String),
    #[error("{0}")]                         InvalidUrl(String),         // FR-6 reasons, user-readable
    #[error("{0}")]                         InvalidInput(String),       // metadata validation FR-20
    #[error("already on the shelf: {0}")]   Duplicate(String),          // FR-5
    #[error("not a git repository: {0}")]   NotARepo(String),
    #[error("repository folder is missing: {0}")] RepoMissing(String),  // FR-4
    #[error("repository is damaged: {0}")]  RepoCorrupt(String),        // NFR-O3
    #[error("cancelled")]                   Cancelled,
    #[error("{0}")]                         Refused(String),            // e.g. FF not possible, diverged
    #[error("network: {0}")]                Network(String),
    #[error("git: {0}")]                    Git(#[from] git2::Error),
    #[error("database: {0}")]               Db(#[from] rusqlite::Error),
    #[error("io: {0}")]                     Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error { pub fn code(&self) -> &'static str; /* "notFound", "invalidPath", … */ }
```

`src-tauri/ipc_error.rs` serialises it as `{ code: string, message: string }` (TS `IpcError`); git errors are
mapped to friendly text (auth/401/403 → "The server asked for credentials: private or missing repository. Only public
repositories can be cloned."; 404; Net/Http/Ssl → "Could not reach <host>").

### 3.2 Framework modules

```rust
// paths.rs
pub struct Dirs { pub data: PathBuf, pub archive: PathBuf }
pub fn resolve_dirs() -> Dirs;                // RS_DATA_DIR / RS_ARCHIVE_DIR, else %LOCALAPPDATA%\dev.schwi.reposhelf, Documents\RepoShelf
pub fn db_file(d: &Dirs) -> PathBuf;          // <data>/reposhelf.db
pub fn long(p: &Path) -> PathBuf;             // \\?\ prefix on Windows for fs ops on deep archive paths (NFR-W1)
pub fn canonical(p: &Path) -> Result<PathBuf>; // dunce-style: canonicalize, strip \\?\ for display/dup detection

// db.rs  — one Connection behind Mutex (writes + short reads); WAL; busy_timeout 5s; foreign_keys on
pub struct Db { /* Mutex<Connection> */ }
impl Db {
    pub fn open(file: &Path) -> Result<Db>;   // runs migrations
    pub fn open_in_memory() -> Result<Db>;     // tests
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T>;
    pub fn tx<T>(&self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T>;
}
pub const SCHEMA_VERSION: i64;                 // = number of migrations

// settings.rs  — typed view over the `settings` table (key → JSON)
pub struct Settings { pub theme: Theme /* light|dark|system */, pub sort: SortSpec, pub source_filter: SourceFilter,
                      pub show_dirty_note: bool, pub recent: Vec<i64> }
pub fn load(db: &Db) -> Result<Settings>;  pub fn save(db: &Db, s: &Settings) -> Result<()>;

// validate.rs  (NFR-S4, NFR-S5, FR-20)
pub fn git_path(p: &str) -> Result<GitPath>;    // "" = root. Refuses: leading '/', '\\', NUL, ':' drive prefix
                                                // (^[A-Za-z]:), "." or ".." components, empty components, > 4096 bytes
pub fn rev(s: &str) -> Result<RevSpec>;         // 4..=40 hex → OidPrefix; else must pass Reference::is_valid_name
                                                // for "refs/heads/<s>"; refuses "..", "@{", "^", "~", ":", "\\", spaces, ctrl
pub fn clone_url(input: &str) -> Result<CloneUrl>; // §5.3
pub fn topics(v: &[String]) -> Result<Vec<String>>; // lowercase, ^[a-z0-9][a-z0-9-]{0,49}$, ≤ 20, dedup
pub fn homepage(s: &str) -> Result<Option<String>>; // http/https, no userinfo
pub struct GitPath(String);  pub enum RevSpec { Name(String), OidPrefix(String) }
pub struct CloneUrl { pub url: String /* rebuilt https://host[:port]/path.git? */, pub host: String,
                      pub owner: String, pub name: String, pub rel_dir: PathBuf /* host/owner/name.git */ }

// winpath.rs  (reimplementation of PM winsafe ideas; pure functions)
pub fn path_ok_here(git_path: &str) -> bool;    // NT-invalid chars, trailing dot/space, DOS devices, .git/git~1
pub fn ref_name_ok_here(name: &str) -> bool;
pub fn storable_refs(names: &[String], existing: &[String], prefer: Option<&str>, ignore_case: bool)
    -> (Vec<String>, Vec<String>);              // (keep, skipped), case-fold + file/dir clash rule
pub fn refspecs(src: &str, dst: &str, keep: &[String], skipped: &[String]) -> Vec<String>; // wildcard when none skipped

// jobs.rs  — no Tauri; the command layer passes a sink closure
pub type JobId = u64;
pub enum JobKind { Clone { url: String }, Fetch { repo_id: i64 }, FetchAll }
pub enum JobState { Waiting, Running, Done, Failed, Cancelled }
pub struct JobInfo { pub id: JobId, pub kind: JobKind, pub state: JobState, pub progress: Option<Progress>,
                     pub title: String, pub result: Option<JobResult>, pub error: Option<String>, pub started_at: i64 }
pub enum Progress { Connecting, Receiving { received: usize, total: usize, bytes: usize },
                    Resolving { done: usize, total: usize }, Finalizing }
pub struct JobRunner { /* Mutex<Vec<JobInfo>>, gate: Gate(3), cancel flags, sink */ }
impl JobRunner {
    pub fn new(max_concurrent: usize, sink: Arc<dyn Fn(&JobInfo) + Send + Sync>) -> Self;
    pub fn submit(&self, kind: JobKind, title: String,
                  work: impl FnOnce(&JobCtx) -> Result<JobResult> + Send + 'static) -> JobId; // spawns a thread
    pub fn cancel(&self, id: JobId) -> bool;     pub fn retry(&self, id: JobId) -> Result<JobId>;
    pub fn list(&self) -> Vec<JobInfo>;          pub fn dismiss(&self, id: JobId);
}
pub struct JobCtx { pub cancel: Arc<AtomicBool>, /* … */ }
impl JobCtx { pub fn progress(&self, p: Progress); /* throttled ≥ 80 ms except terminal */ pub fn cancelled(&self) -> bool; }
pub struct Gate;  // Mutex<usize> + Condvar; acquire(cancel) waits in 100 ms slices so a Waiting job can be cancelled
pub struct CancelToken(Arc<AtomicBool>);   // for request-scoped work (lastcommit, langs, ahead/behind)
```

### 3.3 Data types (`types.rs`, mirrored 1:1 in `src/ipc/types.ts`)

```rust
pub enum Source { Imported, Cloned }
pub struct RepoSummary {          // dashboard row (FR-1), all from DB — no git on list
    id: i64, owner: String, name: String, source: Source, path: String, description: Option<String>,
    topics: Vec<String>, homepage: Option<String>, origin_url: Option<String>, origin_host: Option<String>,
    default_branch: Option<String>, primary_language: Option<String>, last_commit_at: Option<i64>,
    added_at: i64, last_opened_at: Option<i64>, last_fetched_at: Option<i64>, status: RepoStatus }
pub enum RepoStatus { Ok, Missing, Corrupt { message: String }, Empty }
pub struct RepoDetail {           // S2 header + About (FR-12, FR-18)
    summary: RepoSummary, is_bare: bool, head: HeadState, branch_count: u32, tag_count: u32,
    release_count: u32, license: Option<String>, fetchable: bool, can_fast_forward: Option<bool>, dirty: Option<bool> }
pub enum HeadState { Branch { name: String }, Detached { oid: String }, Unborn { name: String } }
pub struct ResolvedRev { input: String, kind: RefKind, full_name: Option<String>, commit: String, tree: String }
pub enum RefKind { Branch, RemoteBranch, Tag, Commit }
pub struct RefItem { name: String, full_name: String, kind: RefKind, target: String, is_default: bool }
pub struct RefsList { default_branch: Option<String>, branches: Vec<RefItem>, remote_branches: Vec<RefItem>, tags: Vec<RefItem> }
pub struct Signature { name: String, email: String, time: Time }
pub struct CommitSummary { oid: String, summary: String, author: Signature, committer: Signature, parents: Vec<String> }
pub struct LatestCommit { commit: CommitSummary, count: Option<u64> /* None = computing, see commit_count */ }
pub enum EntryKind { Dir, File, Exec, Symlink, Submodule }
pub struct TreeEntry { name: String, path: String, path_raw: Option<Vec<u8>>, kind: EntryKind, oid: String,
                       size: Option<u64>, storable_here: bool }
pub struct TreeListing { rev: ResolvedRev, path: String, entries: Vec<TreeEntry> /* sorted FR-15 */,
                         readme: Option<String> /* entry name */ }
pub struct EntryCommit { name: String, commit: Option<CommitSummary> /* None = older than cap */ }
pub struct LastCommits { tip: String, path: String, entries: Vec<EntryCommit>, capped: bool }
pub enum BlobKind { Text, Image { mime: String }, Binary, LfsPointer { size: Option<u64> }, Submodule { url: Option<String> },
                    Symlink { target: String } }
pub struct BlobMeta { path: String, oid: String, size: u64, kind: BlobKind, lines: Option<u32>, sloc: Option<u32>,
                      lossy: bool, highlight: bool /* size ≤ 1 MB */, displayable: bool /* size ≤ 5 MB */, lang_hint: Option<String> }
pub struct BlobText { meta: BlobMeta, text: Option<String> /* None if !displayable or not text */ }
pub struct Rendered { html: String, headings: Vec<Heading>, source_path: String }  // sanitised
pub struct Heading { level: u8, text: String, anchor: String }
pub struct LanguageStat { name: String, bytes: u64, percent: f32 }
pub struct Languages { tree: String, items: Vec<LanguageStat> /* top 6 + "Other" */ }
pub struct CommitsPage { tip: String, skip: u32, commits: Vec<CommitSummary>, has_more: bool, scanned: u32, capped: bool }
pub enum ChangeKind { Added, Deleted, Modified, Renamed, Copied, TypeChange }
pub struct FileChange { index: u32, kind: ChangeKind, old_path: Option<String>, new_path: Option<String>,
                        additions: u32, deletions: u32, binary: bool, too_large: bool, generated: bool }
pub struct DiffLine { kind: LineKind /* Ctx|Add|Del|NoNewline */, old_no: Option<u32>, new_no: Option<u32>, text: String }
pub struct Hunk { header: String, lines: Vec<DiffLine> }
pub struct FileDiff { index: u32, hunks: Vec<Hunk>, truncated: bool }
pub struct CommitDetail { commit: CommitSummary, message: String, parent_compared: Option<String>, is_merge: bool,
                          files: Vec<FileChange>, additions: u32, deletions: u32,
                          diffs: Vec<FileDiff> /* inline unless over commit caps */, diffs_deferred: bool }
pub struct BranchRow { item: RefItem, last: CommitSummary, ahead_behind: Option<AheadBehind> }
pub struct AheadBehind { ahead: u32, behind: u32, capped: bool }
pub struct TagRow { item: RefItem, commit: CommitSummary, annotated: bool, tagger: Option<Signature>, date: i64 }
pub struct Release { tag: String, commit: String, date: i64, title: String, notes_html: Option<String>,
                     prerelease: bool, latest: bool, previous_tag: Option<String>, gh_notes: bool }
pub struct ImportPreview { path: String, name: String, is_bare: bool, origin_url: Option<String>, duplicate_of: Option<i64> }
pub struct ClonePreview { url: String, host: String, owner: String, name: String, target_dir: String }
pub enum JobResult { Cloned { repo_id: i64, skipped_refs: Vec<String> }, Fetched(FetchOutcome) }
pub struct FetchOutcome { new_commits: u32, new_tags: Vec<String>, kept_local_tags: Vec<String>, skipped_refs: Vec<String>,
                          updated_branches: Vec<String> }
pub struct AppInfo { version: String, data_dir: String, archive_dir: String, cache_bytes: u64, repo_count: u32,
                     archive_bytes: Option<u64> }
```

### 3.4 Data modules (public signatures)

`Shelf` is the facade the command layer holds (`Arc<Shelf>`): `pub struct Shelf { pub db: Db, pub dirs: Dirs }`.
Every repo-scoped function takes `&Shelf, repo_id: i64` and opens via `repo::open(shelf, id)` which looks the path
up in the DB (never a path from the UI) and maps failures to `RepoMissing`/`RepoCorrupt`.

```rust
// library.rs  (FR-1..5, FR-20, FR-36)
pub fn list(s: &Shelf) -> Result<Vec<RepoSummary>>;                      // DB only (NFR-P1)
pub fn refresh_summaries(s: &Shelf, cancel: &CancelToken) -> Result<Vec<RepoSummary>>; // per repo: exists? head tip changed?
                                                                         // → update last_commit_at/default_branch/lang
pub fn by_slug(s: &Shelf, owner: &str, name: &str) -> Result<RepoSummary>;
pub fn preview_import(s: &Shelf, dir: &Path) -> Result<ImportPreview>;   // dir comes from the native dialog
pub fn import(s: &Shelf, dir: &Path) -> Result<RepoSummary>;             // canonical-path duplicate check; no writes to repo
pub fn register_clone(s: &Shelf, path: &Path, url: &CloneUrl) -> Result<RepoSummary>;
pub fn set_metadata(s: &Shelf, id: i64, m: MetadataInput) -> Result<RepoSummary>; // validated
pub fn relocate(s: &Shelf, id: i64, dir: &Path) -> Result<RepoSummary>;  // FR-4 Locate…
pub fn remove(s: &Shelf, id: i64) -> Result<Removed>;                    // returns path + source; trash done in src-tauri
pub fn touch_opened(s: &Shelf, id: i64) -> Result<()>;
pub fn search(list: &[RepoSummary], q: &str) -> Vec<&RepoSummary>;       // name/description/topics, case-insensitive (FR-2)

// repo.rs
pub fn open(s: &Shelf, id: i64) -> Result<(RepoSummary, git2::Repository)>;
pub fn detail(s: &Shelf, id: i64) -> Result<RepoDetail>;
pub fn dirty(repo: &Repository) -> Result<bool>;                         // D-16, no UPDATE_INDEX

// refs.rs  (FR-13, FR-29, FR-30)
pub fn resolve(repo: &Repository, spec: &RevSpec) -> Result<ResolvedRev>;  // §3.5
pub fn list(repo: &Repository) -> Result<RefsList>;
pub fn branches(repo: &Repository) -> Result<Vec<BranchRow>>;            // without ahead/behind
pub fn ahead_behind(repo: &Repository, base: &str, names: &[String], cap: u32, cancel: &CancelToken) -> Result<Vec<(String, AheadBehind)>>;
pub fn tags(repo: &Repository) -> Result<Vec<TagRow>>;                   // newest first by tagger/commit date

// tree.rs  (FR-15, NFR-P2)
pub fn list_dir(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<TreeListing>;
pub fn entry_kind(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<EntryKind>; // blob route vs tree route

// blob.rs  (FR-22..25)
pub fn meta(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<BlobMeta>;
pub fn text(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<BlobText>; // lossy decode, BOM stripped, ≤ 5 MB
pub fn bytes(repo: &Repository, rev: &ResolvedRev, path: &GitPath, max: u64) -> Result<Vec<u8>>; // raw/save/image
pub fn sniff(path: &str, head: &[u8], size: u64) -> BlobKind;            // NUL in first 8 KB → Binary; LFS pointer; image ext

// commits.rs  (FR-14, FR-26, NFR-P5)
pub fn latest(s: &Shelf, repo: &Repository, rev: &ResolvedRev) -> Result<LatestCommit>;  // count from cache or None
pub fn count(s: &Shelf, repo_id: i64, repo: &Repository, tip: Oid, cancel: &CancelToken) -> Result<u64>; // cached per tip
pub fn page(repo: &Repository, q: &PageQuery, cancel: &CancelToken) -> Result<CommitsPage>;
pub struct PageQuery { pub tip: Oid, pub hide: Option<Oid> /* "since previous tag" */, pub path: Option<GitPath>,
                       pub skip: u32, pub limit: u32 /* 35 */, pub scan_cap: u32 /* 20_000 for path filter */ }
pub fn file_last_commit(s: &Shelf, repo_id: i64, repo: &Repository, rev: &ResolvedRev, path: &GitPath,
                        cancel: &CancelToken) -> Result<Option<CommitSummary>>; // via lastcommit cache of the parent dir

// diff.rs  (FR-27, FR-28)
pub fn commit_detail(repo: &Repository, oid: Oid, limits: &DiffLimits) -> Result<CommitDetail>;
pub fn file_diff(repo: &Repository, oid: Oid, index: u32, force: bool) -> Result<FileDiff>; // "Load diff"
pub struct DiffLimits { pub max_files: u32 /*300*/, pub max_lines: u32 /*20_000*/, pub file_lines: u32 /*2000*/,
                        pub blob_bytes: u64 /*1 MB → binary/too_large*/ }

// lastcommit.rs  (FR-16, NFR-P3, R-1)
pub fn for_dir(s: &Shelf, repo_id: i64, repo: &Repository, tip: Oid, dir: &GitPath, cap: u32, cancel: &CancelToken) -> Result<LastCommits>;

// langs.rs / langs_table.rs  (FR-19, NFR-P6)
pub fn for_tree(s: &Shelf, repo_id: i64, repo: &Repository, tree: Oid, cancel: &CancelToken) -> Result<Languages>;
pub fn classify(path: &str) -> Option<&'static str>;                     // None = excluded/unknown

// readme.rs / markdown.rs / license.rs  (FR-17, FR-18)
pub fn find_readme(entries: &[TreeEntry]) -> Option<&TreeEntry>;
pub fn render_readme(repo: &Repository, rev: &ResolvedRev, dir: &GitPath) -> Result<Option<Rendered>>;
pub fn render_file(repo: &Repository, rev: &ResolvedRev, path: &GitPath) -> Result<Rendered>; // .md preview
pub fn render(md: &str, base_dir: &GitPath) -> Rendered;                 // comrak → rewrite → ammonia
pub fn detect_license(repo: &Repository, tree: &git2::Tree) -> Option<String>;

// releases.rs  (FR-31)
pub fn list(repo: &Repository) -> Result<Vec<Release>>;
pub fn is_version_tag(name: &str) -> Option<Version>;                    // ^v?\d+(\.\d+){0,3}([-+.][0-9A-Za-z.-]+)?$

// clone.rs / fetch.rs  (FR-6..11)
pub fn preview(s: &Shelf, input: &str) -> Result<ClonePreview>;
pub fn clone(s: &Shelf, url: &CloneUrl, ctx: &JobCtx) -> Result<(PathBuf, Vec<String>)>;  // bare, atomic, §5.3
pub fn cleanup_partials(archive: &Path) -> Result<u32>;                  // startup, FR-8
pub fn fetch(s: &Shelf, repo_id: i64, ctx: &JobCtx) -> Result<FetchOutcome>;
pub fn fast_forward_default(s: &Shelf, repo_id: i64) -> Result<String>;  // FR-11, managed only; Refused if diverged
#[cfg(any(test, feature = "testkit"))]
pub fn clone_from_unchecked(s: &Shelf, url: &str, ctx: &JobCtx) -> Result<(PathBuf, Vec<String>)>; // D-14

// ghreleases.rs (feature "ghreleases", optional FR-31)
pub fn fetch_github_notes(s: &Shelf, repo_id: i64) -> Result<Vec<Release>>; // one GET api.github.com, cached in DB

// testkit.rs (feature "testkit" or cfg(test))
pub fn repo(dir: &Path) -> TestRepo;   // builder: .file(path, bytes).commit(msg, time).branch().tag(annotated?).merge()
pub fn fixtures(root: &Path) -> Fixtures; // basic, wide-dir(1000), long-history(10k), merges, winnames, refclash,
                                          // security-readme, binaries(lfs/symlink/submodule/large/non-utf8), releases
```

### 3.5 Ref resolution (D-7)

The UI sends the decoded ref string. `refs::resolve` tries, first hit wins:
1. Exact full name if it starts with `refs/`.
2. `refs/heads/<s>` (local branch).
3. `refs/tags/<s>` (tag, peeled to commit).
4. `refs/remotes/<s>` (e.g. `origin/main`).
5. `HEAD` literal.
6. Hex 4–40 chars → `revparse_single` limited to commit objects (ambiguous prefix → `InvalidRev`).

A tag and branch with the same name resolve to the branch (git's own order); the ref switcher always links tags as
`refs/tags/<name>` so both stay reachable. Not found → `NotFound{what:"ref"}`; S3 shows "ref not found" + link to
the default branch.

---

## 4. SQLite schema

File `<data>/reposhelf.db`. Migrations are numbered SQL files in `crates/rs-core/src/migrations/` embedded with
`include_str!`, applied in order inside one transaction each, tracked with `PRAGMA user_version`. Forward-only; a DB
with a higher `user_version` than the app knows is opened read-only with a banner (never downgraded). Caches are
pure derivatives keyed by oids: "Clear cache" = `DELETE` from the `cache_*` tables; dropping them on a migration is
always allowed.

```sql
-- 0001_init.sql
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
CREATE TABLE clone_jobs (                         -- FR-7 failures survive restart with Retry
  id INTEGER PRIMARY KEY, url TEXT NOT NULL, state TEXT NOT NULL,   -- waiting|running|done|failed|cancelled
  error TEXT, repo_id INTEGER REFERENCES repos(id) ON DELETE SET NULL,
  skipped_json TEXT, created_at INTEGER NOT NULL, finished_at INTEGER
);
CREATE INDEX cache_last_commit_age ON cache_last_commit(created_at);
```

Cache pruning: on start, keep at most 2,000 `cache_last_commit` rows (oldest deleted) and drop cache rows whose
`tip` is neither a current ref tip nor used in the last 30 days (cheap: `created_at`). Rows in `clone_jobs` with
state `running`/`waiting` at startup become `failed` with "Interrupted (app closed)" — Retry available.

---

## 5. IPC

### 5.1 Commands

All commands: `async fn`, `tauri::async_runtime::spawn_blocking` around rs-core, return
`Result<T, IpcError>`. Repo-scoped args: `repoId: number`, `rev: string`, `path: string` (git path). TS names are
camelCase; Rust command names snake_case.

| Command | Args | Returns | FR |
|---|---|---|---|
| `app_info` | – | `AppInfo` | 35, S10 |
| `settings_get` / `settings_set` | – / `Settings` | `Settings` | 3, 33 |
| `cache_clear` | – | `AppInfo` | S10 |
| `library_list` | – | `RepoSummary[]` (DB) | 1, NFR-P1 |
| `library_refresh` | `requestId` | `RepoSummary[]` | 1, 4, R-9 |
| `repo_by_slug` | `owner, name` | `RepoSummary` | 12, 34 |
| `import_pick` | – | `ImportPreview \| null` (opens folder dialog in Rust) | 5 |
| `import_confirm` | `token` (from preview; backend keeps the picked path) | `RepoSummary` | 5 |
| `repo_locate` | `repoId` (opens folder dialog in Rust) | `RepoSummary` | 4 |
| `repo_remove` | `repoId` | `{ trashed: bool }` (cloned → `trash::delete`) | 36 |
| `repo_set_metadata` | `repoId, description, homepage, topics` | `RepoSummary` | 20 |
| `repo_detail` | `repoId` | `RepoDetail` (+ `touch_opened`) | 12, 18 |
| `repo_open_folder` | `repoId` | `()` (opener reveal, from Rust) | S2 |
| `refs_list` | `repoId` | `RefsList` | 13 |
| `rev_resolve` | `repoId, rev` | `ResolvedRev` | 13 |
| `tree_list` | `repoId, rev, path` | `TreeListing` | 15 |
| `tree_last_commits` | `repoId, rev, path, requestId` | `LastCommits` | 16 |
| `commit_latest` | `repoId, rev` | `LatestCommit` | 14 |
| `commit_count` | `repoId, rev, requestId` | `number` | 14, 12 |
| `readme_render` | `repoId, rev, path(dir)` | `Rendered \| null` | 17 |
| `markdown_render_file` | `repoId, rev, path` | `Rendered` | S4 preview |
| `languages` | `repoId, rev, requestId` | `Languages` | 19 |
| `blob_meta` | `repoId, rev, path` | `BlobMeta` | 22–25 |
| `blob_text` | `repoId, rev, path` | `BlobText` | 22, 23 |
| `blob_bytes` | `repoId, rev, path` | raw `ArrayBuffer` (`tauri::ipc::Response`), ≤ 20 MB | 17 images, 24 |
| `blob_save_as` | `repoId, rev, path` | `string \| null` (save dialog in Rust, writes bytes) | 23 |
| `file_last_commit` | `repoId, rev, path, requestId` | `CommitSummary \| null` | 22 |
| `commits_page` | `repoId, rev, path?, since?, skip, tip?` | `CommitsPage` | 26, 31 compare |
| `commit_detail` | `repoId, oid` | `CommitDetail` | 27, 28 |
| `commit_file_diff` | `repoId, oid, index, force` | `FileDiff` | 28 |
| `branches_list` | `repoId` | `BranchRow[]` | 29 |
| `branches_ahead_behind` | `repoId, names[], requestId` | `[name, AheadBehind][]` | 29 |
| `tags_list` | `repoId` | `TagRow[]` | 30 |
| `releases_list` | `repoId` | `Release[]` (cached GH notes merged if present) | 31 |
| `releases_fetch_github` | `repoId` | `Release[]` (feature `ghreleases`; network) | 31 opt |
| `clone_preview` | `input` | `ClonePreview` | 6 |
| `clone_start` | `input` | `JobId` | 6–9 |
| `fetch_start` | `repoId` | `JobId` | 10 |
| `fetch_all_start` | – | `JobId` (sequential, cancellable; optional) | S1 |
| `fast_forward` | `repoId` | `string` (new tip) | 11 |
| `jobs_subscribe` | `channel: Channel<JobInfo>` | `JobInfo[]` snapshot | 7, NFR-P7 |
| `jobs_cancel` / `jobs_retry` / `jobs_dismiss` | `jobId` | `bool` / `JobId` / `()` | 7 |
| `request_cancel` | `requestId` | `bool` | 16, NFR-P3 |
| `open_external` | `url` | `()` (http/https only; frontend confirmed first) | 12, NFR-S3 |

### 5.2 Validation rules (enforced in the command layer before rs-core does anything)

- `repoId` must exist in `repos`; the filesystem path is read from the DB only.
- `path` → `validate::git_path` (no leading `/`, no `\`, no NUL, no `X:` drive prefix, no `.`/`..`/empty
  components, ≤ 4096 bytes). Resolution is only `tree.get_path()` from the resolved commit tree — never
  `Path::join` onto a filesystem path. Working trees are never read for browsing.
- `rev` → `validate::rev`; `oid` → 40 hex exactly.
- `index`, `skip` bounded (`skip ≤ 10_000_000`, `limit` fixed at 35 server-side).
- `url` (open_external) → parsed with `url::Url`, scheme `http`/`https`, no userinfo; anything else refused.
- Filesystem paths **never** arrive from the UI: import/locate use the Rust-side folder dialog, save-as the Rust-side
  save dialog. `import_confirm` takes an opaque token mapped to the path the dialog returned (expires after 10 min).
- `blob_bytes` refuses > 20 MB; `blob_text` returns `text: null` > 5 MB (NFR-P4).
- Tests: a table of hostile inputs (`..`, `a/../b`, `/etc`, `C:\x`, `c:/x`, `a\b`, `a\0b`, `refs/../x`, `HEAD~1`,
  `a..b`, `@{-1}`) asserts `InvalidPath`/`InvalidRev` for every command that takes them (PoC criterion 4).

### 5.3 Clone URL rules (FR-6, NFR-S5, D-1)

`validate::clone_url(input)`: trim; reject `git@…`, `ssh://`, `git://`, `file://`, `http://`, anything with `\`, a
drive letter, a leading `/` or `.`; `owner/repo` (exactly two segments, `[A-Za-z0-9._-]`) → `https://github.com/owner/repo`;
`host/owner/repo` without scheme → `https://host/owner/repo`. Then `url::Url::parse`: scheme `https`, host is a
domain or IP (no empty host), no userinfo (refused, not stripped, so the user sees why), query/fragment dropped,
path ≥ 2 segments, each segment `[A-Za-z0-9._~-]+` and not `.`/`..`; trailing `.git` and `/` normalised. Rebuilt
string = `https://<host>[:port]/<seg>/…/<name>.git`. owner = first segment, name = last segment (without `.git`);
`rel_dir = <host>/<owner>/<name>.git` (middle segments of nested groups are folded into the name dir as
`<seg2>-…-<name>.git` to keep depth fixed). A unit test proves libgit2 is never called on rejection: the validator is
pure and `clone()` only accepts a `CloneUrl` value (type-level guarantee).

### 5.4 Clone and fetch algorithms (bare, R-4, R-5)

**Clone** (`clone.rs`), inside a job (gate ≤ 3; others `Waiting`):
1. `partial = <archive>/<host>/<owner>/.<name>.git.partial-<ms>-<seq>`; `Repository::init_bare(long(partial))`;
   set `core.longpaths=true` (Windows), `core.ignorecase` as init recorded.
2. `remote("origin", url)`; connect with proxy auto, **no credentials callback**, `RemoteRedirect::Initial`
   (libgit2 option `follow_redirects`; https→https only since only https URLs are ever given and WinHTTP refuses
   downgrade); list advertised heads, tags, default branch (PM `advertised` logic: symref target, else
   `default_branch()`, else main/master at HEAD oid).
3. `storable_refs` for heads (prefer default) and tags; if the default branch itself is unstorable → `Refused`.
4. `fetch(refspecs(heads → refs/remotes/origin/), refspecs(tags → refs/tags/))`, `AutotagOption::None`,
   transfer progress → `ctx.progress` (throttled 80 ms), callback returns `!cancelled`.
5. Create `refs/heads/<default>` at the origin tip; `HEAD → refs/heads/<default>`;
   `refs/remotes/origin/HEAD` symbolic; set standard `remote.origin.fetch`.
6. Under a global `FINALIZE` mutex: pick unique `<name>.git` / `<name>-2.git`; `rename(partial, target)`.
7. Any error/cancel → `remove_dir_all(long(partial))`; `cleanup_partials` on startup removes leftovers from a
   killed process (FR-8). Then `library::register_clone`; job result lists skipped refs (FR-9).

**Fetch** (`fetch.rs`), managed or imported-with-https-origin (origin URL re-validated with `clone_url`; else
`Refused("Fetch needs an https origin")`):
1. Snapshot `refs/remotes/origin/*` tips and tag names.
2. List advertised refs; `storable_refs(heads, existing = local remote-tracking names)`,
   `storable_refs(tags, existing = local tag names)`.
3. Delete any `refs/reposhelf-staging/tags/*`; fetch heads → `refs/remotes/origin/*`, tags →
   `refs/reposhelf-staging/tags/*`, `AutotagOption::None`, `prune` off.
4. Adopt staged tags whose name is free in `refs/tags/`; a differing existing local tag is kept and reported
   (`kept_local_tags`); delete staging refs (also on error).
5. `new_commits` = Σ over changed remote-tracking refs of revwalk(push new, hide old) (capped 100k).
6. Never touches `refs/heads/*`, HEAD, index, config or work tree. `last_fetched_at` updated; summary refreshed.

**Fast-forward** (FR-11, managed only): `graph_descendant_of(origin_tip, local_tip)` → `reference(refs/heads/<d>,
origin_tip, force=true, "fast-forward")`; equal → no-op; otherwise `Refused("Branch has diverged from origin")`.

### 5.5 Background work, progress and cancel

- **Jobs** (clone, fetch, fetch-all) live in `JobRunner` inside app state, independent of any dialog (L-6). A
  `Gate` caps running network jobs at 3; extra jobs are `Waiting` and cancellable while waiting.
- **Progress transport**: on startup the frontend jobs store calls `jobs_subscribe(channel)` once (and again after
  a webview reload); the backend keeps the live `Channel<JobInfo>` list and pushes every state change and throttled
  progress (`JobCtx::progress` drops events < 80 ms apart, terminal states always sent). The returned snapshot lets
  the popover rebuild after reload. Finished rows auto-hide in the UI after 5 s; failures stay with Retry (FR-7).
- **Request-scoped work** (last commits, counts, languages, ahead/behind, library refresh): the UI generates a
  `requestId` (uuid-ish string), the backend registers a `CancelToken` under it for the duration of the call;
  `request_cancel(requestId)` sets it; walks check it every 256 commits / 1024 entries. The `useQuery` hook cancels
  on unmount/route change (FR-16). A separate **compute gate of 2** keeps heavy walks from starving clone threads
  (share-machine rule).
- Results of request-scoped work are always written to the cache before returning, so a cancelled-then-revisited
  directory restarts the walk but a completed one is instant.

---

## 6. Algorithms

### 6.1 Last commit per entry (FR-16, NFR-P3, R-1)

Input: tip commit `T`, directory `D` (git path), entry names `E` (from `T`'s tree at `D`). Cache key
`(repo_id, T, D)`; hit → return.

```
pending = {e → oid_at(T, D/e)} ; result = {}
walk = revwalk(push T, sort TIME)            // git-log order; follows all parents
for C in walk (stop at cap = 10_000 or pending empty or cancel):
    sub_C = subtree oid of D in C (None if D absent)
    parents = C.parents (0..n)
    if any parent P has subtree(P, D) == sub_C: continue      // TREESAME for D → nothing in D changed vs that parent
    entries_C = map name → oid from tree sub_C (only names in pending)
    for e in pending:
        oid_C = entries_C.get(e)
        changed = parents.is_empty() ? oid_C.is_some()
                 : parents.all(|P| entry_oid(P, D/e) != oid_C)
        if changed: result[e] = summary(C); pending.remove(e)
remaining pending → None ("older than 10,000 commits", capped=true)
```

Only subtree oid comparisons and one tree read per touched commit; unchanged subtrees are skipped with an oid compare
(cheap). Subtree lookups per commit are memoised by commit oid within the walk (parents are looked up again as
children). The "differs from every parent" rule matches git's history simplification for merges well enough for a
"last touched" column; a test compares against `git log -1 --format=%H -- <path>` on linear and merge fixtures
(merge fixture: entries touched only on a side branch must report the side commit, not the merge). Result cached as
JSON; file view's "last commit for this file" reads the parent directory's cache entry or runs the same walk with
`E = {name}`.

### 6.2 Language stats (FR-19, NFR-P6)

Cache key `(repo_id, root tree oid)`. Recursive `tree.walk(PreOrder)`; skip subtrees whose path matches the
exclusion list (`vendor/`, `third_party/`, `node_modules/`, `dist/`, `build/`, `target/`, `.git*`, `docs/`,
`Documentation/`, `*.min.js`, `*.min.css`, `*.lock`, `package-lock.json`, `*.pb.go`, `*_generated.*`); classify by
filename (`Makefile`, `Dockerfile`, `CMakeLists.txt`) then extension via `langs_table.rs` (≈ 80 languages; markup
like Markdown/JSON/YAML counted as "data" and excluded like linguist's default, except HTML/CSS). Blob size from
`odb.read_header(oid)` (no content read). Sum bytes per language; percent = bytes/total; sort desc; top 6, rest
"Other". Cancellable; runs off the UI path; the Code tab shows the bar when ready. `primary_language` of the default
branch tip is written to `repos` for the dashboard. Colours: TS `util/lang-colors.ts`, own palette, theme-aware.

### 6.3 Commit counts (FR-14, NFR-P5)

`revwalk(push tip)` counting oids (no commit parsing beyond what revwalk needs), cancellable, cached
`(repo_id, tip)`. `commit_latest` returns the cached count or `null`; the UI then calls `commit_count` (spinner →
"N commits"). Header tab counts: branches/tags cheap (ref iteration); releases = version-like tags; commits count of
the default branch from cache.

### 6.4 Commit paging (FR-26, D-9)

`commits_page(rev, path?, since?, skip, tip?)`: tip = `tip` arg if given (pins the walk across pages) else resolved
rev. `revwalk(push tip, hide since?, sort TIME)`, skip `skip` matching commits, take 36 (35 + has-more probe). With a
path filter a commit matches when its entry oid at `path` differs from its first parent's (or it is root and
present); scanning stops at `scan_cap = 20_000` per call → `capped` ("Search older commits" button continues with
the returned skip). A 10k linear history page at skip 9,965 walks 10k oids — measured < 200 ms in the bench. Order is
identical to one full walk because every page re-walks from the same pinned tip (test on the merge fixture: the
concatenation of all pages equals `git log --format=%H <tip>`).

### 6.5 Diff generation (FR-27, FR-28, R-8)

`diff_tree_to_tree(first_parent_tree | empty tree, commit_tree)` with `DiffOptions{ max_size: 1 MB, context 3,
ignore_submodules }`, then `find_similar(renames + copies off, rename threshold 50, limit 1000)`. For each delta:
kind, paths, binary flag, `too_large` (either side > 1 MB); `generated` heuristically (lockfiles, `*.min.*`,
`dist/`). Per-file stats via `Patch::from_diff(i).line_stats()`, computed only while total files ≤ 300; beyond that
`diffs_deferred = true` and only the file list (from deltas, no stats) is sent. Inline hunks are included for files
with ≤ 2,000 changed lines while the running total ≤ 20,000 lines; others are collapsed with "Load diff" →
`commit_file_diff(index, force=true)` (still refuses binary; > 1 MB shows "Large diff not rendered"). Lines are lossy
UTF-8 with CRLF stripped and a `NoNewline` marker. Merge commits: diff against first parent + note "Showing changes
from the first parent". Totals match `git show --stat` (test).

### 6.6 README discovery (FR-17)

In the listed directory, case-insensitive match, priority: `README.md`, `README.markdown`, `README.mdown`,
`README.mkd`, `README.rst`, `README.txt`, `README`, then any `README.*`; at the repo root also `.github/README.md`
and `docs/README.md` if none found. Markdown → `markdown::render`; others → escaped `<pre class="readme-plain">`.
Rendering pipeline (`markdown.rs`):
1. comrak with `extension.{table, strikethrough, tasklist, autolink, footnotes, header_ids = "user-content-"}`,
   `render.unsafe_ = true` (raw HTML passes to step 3 — ammonia is the single gate), `render.github_pre_lang = false`.
2. Rewrite via ammonia's `url_relative(UrlRelative::Custom)`: relative URLs are resolved against `base_dir` into
   `rsrepo:/<git path>[#frag]` (`..` resolved lexically; escaping above root → link dropped). Fragment-only links
   stay `#user-content-…`.
3. ammonia allow-list: GFM tags (h1–h6, p, a, img, ul/ol/li, table/thead/tbody/tr/th/td, pre, code, blockquote,
   em/strong/del, hr, br, details/summary, sup/sub, kbd, input[type=checkbox, disabled, checked]); attributes:
   `a[href,title,id]`, `img[src,alt,title,width,height]`, `th/td[align]`, `h*[id]`, `code[class]` only for
   `language-<id>` from a fixed list; URL schemes `http`, `https`, `mailto`, `rsrepo`; `data:image/(png|gif|jpeg|webp)`
   only in `img` (ammonia attribute filter); everything else (script, style, iframe, object, embed, form, `on*`,
   `style=`, `javascript:`) stripped. `rel="noopener noreferrer"` set on links.
4. Headings collected for the outline button.

Frontend (`MarkdownBody.tsx`): HTML is parsed into an **inert** `<template>` (images don't load, nothing runs);
then `rsrepo:` anchors → in-app `blob` routes (Blob view redirects to tree view when the path is a directory);
`rsrepo:` images → `blob_bytes` → `blob:` URL (revoked on unmount); remote `http(s)` images → replaced by a link
chip "Remote image: <host>" (NFR-S2, offline); `code.language-*` → highlighter worker; then the fragment is
mounted. All link clicks are intercepted (NFR-S3).

### 6.7 Releases (FR-31)

Tags whose name passes `is_version_tag`, newest first by (tagger date for annotated, else commit date), ties by
semver desc. `prerelease` = has `-` suffix (`-rc1`, `-beta`). `latest` = first non-prerelease. Title = first line of
the annotation if annotated and not equal to the tag name, else tag name; notes = rest of the annotation rendered by
`markdown::render(base_dir = "")`. `previous_tag` = next older release → "Commits since <prev>" link to
`/commits/<tag>?since=<prev>`. Optional GitHub notes (`ghreleases`, github.com origins only, one GET of
`/repos/<o>/<r>/releases?per_page=100` via ureq/rustls, 10 s timeout, cached with `fetched_at`) replace notes for
matching tag names; the body goes through the same `render`.

### 6.8 Other

- **Text detection** (R-7): NUL in first 8 KB → binary; UTF-8 BOM stripped; invalid UTF-8 → `from_utf8_lossy`,
  `lossy=true` notice. LFS pointer = starts with `version https://git-lfs.github.com/spec/v1` and ≤ 1 KB.
- **License**: root `LICENSE*`/`COPYING*`/`LICENCE*`; first 2 KB matched against phrases for MIT, Apache-2.0,
  GPL-2.0/3.0, LGPL, AGPL-3.0, BSD-2/3-Clause, MPL-2.0, ISC, Unlicense, CC0; else "Other".
- **Summary refresh** (`library_refresh`, startup + window focus, R-9): per repo, exists? open? resolve default tip;
  if tip ≠ `head_tip`, update `last_commit_at`, and recompute `primary_language` in the background (cache by tree).

---

## 7. Frontend architecture

### 7.1 Routes (hash, D-6/D-7/D-8)

| Pattern | Screen | Notes |
|---|---|---|
| `#/` | S1 Dashboard | `?q=` filter |
| `#/settings` | S10 Settings page | |
| `#/:owner/:repo` | S3 Code at default branch, root | |
| `#/:owner/:repo/tree/:ref/*path` | S3 Code | `:ref` = one encoded segment |
| `#/:owner/:repo/blob/:ref/*path` | S4 File | `?plain=1` raw, `?L=10-20`, `?view=code` for md |
| `#/:owner/:repo/commits/:ref` | S5 Commits | `?path=` `?since=` `?page=` `?tip=` |
| `#/:owner/:repo/commit/:oid` | S6 Commit | `?file=N` scroll |
| `#/:owner/:repo/branches` · `/tags` · `/releases` | S7 · S8 · S9 | |
| anything else | NotFound | |

`router.ts`: `parse(hash) → {name, params, query}`, `href(name, params, query)`, `navigate(to, {replace})`,
listens to `hashchange`; history entries get a key (in `history.state`) → `Map<key, scrollY>` restores scroll on
back/forward (FR-34). Mouse buttons 3/4 and Alt+←/→ call `history.back/forward`. "Copy location" produces
`<owner>/<repo>/tree/<ref>/<path>` (decoded, GitHub-style).

### 7.2 Component tree per screen

```
App
├─ ThemeProvider (data-theme on <html>)  ├─ GlobalHeader (S0): Logo · RepoJump(search+dropdown, "/") · AddMenu(+)
│                                         │   · JobsButton(+JobsPopover rows: progress/cancel/retry) · ThemeToggle · SettingsLink
└─ RouteView
   ├─ Dashboard (S1): RepoToolbar(filter, SortMenu, SourceFilter, NewButton) · RepoList(virtual > 500) → RepoRow
   │                 (name, SourceBadge, description, TopicChips, LangDot, origin, RelativeTime, MissingRow)
   │                 · Sidebar(RecentList, ArchiveStats, FetchAllButton) · EmptyState
   ├─ RepoLayout (S2): RepoHeader(title, SourceBadge, FetchButton, OpenFolder, MoreMenu→RemoveDialog) · TabBar(counts)
   │   · RepoBanner(missing/corrupt) · <Outlet>
   │   ├─ CodeTab (S3): RefSwitcher(Branches|Tags tabs, filter) · CountsLinks · CodeMenu · Breadcrumbs
   │   │     · LatestCommitBox(Initials, author, summary, oid, RelativeTime, CommitCountLink)
   │   │     · FileTable(virtual > 500; rows: EntryIcon, name, LastCommitCell(Shimmer), date)
   │   │     · ReadmeCard(Outline, MarkdownBody) · AboutSidebar(gear→EditAboutDialog, chips, links,
   │   │       ReleasesBox, LanguagesBox(LangBar, LangList)) · edge states (Empty, Detached, RefNotFound)
   │   ├─ FileView (S4): Breadcrumbs+CopyPath · FileCommitHeader · FileToolbar(lines/sloc/size, Raw, Copy, Save, History)
   │   │     · CodeViewer(virtual gutter+lines, selection L=) | ImagePreview(checker) | Placeholder | MarkdownTabs
   │   ├─ CommitsTab (S5): RefSwitcher · PathChip · DayGroup → CommitRow(expand body, CopyOid, BrowseTree) · Pager
   │   ├─ CommitPage (S6): CommitHeader · DiffSummary · FileDiffCard(header, MiniBar, DiffTable | Collapsed) [virtual]
   │   ├─ BranchesTab (S7): Filter · BranchSection(Default/Yours/Remote) → BranchRow(AheadBehindBar lazy)
   │   ├─ TagsTab (S8): ReleasesTagsToggle · TagRow
   │   └─ ReleasesTab (S9): ReleaseCard(left meta, title, LatestBadge, MarkdownBody, compare link) · GhNotesButton
   ├─ Settings (S10): theme radio, DataDir/ArchiveDir (+Open), cache size + Clear, dirty-note toggle
   └─ Dialogs (S10): ImportDialog · CloneDialog(live validation via clone_preview, debounced) · EditAboutDialog
                     · RemoveDialog · ConfirmExternalLink
```

### 7.3 State

- No state library. Global stores with `useSyncExternalStore`: `theme`, `settings`, `library` (list + refresh),
  `jobs` (subscribed channel), `recent`.
- Server data via `useQuery(key, fn, {cancelable})` in `ipc/query.ts`: in-memory LRU (200 entries) keyed by
  command+args — oid-keyed results (tree at resolved commit, blobs, commit detail) never go stale; ref-keyed ones
  (`refs_list`, `repo_detail`, `library`) are invalidated on window focus and after jobs finish. Gives instant
  back-navigation (FR-16 "second visit instant", FR-34).
- Each repo route first resolves `slug → repoId` (`repo_by_slug`, cached) and `rev → ResolvedRev`; children receive
  the resolved commit oid so all their queries are oid-keyed.

### 7.4 IPC layer and mock

- `bridge.ts`: the only module importing `@tauri-apps/api` (`invoke`, `Channel`). `isTauri = '__TAURI_INTERNALS__' in window`;
  otherwise every call routes to `mock/`. Errors normalised to `IpcError {code, message}`.
- `api.ts`: one typed function per command (§5.1). Screens import only `api.ts`.
- `mock/`: fixtures in TS for 4 repos (one per kind: imported Rust repo with README/table/code/relative image, cloned
  TS repo with releases, empty repo, missing repo) + a security README; switches read from `location.search` at
  start: `?empty` (no repos), `?slow` (600 ms delay on every call, shimmer visible), `?clonefail`, `?fetchfail`,
  `?huge` (1,000-entry dir, 10k-commit paging, 400-file commit). Mock jobs simulate progress with timers. FR-37.
- `.claude/launch.json` → `npm run dev` on 1450; Vite `strictPort`.

### 7.5 CSS token system

`styles/tokens.css`: `:root[data-theme="light"]` and `[data-theme="dark"]` (system = media query resolved in
`theme.ts`). Own palette (accent is a teal-indigo, not GitHub blue). Groups:
- surfaces: `--bg-canvas`, `--bg-subtle`, `--bg-inset`, `--bg-overlay`, `--bg-hover`, `--bg-selected`
- text: `--fg-default`, `--fg-muted`, `--fg-subtle`, `--fg-on-accent`, `--fg-link`
- borders: `--border-default`, `--border-muted`, `--focus-ring`
- accents/states: `--accent`, `--accent-subtle`, `--success`, `--danger`, `--attention`, `--done`
- diff: `--diff-add-bg`, `--diff-add-num-bg`, `--diff-del-bg`, `--diff-del-num-bg`, `--diff-hunk-bg`, `--diff-hunk-fg`
- code tokens: `--hl-keyword`, `--hl-string`, `--hl-number`, `--hl-comment`, `--hl-function`, `--hl-title`,
  `--hl-type`, `--hl-attr`, `--hl-variable`, `--hl-meta`, `--hl-deletion`, `--hl-addition`
- layout: `--radius-sm/md`, `--space-1..6`, `--font-sans` (Inter var), `--font-mono` (JetBrains Mono var),
  `--header-h`, `--content-max` (1280px), breakpoint 1000 px (sidebar stacks).
`code.css` maps `.hljs-keyword` etc. to tokens. A vitest checks WCAG contrast ≥ 4.5:1 for every text token pair in
both themes (FR-33) by parsing `tokens.css`.

### 7.6 Highlighter integration (D-5)

- `hl.worker.ts` imports `highlight.js/lib/core` and lazily `import()`s `highlight.js/lib/languages/<id>` (Vite
  emits one small chunk per language; NFR-P8). Message: `{id, langId, text}` → `{id, lines: string[]}` where each
  line is a self-contained HTML string (`splitLines.ts` closes open `<span>`s at `\n` and reopens them on the next
  line — unit-tested). hljs escapes text; output contains only `<span class="hljs-…">`.
- `languages.ts`: extension/filename/shebang → hljs id (≈ 45 languages registered on demand); unknown → plain.
- Files > 1 MB (`meta.highlight=false`) or > 20k lines → plain text; chunks of 2,000 lines posted back progressively
  so the first screen appears immediately (NFR-P4). CodeViewer virtualises lines; raw lines show first, highlighted
  HTML swaps in per chunk.
- The worker is the only path that produces HTML from file content besides the ammonia-sanitised markdown.

---

## 8. Security

**CSP** (`tauri.conf.json` → `app.security.csp`):

```
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; font-src 'self';
connect-src ipc: http://ipc.localhost; worker-src 'self'; object-src 'none'; base-uri 'none';
form-action 'none'; frame-src 'none'; media-src 'none'
```

`app.security.devCsp` (exists in Tauri 2's schema, verified in PM's `@tauri-apps/cli/config.schema.json`) = same
string plus `style-src 'self' 'unsafe-inline'` and `connect-src … ws://localhost:1450` for Vite's injected
`<style>` tags and HMR — dev only. Tauri adds hashes/nonces for its own scripts. `assetProtocol` disabled (D-11).
React `style={{…}}` props are applied via CSSOM (allowed under `style-src 'self'`); used only for computed widths
(language bar, virtual list offsets). Done-when (M8): zero CSP violations in the real app console while visiting
every screen including worker highlighting, virtualised lists and the security README.

**Capabilities** (`capabilities/default.json`, window `main`): `["core:default"]` only — dialogs, opener and trash
are invoked from Rust commands (D-10), so the webview has no `dialog:*`, `opener:*`, `fs:*`, `shell:*`, `http:*`.
The plugins are still registered in `lib.rs` for Rust-side use. (If `core:default` exposes window/webview APIs we do
not need, narrow to `core:event:default`, `core:app:default` in M4 and confirm the app still runs.)

**Sanitisation**: §6.6 (ammonia allow-list, inert template, remote images blocked). File contents are never inserted
as HTML except hljs output. SVG files only via `<img src=blob:…>` (scripts never run in image context, NFR-S7);
`blob_bytes` sets the mime from the extension allow-list (png, jpeg, gif, webp, bmp, x-icon, svg+xml). No HTML/PDF
previews.

**Links**: a global click handler in `MarkdownBody` and for About/origin links: in-app (`rsrepo:`, `#/…`) →
router; `http(s)`/`mailto` → `ConfirmExternalLink` dialog showing the full URL → `open_external` (Rust re-validates,
opener plugin). Everything else ignored. `window.open` is never used; Tauri's default new-window handling is
denied via `on_navigation` returning false for any non-app URL (defence in depth).

**Network**: only `clone_start`, `fetch_start`/`fetch_all_start`, `releases_fetch_github`. No credentials callback,
no ssh transport compiled in, no submodule recursion, no LFS. `GIT_TERMINAL_PROMPT` irrelevant (libgit2). A
test asserts no git2 remote or ureq call happens outside those commands (a counting hook in rs-core under
`testkit`), plus the M8 browse-only run with the network adapter disabled (NFR-O1).

**Imported repos**: opened read-only by convention (no index, lock or config writes; `statuses()` only behind D-16).
`.git` hash test before/after the browse-only e2e (NFR-O2).

---

## 9. Testing strategy

- **Rust unit tests (rs-core)** with fixture repos built in `tempfile::TempDir` by `testkit` (git2 only; no git CLI
  needed, but where `git` is on PATH, comparison tests against `git log`/`rev-list`/`show --stat` run and are
  skipped otherwise). Coverage: validate (hostile table), winpath (PM's cases: `a:b.txt`, `CON.txt`, `llm`+`LLM/x`),
  db migrations (fresh + reopen), settings round-trip, jobs (gate cap 3, waiting cancel, throttle, retry), refs
  resolution order, tree sort + kinds, blob sniff (binary, LFS, symlink, submodule, non-UTF-8, BOM, >1 MB, >5 MB),
  commits paging (pages concatenated == one walk on the merge fixture), counts cache, lastcommit (linear + merge +
  cap + cancel), langs (.rs/.ts percentages, exclusions), diff (stats vs expected, renames, root commit, merge,
  caps), markdown (security fixture: script, onerror, javascript:, iframe, style attr, remote img, data:text/html
  stripped; relative link/img rewritten; task lists, tables, code class kept), releases, clone (via
  `clone_from_unchecked` with `file://` source: bare result, skipped refclash, cancel leaves no folder,
  partial cleanup), fetch (2 new commits + 1 tag seen; local same-name tag untouched; imported fixture: only allowed
  refs changed), fast-forward + diverged refusal, import (duplicate, non-repo, bare), metadata validation.
- **Imported-repo immutability test**: hash every file under `.git` (and work tree) of an imported fixture, run every
  read function, re-hash → equal (NFR-O2), with D-16 on and off.
- **rs-cli bench**: `rs fixtures <scratch>` then `rs bench <scratch>` (release build) times `tree_list` on the
  1000-entry dir (< 300 ms incl. sizes), `lastcommit` second visit (< 50 ms), `commits_page` at skip 0 and 9,965
  (< 200 ms), languages on a 50k-file tree (< 3 s), `commit_detail` on a 400-file commit (< 1 s); exits non-zero on
  a miss (PoC criterion 3).
- **TS unit tests** (vitest + happy-dom): router parse/href round-trips (encoded refs, `?L=`), splitLines, languages
  map, format (relative time, day groups, bytes), linkRewrite on an inert template (rsrepo:, remote img, javascript:),
  query cache/cancel, mock switches, tokens contrast, component tests for FileTable sort/`..` row, RefSwitcher
  filter, CloneDialog validation messages.
- **Mock-UI browser checks**: `npm run dev` (launch config `reposhelf-mock`), visit every route × light/dark ×
  switches listed in `tools/shots/mock-shots.md` using the browser preview; screenshots reviewed; no console errors.
- **Real app / e2e (M8)**: `tools/e2e/run-e2e.ps1` modelled on PM: builds scratch `RS_DATA_DIR`/`RS_ARCHIVE_DIR`,
  fixtures via `rs fixtures`, starts the portable exe through tauri-driver + msedgedriver (window moved offscreen
  in-process), smoke dashboard → repo → file → commits → commit → branches/tags/releases → clone (debug-only file
  clone seam `RS_TEST_FILE_CLONE=1`, compiled only with `debug_assertions`) → fetch; network clone of 2 small public
  repos is a separate opt-in `-Network` switch. Kills its process tree (`taskkill /T /F`), then counts leftover
  `reposhelf`, `tauri-driver`, `msedgedriver`, `msedgewebview2` children it started → must be 0.

---

## 10. Milestones

Every milestone ends with: all earlier done-when commands still green, process-tree kill + leftover check if anything
was launched, WORKLOG entry, and a local commit (`<area>: M<n> <summary>` + Co-Authored-By line). Never push.
Builds use the repo's own `target\`. Commands run from `C:\Dev\projects\repo-shelf`.

### M0 — Project layout
- Scope: workspace + empty crates that compile with **all registry deps pulled in** (comrak, ammonia, highlight.js,
  react-virtual); Tauri shell showing "RepoShelf" in a plain page; icon generated from own SVG; docs scaffolding.
- Files: `Cargo.toml`, `crates/rs-core/{Cargo.toml,src/lib.rs}`, `crates/rs-cli/{Cargo.toml,src/main.rs}`,
  `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,src/main.rs,src/lib.rs,icons/*}`,
  `package.json`, `vite.config.ts`, `vitest.config.ts`, `tsconfig.json`, `index.html`, `src/main.tsx`, `src/App.tsx`,
  `.gitignore`, `.claude/launch.json`, `docs/{DECISIONS.md,WORKLOG.md,icon/reposhelf.svg}`.
- Done when: `npm install` ok; `cargo build --workspace` ok (git2 vendored, comrak, ammonia compile on Rust 1.99);
  `cargo test --workspace` ok (1 smoke test that renders a markdown string through comrak+ammonia);
  `npm run typecheck`, `npm test` (1 test that imports `highlight.js/lib/core`), `npm run build` ok;
  `npm run dev` serves on 1450; DECISIONS.md has D-1..D-17.

### M1 — Framework layer (rs-core)
- Scope: `error`, `paths`, `db` + `migrations/0001_init.sql`, `settings`, `validate`, `winpath`, `jobs` (Gate,
  JobRunner, throttle, CancelToken), `types`, `testkit` builder basics.
- Done when: `cargo test -p rs-core` covers: env overrides honoured (`RS_DATA_DIR`, `RS_ARCHIVE_DIR`), migration on
  fresh + reopen (user_version), settings round-trip, hostile path/rev/url tables, topics/homepage rules, winpath PM
  cases, gate never > 3 running with 6 submitted, waiting job cancel, progress throttle ≥ 80 ms; `cargo clippy
  --workspace --all-targets -- -D warnings` clean.

### M2 — Data layer: git read model (rs-core)
- Scope: `repo`, `refs`, `tree`, `blob`, `commits`, `diff`, `lastcommit`, `langs`(+table), `readme`, `markdown`,
  `license`, `releases`; testkit fixtures (basic, wide-dir, long-history, merges, binaries, security-readme,
  releases); `rs fixtures` + `rs bench` in rs-cli.
- Done when: `cargo test -p rs-core` passes all read-model tests in §9 (incl. paging-equals-full-walk on merges,
  lastcommit vs `git log -1 -- path`, diff stats vs `git show --stat`, markdown security fixture);
  immutability hash test green; `cargo run -p rs-cli --release -- fixtures <scratch>` +
  `-- bench <scratch>` meets NFR-P2/P3/P5/P6 numbers and FR-28's < 1 s.

### M3 — Data layer: library, clone, fetch (rs-core)
- Scope: `library` (import, list, refresh, slug, metadata, relocate, remove record), `clone` (bare, atomic,
  refclash skip, cleanup_partials, `clone_from_unchecked`), `fetch` (staging tags, new-commit count), fast-forward.
- Done when: `cargo test -p rs-core` covers FR-5 (import/duplicate/non-repo/bare, repo hash unchanged), FR-6 (every
  rejected form refused, validator pure), FR-7/8 (cancel → no folder; leftover partial removed at startup), FR-9
  (refclash + invalid ref names skipped and reported; `a:b.txt`/`CON.txt` fixture clones bare fine), FR-10 (2 commits +
  1 tag; local same-name tag untouched; imported fixture: only allowed refs changed), FR-11 (FF + diverged refusal),
  FR-20 persistence, FR-4 missing status.

### M4 — IPC commands + tests (src-tauri)
- Scope: `state.rs` (Shelf, JobRunner with Channel subscribers, CancelRegistry, compute gate), `ipc_error.rs`, all
  commands in §5.1 under `cmd/`, Rust-side dialogs/opener/trash, startup (dirs, DB, cleanup_partials, interrupted
  jobs), CSP + devCsp + capabilities, `on_navigation` guard.
- Done when: `cargo test -p reposhelf` (command-layer tests call the command functions with a test `Shelf`, no
  window): hostile inputs refused for every path/rev-taking command; `open_external` refuses non-http(s); `blob_bytes`
  cap; job subscribe snapshot; `cargo build -p reposhelf` ok; `npx tauri dev` opens the M0 page with no CSP errors
  (then process tree killed, leftovers 0); `src/ipc/types.ts` written to match `types.rs` (checked by a Rust test
  that serialises one sample of every type to `target/ipc-samples.json` and a vitest that validates those samples
  against the TS fixture shapes).

### M5 — Frontend shell + mock
- Scope: router, bridge/api/query, mock backend with all switches, theme store + tokens.css/base/layout/components
  CSS, GlobalHeader (search jump, Add menu, jobs popover, theme toggle, settings link), RepoLayout with header + tabs
  (placeholder tab bodies), hl worker + splitLines, MarkdownBody pipeline, dialogs skeleton.
- Done when: `npm run typecheck`, `npm test` (router, splitLines, languages, linkRewrite, query cancel, contrast,
  mock switches) green; `npm run build` initial JS < 1.5 MB (check `dist/assets` sizes; highlight languages are
  separate chunks); mock in browser on 1450: header works on every route, `?empty`/`?slow` visible, light + dark
  screenshots of shell.

### M6 — Screens A: Dashboard, Code tab, File view
- Scope: S1, S2 complete, S3 (ref switcher, latest commit, file table with lazy last-commit + virtualisation,
  README card, About sidebar, languages, edge states), S4 (code viewer with line anchors/selection, raw, copy, save,
  image, placeholders, markdown preview/code tabs), EditAbout/Import/Clone/Remove dialogs wired.
- Done when: component tests (FileTable order + `..`, RefSwitcher filter, CodeViewer `?L=` selection, Clone dialog
  validation) green; mock screenshots of S1, S3, S4 (+ every placeholder fixture, `?huge` 1000-entry dir) in light and
  dark reviewed; no console errors.

### M7 — Screens B: Commits, Commit, Branches, Tags, Releases, Settings
- Scope: S5 (day groups, paging with pinned tip, path filter chip, since), S6 (header, summary, file cards, collapsed
  large/binary, Load diff, rename display, virtualised), S7 (sections, lazy ahead/behind bars), S8, S9 (+ optional GH
  notes button behind a feature check), S10 Settings page, jobs popover final (auto-hide, retry).
- Done when: vitest for day grouping, pager, diff table rendering, ahead/behind bar; mock screenshots of S5–S10 in
  light/dark incl. `?huge` 400-file commit and `?clonefail`/`?fetchfail`; no console errors.

### M8 — Integration / real-app run
- Scope: real app against scratch dirs; `tools/e2e/{run-e2e.ps1,wd-e2e.mjs,check-e2e.ps1}`; security pass;
  performance pass; network-off pass; `docs/TESTING.md`.
- Done when: `npm run build:portable` produces the exe; `powershell -ExecutionPolicy Bypass -File
  tools\e2e\run-e2e.ps1` passes the smoke path of PoC criterion 5 (file-clone seam) and prints JSON results;
  `-Network` run clones 2 small public repos (one with Windows-invalid names) and fetches; console shows **zero CSP
  violations** across all screens (worker, virtual lists, security README); hostile IPC table refused in the real
  app; imported fixture `.git` hash unchanged after the browse-only run; `rs bench` targets met on the release build;
  browse-only run with the network adapter disabled works (manual step, listed in TESTING.md for the user if not
  automatable without admin); process trees killed, leftovers 0.

### M9 — Docs + release build
- Scope: `README.md` (what it is, data locations, env overrides, build/test commands), DECISIONS/WORKLOG final,
  version 0.1.0 in `package.json`/`Cargo.toml`/`tauri.conf.json`, NSIS installer (`bundle.targets: ["nsis"]`,
  `installMode: currentUser`) and portable exe via `tools/build-portable.mjs` (as PM), copies into
  `_REVIVE/releases/` if the kit exists.
- Done when: `npm run build:installer` produces `target/release/bundle/nsis/RepoShelf_0.1.0_x64-setup.exe` and
  `npm run build:portable` produces `target/release/portable/RepoShelf_0.1.0_x64-portable.exe`; the portable exe
  starts against scratch `RS_DATA_DIR`/`RS_ARCHIVE_DIR` and shows the dashboard (then killed, leftovers 0); the
  installer is **built but never run** (no install/uninstall on this machine); all test commands from M0–M8 green;
  tag `v0.1.0` (local).

---

## 11. Risks remaining after this design

- comrak 0.59 / ammonia 4.2 / highlight.js 11.12 / react-virtual 3.14 are not yet compiled on this machine (M0 gates
  it; fallback for comrak is pulldown-cmark 0.13, already in the local cargo cache).
- The last-commit "differs from every parent" rule can differ from `git log` on unusual merge topologies (evil
  merges); accepted, documented, tested on the common cases.
- `core:default` may grant more than needed; M4 narrows it if the app runs without.
- Path-filtered history on huge repos is bounded by `scan_cap`, so "History" may need several "Search older" clicks.
- The network-adapter-off check may need the user (admin) — listed as a hand step if it cannot be automated.
