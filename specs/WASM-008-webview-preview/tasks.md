# Tasks — The web-view Preview

- **Status:** in progress — T1–T10 done (results at the bottom); T11–T13 are open and named
- **Plan:** ./plan.md   **Date:** 2026-10-20

Commits (worktree branch `worktree-agent-a97ba65c65571c2df`, never pushed): `2eded9b0` (seam, server, presets, protocol),
`d63c6faf` (helper process, settings section), `93b9124e` (controller, controls window, wiring),
`d97f4db6` (plugin rules and skills, System KB, regenerated store); the closing commit carries the specs' plan and tasks.

- [x] **T1 — The build/serve seam** (R1, R10)
  - Files: `crates/cobolt-project-tools/src/web/mod.rs`
  - Done: `WebBuildBackend` (build, bundle_dir, serve), `WebBuildRequest::from_manifest`, `BuildReport {bundle_dir, size, startup_ms, warnings, duration_ms}`, `WebBuildError {NotYetLinked, Refused, Failed}`, `link` / `unlink` / `backend`, `NotYetLinked`.
  - Verify: `cargo test -p cobolt-project-tools --lib web::tests`.

- [x] **T2 — The development server** (R8)
  - Files: `crates/cobolt-project-tools/src/web/devserver.rs`
  - Done: loopback only, Host-header check, MIME table (`.wasm` = `application/wasm`), COOP/COEP from the project's isolation setting, `/__prc/version` + `/__prc/livereload.js`, 15 spellings of a path outside the bundle refused, symlinks that leave the bundle not followed, GET/HEAD only.
  - Verify: `cargo test -p cobolt-project-tools --lib web::devserver` (prints the request timings).

- [x] **T3 — Presets** (R4)
  - Files: `crates/cobolt-project-tools/src/web/presets.rs` — 7 presets × 2 orientations, pixel ratios 1/1.5/2/3, safe-area insets that turn with the screen.

- [x] **T4 — The helper process and its protocol** (R2, R5, R8)
  - Files: `crates/cobolt-web-preview/`, `crates/cobolt-project-tools/src/web/preview.rs`
  - Done: JSON lines pinned from both sides; `open` carries the emulation; `emulate` is one reload; console / error capture; navigation and new-window policy; the system-browser fall-back; `PRC_WEB_PREVIEW` and `PRC_PREVIEW_NO_BROWSER` (tests).
  - Verify: `cargo test -p cobolt-web-preview` (5) and `cargo test -p cobolt-project-tools --lib web::preview` (5).

- [x] **T5 — The controller** (R1, R3, R4, R7)
  - Files: `crates/cobolt-ide/src/panels/web_preview.rs`
  - Done: build job, serve, session, `SourceWatcher`, rebuild + `notify_changed`, `Controls::changes_from`, `Note`s rendered from `Tr`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` (13).

- [x] **T6 — The controls window** (R4, AC3)
  - Done: 420 x 560 in six languages, one size; buttons and checkboxes in fixed boxes; the log takes what is left.
  - Verify: `the_controls_window_keeps_its_size`.

- [x] **T7 — Wiring** (R10, AC9)
  - Files: `crates/cobolt-ide/src/app.rs` — designer Preview (toggle), Run (browser), Build (bundle), Stop and *open another project* end it; File ▸ Build label by kind.

- [x] **T8 — Strings** — `pk_build_*` (9), `pk_prev_*` (25), `pk_ps_*` (7) in six languages.

- [x] **T9 — Agent surface** — `build` for a Web project returns the `BuildReport`; `list_files` shows `web_assets`; the plugin rules and skills mention the Preview (see WASM-001 T11).

- [x] **T10 — Final sweep** — results below.

- [ ] **T11 — A window, once, by the operator.** Open a Web project, press Preview on a dev build with `cargo build -p cobolt-web-preview` beside the IDE, and look. Nothing here can substitute for that.
- [ ] **T12 — The real generator.** When `cobolt-wasm` / the bundler are linked (`web::link(...)` at program start), run AC1–AC2 and fill §6 of plan.md with the real build and update times.
- [ ] **T13 — R6, R9.** The Backend started on request (needs the backend agent's binary) and the headless render / scripted run of a web form (WASM-015 R1–R2).

## Results (T10, 2026-10-20)

- `cargo test -p cobolt-project-tools --lib web::` — 29 of the lib's 100 tests, all passing; `web::preview` 5.
- `cargo test -p cobolt-web-preview` — 5 passed (the helper's `run` module is compiled on macOS, so the wry/tao calls are type-checked).
- `cargo test -p cobolt-ide --bin cobolt-ide web_preview` — 13 passed. What they print: 120 note renderings (20 notes x 6 languages, none empty, none with a `{}` left); build -> serve -> web view in 337-499 ms with a stand-in generator and helper, a form edit rebuilt and republished (`/__prc/version` rose), the controls reached the helper as 5 lines of which 1 was `emulate`; the controls window held 420 x 560 points over 120 frames (6 languages x busy and empty), the log never under 136 points.
- Dev server (debug build, loopback): 200 requests for a 17-byte file in 114 ms (1744 req/s); a 3 MiB `.wasm` 10 times in 157 ms (191 MiB/s); a changed stylesheet moved the change count in 166 ms (watch interval 150 ms); 15 spellings of a path outside the bundle refused, none leaked.
- Whole IDE binary: 1866 passed, 3 failed — the same three that WASM-001 tasks.md names (not related).
- **Not run, and not claimed:** a real window (no agent drives one), the helper on Windows or Linux, AC1 and AC2 with a real generator, AC8.
