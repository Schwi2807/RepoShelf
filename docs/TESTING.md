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
| Rust, everything | `cargo test --workspace --all-features` | rs-core (incl. testkit + ghreleases), src-tauri commands (incl. the e2e seams), rs-cli. No network: clones/fetches use `file://` through the unchecked seam; every HTTP request (GitHub info / notes / assets, README images) goes to `Net::offline()` or canned answers (`net::Canned`). One live GitHub smoke is `#[ignore]` (opt-in, never run by the agents). Explore (v0.3.0) tests use canned GitHub JSON and the in-memory credential store (`explore::cred::MemoryStore`); the second `#[ignore]` test, `real_credential_manager_round_trip`, writes one throw-away entry to the REAL Windows Credential Manager (hand test, §5). |
| Rust, shipped feature sets | `cargo test -p reposhelf` · `cargo test -p reposhelf --features e2e` · `cargo test -p reposhelf --no-default-features` | default = `ghreleases`; off → `releases_fetch_github` answers `unsupported`. |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings` · `cargo clippy -p reposhelf --all-targets --features e2e -- -D warnings` | |
| TypeScript | `npm run typecheck` · `npm test` | vitest + happy-dom; `types.test.ts` checks the committed IPC golden `src/ipc/__samples__/ipc-samples.json` (`UPDATE_SAMPLES=1 cargo test -p reposhelf ipc_samples` rewrites it). |
| UI build + bundle size | `npm run build`, then the initial-JS check of TASKS Gate M5 (< 1.5 MB, NFR-P8) | |
| Read-model performance | `cargo run -p rs-cli --release -- fixtures target\scratch\bench` then `cargo run -p rs-cli --release -- bench target\scratch\bench` | Exit 1 on a missed target (NFR-P2/P3/P5/P6, 400-file diff). Delete the scratch folder afterwards. |

The full gate per milestone is in `docs/TASKS.md` (Gate M0 … Gate M9, Gate M13 for v0.2.0, Gate M10c for v0.3.0); each later gate repeats the earlier commands.

## 2. Mock UI in a browser (no backend)

`npm run dev` → `http://localhost:1450/` (launch config `reposhelf-mock` in `.claude/launch.json`). Without
`__TAURI_INTERNALS__` the app loads the mock backend (`src/ipc/mock/`, invented data). Switches go in the page query
before the `#`: `?empty`, `?slow`, `?huge`, `?clonefail`, `?fetchfail`, `?gh` (GitHub release lists already cached,
so the Releases page shows assets at once), `?ratelimit` (Explore starts with 2 requests left, then answers as rate-limited) — e.g. `http://localhost:1450/?huge#/northwind/parsekit`.
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
(window created off-screen), `RS_E2E_TRASH_LOG` (Remove deletes the clone folder inside the scratch archive and logs
it instead of using the real Recycle Bin, D-91), and since M13 (D-113) `RS_E2E_NET` (a routes file: the app's HTTP
client answers every request — GitHub info, release lists, asset downloads, README images — from it; anything else
fails as "no canned answer", so an e2e run has no path to the internet), `RS_E2E_NET_LOG` (one line per request,
`get\t<url>\t<status|unrouted>`) and `RS_E2E_GIT_MAP` (`<https URL>=<file:// URL>;…`: a Clone / Fetch of a mapped
github.com repo downloads from a local upstream and keeps its https origin; every other non-`file:` git remote is
refused unless the list contains `!live`). Since M10c (D-133, D-134) the same canned transport answers the Explore flows, a net-log
line gets a `\tauth` field when the request carried an `Authorization` header (never its value), and the e2e build's credential
store is the in-memory fake (D-117), so the run can never write to the real Credential Manager. `run-e2e.ps1` refuses an e2e exe
without the trash, network or git-map seam — rebuild it after pulling changes.

### Run (M8.2–M8.5)

```powershell
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1            # add -Keep to keep the scratch folder
powershell -ExecutionPolicy Bypass -File tools\e2e\check-e2e.ps1
```

