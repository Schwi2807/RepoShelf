# RepoShelf — requirements

## User's request (2026-11-11)

> Based on Project Manager project, create another app where it resembles GitHub. I decided to make it since they
> (GitHub) didn't bother to create a desktop version AND I want to have an archive of my own. Project Manager somewhat
> achieved that but now I want it to look almost like GitHub UI arrangements, but purely from scratch.

## Fixed assumptions (decided by the lead, not open for re-analysis)

1. **"Purely from scratch"** = a brand-new codebase in `C:\Dev\projects\repo-shelf`. Not a fork or branch of Project
   Manager, no path dependency on `pm-core`. Reading Project Manager (`C:\Dev\projects\project-manager`, read-only) for
   lessons and gotchas is allowed: Windows-safe clone handling (`crates/pm-core/src/winsafe.rs`, `clone.rs`), git2 0.21
   accessors return `Result`, the mock-IPC browser preview, the tauri-driver e2e harness (`tools/e2e`).
2. **"Resembles GitHub"** = the *layout and arrangement* of github.com (header bar, repo header with tabs, file tree
   table, README card, About sidebar, language bar, commit list, diff view, branches/tags/releases pages). Own name
   (**RepoShelf**), own icon, own app identifier. No GitHub name, Octocat/logo, Primer CSS files or copied assets; all
   CSS and icons written from scratch (lucide-react icons are fine).
3. **Stack** = the one already proven on this laptop: Tauri 2 + Rust (git2 0.21 vendored libgit2) + React 19 +
   TypeScript + Vite, npm. Windows first; code stays cross-platform.
4. **PoC scope** — a github.com-style browser over a *local archive* of git repositories:
   - Dashboard / repository list (search, sort, language dot, last updated, description).
   - Repo page **Code** tab: branch/tag switcher, file tree with last-commit message + date per entry (loaded lazily),
     rendered README, About sidebar (description, topics, homepage, origin), language bar, latest commit strip,
     commit count, branch/tag counts.
   - File view: syntax-highlighted source with line numbers, raw, image preview, binary/large placeholders, breadcrumbs.
   - Commits list (per branch, paged) and a single-commit page with a unified diff.
   - Branches page, Tags page, Releases page (tags + annotated messages; GitHub release notes when available is optional).
   - Add repositories: import an existing local folder (registered in place) or clone a public HTTPS URL into the
     managed archive dir. Fetch/update an archived clone.
   - Local metadata the user can edit: description, topics, homepage (stored in the app DB, not in the repo).
   - Light + dark theme.
5. **Deferred (not in the PoC):** issues, pull requests, Actions, wikis, projects, stars/social, sign-in/auth,
   pushing, editing files, private-repo cloning.

## Hard constraints for every subagent

