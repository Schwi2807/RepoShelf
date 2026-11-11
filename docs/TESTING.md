# RepoShelf — testing

Everything here runs against scratch folders. No command below reads or writes the real shelf
(`%LOCALAPPDATA%\dev.schwi.reposhelf`), the real archive (`Documents\RepoShelf`) or any Project Manager data.

Before any `cargo` / `npm` / `npx` command in a PowerShell window, refresh the process PATH (process only):

```powershell
$env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
```

## 1. Automated tests

| What | Command | Notes |
|---|---|---|
| Rust, everything | `cargo test --workspace --all-features` | rs-core (incl. testkit + ghreleases), src-tauri commands (incl. the e2e seams), rs-cli. No network: clones/fetches use `file://` through the unchecked seam; GitHub notes use canned JSON. One live GitHub smoke is `#[ignore]`. |
| Rust, shipped feature sets | `cargo test -p reposhelf` · `cargo test -p reposhelf --features e2e` · `cargo test -p reposhelf --no-default-features` | default = `ghreleases`; off → `releases_fetch_github` answers `unsupported`. |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings` · `cargo clippy -p reposhelf --all-targets --features e2e -- -D warnings` | |
| TypeScript | `npm run typecheck` · `npm test` | vitest + happy-dom; `types.test.ts` checks the committed IPC golden `src/ipc/__samples__/ipc-samples.json` (`UPDATE_SAMPLES=1 cargo test -p reposhelf ipc_samples` rewrites it). |
| UI build + bundle size | `npm run build`, then the initial-JS check of TASKS Gate M5 (< 1.5 MB, NFR-P8) | |
| Read-model performance | `cargo run -p rs-cli --release -- fixtures target\scratch\bench` then `cargo run -p rs-cli --release -- bench target\scratch\bench` | Exit 1 on a missed target (NFR-P2/P3/P5/P6, 400-file diff). Delete the scratch folder afterwards. |

The full gate per milestone is in `docs/TASKS.md` (Gate M0 … Gate M8); each later gate repeats the earlier commands.

## 2. Mock UI in a browser (no backend)

`npm run dev` → `http://localhost:1450/` (launch config `reposhelf-mock` in `.claude/launch.json`). Without
`__TAURI_INTERNALS__` the app loads the mock backend (`src/ipc/mock/`, invented data). Switches go in the page query
before the `#`: `?empty`, `?slow`, `?huge`, `?clonefail`, `?fetchfail` — e.g. `http://localhost:1450/?huge#/northwind/parsekit`.
The route × theme × switch list to check is `tools/shots/mock-shots.md`. Stop the dev server by process tree
(`taskkill /T /F /PID <pid>`).

## 3. Real app e2e (M8)

### Build (M8.1)

```powershell
npm run build; if ($LASTEXITCODE -ne 0) { throw 'ui build' }
$env:CARGO_TARGET_DIR = (Join-Path (Get-Location) 'target\e2e'); npx tauri build --no-bundle --features e2e; $rc = $LASTEXITCODE; Remove-Item Env:CARGO_TARGET_DIR; if ($rc -ne 0) { throw 'e2e build' }
```

→ `target\e2e\release\reposhelf.exe`. `CARGO_TARGET_DIR` must be absolute: the tauri CLI runs cargo inside
`src-tauri\`, so a relative `target\e2e` lands in `src-tauri\target\e2e` (D-85). `tauri build` embeds `dist\`, so the
exe renders with nothing listening on port 1450. The e2e feature adds test seams that a shipped build never contains
(D-14, D-20, D-84): `RS_TEST_FILE_CLONE=1` (clone/fetch `file://` sources), `RS_E2E_PICK` (folder dialogs take the
next path of a `;` list), `RS_E2E_OPEN_LOG` (browser/Explorer opens are written to a file instead), `RS_E2E_OFFSCREEN=1`
(window created off-screen).

### Run (M8.2–M8.5)

```powershell
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1            # add -Keep to keep the scratch folder
powershell -ExecutionPolicy Bypass -File tools\e2e\check-e2e.ps1
```

`run-e2e.ps1`:
1. makes `target\scratch\e2e-<time>\` with `data`, `archive`, `fx` (all fixtures via `rs fixtures`, plus 50 copies of
   the basic fixture registered with `rs fixtures --register 50`), and `up\demo-upstream` (a small git repo, isolated
   git config) as the `file://` clone source;
2. hashes the `.git` + work tree of the 8 fixtures the UI will import;
3. starts tauri-driver on a free port (msedgedriver from user env `MSEDGEDRIVER`, tauri-driver from
   `C:\Dev\tools\cargo\bin`) and runs `tools\e2e\wd-e2e.mjs`;
