<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Form-host parity (spec 042)

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-08-06

Ordered so the workspace builds and tests green after every task. The
extraction tasks (T2–T6) move code **verbatim** out of
`crates/cobolt-cli/src/form_gui.rs` into the new crate, with `rcrun run-form`
consuming it at each step — host A must behave identically throughout
(its 4 existing tests are the tripwire, run at every task).

- [x] **T1 — Crate skeleton `cobolt-form-host`** (R1)
  - Files: `Cargo.toml` (workspace members), `crates/cobolt-form-host/Cargo.toml`,
    `crates/cobolt-form-host/src/lib.rs`
  - Do: new library crate; dependencies exactly as plan § 2 (cobolt-forms
    `render`, cobolt-runtime, cobolt-media, egui, eframe, egui_extras, image,
    rfd, pollster — no new externals); crate-level doc comment naming the two
    consumers and the R30 seam list.
  - Verify: `cargo build -p cobolt-form-host` and `cargo test --workspace
    --no-fail-fast` green (collect every "test result" line).

- [x] **T2 — Move the control state: one `CtrlState`** (R2)
  - Files: `crates/cobolt-form-host/src/state.rs`,
    `crates/cobolt-cli/src/form_gui.rs`, `crates/cobolt-cli/Cargo.toml`
  - Do: move `CtrlState` (with the 1.60.33 case-variant dedupe), `Default`,
    `from_control`, `state_entry_mut` and the `LiveState`/`FormState` glue out
    of host A verbatim; cli imports them from the new crate; delete cli's
    copy. Add state unit tests in the new crate: dedupe, case-insensitive
    read, template-seeded instance entry (assertions moved from any existing
    host-A coverage, plus new ones for the merge rules).
  - Verify: `cargo test -p cobolt-form-host -p cobolt-cli` green; host A's 4
    tests unchanged and green.

- [x] **T3 — Move the file-dialog module** (R25)
  - Files: `crates/cobolt-form-host/src/file_dialog.rs`,
    `crates/cobolt-cli/src/file_dialog.rs` (deleted or re-export shim),
    `crates/cobolt-cli/src/main.rs`/`form_gui.rs` imports
  - Do: move `DialogSpec` + async open/save/filter implementation verbatim;
    grep cobolt-cli for other users and re-point them. (cobolt-ide keeps its
    own IDE-side copy — out of scope.)
  - Verify: `cargo build -p cobolt-cli`; `cargo test -p cobolt-cli` green.

- [x] **T4 — Move diagnostics + unify `env_flag`** (R27, R28)
  - Files: `crates/cobolt-form-host/src/diagnostics.rs`,
    `crates/cobolt-cli/src/form_gui.rs`
  - Do: move host A's `env_flag` (`1`/`true`/`on`), dump-file writers and the
    `COBOLT_FRAME_DIAGNOSTICS` gate; add host B's launch preamble +
    per-update "NO SUCH CONTROL" trace (lifted from the template) as shared
    functions so both hosts emit both. Unit-test the truthiness rule and that
    the trace names existing ids on a miss.
  - Verify: `cargo test -p cobolt-form-host -p cobolt-cli` green.

- [x] **T5 — Move object seeding** (R20)
  - Files: `crates/cobolt-form-host/src/seeding.rs`,
    `crates/cobolt-cli/src/form_gui.rs`
  - Do: move `seed_objects`'s seed-building (designed props, `Name`/`Visible`/
    `Enabled`/`X`/`Y`/`Width`/`Height`/`TabOrder`,
    `append_data_binding_seed_props`, Maps/Search API-key seeds) into a shared
    `build_object_seed(&Form, &[Control], keys) -> Vec<…>`; cli calls it and
    still passes the result to `Interpreter::seed_objects`. Unit tests: a
    control's designed caption and geometry appear in the seed; `_Binding*`
    present for a bound grid; key seeds only when the env provides them.
  - Verify: `cargo test -p cobolt-form-host -p cobolt-cli` green.

- [x] **T6 — Move `FormApp` → `FormHost` (the big verbatim move)** (R1, R3
  groundwork; carries R5–R10, R12–R19, R21–R24, R26 logic)
  - Files: `crates/cobolt-form-host/src/lib.rs` (+ `src/fx.rs`,
    `src/window.rs`), `crates/cobolt-cli/src/form_gui.rs`
  - Do: move host A's `FormApp` struct, `impl eframe::App` (`ui`,
    `clear_color`), `paint_fx_frame`/`paint_face`/`fx_duration_ms`,
    `backdrop()`, viewport assembly (decorations, transparency, start
    position, window state, icon plumbing, exact size), lifecycle/supervisor
    handling, event dispatch incl. control-array routing, timer coalescing,
    DISPLAY pump with flush — verbatim, renamed `FormHost` +
    `FormHostConfig`. Introduce `HostHooks` (default no-op `per_frame`).
    Add the R17 title rule: `form.title`, else `config.title_fallback`; cli
    passes an empty fallback (behaviour unchanged). `rcrun run-form` becomes
    glue: args, parse/check, debug-wired interpreter thread, disk theme
    discovery, `FormHostConfig` build, `eframe::run_native(FormHost)`.
  - Verify: `cargo test -p cobolt-form-host -p cobolt-cli` green (A's 4 tests
    untouched); manual smoke: `rcrun run-form` on a PowerDemo3 form — window,
    entrance, close behaviour identical to before the move.

