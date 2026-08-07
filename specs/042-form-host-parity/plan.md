<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Form-host parity (spec 042)

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-08-06

## 1. Approach

**Extract, don't rewrite.** Host A (`crates/cobolt-cli/src/form_gui.rs`) is the
behaviourally complete host — every capability in the parity set already works
there. The plan lifts A's `FormApp` and its supporting logic **verbatim** into
a new crate, `cobolt-form-host`, and turns both consumers into thin glue
around it:

```
cobolt-form-host  (new crate: the ONE form host — R1)
   ├── FormHost           eframe::App (A's FormApp, renamed; state, backdrop,
   │                      038 fx playback, 037 lifecycle, event dispatch,
   │                      control arrays, pacing, DISPLAY pump w/ flush)
   ├── CtrlState          the single control-state type (R2)
   ├── seeding            seed_objects + _Binding* + API-key seeds (R20)
   ├── window             ViewportBuilder assembly: title rule, icon,
   │                      decorations, state, start position, exact size,
   │                      fx transparency/chrome (R7, R17)
   ├── file_dialog        A's DialogSpec module (open/save/filters) (R25)
   ├── diagnostics        env_flag (1/true/on), launch preamble, live
   │                      per-update trace, A's file dumps (R27, R28)
   └── HostHooks (seam)   the ONLY per-host extension point (R30)

cobolt-cli  run-form      arg parsing, .cbl parse/check diagnostics,
                          interpreter thread WITH the @DBG debug channel,
                          disk theme-pack discovery  → FormHost
cobolt-compiler template  embedded form/theme/AST loading, interpreter thread
                          WITH register_exec_rust_blocks + set_painter_ready,
                          cobolt_windows::show_all as a per-frame hook,
                          PROJECT_FX_* consts baked at build time (R11)
                          → FormHost   (thin glue only — R3)
```

- **Who spawns the interpreter:** each host, as today. The debug stdin loop
  (A) and compiled-block registration (B) are exactly the per-host differences
  R30 names; keeping the interpreter spawn in the glue means the shared crate
  never needs to know about either. `FormHost` receives the four channels
  (`FormEvent`, `StateUpdate` ×2, `DISPLAY`), the `finished` flag, and the
  supervisor request channel — the same shapes both hosts already use.
- **Effects in the built application (R5–R11):** the compiler resolves the
  `[forms]` effect settings at build time (removing the `#[allow(dead_code)]`
  markers) into three generated consts — `PROJECT_FX_ENTRANCE` /
  `PROJECT_FX_EXIT` as the same `id:ms:easing` triples `--fx-entrance` speaks,
  plus `PROJECT_FX_ON_RESTORE` — parsed by the existing `FxSpec::parse`. The
  glue applies A's resolution rule (settings × `form.window_effects` ×
  `PRC_NO_WINDOW_FX`) and hands two `FxSpec`s to `FormHost`, which already
  contains the playback, chrome, transparency, `clear_color`, restore-replay
  and load-animation gating logic (R10) because that logic moved from A.
- **Lifecycle in the built application (R12–R19):** comes free with the move —
  `FormHost` carries A's supervisor wiring, veto/`CancelClose`, lifecycle
  events, `me::` methods, program-end close, window properties. The glue's
  only lifecycle job is calling `interp.set_form_host(...)` and passing the
  designed window properties through `FormHostConfig`.
- **Title rule (R17):** `FormHostConfig { title_fallback: String }` — the
  window titles itself `form.title`, or the fallback (`"{AppName} v{Version}"`
  in B, the form file name in A… A today uses `form.title` always; A passes an
  empty fallback so behaviour is unchanged) when the designed title is blank.