- Work only inside `C:\Dev\projects\repo-shelf`. Project Manager is read-only reference: no edits, no builds in its
  `target\`, no branch switches.
- Own app id `dev.schwi.reposhelf`, own data dir and managed dir. Never read or write
  `%LOCALAPPDATA%\dev.schwi.projectmanager*` or `Documents\ProjectManager*`.
- Tests use scratch dirs only (env overrides `RS_DATA_DIR`, `RS_ARCHIVE_DIR`). Never install/uninstall anything,
  never touch PATH, never change AV/system settings.
- Kill whole process trees after tests (`taskkill /T /F`). Test windows offscreen or on the main display.
- Commit locally per milestone with the repo-local identity (already configured). Never push.
- Untrusted content: READMEs/files of cloned repos are untrusted. Sanitise rendered HTML, strict CSP, no scripts.

---

## Detailed analysis

### 1. Core problem and user needs

GitHub has no desktop client for *reading* repositories, and the user wants a personal, offline archive of the repos
they care about (their own and public ones) that stays browsable when GitHub is down, a repo is deleted or
force-pushed, or there is no network. Project Manager indexes projects but presents them as a project list with
panels; RepoShelf instead presents each archived repo the way github.com does, so browsing feels familiar.

User needs:
- N1 One shelf of repos: some already on disk (registered in place, never moved or modified), some cloned by the app
  into a managed archive folder.
- N2 GitHub-like browsing: the same arrangement of header, tabs, file table, README, About, commits and diffs.
- N3 Read-only by default: browsing any branch/tag/commit never changes the repository or its working tree.
- N4 Works offline; network only when the user clicks Clone or Fetch (or the optional release-notes fetch).
- N5 Survives real-world repos: Linux-made names Windows cannot store, deep paths, huge histories, big files.
- N6 Personal metadata (description, topics, homepage) kept in the app, not written into repos.

**Key design rule (drives FRs, NFRs and risks):** every view is served from the git object database, addressed as
`(repo id, ref or commit oid, git path)`. The UI never sends filesystem paths. Switching branch/tag never checks
anything out; working trees are ignored for browsing (imported repos may show a "has uncommitted changes" note only).

### 2. Functional requirements

Each FR ends with its acceptance check (→).

**Library / dashboard**
- FR-1 Repository list shows name, description, primary-language dot + name, last-updated (newest commit date on the
  default branch), origin host/owner, and source badge (Imported / Cloned). → 3 fixture repos show all fields correctly.
- FR-2 Search filters by name, description and topics (case-insensitive, as-you-type). → typing "pars" leaves only
  repos matching in any of those fields.
- FR-3 Sort by last updated (default), name, date added; direction reversible; choice persisted. → order changes and
  survives restart.
- FR-4 Missing repo (folder deleted/moved/drive absent) shows a "Not found" state with Locate… and Remove from shelf.
  → renaming a fixture folder marks it missing without crashing the list.

**Adding repositories**
- FR-5 Import local folder: pick a folder; a non-bare or bare git repo is registered in place (nothing written to it).
  Non-repo folders are rejected with a clear message; duplicates are detected by canonical path. → import, re-import
  (refused as duplicate), non-repo (refused); repo's files and `.git` mtimes unchanged.
- FR-6 Clone public HTTPS URL into the managed archive dir (`RS_ARCHIVE_DIR` override). Accepts `https://host/…`
  (and `host/owner/repo` shorthand for github.com). Rejects ssh, `git@`, `file://`, local paths, URLs with userinfo or
  credentials; no credential callback, no submodules, no LFS. → valid URL clones; each rejected form gives an error and
  libgit2 is never called.
- FR-7 Clone runs in the background with progress (objects, bytes, phase) and Cancel; several clones at once are
  capped (≤3 concurrent, others "Waiting"); failures stay listed with Retry. → two clones run while browsing; cancel
  leaves no folder behind.
- FR-8 Clone is atomic: hidden `.<name>.partial-<ts>` sibling, renamed on success, deleted on failure/cancel; folder
  name made unique. → kill mid-clone, restart: no half folder under a real name.
- FR-9 Windows-unsafe content still clones: refs whose names clash case-insensitively or are invalid are skipped;
  files with invalid names/over-long paths are left out of the working tree (skip-worktree) or the clone has no
  working tree at all (design decision, see R-4); skipped items are reported in the job result. → fixtures with
  `a:b.txt`, `CON.txt`, `llm` + `LLM/x` branches clone and the report lists them.
- FR-10 Fetch/update an archived (managed) clone: fetches `refs/remotes/origin/*` and new tags only; never overwrites
  an existing local tag, never moves local branches automatically; shows "N new commits" afterwards. Imported repos:
  Fetch is offered only if they have an https origin, with the same "remote-tracking refs only" rule (see concerns).
  → fetch after upstream adds 2 commits + 1 tag shows them; a local tag with the same name is untouched.
- FR-11 Fast-forward the managed clone's default branch to origin after fetch (button, only if fast-forward possible).
  → branch moves forward; diverged branch refuses with a message.

**Repo page: header + Code tab**
- FR-12 Repo header: owner/name, source badge, origin link (opens system browser only after confirm), tabs Code /
  Commits / Branches / Tags / Releases with counts where cheap. → tabs switch without reload; counts match `git`.
- FR-13 Ref switcher (branches + tags tabs, filter box, default branch marked); selection is in the route; no
  checkout. → switching to a tag changes the tree; working tree and HEAD file unchanged.
- FR-14 Latest-commit strip above the file table: author, message summary, short oid (link to commit), relative date,
  total commit count on the ref (cached per tip oid). → values match `git log -1` / `git rev-list --count`.
- FR-15 File table for any directory: folders first, then files, case-insensitive name order; icon per kind (dir,
  file, symlink, submodule); `..` row in subfolders. → 1000-entry fixture renders in order.
