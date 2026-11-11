# Mock screenshots (M5–M7)

Browser preview of the mock backend: `npm run dev` → `http://localhost:1450/`. Switches go in the page query
(before the `#`), routes in the hash: `http://localhost:1450/?slow#/northwind/parsekit`. Theme: header toggle
(System → Light → Dark) or a `prefers-color-scheme` emulation. Every row is taken in **light and dark**; the console
must stay free of errors.

Mock repos: `northwind/parsekit` (imported Rust, rich README, branches `feature/streaming` `fix/utf8`, tags,
`?huge` data), `lumen-ui/lumen` (cloned TS, releases, fast-forward possible), `inkwell/quill` (cloned, codeberg.org,
default branch `trunk`), `local/beacon` (imported Go, detached HEAD, plain-text README), `local/scratchpad` (empty),
`orbit-labs/telemetry` (missing folder), `sandbox/hostile-readme` (security README), `tidewater/harbor-cli`
(fresh clone).

| # | Milestone | Route (hash) | Switch | What to look at |
|---|---|---|---|---|
| 1 | M5 | `#/` | – | header (logo, search, +, jobs, theme, gear); placeholder repo list |
| 2 | M5 | `#/` | `?empty` | empty shelf with Import / Clone |
| 3 | M5 | `#/northwind/parsekit` | – | repo header, source badge, tab counts, README via MarkdownBody (table, highlighted code, logo image, remote badge chip) |
| 4 | M5 | `#/northwind/parsekit` | `?slow` | shimmers in header/tabs while loading |
| 5 | M5 | `#/northwind/parsekit/commits/main`, `/branches`, `/tags`, `/releases`, `/commit/<oid>` | – | active tab underline per tab |
| 6 | M5 | `#/northwind/parsekit/tree/feature%2Fstreaming/src` | – | encoded ref in the route |
| 7 | M5 | `#/orbit-labs/telemetry` | – | missing-folder banner + Locate… |
| 8 | M5 | `#/local/scratchpad` | – | "This repository is empty." |
| 9 | M5 | `#/settings` | – | settings placeholder (folders) |
| 10 | M5 | `#/x/y/z` | – | NotFound |
| 11 | M5 | header "+" → Clone URL… | `?clonefail` | jobs popover: progress, then failure with Retry |
| 12 | M5 | header search, press `/` | – | dropdown: Recent, then matches |
| 13 | M5 | `#/sandbox/hostile-readme` | – | no remote image loaded (chip), no script, dead links inert |
| 14 | M6 | `#/` | – / `?empty` | S1 dashboard: toolbar, rows, missing row, sidebar |
| 15 | M6 | `#/northwind/parsekit`, `/tree/main/src`, `/tree/v0.2.0` | – | S3: file table, latest commit, README card, About, languages |
| 16 | M6 | `#/northwind/parsekit/tree/main/generated` | `?huge` | 1,000-entry dir (virtualised) |
| 17 | M6 | `#/northwind/parsekit/blob/main/src/lexer.rs?L=10-20` | – | S4 code view, selected lines |
| 18 | M6 | blob of `docs/logo.svg`, `docs/screenshot.png`, `assets/sample.bin`, `data/corpus.json`, `data/export.log` (> 5 MB), `vendor/ucd`, `latest` | – | image preview + each placeholder kind |
| 18a | M6 | blob of `data/readings.csv?L=20000` (> 1 MB, 26k lines), `docs/legacy-notes.txt` (lossy), `odd/a:b.txt` (not checked out), `docs/architecture.md` (Preview / Code / Raw) | – | virtualised plain viewer + anchor scroll, notices, markdown tabs |
| 18b | M6 | `#/northwind/parsekit/tree/main/odd`, `#/local/beacon`, `#/northwind/parsekit/tree/nope/` | – | NFR-W3 + non-UTF-8 badges, detached HEAD (short oid), ref not found |
| 19 | M6 | all dialogs (Import: picks cycle new → not a repo → duplicate; Clone; Edit About; Remove; Fetch confirm on `northwind/parsekit`) | – | S10 dialogs |
| 19a | M6 | saved PNGs (light, headless Edge 1280×1000): `docs/screenshots/m6-dashboard.png`, `docs/screenshots/m6-code.png` | – | reference look |
| 20 | M7 | `#/northwind/parsekit/commits/main` (Older → page 2 has the merge `feature/lexer-tables`), `…/commits/main?path=src/lexer.rs` (`?huge` for several pages), `#/lumen-ui/lumen/commits/refs%2Ftags%2Fv2.0.0-beta.1?since=refs%2Ftags%2Fv1.1.0` | – / `?huge` | S5 day groups, expandable description, copy oid, pager with pinned tip, path chip, since banner |
| 21 | M7 | `#/northwind/parsekit/commit/38ce93f337ce92603ace971939ce95863cce9a3f` (rename, deletion, no-newline; `?file=4`), lumen `Merge branch 'next-dialog'` (2 parents), quill root (last commit, `commits/trunk?page=2`), parsekit first commit under `?huge` (400 files, deferred, Load diff) | – / `?huge` | S6 header, notes, file list, cards, collapsed + Load diff |
| 22 | M7 | `#/northwind/parsekit/branches` (`?huge`: `docs/tutorial` capped "1,000+"), `/tags`, `#/lumen-ui/lumen/releases` (Load GitHub release notes → confirm) | – / `?huge` | S7 lazy bars, S8 badges, S9 cards (Latest / Pre-release, notes, compare) |
| 23 | M7 | `#/settings` (theme radio, folders + Open, Clear cache, uncommitted-changes toggle → note on `northwind/parsekit`) | – | S10 settings page |
| 24 | M7 | jobs popover: Fetch on `lumen-ui/lumen` (report with new tag / kept tag / skipped ref), Clone URL… | `?fetchfail` / `?clonefail` | failure text + Retry; clone done → Open repository |
| 25 | M7 | saved PNGs (light, headless Edge 1280×1000): `docs/screenshots/m7-commits.png`, `docs/screenshots/m7-commit.png` | – | reference look |
