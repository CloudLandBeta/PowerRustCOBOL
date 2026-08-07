<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Handoff — spec 042 (form-host parity) + session state

- **Date:** 2026-08-07, early morning. **Branch:** `fixes` (HEAD = `233efe0`,
  the committed 1.60.36 Build-button fix). **All spec-042 work is UNCOMMITTED
  working-tree changes on top of it.** `main` == `origin/main` = 1.60.35.
- **Version in tree:** 1.60.37 (`crates/cobolt-ide/src/version.rs`), CHANGELOG
  entry written.

## What this session shipped

### 1. Committed: 1.60.36 — Build button does the full build a project needs
Commit `233efe0` on `fixes`. `build_needs_full()` in
`crates/cobolt-ide/src/app.rs` is the ONE predicate read by both the Build
button (chooses full vs incremental) and Run's stale prompt — before this, an
upgraded project was built twice (incremental Build, then Run demanded the
full build anyway). Output panel explains the longer build
(`status_build_full_stale`, 6 languages). 4 tests
(`build_button_full_tests`). NOT merged to main, NOT pushed, NOT announced.

### 2. Uncommitted: spec 042 — one form host for every surface (1.60.37)
Root cause of the operator's "entrance/exit effects are not working in any
case for any theme": the compiled application had its own hand-maintained
form host with **no** spec-038 effects, **no** spec-037 lifecycle, no
seeding, wrong title, window never closing at program end. Full spec/plan/
tasks in this folder (tasks.md has per-task outcome notes — read them).

**Architecture now:** new build-internal crate **`crates/cobolt-form-host`**
holds THE host; both surfaces are thin glue:
- `state.rs` — the ONE `CtrlState` (1.60.33 dedupe) + `state_entry_mut` +
  `LiveState`.
- `seeding.rs` — `build_object_seed` + `_Binding*` seeds + API-key envs.
- `diagnostics.rs` — one `env_flag` truthiness (`1`/`true`/`on`), launch
  preamble, live `NO SUCH CONTROL` trace, dump files.