- FR-16 Per-entry last commit (message + relative date) loaded lazily after the table renders, cached by
  `(tip oid, dir path)`; a placeholder shimmer until known; cancellable when the user navigates away. → table usable
  before messages arrive; second visit is instant.
- FR-17 README card under the table for the current directory (README.md/.markdown/.txt/.rst/plain, case-insensitive,
  .md preferred): GitHub-flavoured markdown (tables, task lists, fenced code highlighted, autolinks, heading anchors),
  sanitised; relative links go to in-app routes; relative images load as blobs from the same ref. → fixture README
  with table, code, relative image and link renders; `<script>`/`onerror` stripped.
- FR-18 About sidebar: description, homepage link, topics as chips, origin URL, "Imported from <path>" or "Cloned
  <date>", license name if a LICENSE file is recognised, counts (branches, tags), last fetched. → shows edited values.
- FR-19 Language bar + list: bytes per language from blob sizes at the viewed commit (extension map, vendored/
  generated/docs/binary excluded), cached by tree oid; top 6 + "Other". → fixture with .rs/.ts gives the right %.
- FR-20 Edit metadata (description, topics, homepage) in a dialog from the About gear; stored in the app DB only;
  validation (topics lowercase, `a-z0-9-`, ≤ 20; homepage http/https only). → edit, restart, values persist; repo
  files unchanged.

**File view**
- FR-21 Breadcrumbs (repo / dir / … / file), each segment a link; "Copy path". → clicking a segment opens that dir.
- FR-22 Text file: syntax highlighting by extension/shebang, line numbers, line anchor (`#L10`, click to select range
  `#L10-L20`), file header with size, line count, last commit for the file. → anchor scrolls and highlights.
- FR-23 Raw view (plain text, no highlight) and "Save a copy…" (writes the blob to a user-chosen path via dialog).
  → raw content equals `git show ref:path` byte-for-byte.
- FR-24 Images (png, jpg, gif, webp, bmp, ico; svg only via `<img>` from a blob URL) previewed. → svg with script
  shows the picture, script does not run.
- FR-25 Placeholders: binary ("Binary file, N KB, not shown"), large (> 1 MB: no highlighting; > 5 MB: not displayed,
  Raw/Save only), LFS pointer ("Stored with Git LFS, not in this archive"), submodule (target oid + URL as text),
  symlink (target path as text), non-UTF-8 text decoded lossily with a notice. → each fixture shows its placeholder.