4. kills the whole driver tree (`taskkill /T /F`), then anything else it saw start, and counts leftovers (must be 0);
5. re-hashes the imported fixtures (must be equal: the run never writes into an imported repo, NFR-O2) and writes
   `target\e2e\run.json`; removes the scratch folder unless `-Keep` or something was left running.

`wd-e2e.mjs` (results in `target\e2e\result.json`, screenshots in `target\e2e\shots\`):
- **Session A / B start**: process start → first dashboard rows (`performance.mark('rs:dashboard-rows')`) with 50
  repos; B (warm) must be < 1.5 s (NFR-P1).
- **Collector**: console errors/warnings, `securitypolicyviolation`, uncaught errors and `alert()` calls, installed
  before the app's scripts via CDP where the driver allows it; a positive control (an injected inline script must be
  blocked and seen) proves the collector works before "0 violations" counts.
- **Import** ×8 through the real Import dialog (picker queue), **every screen S0–S10 in light and dark** with a
  screenshot (dashboard, code tab, file view with worker highlighting, SVG image, commits, commit page, merge commit,
  branches, tags, releases, security README, settings, Import and Clone dialogs).
- **Backend checks**: README table/highlighted code/relative image, path-filtered commits page 2 (chained skip, no
  overlap), `?file=250` on the 400-file commit, ahead/behind against HEAD's branch, GitHub-notes button only for a
  github.com origin (not clicked), `app_open_dir` and `repo_open_folder` reach the (recording) opener.
- **Security**: hostile README (no script/iframe/form/on*/style, no `javascript:`/`data:` links, remote image as a
  chip), external link → confirm dialog (cancelled; no URL opened), SVG with a script shown only as `<img>`, a table of
  hostile IPC calls from the webview (paths, revs, oids, URLs, unknown repo, plugin commands) — all refused.
- **NFR-P2**: after a reload, dashboard → click the 1,000-entry repo → file table rows rendered (< 300 ms).
- **NFR-O1 proxy**: no resource request other than the app origin / IPC during the run.
- **Clone → fetch → fast-forward → remove** through the `file://` seam: clone the upstream, add a commit + tag to the
  upstream, Fetch ("1 new commit"), Fast-forward (to the new upstream tip), Remove (the scratch clone goes to the
  **real Recycle Bin**, D-30; empty it at will).

### Network run (M8.8, opt-in)

```powershell
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1 -Network [-NetworkRepos owner/a,owner/b]
```

Adds session C: clones the given public GitHub repos (default `octocat/Hello-World`, `octocat/Spoon-Knife`) through
the https URL rules (only `file:` inputs use the seam), then fetches each. Use a repo with Windows-invalid ref names
to see the skipped-refs report. Implemented in M8 but **never executed yet** — the first run is a hand step.

## 4. Hand-test checklist (needs a person)

Run these on the e2e exe with scratch folders, or on the portable build of M9 with `RS_DATA_DIR` / `RS_ARCHIVE_DIR`
set to empty scratch folders:

```powershell
$env:RS_DATA_DIR = 'C:\Dev\projects\repo-shelf\target\scratch\hand\data'; $env:RS_ARCHIVE_DIR = 'C:\Dev\projects\repo-shelf\target\scratch\hand\archive'
& .\target\e2e\release\reposhelf.exe
```

- [ ] **Network adapter off (NFR-O1, needs admin)**: import two local repos, then turn the network adapter off
      (Settings → Network & internet → Wi-Fi/Ethernet off, or airplane mode). Restart the app and browse every tab of
      both repos, open files, commits and a commit page. Everything works; no error mentions the network. Only
      Fetch / Clone / GitHub notes may fail, with a readable "Could not reach …" message. Turn the adapter back on.
- [ ] **Theme + restart**: switch System → Light → Dark in the header, close the app, start it again: the theme is kept;
      with System, changing the Windows colour mode switches the app live.
- [ ] **Mouse back/forward**: browse dashboard → repo → file → commits; mouse buttons 4/5 and Alt+Left/Right go back
      and forward and restore the scroll position.
- [ ] **Real folder dialogs**: "+" → Import folder… opens the Windows folder picker; Cancel leaves the dialog as it was.
- [ ] **Open folder / Settings → Open**: Explorer opens the work tree (imported) or selects `<name>.git` (cloned).
- [ ] **External link**: click the origin link in a repo header → confirm → the default browser opens that URL.
- [ ] **Clone of a big repository** (e.g. a 200 MB+ public repo): progress moves in the jobs popover, Cancel removes
      the partial folder, a second clone finishes and browses normally.
- [ ] **Recycle Bin**: Remove a cloned repo → it is in the Recycle Bin under its `<name>.git` folder and can be
      restored; an imported repo is only taken off the shelf (folder untouched).
- [ ] **Network e2e** (if not run by the agent): `run-e2e.ps1 -Network`.
