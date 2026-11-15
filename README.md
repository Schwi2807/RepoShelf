# RepoShelf

A desktop shelf for your git repositories that looks and feels like browsing github.com, but reads everything from
your own disk. Import repositories you already have, or clone public `https://` repositories into an archive folder,
then browse code, commits, branches, tags and releases in a GitHub-style layout. Everything is read-only: RepoShelf
never writes into an imported repository. New in 0.3.0: **Explore** — search GitHub and browse any public repository
online in the same screens, without cloning it, then "Save to shelf" to keep it.

Version **0.3.0** (Windows first; Tauri 2 + Rust + React 19 + TypeScript + Vite). Own name, own icon, own app id
`dev.schwi.reposhelf`; all CSS and layout written from scratch (lucide icons).

![Code tab (light)](docs/screenshots/real-code-light.png)

| | |
|---|---|
| ![Code tab (dark)](docs/screenshots/real-code-dark.png) | ![Commit page (light)](docs/screenshots/real-commit-light.png) |
| ![Explore: search GitHub (real app, canned answers)](docs/screenshots/real-explore-search-light.png) | ![Explore: a repository read online, with the request quota (real app, canned answers)](docs/screenshots/real-explore-code-light.png) |
| ![Releases with assets (real app, canned GitHub list)](docs/screenshots/real-release-assets-light.png) | ![Centred README with a remote badge (real app)](docs/screenshots/real-align-readme-light.png) |
| ![GitHub-style centred README (mock data)](docs/screenshots/m12-readme.png) | ![Release cards with assets (mock data)](docs/screenshots/m12-releases.png) |
| ![Dashboard (mock data)](docs/screenshots/m6-dashboard.png) | ![Commits list (mock data)](docs/screenshots/m7-commits.png) |

## What it does

| Screen | What you get |
|---|---|
| Header (all screens) | Logo / home, Back / Forward, "Search or jump to a repo…" (`/`), "+" (Import folder…, Clone URL…), jobs popover with progress, Cancel, Retry, Show in folder for downloads, theme toggle (System / Light / Dark), settings |
| Dashboard | Repository list with filter, sort, source filter (Imported / Cloned), language dot, topics, "Updated …"; Recent (last 5 opened); archive size and count; Fetch all clones |
| Repo header | `owner / name`, source badge, Fetch (clones and https-origin imports; for github.com a menu: fetch with / without the release download), Open folder, Remove; tabs Code · Commits · Branches · Tags · Releases with counts |
| Code tab | Branch/tag switcher, file table with last-commit message and date per entry (loaded lazily), rendered README (sanitised, GitHub-style centring, remote images loaded by the app and cached), About sidebar (description, topics, homepage, origin — your edits are stored in the app DB and win over the values fetched from GitHub), language bar, latest-commit strip, commit count |
| File view | Breadcrumbs, syntax highlighting (in a worker) with line numbers, raw view, Markdown rendering, images (SVG only as `<img>`), binary / large / LFS-pointer placeholders, Save as… |
| Commits / Commit | Paged history per branch (optional path filter), commit page with a unified diff per file (merge commits compared with their first parent, parents linked) |
| Branches / Tags / Releases | Ahead / behind compared with the checked-out (HEAD) branch, tags with dates, releases from tags + annotated messages; for github.com origins GitHub's release notes (contributor avatars shown as `@user`) and an Assets list per release (size, downloads, Download, Show in folder, Open on GitHub, source code zip / tar.gz) |
| Explore (new in 0.3.0) | Header entry "Explore": search GitHub repositories (results with stars, language, updated; Open or Save to shelf) or type `owner/repo`. A repository opens in the same Code, file, README, Commits, commit, Branches, Tags and Releases screens, read from api.github.com. The strip at the top shows the requests left of GitHub's hourly limit and, when it is used up, when it resets (pages you opened before keep opening from the saved copy). Files over 1 MB show "Too large to preview online — Save to shelf". **Save to shelf** starts the normal bare clone in the background (with or without the latest release download); afterwards the page offers **Open in shelf** |
| Settings | Theme, data and archive folders (Open), Network (README images, GitHub release notes, GitHub repository info, release download on Clone / Import / Fetch, on Fetch all, size limit), **Explore** (on / off, optional GitHub token, Clear Explore cache), cache size + Clear cache, version |