**Commits**
- FR-26 Commits list for the selected ref (and optionally a path filter from the file view's "History"), grouped by
  day ("Commits on Nov 11, 2026"), each row: summary, author name, relative date, short oid, copy oid, browse-tree
  link. Paged 35 per page with Older/Newer, using a cursor oid (no full walk). → 10k-commit fixture pages instantly.
- FR-27 Commit page: full message, author/committer, date, parent link(s), stats (files changed, +/−), file list, and
  unified diff per file with line numbers and hunk headers; root commit diffs against the empty tree; merges against
  the first parent (note shown). → numbers match `git show --stat`.
- FR-28 Diff limits: per-file > 2000 changed lines or binary → collapsed with "Load diff"/"Binary file"; whole commit
  > 300 files or > 20k lines → file list first, diffs on demand; renames detected and shown as `old → new`.
  → large fixture commit opens in < 1 s with collapsed files.

**Branches / Tags / Releases**
- FR-29 Branches page: default branch card, then all local + remote-tracking branches, each with last-commit date,
  author, ahead/behind vs default (computed lazily, capped), filter box. → counts match `git rev-list --left-right`.
- FR-30 Tags page: tags newest first (by target commit date), each with commit oid, date, annotated/lightweight
  badge, links to tree and commit. → order and badges match fixture.
- FR-31 Releases page: one card per version-like tag (semver-ish), newest first, "Latest" badge, annotated message
  rendered as sanitised markdown, link to the tag tree and to the compare view's commit list since previous tag
  (list only). Optional: "Load GitHub release notes" button for github.com origins (one request, cached). → fixture
  with 3 annotated tags shows 3 cards with messages.

**App-wide**
- FR-32 Global header: app logo/name (home), repo search box (jumps to a repo), Add (Import / Clone), jobs indicator,
  theme toggle, settings. → each control works from any screen.
- FR-33 Light, dark and "follow system" themes; persisted; code highlighting and diff colours follow. → toggle,
  restart, theme kept; contrast ≥ 4.5:1 for text in both.
- FR-34 Routing with back/forward (mouse buttons + Alt+←/→) and a copyable in-app location
  (`repo/<name>/tree/<ref>/<path>`). → back returns to the previous dir with scroll restored.
- FR-35 Data locations: data dir and archive dir default under the app's own id `dev.schwi.reposhelf`; overridable
  by `RS_DATA_DIR` / `RS_ARCHIVE_DIR`; Settings shows both. → tests run fully in scratch dirs.
- FR-36 Remove from shelf: imported → unregister only; cloned → unregister, folder moved to Recycle Bin after
  confirm. → imported folder untouched; cloned folder in Recycle Bin.
- FR-37 Browser mock backend: UI runs in a plain browser with fixture data and query switches (`?empty`, `?slow`,
  `?clonefail`, `?fetchfail`, `?huge`). → every screen screenshot-able without Tauri.

**Traceability (PoC scope → FRs):** dashboard 1–4 · Code tab 12–19 · file view 21–25 · commits 26–28 · branches/tags/
releases 29–31 · import/clone/fetch 5–11 · metadata 20 · light/dark 33 · plus app shell 32, 34–37.

### 3. Non-functional requirements

**Performance** (measured on the dev laptop, release build, warm cache unless stated)
- NFR-P1 App cold start to dashboard with 50 repos < 1.5 s; list data comes from the app DB cache, refreshed in the
  background.
- NFR-P2 Directory of 1000 entries: table rendered < 300 ms after click (names, icons, sizes); virtualised above 500.
- NFR-P3 Last-commit-per-entry: computed off the UI thread in one history walk per directory (stop when every entry is
  resolved or after a cap of 10k commits → "older"); cached by `(tip oid, dir path)` in the DB; second visit < 50 ms.
- NFR-P4 Files > 1 MB not highlighted; > 5 MB not sent to the webview; highlighting runs in a worker / chunked so the
  UI never blocks > 100 ms.
- NFR-P5 Commits page of 35 for a 10k-commit repo < 200 ms; commit count computed once per tip and cached.
- NFR-P6 Language stats cached by tree oid; first compute on 50k-file tree < 3 s in background, never blocks Code tab.
- NFR-P7 Clone/fetch progress throttled (≥ 80 ms between events) over a Tauri Channel.
- NFR-P8 Frontend bundle: highlighter loads grammars on demand; initial JS < 1.5 MB.

**Security** (cloned content is untrusted)
- NFR-S1 Markdown rendered to HTML then sanitised with an allow-list (no script, iframe, object, embed, form, style
  attributes beyond a safe set, no `on*`, no `javascript:`/`data:` links except `data:image/*` in img).
- NFR-S2 Strict CSP: `default-src 'self'; script-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none';
  img-src 'self' data: blob: asset:`; no remote `img-src` (remote README images show as a link placeholder; keeps
  the app offline and untracked). Aim for no `style-src 'unsafe-inline'`; if the highlighter needs inline styles,
  use class-based themes instead.
- NFR-S3 Links in rendered content never navigate the webview: in-repo relative links → in-app routes; external links
  → confirm, then system browser (http/https only).
- NFR-S4 IPC takes `repo id` (looked up in the DB) + ref + git path. Paths are validated: no absolute paths, no `..`,
  no `\`, no NUL, no drive letters; resolved only through tree lookup, never joined onto a filesystem path. Files are
  never read from the working tree for browsing. Any filesystem path from the UI (import, save-as) comes from a native
  dialog result in the backend.
- NFR-S5 Clone/fetch URLs parsed and rebuilt (scheme https, host, path); userinfo/query/fragment dropped or refused;
  no credentials ever stored; no ssh; no submodule recursion; no LFS smudge; redirects only to https.
- NFR-S6 Tauri capabilities minimal (no shell plugin, no fs plugin scope for the webview, dialog only).
- NFR-S7 SVG only via `<img>`; no HTML/PDF preview of repo files.

**Windows specifics**
- NFR-W1 Managed clones set `core.longpaths=true`; app paths use `\\?\` where needed; archive dir path may contain
  spaces.
- NFR-W2 Windows path rules mirror libgit2 (NT-invalid chars, trailing dot/space, DOS device names, `.git`/`git~1`)
  plus retry on libgit2's own invalid-path error; case-folding ref clashes detected before fetch.
- NFR-W3 Git paths with names Windows cannot store are still fully browsable (object DB), with a small badge
  "not checked out on this PC".

**Offline / reliability / portability**
- NFR-O1 Zero network unless the user starts Clone, Fetch or the optional release-notes fetch. Verified by a test run
  with the network adapter blocked (all screens work).
- NFR-O2 Never writes into imported repos (no index refresh, no lock files, no config changes); verified by hashing
  `.git` before/after the e2e run.
- NFR-O3 Corrupt or partially missing repo → per-repo error card, rest of the app unaffected.
- NFR-O4 Code stays cross-platform (no Windows-only APIs outside `cfg(windows)`).
- NFR-A1 Keyboard: `t` file finder optional; `/` focus search; tab order sane; focus rings visible.

### 4. Screen inventory (GitHub-style regions)

**S0 Global header (all screens)** — left: RepoShelf logo + name (home). Centre: search box ("Search or jump to a
repo…", `/` shortcut) with dropdown of matches. Right: "+" menu (Import folder…, Clone URL…), jobs icon with active
count badge + popover (progress rows, Cancel, Retry), theme toggle, settings gear.

**S1 Dashboard (repo list)** — two columns. Main: title "Repositories" + count; toolbar with filter input, Sort
dropdown, Source filter (All / Imported / Cloned), "New" (Add) button. Rows: name (link, bold), source badge,
description (2 lines max), topics chips, footer line: language dot + name · origin host/owner · "Updated 3 days ago".
Missing repos show a warning row. Sidebar (right, narrow): "Recent" (last 5 opened), archive dir size and repo count,
"Fetch all clones" (sequential, cancellable — optional). Empty state: illustration-free card with Import / Clone.

**S2 Repo header (all repo screens)** — row 1: book icon, `owner / name` (owner = origin owner or "local"), source
badge (Imported/Cloned), right side buttons: Fetch (clones), Open folder, "…" (Remove). Row 2: tab bar with icons —
Code, Commits (count), Branches (count), Tags (count), Releases (count); active tab underlined. Missing/corrupt repo
banner below when applicable.

**S3 Code tab** — two columns (≈ 3:1); stacks under 1000 px width.
- Main column top bar: ref switcher button (branch icon + name, dropdown with Branches/Tags tabs + filter), "N
  branches", "N tags" links, spacer, "Go to file" (optional), green-ish "Code" dropdown replaced by "Copy clone URL /
  Open folder".
- Latest-commit box (bordered card header): avatar-less initials circle, author, summary (link), short oid, relative
  date, history icon + "N commits".
- File table (same card body): rows icon | name | last-commit message (muted, link to commit) | relative date (right).
  `..` row in subdirs. Shimmer in message/date columns while loading.
- In subdirectories: breadcrumbs replace the top-left title.
- README card: header with book icon + "README" (file name) + outline (heading list) button; body = rendered markdown.
- Sidebar: "About" heading + gear (edit), description, homepage link, topics chips, origin URL, license, "Imported
  from <path>" / "Cloned <date> · last fetched <date>"; "Releases" box (latest tag + count, link); "Languages" box
  with segmented bar (colours from own palette) and list "Rust 61.2%".
- Edge states: empty repo / unborn HEAD ("This repository is empty"), detached HEAD (switcher shows short oid),
  no README (card hidden), ref not found (message + link to default branch).

**S4 File view** — breadcrumbs + "Copy path" above a card. Card header: last commit for the file (author, summary,
date, History link). Second header bar: "N lines (M sloc) · 12.3 KB", buttons Raw, Copy, Save a copy, (History).
Body: line-number gutter + code (highlighted), selected-line highlight, or image preview centred on a checkerboard,
or placeholder message (binary/large/LFS/submodule/symlink/not checked out). Markdown files: tabs Preview / Code.

**S5 Commits tab** — top bar: ref switcher, optional path filter chip ("History for src/main.rs ×"). Body: per-day
groups, each a heading "Commits on Nov 11, 2026" and a card of rows: summary (link) + expand "…" for body, author +
"committed 2 hours ago", right: short oid button (copy), browse-tree icon. Footer: Newer / Older buttons.

**S6 Commit page** — header card: summary (large), body (pre-wrapped), author/committer line, "N parents" with oid
links, full oid copy. Below: "Showing N changed files with A additions and D deletions" + Unified/Split toggle
(split optional). Left optional file tree; right list of file diff cards: header (expand chevron, +/− mini bar, path or
`old → new`, View file link), body = diff table (old no | new no | marker | code), hunk header rows blue-tinted,
added green, removed red (theme tokens). Collapsed cards for large/binary/generated files.

**S7 Branches tab** — filter input; sections "Default", "Your branches" (local), "Remote branches" (origin/*). Row:
name (link to tree), "Updated 2 days ago by X", ahead/behind bar vs default, commit link.

**S8 Tags tab** — Releases | Tags sub-toggle (like GitHub). Tag rows: name, date, short oid, annotated badge, zip
icon replaced by "Browse files" link.

**S9 Releases tab** — cards newest first: left column tag + short oid + date; right: title (tag name or first line of
annotation), "Latest" badge on newest stable, rendered notes (sanitised), "Compare"/"Commits since previous" link.
Optional "Load GitHub release notes" button per repo with github.com origin (cached, shows fetched date).

**S10 Dialogs** — Import (native folder picker → summary + confirm), Clone (URL input with live validation, target
folder preview, Clone/Cancel), Edit About (description, homepage, topics chip input), Remove (confirm with what
happens on disk), Settings (theme, data dir, archive dir, cache size + Clear cache).

### 5. PoC success criteria and out of scope

Success criteria (all must hold):
1. Import 3 local repos and clone 2 public repos (one with Windows-invalid names) into scratch dirs; all browsable.
2. Every screen S0–S10 renders in the real app and in the mock browser preview, in light and dark.
3. NFR-P2/P3/P5 targets met on fixture repos (1000-entry dir, 10k commits), measured by an automated script.
4. Security fixtures pass: malicious README (script, onerror, javascript: link, remote image, iframe), path traversal
   attempts over IPC (`..`, absolute, `C:\`, NUL) refused, imported repo `.git` hash unchanged after browsing.
5. Rust unit tests (git reading, path validation, clone/fetch rules, caches) and TS unit tests pass; tauri-driver e2e
   smoke covers dashboard → repo → file → commits → commit → branches/tags/releases → clone → fetch.
6. No network traffic during a browse-only e2e run.

Out of scope: issues, pull requests, Actions, wikis, projects, discussions, stars/watch/fork, sign-in/auth/tokens,
private repo cloning, ssh, pushing, committing, editing files, checkout/switching working trees of imported repos,
blame view, code search across files, split diff (optional only), compare between arbitrary refs, zip/tarball
download, LFS fetch, submodule recursion, GitHub API beyond the optional release-notes fetch, auto-update of the app,
installer polish (a portable build is enough for the PoC), multi-user, sync between machines.

### 6. Risks and mitigations

- R-1 Last-commit-per-entry cost (libgit2 has no fast `log -1 -- path` for many paths). → one revwalk per directory
  diffing parent→child trees only for that directory's subtree oid (cheap oid compares, skip unchanged subtrees);
  stop when all entries are resolved; cap + "older than N commits"; cache by `(tip oid, dir)`; run on a worker pool
  with cancellation. Optional later: commit-graph file if present.
- R-2 Markdown rendering fidelity vs safety. → render in Rust (comrak, GFM extensions) or in TS (markdown-it/
  micromark + GFM) and sanitise with an allow-list (ammonia in Rust / DOMPurify in TS); one pipeline only; security
  fixture tests; images rewritten to blob URLs; no raw HTML passthrough beyond the allow-list.
- R-3 Syntax highlighting bundle size / speed. → choose one highlighter with lazy grammars (e.g. Shiki fine-grained
  or highlight.js core + per-language imports, or syntect in Rust returning class spans); size cap per file; class-
  based themes to keep CSP free of inline styles; decision recorded in DECISIONS.md.
- R-4 libgit2 clone on Windows: invalid file names, case-clashing refs, long paths. → reuse PM's *approach* (own
  init → remote → ls → filtered fetch → checkout; skip-worktree for unstorable files; skip + report clashing refs;
  `core.longpaths`). Simpler alternative to evaluate in design: managed clones without a working tree (bare or
  no-checkout), since browsing uses the object DB anyway — removes the file-name/long-path checkout problem, leaves
  only ref clashes.
- R-5 Tag auto-follow overwriting local tags (seen in PM). → fetch tags into a staging namespace, copy only free
  names, delete staging refs; test for it.
- R-6 Huge repos (Linux-size, many refs, big packs): clone time/disk. → show size estimate when known, progress +
  cancel, cap concurrent clones at 3, partial folder cleanup.
- R-7 Non-UTF-8 paths/content, CRLF, BOM. → lossless oids in IPC, lossy display strings with a marker; text detection
  by NUL bytes in first 8 KB.
- R-8 Diff blow-up (generated files, minified JS). → per-file and per-commit caps (FR-28), diff computed on demand
  per file.
- R-9 Imported repos changing on disk while open (user commits elsewhere). → caches keyed by oids, so stale entries
  are harmless; refresh refs on window focus.
- R-10 Wider clone surface than PM (any https host instead of github.com only). → strict URL rebuild, https only, no
  credentials, redirects https-only, user confirms host in the dialog.
- R-11 GitHub-likeness drifting into copying assets/trademarks. → own icons (lucide), own palette and CSS, own name;
  layout only (fixed assumption 2).
- R-12 E2E windows popping up on the user's screen. → tauri-driver with an in-process offscreen window mover (PM's
  approach); kill process trees after runs.

### 7. Lessons from Project Manager (ideas only, no code dependency)

What the user valued: public clones from a pasted URL; background clones with a jobs tray; repos with Windows-invalid
names cloning anyway (with a report); one-click Fetch that is harmless; releases timeline from tags; everything
local-first with network only on click.

Reuse as ideas:
- L-1 Split crates: `rs-core` (pure logic, testable without Tauri) + `src-tauri` (thin commands). Errors as typed enums
  serialised to the UI with user-readable text.
- L-2 git2 0.21 vendored, `https` feature (WinHTTP on Windows), no ssh, no vendored OpenSSL. Remember 0.21 accessors
  return `Result` (e.g. names/messages) — handle, don't unwrap.
- L-3 URL parse → rebuild before libgit2 sees it; clone into `.partial-<ts>` sibling and rename; unique folder names
  picked under a lock at completion; ≤ 3 concurrent clones via a gate; progress throttled to 80 ms over a Channel.
- L-4 `winsafe` rules: mirror libgit2's NT checks + retry on its invalid-path error; case-fold ref clash detection;
  skip-worktree for unstorable files; `git status`-style dirty checks must ignore those entries.
- L-5 Fetch writes only remote-tracking refs, new tags (staging namespace) and FETCH_HEAD; no unlock gate needed;
  never fast-forward or move anything without an explicit click.
- L-6 Jobs live in app state (not inside a dialog); finished rows auto-hide after a few seconds; failures stay with
  Retry.
- L-7 Browser mock backend behind one `bridge.ts` module (the only file importing Tauri), with query switches for
  empty/slow/failure states; enables screenshot verification without the app.
- L-8 E2E via tauri-driver + msedgedriver against scratch data dirs; window moved offscreen in-process (WebDriver
  minimise doesn't reach WebView2; Win32 minimise detaches it); kill process trees; port 4444 may be busy from other
  sessions.
- L-9 Hand-built components on CSS tokens + lucide-react (no UI library) kept the bundle small and the look controlled.
- L-10 Tooling gotchas: Vite's watcher on Windows can miss quick successive edits (touch + reload); PowerShell
  double-quoted here-strings corrupt TS template literals (edit TS with file tools); `[IO.File]` resolves relative
  paths against the process dir; serde `rename_all` on enums doesn't rename struct-variant fields
  (`rename_all_fields`); `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` debugging port doesn't work with Tauri.
- L-11 Keep DECISIONS.md (one line: decision — why — rejected alternative) and a WORKLOG with handoffs from day one.
- L-12 Store summaries (language, last updated, counts) in the index so the list never rescans repos on start.
