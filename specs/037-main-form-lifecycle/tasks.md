<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Main form, window lifecycle & Sync/Async form invocation

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-07-29

Ordered, small, independently-verifiable tasks. Each names the files it
touches, the requirement(s) it satisfies, and how to verify it. Check off as
completed. (R6 was removed from the spec; numbering is stable.)

- [ ] **T1 — Multi-viewport spike** (de-risks plan §5 risk 1; R28 feasibility)
  - Files: scratch example under `crates/cobolt-cli/examples/mv_spike.rs`
    (deleted at the end of the task — findings recorded in plan.md §5).
  - Do: prove `show_viewport_immediate` supports 3-deep nesting, a modal
    child (input blocked to parent), `with_taskbar(false)`,
    `with_decorations` toggle, `Fullscreen(bool)`, min/max buttons off,
    on macOS. Record per-OS caveats in plan §5.
  - Verify: `cargo run -p cobolt-cli --example mv_spike` — manual check by
    operator; findings appended to plan.md; example removed afterwards.

- [x] **T2 — Form model + .cfrm round-trip** (R1, R9, R12, R13, R14, R15)
  — done 2026-07-29: 98/98 `cobolt-forms --lib` tests green (2 new: 037
  round-trip, pre-037 defaults + byte-stability).
  - Files: `crates/cobolt-forms/src/model.rs`, `crates/cobolt-forms/src/xml.rs`
  - Do: add the 7 fields per plan §3 (`main_form`, `taskbar_icon`,
    `can_minimize`, `can_maximize`, `window_state` enum, `full_screen`,
    `title_visible`) with defaults; save/load as `<Form>` attributes.
  - Verify: `cargo test -p cobolt-forms --lib` green, incl. new tests:
    round-trip all 7 fields; a pre-037 `.cfrm` string loads with defaults
    (test prints each parsed value).

- [x] **T3 — Main-form normalisation + first-created default** (R3, R5)
  — done 2026-07-29: new `main_form.rs` module; wired at project open, form
  create, form delete; 4/4 unit tests + 3/3 i18n tests green (notices in 6
  languages). Note: repair writes happen AT the normalisation point (open/
  create/delete) rather than deferred to next save — stronger than plan §3's
  parenthetical, matches AC2 literally.
  - Files: `crates/cobolt-ide/src/app.rs` (project open / form create /
    form delete paths)
  - Do: `normalize_main_form`: zero or multiple mains ⇒ first form in the
    project `forms` list wins (status notice via existing output panel);
    first form created in a project ⇒ `main_form = true`; main-form
    deletion ⇒ auto-assign first remaining + notice (spec Q1 proposal);
    Run/Debug of the project starts at the main form.
  - Verify: `cargo test -p cobolt-ide normalize_main_form` — new unit
    tests: zero/two mains, first-created, delete-main; each prints the
    resulting holder. `cargo build -p cobolt-ide` green.

- [x] **T4 — MainForm checkbox + one-action reassignment undo** (R1, R2, AC1, AC2)
  — done 2026-07-29: Window section in the Form properties (MainForm checkbox
  read-only on holder + TaskbarIcon main-only + CanMin/CanMax/WindowState/
  FullScreen/TitleVisible rows, labels ×6); claim rides Cmd::SetFormProp with
  app-side cross-file settlement + previous-holder restore on undo. 6/6
  main_form tests, 3/3 i18n tests green.
  - Files: `crates/cobolt-ide/src/panels/properties.rs`,
    `crates/cobolt-ide/src/app.rs`
  - Do: Form-section checkbox (read-only when the form is the holder);
    reassignment action clears the previous holder, saves both `.cfrm`s,
    registers one app-level undo entry restoring both (plan D2).
  - Verify: `cargo test -p cobolt-ide main_form_reassign` — asserts both
    forms flip in one action and undo restores both (prints before/after
    holders). Manual: checkbox behaviour in the panel.

- [x] **T5 — Crown icon in the Forms tree** (R4, AC3) — done 2026-07-29:
  gold crown vector icon; open-designer claims outrank on-disk flags so the
  crown moves immediately. Build green; visual check pending (operator).
  - Files: `crates/cobolt-ide/src/panels/project.rs`
  - Do: `draw_crown_icon` vector stroke (match `draw_document_icon` style);
    used for the main form's tree row.
  - Verify: `cargo build -p cobolt-ide`; manual: crown on exactly one form,
    moves on reassignment.