- **Host C retirement (R4):** delete the `FormRuntime` struct, its `CtrlState`
  copy and interpreter-thread plumbing from
  `crates/cobolt-ide/src/form_runtime.rs`, plus `show_running_form_window`,
  the `form_runtimes: Vec<FormRuntime>` field and its viewport/repaint glue in
  `app.rs`. **Surgical, not file deletion:** the same file's live residents —
  `ExternalFormRun`, `BuiltAppRun`, `resolve_fx_args`/`FormFxArgs`,
  `RunDiagnostics`, the API-key resolvers — stay. The old per-control renderer
  is already gone (`app.rs:12222`), so no rendering tests hang off C; the
  tests in `form_runtime.rs` (`built_app_run_tests`,
  `form_codegen_roundtrip_tests`, the fx-args tests) belong to the live
  residents and stay. Any C-only assertions found during removal (e.g. a
  `CtrlState` merge test) are re-pointed at the shared type before the copy is
  deleted. Reachability is re-verified first (grep for construction; then the
  build's dead-code warnings after removal act as the second witness).
- **Parity suite (R29):** lives in `cobolt-form-host`, driving `FormHost`
  headlessly with the `begin_pass`/`end_pass` harness idiom the workspace
  already uses (texture-delta clearing per the 0.36 upgrade). Because both
  live hosts are thin glue over the same `FormHost`, testing it once *is* the
  parity argument; what remains per-host is pinned by glue-level tests (§ 6).

## 2. Affected crates / files

- `crates/cobolt-form-host/` — **new crate**: `src/lib.rs` (FormHost,
  FormHostConfig, HostHooks), `src/state.rs` (CtrlState + state_entry_mut +
  LiveState), `src/seeding.rs`, `src/window.rs` (viewport assembly + icon),
  `src/fx.rs` (paint_fx_frame/paint_face/fx_duration_ms — moved, not copied),
  `src/file_dialog.rs` (moved from cobolt-cli), `src/diagnostics.rs`
  (env_flag + preamble + live trace + dump files), `tests/parity.rs`.
  Dependencies: `cobolt-forms` (render), `cobolt-runtime`, `cobolt-media`,
  `egui`/`eframe`/`egui_extras`/`image`/`rfd`/`pollster` — all already in both
  consumers today; **no new external dependencies**.
- `crates/cobolt-cli/src/form_gui.rs` — shrinks to run-form glue (args,
  parse/check, debug interpreter thread, disk themes, FormHost). Its 4 tests
  stay; logic-level tests move with the logic.
- `crates/cobolt-cli/src/file_dialog.rs` — moves to the host crate; cli
  re-exports if anything else references it.
- `crates/cobolt-cli/Cargo.toml` — add `cobolt-form-host`; drop deps that
  moved (keep what run-form glue still uses).
- `crates/cobolt-compiler/src/lib.rs` — `FormsConfig` effect fields lose
  `#[allow(dead_code)]` (R11); new `fx_spec()` resolver; `generate_main_rs`
  gains the three `PROJECT_FX_*` consts and swaps the ~430-line
  `form_runtime_code` template for ~80 lines of glue; `generate_cargo_toml`
  adds `cobolt-form-host = {{ path = "{cp}/cobolt-form-host" }}` (egui/eframe
  stay — EXEC RUST blocks compile against them); template-glue tests updated.
- `crates/cobolt-ide/src/form_runtime.rs` — remove `FormRuntime` + its
  `CtrlState`; live residents untouched.
- `crates/cobolt-ide/src/app.rs` — remove `form_runtimes`,
  `show_running_form_window` and glue; remove now-unused imports.
- `crates/cobolt-ide/src/i18n.rs` — remove `Tr` keys used only by the deleted
  path (verified per key, removed from all six languages together); no new
  keys expected.
- System KB text in `crates/cobolt-compiler/src/lib.rs` (038/037 sections) —
  say effects/lifecycle hold in built applications; retire the parity
  placeholder comment; regenerate `assets/knowledge/chunked.data`.
- `docs/developers-guide-en.md` — window-effects and window/lifecycle sections
  updated to cover built applications; `CHANGELOG.md` + `version.rs` (z bump).

## 3. Data / model changes

**None on disk.** `.cfrm` unchanged; `.project.toml` / `cobolt.toml` `[forms]`
effect keys already exist (the compiler finally reads them). The generated
`main.rs` gains consts and loses the inline host — still banner-marked
generated code. New crate changes the *generated project's* dependency list
only (path dependency, same `{cp}` mechanism as today).

## 4. Key decisions & alternatives

- **Decision: new crate `cobolt-form-host`** (resolves spec Q1). Why:
  `cobolt-forms` cannot host it — the host needs `cobolt-runtime` types
  (channels, `form_host::FormSupervisor`, `Interpreter::seed_objects`), and
  forms→runtime would invert the workspace's dependency direction; `cobolt-cli`
  cannot be a dependency of generated projects without dragging the whole CLI
  in. Rejected: host in `cobolt-forms` (dependency inversion); host in
  `cobolt-runtime` (runtime would grow egui/eframe — GUI in the interpreter
  crate); leaving B a copy with "sync discipline" (that discipline is what
  already failed).
- **Decision: interpreter thread stays in per-host glue.** Why: it is exactly
  where the intentional differences live (debug stdin vs block registration);
  sharing it would force the seam through the shared crate's API for no
  parity gain. Rejected: a `SpawnInterpreter` trait in the host crate — more
  seam surface, same behaviour.
- **Decision: `HostHooks` is a small trait with default no-ops** (today:
  `per_frame(ctx)` for `cobolt_windows::show_all`; nothing else). Why: R30
  demands the per-host list be short and named; a trait keeps it enumerable
  and documented in one place. Rejected: closures in the config struct
  (uncountable, undocumentable seam).
- **Decision: bake effect settings as generated consts** parsed by
  `FxSpec::parse`. Why: R11 (no manifest at run time), one parser for CLI args
  and baked consts, and the format already exists. Rejected: embedding a TOML
  snippet (second parser); reading `cobolt.toml` beside the binary (violates
  R11, breaks shipped apps).
- **Decision: diagnostics merge, not pick-one** (R27): A's dump files + B's
  live trace both move into the shared crate behind the unified `env_flag`.
  Rejected: dropping either (each caught real bugs — 1.60.32/1.60.33).
- **Decision: retire C surgically inside `form_runtime.rs`** rather than
  deleting the file. Why: the file's other residents are live (external run,
  built-app tracking, fx args). Rejected: whole-file removal (would take live
  code with it — the exact class of mistake the sacred-code rule exists for).

