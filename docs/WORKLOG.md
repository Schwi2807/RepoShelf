# RepoShelf — worklog

One entry per milestone: date, steps done, test counts, gate output summary, open items, handoff notes.

## M0 — Project layout (2026-11-11)

Steps: M0.1–M0.9 done; M0.10 (optional `npx tauri dev` smoke) skipped — mandatory in M4.14.

- Workspace (`resolver = "3"`, edition 2024, version 0.0.1) with `crates/rs-core`, `crates/rs-cli` (bin `rs`, usage
  only), `src-tauri` (package `reposhelf`, lib `reposhelf_lib`, features `e2e = ["rs-core/testkit"]`). All registry
  deps resolved and compiled on Rust 1.99.0: git2 0.21.0 (vendored, https), rusqlite 0.40.2, comrak 0.59.0, ammonia
  4.2.4, ureq 3.4.2 (via `ghreleases`), tauri 2.11.5, dialog/opener plugins, trash.
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