Clones are bare (`<archive>\<host>\<owner>\<name>.git`), use Windows-safe handling (refs Windows cannot store are skipped and
reported; nothing is checked out, so file names never hit the disk), run in the background with progress, and can be cancelled. Removing a cloned repo moves its
folder to the Recycle Bin; removing an imported repo only takes it off the shelf.

## Download / run

- Installer (per user, no admin): `RepoShelf_0.3.0_x64-setup.exe` — installs to your user profile, adds a Start menu
  entry and an uninstaller in Settings → Apps.
- Portable: `RepoShelf_0.3.0_x64-portable.exe` — one exe, nothing installed. It uses the same data folders as the
  installed app (below).

Both are built from this repo (see Build) and copied with `SHA256SUMS.txt` into `_REVIVE\releases\v0.3.0\` (the 0.1.0
and 0.2.0 files stay in their own folders). Installing 0.3.0 over 0.2.0 (or 0.1.0) keeps your shelf: the database is
upgraded in place on the first start (0.3.0 adds two tables for the Explore cache and quota). They are unsigned: Windows SmartScreen asks once (More info →
Run anyway). Needs the WebView2 runtime (part of Windows 11).

## Data locations

| What | Default | Override (env var) |
|---|---|---|
| App data: database `reposhelf.db` (library, metadata, caches, settings, jobs) and the WebView profile | `%LOCALAPPDATA%\dev.schwi.reposhelf` | `RS_DATA_DIR` |
| Archive: bare clones made by RepoShelf | `Documents\RepoShelf` | `RS_ARCHIVE_DIR` |
| Downloaded release files | `<archive>\_releases\github.com\<owner>\<name>\<tag>\` | follows the archive |
| README image cache (up to 200 MB, emptied by Clear cache) | `<data>\img-cache\` | follows the data dir |
| Explore cache (answers from api.github.com, up to 100 MB, in the database; emptied by Clear Explore cache and by Clear cache) | `<data>\reposhelf.db` | follows the data dir |
| GitHub token (optional) | Windows Credential Manager, entry `dev.schwi.reposhelf` / `github-token` — never in the database, files or logs | — |

Imported repositories stay where they are and are only read. With `RS_DATA_DIR` set, the WebView profile goes to
`%RS_DATA_DIR%\webview`, so a scratch run shares nothing with your real shelf. Example (PowerShell):

```powershell
$env:RS_DATA_DIR = 'C:\Temp\rs-try\data'; $env:RS_ARCHIVE_DIR = 'C:\Temp\rs-try\archive'
& .\RepoShelf_0.3.0_x64-portable.exe
```

## Network use

Browsing your shelf never needs the network: code, commits, branches, tags and releases are read from disk. The app goes
online only for the things below, and each can be switched off in Settings (Network, Explore). All requests are made by
the app itself (never by the page), over https to public hosts only, without cookies or sign-in.

| What | When | Setting (default) |
|---|---|---|
| git Clone / Fetch / Fetch all | when you start them | — (only on request) |
| GitHub repository info (description, topics, homepage) | after Clone, Import, Fetch and Fetch all of a github.com repository | Fetch GitHub repository info (on) |
| GitHub release notes and asset lists | same, and the "Load GitHub release notes" button on Releases | Fetch GitHub release notes (on) |
| The latest release file for this PC | after Clone, Import, Fetch (per action: the checkbox in the Clone / Import dialog, the Fetch menu) and the Download buttons on Releases | Download the latest release for this PC (on); Also download on "Fetch all" (off); size limit (1024 MB) |
| README images from other sites | when a README with remote images is shown (then cached, so they also show offline) | Load remote images (on); off = a "Remote image: host" chip |
| Explore: search and repository pages | only when you open the Explore entry, search, or open an `explore/…` page; every answer comes from `api.github.com` (nothing else is contacted, no redirects except GitHub's own repository-moved one), is kept in the database (ETag, up to 100 MB) and reused | Enable Explore (on); off = no entry and no request |
| GitHub token | only if you paste one in Settings → Explore; sent as `Authorization: Bearer` to `api.github.com` and nowhere else | optional; Remove deletes it |

Downloaded release files are never run or unpacked by RepoShelf. Windows marks them as downloaded from the internet,
and your antivirus may scan (or hold back) a downloaded `.exe` before you can open it. GitHub allows about 60
unauthenticated API requests per hour (10 searches a minute); Explore shows what is left, reuses saved answers instead
of asking again, and when the limit is reached says when it resets while saved pages keep opening. A GitHub token (a
classic or fine-grained one with no permissions is enough for public repositories) raises the limit to 5,000 an hour;
it is stored in Windows Credential Manager only, never shown again after saving, and removed with the Remove button
(or in Credential Manager itself). The other jobs get a note when the limit is reached and later requests wait.

## Develop

Needs Node 24 + npm, Rust (stable, edition 2024) and the Tauri 2 prerequisites for Windows (WebView2, MSVC build
tools). In a fresh PowerShell window refresh the process PATH first (see `docs/TESTING.md`).

```powershell
npm install                 # repo-local only
npm run dev                 # UI with the mock backend at http://localhost:1450/ (no Rust needed)
npx tauri dev               # the real app against your real data folders - set RS_DATA_DIR / RS_ARCHIVE_DIR to try it on scratch folders
```

Mock preview: in a plain browser the UI loads an invented mock backend (`src/ipc/mock/`). Switches go before the `#`:
`?empty`, `?slow`, `?huge`, `?clonefail`, `?fetchfail`, `?gh` (GitHub lists already cached) (e.g.
`http://localhost:1450/?huge#/northwind/parsekit`). Launch config `reposhelf-mock` in `.claude/launch.json`.