## 5. Risks & mitigations

- **Risk: regressing host A, the one host that works.** → Extraction is
  move-not-rewrite (same lines, new home); A's four existing tests plus the
  parity suite run at every stage; staged landing (state → seeding →
  window/lifecycle → fx) with `cargo test -p cobolt-cli -p cobolt-form-host`
  green after each; final manual Run Form pass on PowerDemo3.
- **Risk: the generated project fails to build against the new crate** (path,
  features, MSRV). → `generated_binary_source_actually_compiles` already does
  a real `cargo build` of the template; it gates the template swap. The new
  crate uses only dependencies both consumers already compile today.
- **Risk: host B behaviour changes that are *corrections* read as
  regressions** (window title now designed, window closes at STOP RUN,
  effects suddenly play). → Each is a spec'd requirement (R17, R15, R5) with
  its own CHANGELOG line; the f=97 post lists them as observable changes.
- **Risk: C is not actually dead (hidden construction path).** → R4's gate:
  re-grep for construction at implementation time; remove; let the compiler's
  dead-code/unused-field warnings corroborate; any discovered use stops the
  removal and goes back to the operator.
- **Risk: i18n key removal breaks another caller.** → Each candidate key is
  grepped across the workspace before removal; keys with any other use stay.
- **Risk: headless egui tests of window-level behaviour (transparency,
  decorations) can't drive a real OS window.** → The parity suite asserts at
  the decision level (what `ViewportBuilder`/commands the host *emits*, which
  the existing 038 tests in A already model); true windowing is covered by the
  manual pass in § 6. Stated honestly in the suite's summary output.
- **Risk: scope creep inside the big extraction.** → The inventory table in
  spec § 8 is the checklist; anything found beyond it is reported, not
  silently absorbed (report-or-fix discipline).