- [x] **T6 — Window chrome from form fields** (R9, R12, R13, R14 open-state,
  R15, AC5, AC7 partial) — done 2026-07-29: root viewport honours
  CanMinimize/CanMaximize/TitleVisible/FullScreen/WindowState (Minimized via
  first-frame command); main form's TaskbarIcon outranks the project icon.
  cobolt-cli builds clean; 98/98 forms tests.
  - Files: `crates/cobolt-cli/src/form_gui.rs`
  - Do: build the root viewport from the new fields — min/max buttons,
    initial WindowState, fullscreen, decorations (TitleVisible), TaskbarIcon
    (main form only).
  - Verify: `cargo build -p cobolt-cli`; `cargo test -p cobolt-forms`;
    manual: run a form with each field toggled.

- [x] **T7 — WindowRegistry + host FormRequest channel** (R10, R11, R24 core)
  — done 2026-07-29 as `cobolt-runtime/src/form_host.rs`: `FormSupervisor`
  pure state machine emitting `HostAction`s; 8/8 headless tests (handles,
  singleton+focus, NULL broadcast, vetoes, cascades, modal deferral, handle
  methods).
  - Files: `crates/cobolt-cli/src/form_gui.rs`,
    `crates/cobolt-runtime/src/interpreter.rs` (channel seam only)
  - Do: registry (handle → form, viewport, kind, caller, modal, FormState
    mirror); `FormRequest`/`FormReply` channel pair; child interpreter
    thread spawn; handle-NULL broadcast on close; main-form singleton
    (focus + same handle).
  - Verify: `cargo test -p cobolt-runtime open_form_registry` — headless
    tests through the seam: distinct handles, singleton same-handle,
    NULL broadcast (prints handle sequences).

- [x] **T8 — OpenFormSync/Async + `me` + comma-form defaults** (R20, R21,
  AC6, AC12 partial) — done 2026-07-29: `me` receiver + parser comma form
  (`INVOKE obj::"Method"(a, b) RETURNING …`, new `comma_form` AST flag);
  RETURNING binds windowHandler vars; omitted params travel as host-defaulted;
  3/3 end-to-end tests incl. modal blocking (≥120 ms measured) and NULL-handle
  runtime error.
  - Files: `crates/cobolt-runtime/src/interpreter.rs`
  - Do: seed `ME` alias; `OPENFORMSYNC`/`OPENFORMASYNC` in `exec_method`;
    optional trailing args default from the target form's designed
    properties; `modal` default true (Sync); returning handle into
    `USAGE OBJECT` item.
  - Verify: `cargo test -p cobolt-runtime open_form_invoke` — comma form
    with 1..7 args, defaults filled from seeded form properties (prints
    the resolved geometry/state per call).

- [x] **T9 — Space-form INVOKE signature check** (R22, AC12 compile error)
  — done 2026-07-29: `cobolt-semantic` arity + literal-type check naming the
  method and expected signature; comma form exempt. 4/4 tests.
  - Files: `crates/cobolt-semantic/src/lib.rs`
  - Do: signature table for the two methods; INVOKE-form calls validated
    (arg count + literal type); diagnostic names the method and expected
    signature.
  - Verify: `cargo test -p cobolt-semantic open_form_signature` — missing
    arg and wrong-type cases produce the diagnostic (prints its text);
    comma/inline form unaffected.

- [x] **T10 — FormState + close vetoes + onCloseRejected** (R16–R19, AC8
  partial, AC10, AC11) — done 2026-07-29: FormState mirrored to the
  supervisor from the state stream; every close path funnels through
  `try_close`; title-bar close vetoed via CancelClose + onCloseRejected in
  the run-form host. Covered by form_host tests.
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
    `crates/cobolt-cli/src/form_gui.rs`
  - Do: FormState on the form object (runtime-only); host `try_close`
    single close path (user close, handle Close, cascades); Waiting veto +
    onCloseRejected event; Sync-caller veto while a Sync child Waits.
  - Verify: `cargo test -p cobolt-runtime form_state_lifecycle` — veto,
    event fired, Ready-then-close, caller-veto cases (prints event
    sequences).

- [x] **T11 — Close cascades + modal blocking** (R23, R24, R25–R28, AC11,
  AC13, AC14, AC15) — logic done 2026-07-29 (supervisor cascades, async
  survival, main-close-all + Exit, modal reply deferral, handle methods, NULL
  errors; all headless-tested). ⚠ GUI half OPEN: real child viewports
  (SpawnWindow execution) are **gated on the T1 spike findings** — until that
  lands, the single-window host releases child spawns immediately with an
  explicit stderr notice, so OpenForm* never deadlocks but no child window
  appears. Follow-up: multi-viewport host in form_gui.rs per plan D1.
  - Files: `crates/cobolt-cli/src/form_gui.rs`,
    `crates/cobolt-runtime/src/interpreter.rs`
  - Do: Sync children close with caller (veto aborts whole close); Async
    survive; main-form close-all + exit; modal input blocking in the host;
    Sync+modal blocks the caller's interpreter until `FormClosed`; handle
    methods `Close/Focus/SetWindowState/SetFullScreen/SetTitleVisible` +
    FormState read; NULL handle invoke ⇒ standard runtime error.
  - Verify: `cargo test -p cobolt-runtime form_cascades` — cascade, async
    survival, main-close-all, modal-block (thread joins), handle methods,
    NULL error (prints observed sequences).