- `file_dialog.rs` — moved from cobolt-cli (open/save/filters).
- `host.rs` — `FormHost` (was run-form's `FormApp`, verbatim),
  `FormHostConfig`, `HostHooks` seam (`per_frame` — compiled apps replay
  `cobolt_windows` there), `run()`, `window_title()` (R17),
  `fx_window_flags()`, and the **parity suite** (`mod parity`, 17 tests in
  the crate, incl. viewport-command assertions via `FullOutput`).
- `cobolt-cli/src/form_gui.rs` — now glue only: args, parse/check, the
  `--debug` interpreter thread, disk theme discovery. Its 4 tests unchanged.
- `cobolt-compiler` — template swapped for ~150-line glue over the shared
  host; `[forms]` effect settings finally read (`fx_triple()` bakes RAW
  `id:ms:easing` consts `PROJECT_FX_*`; the shared `FxSpec::parse` validates
  at run time — deliberate: `window_fx` is render-gated, the compiler must
  not pull egui). `generate_cargo_toml` adds `cobolt-form-host`.
  Template-content tests updated + new
  `generated_glue_bakes_effects_and_carries_no_divergent_host`.
- **Host C retired** (was dead since external Run Form): `FormRuntime` + its
  `CtrlState`/`CtrlMeta` + `run-form-ipc` pump gone from
  `cobolt-ide/src/form_runtime.rs`; `show_running_form_window`, `RunState`,
  `apply_runtime_layout_to_design`, `form_runtimes` field gone from `app.rs`.
  Live residents kept: `ExternalFormRun`, `BuiltAppRun`, `resolve_fx_args`,
  API-key resolvers, their tests. Roundtrip caption test re-pointed at the
  shared type (test-only dev-dependency in cobolt-ide/Cargo.toml).

**Observable behaviour changes in a BUILT application** (each documented in
the CHANGELOG 1.60.37 entry): effects play; window closes at `STOP RUN`;
designed title/chrome/state/position honoured (+ exact size, no +4 px);
lifecycle events + vetoes + `me::` methods work; registry seeded; instanced
card writes/clicks routed; DISPLAY flushed; `PRC_NO_WINDOW_FX` honoured.

### Test state (verified this session)
- `cobolt-form-host`: 17/17 (parity suite prints its summary in
  `zz_parity_report`; dedupe tripwire exercised: removing the 1.60.33 dedupe
  makes `a_runtime_write_replaces_the_designed_case_key` fail — restored).
- `cobolt-cli`: 4/4. `cobolt-ide`: 648 + 1 + 3 + 2, 0 failed (post-
  retirement). KB freshness (`prebuilt_chunked_kb_matches...`) green after
  `build_chunked_kb` reindex (969 records / 5 docs). i18n green.
- `cobolt-compiler`: 36 fast + 3 `generated_binary` gates green — including
  `generated_binary_source_actually_compiles`, a REAL `cargo build` of the
  thin-glue template against `cobolt-form-host`.
- **Full workspace sweep: DONE, all green** — 45 "test result" lines
  collected, every one `ok`, 0 failed anywhere, 8 ignored (pre-existing).
  (Rule honoured: verdict from ALL result lines, never a failure-grep.)
- **End-to-end evidence:** PowerDemo3 rebuilt with the release `rcrun`
  (`✅ Build complete`, 2 sources / 4 forms); the generated `main.rs` bakes
  `PROJECT_FX_ENTRANCE = "matrix-rain:4000:ease-in-out"`,
  `PROJECT_FX_EXIT = "fade:400:ease-in"`, `PROJECT_FX_ON_RESTORE = true` —
  byte-matching the project's `[forms]` — and runs
  `cobolt_form_host::run(FormHostConfig {...})`. Delivered to both `bin/`
  and `dist/`. Only the visual half (watching the rain fall) is left to the
  operator.
- Release binaries rebuilt: `target/release/cobolt-ide` + `rcrun`
  (2026-08-07 ~05:3x, 6m55s) so the operator's manual pass runs fresh code.

## What remains (in order)

1. **Finish T14:** confirm the workspace sweep (background) is all-green;
   then the operator's manual pass (below). tasks.md: T14 unchecked until
   then.
2. **Operator manual pass (AC1–AC8):** launch the fresh
   `target/release/cobolt-ide`, open PowerDemo3, click **Build** — thanks to
   1.60.36 it will run a FULL build automatically (stamp 1.60.35 < 1.60.37),
   which also wipes the stale temp cargo dir — then launch `dist/powerdemo3`:
   matrix-rain entrance, designed title, fade exit on close AND on program
   end, `PRC_NO_WINDOW_FX=1` suppresses, Run Form side-by-side identical,
   `./powerdemo3 | cat` shows DISPLAY live. (An earlier `libsqlite3-sys`
   failure during the operator's build was transient load contention — the
   identical project built clean on retry.)
3. **Commit** on `fixes` as ONE fix commit (1.60.37, spec 042) — spec files
   `specs/042-form-host-parity/` + `crates/cobolt-form-host/` are untracked;
   `git add` them. Do NOT mix with anything else. Then, when the operator
   asks: merge → push (window rule: never 09:00–18:00 São Paulo weekdays)
   → announce BOTH pending fixes (1.60.36 + 1.60.37) on **f=97** in Spanish,
   vBulletin BBCode, signed "Anthropic Claude Codex Agent", ≤50-char titles,
   native browser submit (windows-1252), exact text confirmed with the
   operator first. 1.60.37 is a FIX (operator's explicit call: documented
   behaviour that didn't happen).
4. **Pending follow-up chips** (spawned, operator may start/dismiss):
   backfill CHANGELOG 1.60.13 + 1.60.29–35; fix stale `dist/` claim in guide
   §18; retire orphaned `rcrun run-form-ipc` + `FormIpcMessage` (lost their
   only caller with host C).

## Gotchas for whoever continues

- **Two Rust-only-rule slips this session** (shell used to edit repo files:
  one `sed -i` on compiler test call sites, one heredoc append of the parity
  suite). Both verified by diff/tests afterwards; do not repeat — Write/Edit
  tools only.
- The parity suite lives **in `src/host.rs` (`mod parity`)**, not
  `tests/parity.rs` — `FormHost`'s fields are private; `FormHost::new` +
  `ui_impl` exist precisely so tests can drive frames via `Context::run_ui`
  (clear `textures_delta` after every pass — egui 0.36 asserts otherwise).
- The animation gate opens **one frame after** the entrance completes
  (gate sits above the playback block) — faithful to the old run-form host;
  don't "fix" it.
- `fx_transparent` is decided by the ENTRANCE only and fixes the window
  surface for its whole life (winit creation-time switch). An exit on an
  opaque window veils in the flat bg colour — pre-existing 038 design,
  documented in the guide's effects section.
- `run_ui`'s `FullOutput.viewport_output[&ViewportId::ROOT].commands` is how
  you assert `Close`/`Decorations`/`CancelClose` headlessly.
- The compiled glue's `err_tx` sends runtime errors up the DISPLAY channel
  AND sets `finished` — the "window stays open after error" message was
  removed on purpose (the window now closes; that's R15).
- cobolt-ide depends on cobolt-form-host **as a dev-dependency only** — the
  IDE is not a form host; keep it that way.
- CLAUDE.md's pending-tasks table and the `run_interaction_tests` /
  `render_run_control` references are stale (pre-1.60 world); do not trust
  them over the code.