- [x] **T7 — cli glue test** (R30)
  - Files: `crates/cobolt-cli/src/form_gui.rs` (test mod)
  - Do: test that the run-form glue wires the debug channel only under
    `--debug`, and that the fx resolution (args × kill switch) reaching
    `FormHostConfig` matches the pre-move rule.
  - Verify: `cargo test -p cobolt-cli` green.
  - *Outcome note:* the fx half was already pinned by
    `fx_args_parse_and_kill_switch` (kept green through the move). The
    debug-channel half is a one-line arg check inside `cmd_run_form` with no
    seam to observe it through; adding a refactor purely to test that line
    would be test-driven scaffolding the plan's move-not-rewrite rule exists
    to avoid. Recorded here rather than silently skipped.

- [x] **T8 — Compiler reads the effect settings; generated deps gain the host
  crate** (R11 groundwork) — *outcome note:* `window_fx` is render-gated in
  cobolt-forms, so instead of a typed `fx_spec()` (which would drag egui into
  the compiler) the compiler bakes RAW `id:ms:easing` triples via
  `fx_triple()` and the ONE parser (`FxSpec::parse`, shared host) validates
  ids and clamps durations at run time — strictly one parser, as R11 intends.
  - Files: `crates/cobolt-compiler/src/lib.rs`
  - Do: remove `#[allow(dead_code)]` from the `FormsConfig` effect fields; add
    `fx_spec(effect, ms, easing) -> FxSpec` (duration clamped to the effect's
    own bounds — same rule as the IDE's `entrance_fx`); `generate_main_rs`
    emits `PROJECT_FX_ENTRANCE` / `PROJECT_FX_EXIT` (`id:ms:easing` triples)
    and `PROJECT_FX_ON_RESTORE`; `generate_cargo_toml` adds
    `cobolt-form-host = { path = "{cp}/cobolt-form-host" }`. Old template
    still compiles (consts carry `#[allow(dead_code)]` at the *const* level
    until T9 consumes them).
  - Verify: `cargo test -p cobolt-compiler` green, including
    `generated_binary_source_actually_compiles`.

- [x] **T9 — Swap the template for thin glue over `FormHost`** (R3, R5–R19,
  R21–R26; AC1–AC11 behaviour lands here)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`form_runtime_code` template)
  - Do: replace the ~430-line inline host with glue that: loads the embedded
    MAIN form + theme pack; spawns the interpreter thread (blocks registered,
    `set_painter_ready`, `set_form_host`, `seed_objects` via the shared
    builder); resolves fx = `PROJECT_FX_*` × `form.window_effects` ×
    `PRC_NO_WINDOW_FX`; builds `FormHostConfig` (designed window props,
    `title_fallback = "{AppName} v{Version}"`, diagnostics flags); passes a
    `HostHooks` whose `per_frame` calls
    `crate::exec_rust_blocks::cobolt_windows::show_all`; keeps the headless
    fallback and the generated banner. Delete the now-dead template host code.
  - Verify: `cargo test -p cobolt-compiler` green —
    `generated_binary_source_actually_compiles` is the gate; template-content
    assertions updated in T10.

- [x] **T10 — Template-content tests** (R29 template half; AC14 greps as tests)
  - Files: `crates/cobolt-compiler/src/lib.rs` (test mods)
  - Do: assert the generated source instantiates `FormHost` and contains no
    divergent-host markers (no `struct CtrlState`, no inline
    `impl eframe::App` beyond the glue); `PROJECT_FX_*` consts render
    correctly from a manifest with effects (and default to `none` triples
    without); headless path intact for `has_forms=false`; source contains no
    `#[allow(dead_code)]` on the effect fields and no "packaged host reaches
    037/038 parity" placeholder.
  - Verify: `cargo test -p cobolt-compiler` green.

- [x] **T11 — Retire host C (surgical)** (R4; AC13) — *outcome notes:*
  reachability re-verified (no `FormRuntime` constructor exists anywhere;
  `form_runtimes` was never pushed). No i18n keys removed — the dead path used
  hardcoded literals, itself a pre-existing violation now moot. The IDE's
  `_Binding*` seed builders (dead with host C) were removed too — the shared
  `cobolt_form_host::seeding` copy is the survivor. `rcrun run-form-ipc` +
  `FormIpcMessage` lost their only caller: flagged as a follow-up task chip,
  not expanded into this spec. Roundtrip caption test re-pointed at the shared
  `CtrlState` via a test-only dev-dependency.
  - Files: `crates/cobolt-ide/src/form_runtime.rs`,
    `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`
  - Do: FIRST re-verify unreachability (grep: no `FormRuntime` construction,
    `form_runtimes` never pushed) — if any use is found, STOP and report to
    the operator. Then remove `FormRuntime`, its `CtrlState` copy and
    interpreter plumbing; remove `show_running_form_window`, the
    `form_runtimes` field, its viewport/repaint/busy glue and dead imports.
    Live residents stay: `ExternalFormRun`, `BuiltAppRun`, `resolve_fx_args`/
    `FormFxArgs`, `RunDiagnostics`, API-key resolvers, and their tests.
    Re-point any C-only assertions at the shared type before deleting. For
    each i18n key that loses its last reference: grep the workspace, and only
    then remove it from `Tr` + all six language blocks together.
  - Verify: `cargo test -p cobolt-ide --no-fail-fast` green (collect all
    "test result" lines); `grep -rn "FormRuntime" crates/cobolt-ide/src`
    shows no struct/construction; build emits no new dead-code warnings from
    the removal.