Layout: `crates/rs-core` (git read model, library, clone/fetch, caches, HTTP client, GitHub info, release downloads; no
Tauri), `crates/rs-cli` (`rs fixtures`, `rs bench`), `src-tauri` (IPC commands, window, OS seams), `src/` (React UI),
`tools/` (portable build, shipped-exe check, e2e harness, mock screenshot list). Design docs in `docs/`.

## Test

```powershell
cargo test --workspace --all-features
cargo clippy --workspace --all-targets -- -D warnings
npm run typecheck
npm test
cargo run -p rs-cli --release -- fixtures target\scratch\bench; cargo run -p rs-cli --release -- bench target\scratch\bench
```

Real-app end-to-end run (separate e2e build with test seams; scratch folders only; window off-screen):

```powershell
npm run build
$env:CARGO_TARGET_DIR = (Join-Path (Get-Location) 'target\e2e'); npx tauri build --no-bundle --features e2e; Remove-Item Env:CARGO_TARGET_DIR
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1     # -Network adds two public clones (opt-in)
```

The default e2e run never reaches the internet: every HTTP request of the app (including the whole Explore flow: search,
browsing, rate limit, token, Save to shelf) is answered from a canned routes file, and the github.com clone goes to a
local upstream (test seams that exist only in the e2e build). The e2e build keeps the GitHub token in an in-memory fake,
so a run never touches the real Credential Manager; the run scans every scratch file for the fake token.

Details, every gate command and the hand-test checklist: `docs/TESTING.md`.

## Build a release

```powershell
npm run build:installer     # target\release\bundle\nsis\RepoShelf_<ver>_x64-setup.exe (Tauri's NSIS bundler, currentUser)
npm run build:portable      # target\release\portable\RepoShelf_<ver>_x64-portable.exe
```

Shipped builds go through the Tauri CLI only, never with `--features e2e` and never with `cargo build --workspace`
(that would compile the test kit in); `build:portable` refuses an exe that contains a test-seam name
(`RS_TEST_FILE_CLONE`, `RS_E2E_` — the e2e network and git seams included) or lacks the real Windows credential store
(`node tools\check-shipped.mjs <exe> --e2e <e2e exe>` runs the same check with the e2e exe as control). Keep `build.assetsInlineLimit: 0` in
`vite.config.ts` (the CSP blocks `data:` fonts). The version lives in `package.json`, `Cargo.toml`
(`workspace.package.version`) and `src-tauri/tauri.conf.json`.