## 6. Test strategy

- **`cobolt-form-host/tests/parity.rs` — the parity suite (R29).** Drives
  `FormHost` headlessly; prints a quantified summary block (host build,
  behaviours exercised, assertion counts) per the operator's test-reporting
  rule. Assertion groups:
  1. *State*: case-variant dedupe (1.60.33) on the single `CtrlState`;
     case-insensitive merge; template-seeded instance entries (R2, R21, R22).
  2. *Seeding*: designed props + geometry + `_Binding*` + API-key seeds reach
     the registry; a read-before-write returns the designed value (R20).
  3. *Effects*: entrance suppresses the live UI until done; load animations
     gated behind the entrance; exit arms on close and the real close fires at
     t=0; kill switch and `WindowEffects=false` produce inert specs; restore
     replay fires no form events (R5–R10).
  4. *Lifecycle*: `onShow`/`onActivate` once after warm-up; veto →
     `CancelClose` + `onCloseRejected`; allowed close → one `onClose`;
     program-end (`finished`) closes — through the exit effect when set
     (R12–R15); `me::` methods produce the right viewport commands (R16).
  5. *Window assembly*: title rule (designed / fallback-when-blank), exact
     size, decorations vs fx, transparency class per effect, start-position
     and window-state commands (R7, R17).
  6. *I/O & pacing*: DISPLAY flush observable on a captured pipe; timer
     coalescing under backlog; repaint stays short with events queued
     (R23, R24).
  7. *Diagnostics*: one `env_flag` truthiness everywhere; preamble + live
     trace emitted under the flag (R27, R28).
- **`cobolt-cli`**: existing 4 tests unchanged (fx-arg parsing, seeding
  contract); a glue test that run-form builds a `FormHostConfig` with the
  debug channel only when `--debug`.
- **`cobolt-compiler`**: `generated_binary_source_actually_compiles` (real
  cargo build) gates the thin template; template-content tests assert the
  glue instantiates `FormHost`, bakes correct `PROJECT_FX_*` consts from a
  manifest with effects, honours `has_forms=false` (headless path), and that
  no `#[allow(dead_code)]` remains on the effect fields (AC14's grep, as a
  test).
- **`cobolt-ide`**: suite stays green after C's removal; a `grep`-style test
  is unnecessary — the field and struct are gone from the source.
- **KB**: `prebuilt_chunked_kb_matches_the_published_documentation` after the
  reindex.
- **Manual/visual (AC1–AC8 spot checks):** build PowerDemo3 (matrix-rain +
  fade exit) → launch from `dist/`: entrance plays, designed title shows,
  `STOP RUN`/close plays fade and exits, `PRC_NO_WINDOW_FX=1` suppresses;
  Run Form on the same form for side-by-side sameness; piped run shows
  DISPLAY live. Operator confirms on their machine (per the
  never-drive-the-application rule, the agent verifies via tests/build and
  the operator checks the UI).

## 7. Steering compliance

- [ ] i18n: no new UI strings expected; any key removed by C's retirement is
  verified unused and removed from all six languages together.
- [ ] Generated-code banner + regenerate-on-action contract preserved (the
  template keeps its banner; COBOL generation untouched).
- [ ] English dev guide updated (effects + lifecycle true for built apps);
  translations untouched.
- [ ] System KB tables/sections updated in the same change; chunked store
  regenerated (`cargo run -p cobolt-ide --example build_chunked_kb`);
  freshness test green.
- [ ] Fix vs feature: **fix** → `z` bump in `version.rs`, CHANGELOG entry,
  f=97 announcement after merge+push (no f=96 component; N1 holds).
- [ ] No "cobolt" in user-facing text (crate name `cobolt-form-host` is
  build-internal, like every other crate); COBOL identifiers stay English.
- [ ] MSRV 1.92 / egui 0.36; no new external dependencies.
- [ ] Commit discipline: this work is fix-only; the pending 1.60.36 Build fix
  on `fixes` merges/pushes per the operator's window rules before or with it.