`run-e2e.ps1`:
1. makes `target\scratch\e2e-<time>\` with `data`, `archive`, `fx` (all fixtures via `rs fixtures`, plus 50 copies of
   the basic fixture registered with `rs fixtures --register 50`, plus `align`: `tests\fixtures\align\README.md` with
   its images and one failing remote image, committed with the git CLI), and `up\demo-upstream` (a small git repo,
   isolated git config) as the `file://` clone source and as the local upstream of the mapped
   `https://github.com/e2e/demo-upstream` and of `https://github.com/e2e/explored` (the Explore Save-to-shelf target); sets
   `RS_E2E_NET` / `RS_E2E_NET_LOG` / `RS_E2E_GIT_MAP` (wd-e2e writes the routes file before the first app start) and the fake
   GitHub token the harness types into Settings;
2. hashes the `.git` + work tree of the 9 fixtures the UI will import;
3. starts tauri-driver on a free port (msedgedriver from user env `MSEDGEDRIVER`, tauri-driver from
   `C:\Dev\tools\cargo\bin`) and runs `tools\e2e\wd-e2e.mjs`;
4. kills the whole driver tree (`taskkill /T /F`), then anything else it saw start, and counts leftovers (must be 0);
5. scans every file of the scratch data dir (SQLite + WAL, WebView profile) and archive, the net / open / trash logs and
   `result.json` for the fake token (`tools\e2e\scan-secret.mjs`: UTF-8 and UTF-16LE, with a control string that must be found);
6. re-hashes the imported fixtures (must be equal: the run never writes into an imported repo, NFR-O2) and writes
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
  upstream, Fetch ("1 new commit"), Fast-forward (to the new upstream tip), Remove (through the trash seam: the scratch
  clone is deleted inside the scratch archive and logged; `check-e2e.ps1` requires exactly one logged deletion inside
  the archive — nothing goes to the real Recycle Bin since M9, D-91. Runs before M9 each left one `demo-upstream.git`
  folder in the Recycle Bin).
- **v0.2.0 (M13), all against canned answers**: Back/Forward disabled in a fresh window, then back and forward between
  two pages; the Import dialog shows "Download the latest release" (checked) only for the github.com fixture, and its
  import fetches GitHub info + the release list and downloads the Windows asset (through GitHub's redirect to its CDN)
  into `<scratch archive>\_releases\…`; the `align` README in WebView2 — h1 text, logo, badge and table centred
  (≤ 3 px), `h2 align=right` flush right, `img align=right` floated flush right at its width, the remote badge from a
  `blob:` URL, the failing image as a chip, no `style` attributes; About shows GitHub's description, an Edit About
  value wins, clearing it brings GitHub's back (`aboutUser` / `aboutGithub`); the release card (tag chip, commit link,
  Open on GitHub, `@user` mention instead of the avatar — never fetched) and its assets: Show in folder for the
  auto-download, Download of another asset → Show in folder (both through the recording opener, files in the scratch
  store); Settings → Network defaults, two toggles saved and kept after a reload, README images off = chips and no
  request, then back to the defaults; a github.com clone through the git map with the dialog's release option
  unchecked (GitHub info fetched, nothing downloaded), then Fetch ▾ "Fetch and download the latest release" (file in
  the store) and Show in folder on its Releases page. Two extra screenshots per theme (`m13-release-assets`,
  `m13-align-readme`).