## Assumptions (fixed for v0.1, still true in 0.3)

1. "From scratch" = a new codebase, not a fork of Project Manager (read only for lessons).
2. "Resembles GitHub" = the layout and arrangement of github.com; own name, icon and app id, no GitHub logo, Primer
   CSS or copied assets.
3. Stack: Tauri 2 + Rust (git2 0.21, vendored libgit2) + React 19 + TypeScript + Vite, npm. Windows first, code stays
   cross-platform.
4. Scope: a github.com-style browser over a local archive — dashboard, Code tab, file view, commits + commit diff,
   branches, tags, releases, import a local folder or clone a public https URL, fetch a clone, editable local metadata
   (description, topics, homepage), light + dark theme.
5. Deferred: issues, pull requests, Actions, wikis, projects, stars/social, sign-in / OAuth (0.3.0 has only the
   optional pasted token), pushing, editing files, private-repo cloning.

## Not in v0.3 (out of scope)

Issues, pull requests, Actions, wikis, projects, discussions, stars / watch / fork, sign-in / OAuth, private repo
cloning, ssh, pushing, committing, editing files, checkout / switching working trees of imported repos, blame, code
search across files (Explore searches repositories, not code), split diff, compare between arbitrary refs, LFS fetch,
submodule recursion, GitHub API beyond repository info, releases and Explore's read-only endpoints, running or
unpacking downloaded release files, auto-update, multi-user, sync between machines.

## Explore mode (0.3.0)

Header → **Explore** (hidden when Settings → Explore is off). Search GitHub's repositories, or type `owner/repo`, and
the same screens fill from `api.github.com`, read-only. Facts worth knowing:

- Every answer is kept in the database and reused (name-addressed ones for 15 minutes without a token, 60 seconds with
  one, then asked again with `If-None-Match`; commit-addressed ones never), so a repository you looked at opens again
  without a request and keeps opening while GitHub's limit is used up. A cold repository page costs 5 of the 60 hourly
  requests; branches and tags are requested only when you open them.
- What GitHub's API cannot give cheaply is absent rather than faked: the last commit per file, ahead / behind, tag
  annotations, release downloads, files over 1 MB (placeholder: "Too large to preview online — Save to shelf") and
  diffs GitHub sends without a patch. Save to shelf (a normal bare clone) has all of it.
- Search runs when you press Enter, never while you type (GitHub allows 10 searches a minute without a token).
- Avatars are never fetched; remote README images follow Settings → Network → Load remote images.

## Known limits

- No real SVG/HTML execution: READMEs are sanitised, remote images are shown only as inert images (SVG too), links to
  other sites ask first.
- The release download picks one file for this PC by its name (OS + architecture; portable > zip > installer); when
  no file matches clearly, nothing is downloaded and the job says why. Any other asset: Download on the Releases page.
- Very long histories: the commits list walks up to 20,000 commits per scan ("Search older commits" continues).
- Explore has only been run against canned GitHub answers (unit tests, the mock UI, the real-app e2e run): real GitHub
  browsing, a real rate-limit answer, saving a token to the real Windows Credential Manager and removing it again, and
  the one `#[ignore]`d Credential Manager test are hand tests (`docs/TESTING.md` §5). Not verified against real GitHub:
  an empty online repository, and a submodule inside a directory listing (it may arrive typed as a file).
- The network-adapter-off run, the `-Network` e2e run, real GitHub requests (info, notes, downloads, README images —
  the agents' runs used canned answers only) and the other hand tests in `docs/TESTING.md` §5 are still to be done by
  a person.

## Docs

`docs/REQUIREMENTS.md` (what and why), `docs/ARCHITECTURE.md` (design), `docs/TASKS.md` (milestones M0–M13, M10a–c and gates),
`docs/DECISIONS.md` (D-1…), `docs/WORKLOG.md` (what was done per milestone), `docs/TESTING.md` (tests, e2e, hand tests).

License: MIT.