- [x] **T12 — Runtime window commands + onFullScreenChanged** (R13, R14,
  R15 runtime, AC7, AC8, AC9) — done 2026-07-29:
  SetWindowState/SetFullScreen/SetTitleVisible on `me` and handles →
  viewport commands; onFullScreenChanged from ACTUAL ViewportInfo
  transitions (seeded with the designed value; FullScreen mirrored onto the
  form object before the event). Manual fullscreen check pending (operator).
  - Files: `crates/cobolt-cli/src/form_gui.rs`,
    `crates/cobolt-runtime/src/interpreter.rs`
  - Do: `SetWindowState`/`SetFullScreen`/`SetTitleVisible` on `me` and
    handles → viewport commands; fullscreen-change detection from
    `ViewportInfo` fires **onFullScreenChanged** once per actual change;
    leaving fullscreen restores the prior WindowState.
  - Verify: `cargo test -p cobolt-runtime window_commands` (command
    emission + single-event-per-change, headless); manual fullscreen check.

- [ ] **T13 — Codegen event paragraphs + System KB docs & store rebuild**
  (R14, R17 events; steering "System KB" hard constraint)
  - Files: `crates/cobolt-codegen/src/…`, `crates/cobolt-compiler/src/lib.rs`,
    `assets/knowledge/chunked.data`
  - Do: onCloseRejected + onFullScreenChanged wired like onLoad/onClose;
    docs-table entries for EVERY new property (MainForm, TaskbarIcon,
    CanMinimize, CanMaximize, WindowState, FullScreen, TitleVisible,
    FormState), method (OpenFormSync, OpenFormAsync, SetFullScreen,
    SetTitleVisible, SetWindowState, handle methods) and event
    (onCloseRejected, onFullScreenChanged). *(Store reindex: SUSPENDED by
    the operator 2026-07-29 — this task's docs-table update stands, but
    `build_chunked_kb` runs only on operator request. The 2026-07-29
    rebuild already ran once after the docs-table entries landed.)*
  - Verify: `cargo test -p cobolt-codegen` + `-p cobolt-compiler` green;
    generated `.cbl` contains the stubs + banner.

- [x] **T14 — Docs & i18n** — done 2026-07-29: "Multi-form applications and
  the main form" section in the English guide (§11, incl. per-OS taskbar note
  answering spec Q4 and the child-window status note); all new Tr strings
  landed ×6 during T3/T4; 3/3 i18n tests green. Translations untouched.
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: "Multi-form applications" guide section (main form, taskbar per-OS
    behaviour incl. spec Q4 answer, FormState, both invoke syntaxes +
    defaulting, windowHandler, cascades, fullscreen/title). New `Tr` fields
    ×6 (labels, tooltips, notices, undo status). Translations untouched.
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations);
    guide section renders in the doc viewer.

- [ ] **T15 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: minor bump → `1.42.0` + feature CHANGELOG entry; full test sweep;
    walk spec §5 AC1–AC15 and check each off (manual ones with the
    operator).
  - Verify: `cargo build --workspace` + `cargo test --workspace` green;
    every AC checked in spec.md; feature isolated in its own commit(s) —
    do **not** commit/push unless the operator asks (push window rules).

- [ ] **T16 (follow-up) — Multi-viewport child-window host** (R7/R8 window
  half, AC5/AC15 input half; plan D1). Gated on T1 spike findings: execute
  `SpawnWindow` as real `show_viewport_immediate` children in form_gui.rs
  (child .cfrm + generated program loading, per-child interpreter, modal
  input blocking, `with_taskbar(false)`), replacing the release-immediately
  stopgap.

- [ ] **T17 (follow-up) — R5: Run/built binary starts at the main form**
  (AC4). The IDE's Run Form is per-designer and the compiled binary's
  entry-form selection predates 037 — point both at the project's main form
  by default.

## Done criteria
All acceptance criteria in spec.md are checked, tests pass, docs updated, and
the change is split into fix/feature commit(s) per the operator's rules (do
**not** commit/push unless the operator asks).