- [x] **T12 — Parity suite** (R29; AC12; verifies R2, R5–R10, R12–R17,
  R20–R24, R27–R28 at the decision level) — *outcome note:* lives as
  `mod parity` inside `src/host.rs` (plus the state/seeding/diagnostics module
  tests), NOT `tests/parity.rs`: driving `FormHost` requires its private
  fields, so in-crate unit tests are the only honest home. `FormHost::new` and
  `ui_impl` were split out of `run()`/`eframe::App::ui` for headless driving;
  viewport commands are asserted from `FullOutput.viewport_output`. The
  dedupe tripwire was exercised: removing the 1.60.33 dedupe makes
  `a_runtime_write_replaces_the_designed_case_key` FAIL (verified locally,
  restored, suite green again).
  - Files: `crates/cobolt-form-host/src/host.rs` (`mod parity`)
  - Do: headless `begin_pass`/`end_pass` harness (clear texture deltas, per
    the 0.36 idiom). Implement the seven assertion groups from plan § 6
    (state, seeding, effects gating, lifecycle order/veto/program-end, window
    assembly incl. title rule, I/O & pacing incl. DISPLAY flush, diagnostics).
    The suite prints a quantified summary block: groups run, assertions per
    group, behaviours covered, and an explicit note that OS-window realities
    (true transparency/decorations) are covered by the manual pass.
  - Verify: `cargo test -p cobolt-form-host -- --nocapture` shows the summary;
    temporarily reverting the dedupe in `CtrlState::set` makes the state group
    fail (do locally, do not commit the revert).

- [x] **T13 — Docs, System KB, chunked store** (R31; AC14) — *outcome note:*
  the literal string "037/038 parity" still greps in `crates/` ONLY as the
  negative assertion inside the template-content test; the placeholder comment
  itself is gone from `FormsConfig`.
  - Files: `docs/developers-guide-en.md`, System KB text in
    `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`
  - Do: guide — window-effects and window/lifecycle sections state they hold
    in built applications too (and the built-app title rule); KB — 038/037
    sections updated the same way; retire the "consumed once the packaged
    host reaches 037/038 parity" comment. No new IDE strings expected (if any
    appear, add `Tr` ×6). Rebuild:
    `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide i18n` green;
    `prebuilt_chunked_kb_matches_the_published_documentation` green;
    `grep -rn "037/038 parity" crates/` empty.

- [x] **T14 — Finalize** (AC15 + manual AC1–AC11) — *agent half done:*
  version 1.60.37, CHANGELOG written, full workspace sweep all green (45
  result lines, 0 failed, 8 pre-existing ignored), release binaries rebuilt,
  PowerDemo3 rebuilt end-to-end with the effects verifiably baked
  (`matrix-rain:4000:ease-in-out` / `fade:400:ease-in` matching its toml) and
  delivered to `bin/` + `dist/`. *Remaining: the operator's visual pass*
  (launch `dist/powerdemo3`; see HANDOFF.md §"What remains").
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump the **fix** number (z); CHANGELOG entry listing the observable
    built-app corrections (effects play; window closes at program end;
    designed title with branded fallback; lifecycle events/vetoes/`me::`
    methods; seeded property reads; DISPLAY flush; exact window size). Full
    sweep: `cargo test --workspace --no-fail-fast` — collect every
    "test result" line, list expected-failure exceptions explicitly (none
    anticipated). Rebuild release binaries (`cargo build --release -p
    cobolt-ide -p cobolt-cli`) so the operator's manual pass runs fresh code.
  - Verify (agent): all suites green; A's 4 tests + parity suite + compile
    gate green.
  - Verify (operator, manual per plan § 6): build PowerDemo3 → launch from
    `dist/`: matrix-rain entrance, designed title, fade exit on close and on
    `STOP RUN`; `PRC_NO_WINDOW_FX=1` suppresses; Run Form side-by-side looks
    identical; `./app | cat` streams DISPLAY live.

## Done criteria

All acceptance criteria AC1–AC15 in spec.md are checked (AC1–AC8's window
realities via the operator's manual pass; the rest by the automated
verifications above), every listed test invocation is green, docs/KB/chunked
store are current, and the change ships as **fix** commit(s) on the `fixes`
branch per the operator's rules — do **not** commit/push/merge or announce on
the forum unless the operator asks (f=97, after merge to main).