- **v0.3.0 Explore (M10c), all against canned GitHub answers** (steps `X0`–`X11`, D-133..D-136): Settings → Explore defaults
  (`exploreEnabled` true, `githubTokenSaved` false, the field is a password input); nav entry → search `explored` (nothing
  requested before Enter, one canned search request, "9 of 10 searches left", two results with Open / Save, direct open) →
  Open; the cold repository page (files, rendered README, About, "55 of 60 requests left", "Add a token", every link under
  `#/explore/`) costs exactly the 5 requests of D-124; folder, file view, history, a commit with its diff, Branches, Tags,
  Releases (assets listed, no Download); a file over 1 MB (GitHub's 403 `too_large` contents answer) → "Too large to preview
  online — Save to shelf"; an unknown repository (404 + token hint); Save to shelf with "without downloading a release" →
  the clone job through the git map → "On your shelf" → Open in shelf → the shelf page (nothing downloaded); a rate-limit 403
  (`remaining: 0`) → full-page state with the reset time, then a saved page still opens under the banner "Rate limit reached,
  resets at HH:MM" without any request; Settings → paste the fake token → field gone, "Token saved ✓", the string in no
  page text / HTML / URL / history / storage / input / `settings_get` / `explore_status`, the limit state lifted, the next
  requests (the `/rate_limit` probe, the repo page) carry `Authorization` and only to api.github.com, Remove brings the field
  back and the requests are anonymous again; Explore off → nav entry gone, pages say so, zero requests; Clear Explore cache →
  "Cleared N saved answers", 0 left. 12 more screenshots (`m10c-explore-search|code|toolarge|ratelimit|banner|settings`, light and dark;
  `banner` = a saved page under the rate-limit strip). After the run `run.json` carries `secretScan` (files / bytes scanned, hits, control hits) and
  `check-e2e.ps1` fails on a hit, a missing control, an `Authorization` to another host, or no authenticated request at all.
- **Rust-side requests**: `run.json` carries the net log; `check-e2e.ps1` requires at least one canned api.github.com
  answer, zero unrouted requests and only the hosts the routes know (Explore only ever uses api.github.com). Together with "no foreign requests" from the
  webview, the default run makes no request outside the machine.

### Network run (M8.8, opt-in)

```powershell
powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1 -Network [-NetworkRepos owner/a,owner/b]
```

Adds session C: clones the given public GitHub repos (default `octocat/Hello-World`, `octocat/Spoon-Knife`) through
the https URL rules (only `file:` inputs use the seam; `-Network` adds `!live` to the git map so real git hosts are
allowed), then fetches each. The app's HTTP requests (GitHub info, notes, downloads) still go to the canned routes, so
they end as "no canned answer" notes on the jobs — the real GitHub requests are a hand test (§5). Use a repo with
Windows-invalid ref names to see the skipped-refs report. Implemented in M8 but **never executed yet** — the first run
is a hand step.

## 4. Release build (M9)

```powershell
npm run build:installer     # target\release\bundle\nsis\RepoShelf_0.3.0_x64-setup.exe  (built only, never run by an agent)
npm run build:portable      # target\release\portable\RepoShelf_0.3.0_x64-portable.exe
```

- Marker check (D-14, D-26, D-84, D-113, D-135): `node tools\check-shipped.mjs target\release\reposhelf.exe target\release\portable\RepoShelf_0.3.0_x64-portable.exe --e2e target\e2e\release\reposhelf.exe`.
  `RS_TEST_FILE_CLONE` and `RS_E2E_` (covers `RS_E2E_NET` and `RS_E2E_GIT_MAP`) must be absent from both shipped exes (the
  installer's payload is compressed, so checking the setup exe proves little; `target\release\reposhelf.exe` is the binary it
  packs), and since v0.3.0 the REAL credential store must be present (the text `Windows Credential Manager failed` of
  `cred::fail`; a build with `testkit` unified in silently gets the in-memory fake and a token that vanishes at restart). The
  e2e exe is the control in both directions: seams present, real store absent. `build:portable` fails on a seam hit or a missing
  real store.
- Portable smoke (agent, M9.4): start the portable exe with scratch `RS_DATA_DIR` / `RS_ARCHIVE_DIR` and
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<free port>`, move its window to −32000,−32000 with
  `SetWindowPos` as soon as it exists (a shipped exe has no off-screen seam, so it shows centred for ~0.1–0.3 s), read
  the page over CDP (`http://tauri.localhost/`, "Repositories" / "Your shelf is empty"), check `reposhelf.db` was
  created in the scratch data dir, `taskkill /T /F`, count leftovers by exe path and scratch path (0), delete scratch.
  Never point `run-e2e.ps1 -App` at a shipped exe: it has no seams, so real dialogs, browser and Recycle Bin would be
  used (`run-e2e.ps1` refuses it anyway: no trash / network seam strings). The shipped exe has the real HTTP client, so
  the smoke stays on the empty shelf — no import, no clone.
- Copies + `SHA256SUMS.txt` go to `_REVIVE\releases\v<ver>\` (the `_REVIVE\` kit is excluded from git via
  `.git\info\exclude`).

## 5. Hand-test checklist (needs a person)

Run these on the e2e exe with scratch folders, or on the portable build with `RS_DATA_DIR` / `RS_ARCHIVE_DIR`
set to empty scratch folders:

```powershell
$env:RS_DATA_DIR = 'C:\Dev\projects\repo-shelf\target\scratch\hand\data'; $env:RS_ARCHIVE_DIR = 'C:\Dev\projects\repo-shelf\target\scratch\hand\archive'
& .\target\release\portable\RepoShelf_0.3.0_x64-portable.exe
```

- [ ] **Installer (yours to run)**: `RepoShelf_0.3.0_x64-setup.exe` → Start menu entry starts the app → it uses
      `%LOCALAPPDATA%\dev.schwi.reposhelf` → Settings → Apps → RepoShelf → Uninstall removes the program, keeps the
      data. SmartScreen asks once (unsigned).
- [ ] **Upgrade over 0.2.0 / 0.1.0 (yours to run)**: with the old version installed and your shelf in use, run the 0.3.0
      setup. After it, Settings shows "RepoShelf 0.3.0", every repository, your edited About values, downloaded releases,
      Recent and the theme are still there, Settings → Network shows your values (defaults: all on except "Also download on
      Fetch all", limit 1024 MB) and Settings → Explore shows "Enable Explore" ticked and an empty token field.
- [ ] **Network adapter off (NFR-O1, needs admin)**: import two local repos, then turn the network adapter off
      (Settings → Network & internet → Wi-Fi/Ethernet off, or airplane mode). Restart the app and browse every tab of
      both repos, open files, commits and a commit page. Everything works; no error mentions the network. Only
      Fetch / Clone / GitHub info, notes and downloads may fail, with a readable "Could not reach …" note; README
      images seen before still show (cache), new ones show as chips. Turn the adapter back on.

v0.3.0 hand tests — Explore (real GitHub and the real Credential Manager; every agent run used canned answers and an in-memory
token store):

- [ ] **Browse online (real GitHub)**: header → Explore → search a project you know (press Enter), Open it. Code tab, a file, the
      README (images follow Settings → Network), Commits → a commit with its diff, Branches, Tags, Releases (assets listed,
      no Download online) all open; the strip shows "N of 60 requests left" and "resets HH:MM". A cold repository costs 5
      requests (the number drops by about 5, then by 1 for a folder or file you open). Open the same repository again: no drop.
- [ ] **Rate limit (real GitHub)**: open many different repositories (or clear the cache and repeat) until the strip says
      "Rate limit reached, resets at HH:MM": pages you opened before still open ("Saved data from HH:MM"), a new repository
      shows the full-page state with the reset time, nothing crashes, and after the reset time browsing works again. Searches
      have their own bucket (10 a minute).
- [ ] **Too large / big repositories**: a file over 1 MB in a real repository shows "Too large to preview online — Save to shelf";
      a repository with a huge README or a file list over 1,000 entries still renders; an empty repository (no commits) and a
      repository with a submodule folder: note what the Code tab shows (both unverified against real GitHub, M10a notes).
- [ ] **Save to shelf (real clone)**: on an Explore page "Save to shelf" starts a background clone with progress in the jobs
      popover; afterwards the page says "On your shelf" and "Open in shelf" opens the cloned copy. The ▾ option "Save without
      downloading a release" starts no download job. A repository already on the shelf shows "On your shelf" at once.
- [ ] **Token in Credential Manager**: Settings → Explore → paste a GitHub token (a classic token with no scopes is enough) →
      the field disappears and shows "Token saved ✓" + Remove; the strip says "token in use" and shows "… of 5,000"; open
      Windows **Credential Manager** → Windows Credentials → a generic entry `github-token` / `dev.schwi.reposhelf` exists
      (its value is the token). Close and start the app again: still "Token saved ✓". Remove → the field returns and the
      Credential Manager entry is gone. The token string appears nowhere in the app's window, in `reposhelf.db` or in files
      under your data / archive folders.
- [ ] **A refused token**: paste an invented token (`ghp_abc`) → "GitHub did not accept this token" and an empty field; a
      token for private repositories lets Explore open a private repository you can read (and without it the 404 page
      mentions that private repositories need a token).
- [ ] **The ignored Credential Manager test (writes one throw-away entry, own target `dev.schwi.reposhelf.handtest`, never the
      app's)**: `cargo test -p rs-core --features ghreleases -- --ignored real_credential_manager_round_trip` → ok, and
      afterwards no `dev.schwi.reposhelf.handtest` entry remains in Credential Manager.
- [ ] **Explore off / Clear cache**: Settings → Explore → untick "Enable Explore": the header entry disappears, an
      `#/explore/…` address says Explore is off, no network traffic (watch with a firewall / Resource Monitor if you like).
      "Clear Explore cache" reports the number of saved answers removed and the size shows 0.
- [ ] **Offline Explore**: with the network adapter off, a repository you opened before still opens with "Saved data from
      HH:MM (no connection)"; a new one says GitHub cannot be reached with a way to retry.

v0.2.0 hand tests (real GitHub; the agents' runs used canned answers only):

- [ ] **Back / Forward (F1)**: the two arrows left of the search are greyed out in a fresh window; after opening a repo
      and a file, Back goes back, Forward goes forward again, and they grey out at the ends.
- [ ] **README images (F2)**: open a cloned github.com repo whose README has badges / screenshots from other sites —
      they appear (not "Remote image: host" chips). Turn Settings → Network → Load remote images off: chips again; on:
      images back. Disconnect and reopen the README: the images seen before still show.
- [ ] **Centred README (F3)**: a README with `<div align="center">` / `<p align="center">` (logo, title, badge row)
      looks centred like on github.com; a right-aligned image floats right with text beside it.
- [ ] **Releases (F4/F5)**: Clone a small github.com project that publishes Windows releases (e.g. a tool with a
      `…-x86_64-pc-windows-msvc.zip` or `…-portable.exe` asset). In the Clone dialog "Download the latest release for
      this PC" is ticked; after the clone the jobs popover shows a download job with progress and then "Show in folder"
      (Explorer opens `Documents\RepoShelf\_releases\github.com\<owner>\<name>\<tag>\`). Releases shows cards with
      title, tag chip, commit link, date, Latest badge, notes with `@user` mentions (no avatar pictures) and Assets
      (size, downloads, Download, Show in folder, Open on GitHub, Source code zip / tar.gz).
- [ ] **Antivirus on downloads**: a downloaded `.exe` (or a zip containing one) may be scanned, quarantined or held
      back by Windows Defender / Bitdefender before you can open it — that is expected; RepoShelf never runs or unpacks
      downloads. If the antivirus removes a file, the asset shows Download again.
- [ ] **Per-action option**: Clone with the box unticked → no download job; Fetch ▾ "Fetch and download the latest
      release" → a download job (or "already downloaded"); plain Fetch follows the setting; Fetch all downloads only
      with "Also download on Fetch all".
- [ ] **GitHub About (F6)**: a cloned github.com repo shows GitHub's description and topics in About ("Some details
      from GitHub"); Edit About → your own description wins; clear it → GitHub's comes back.
- [ ] **Rate limit**: many Fetch all runs in an hour can hit GitHub's limit (about 60 requests per hour without sign
      in) — the job finishes with a note "request limit reached", the fetch itself still succeeds.
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

### v0.4.0 — Project Manager packs (D-137)

Automated cross-app round trip (no UI, no network): from `C:\Dev\projects\project-manager` run
`bash tools/shelf-roundtrip.sh` (it runs `crates/rs-core/tests/pm_pack_roundtrip.rs` here, which is ignored on its own).

- [ ] **Import a pack**: in Project Manager 1.6 pack a GitHub clone (Shelf → Pack). Here: "+" → Import folder… →
      pick `Documents\ProjectManager\.packs\github.com\<owner>\<name>.git` → it browses like a clone (badge Imported).
- [ ] **Update a pack**: Fetch on that repo → when GitHub has new commits a **Fast-forward** button appears in the
      header → click it → the page shows the new commits on the default branch.
- [ ] **Other imported repos unchanged**: an ordinary imported folder still shows no Fast-forward button.
