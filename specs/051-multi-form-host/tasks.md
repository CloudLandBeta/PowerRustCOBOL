<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Multi-form host

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-08-15

Ordered, small, independently-verifiable tasks. Each names the files it
touches, the requirement(s) it satisfies, and how to verify it. The project
stays green after every task. Feature branch: `feat/multi-form-host`
(the `main` commit hook forbids working there).

- [x] **T0 — Branch + commit the spec artifacts**
  - Files: `specs/051-multi-form-host/{spec,plan,tasks}.md`
  - Do: create `feat/multi-form-host` from `main`; commit the three spec
    documents so implementation diffs stay reviewable against them.
  - Verify: `git log --oneline -1` shows the spec commit on the feature
    branch; working tree clean.

- [x] **T1 — Supervisor: `Kind::Embedded` + embedded registration** (R3, R10
  groundwork; plan D3)
  - Files: `crates/cobolt-runtime/src/form_host.rs`
  - Do: add `Kind::Embedded`; an `open_embedded(caller, form_id)` entry that
    allocates a handle with no `SpawnWindow` action; window-only methods
    (`SETWINDOWSTATE`, `SETFULLSCREEN`, `SETTITLEVISIBLE`, `FOCUS`) on an
    Embedded handle return the existing `Err("windowHandler has no method …"
    )`-style error; `GETPROPERTY`/`SETPROPERTY`/`GETFORMSTATE`/`SUPERHANDLE`/
    `CLOSE` work unchanged. New unit tests beside the existing eight (which
    must not change).
  - Verify: `cargo test -p cobolt-runtime --lib` green; the 8 pre-existing
    lifecycle tests untouched (no assertion edits in the diff).

- [x] **T2 — Host: closed-handle fan-out** (R8)
  - Files: `crates/cobolt-form-host/src/host.rs`
  - Do: replace the single `closed_tx` field with a small owned fan-out
    (`Vec<mpsc::Sender<String>>`); `HostAction::NotifyClosed` sends the handle
    to every registered sender; the config's `closed_tx` becomes the root
    interpreter's entry. Unit test: N=3 receivers, one close, 3 deliveries.
  - Verify: `cargo test -p cobolt-form-host` green;
    `cargo build -p cobolt-cli` green (glue still compiles).

- [x] **T3 — Interpreter: SideMenu open methods + RETURNING gate** (R21, R22,
  R23)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
    `crates/cobolt-runtime/tests/test_open_form_invoke.rs`
  - Do: `method_returns_window_handle(name)` helper replacing the
    `starts_with("OPENFORM")` gate at the `Stmt::Invoke` RETURNING site; two
    arms in `exec_method` **before** the property-access fallback, gated on
    the receiver's seeded class == `SideMenu`, sending
    `FormRequest::OpenForm { caller: ROOT_HANDLE, sync, modal: sync, … }`
    with the same optional-args parsing as `me::"OpenFormSync"`; Sync blocks
    on the reply. Non-SideMenu receivers fall through unchanged.
  - Verify: `cargo test -p cobolt-runtime --test test_open_form_invoke` green
    with new cases: Async lands `caller=="W0"`, `sync==false`, RETURNING
    handle registered + NULLed on close; Sync blocks until the test thread
    replies; a *property* named `OpenStandAloneFormSync` on a TextBox still
    resolves as a property (fallback preserved — AC11's last clause).

- [x] **T4 — Semantic: signatures + load-path gate for the new pair** (R24,
  R26-method-half)
  - Files: `crates/cobolt-semantic/src/resolver.rs`,
    `crates/cobolt-semantic/tests/test_open_form_signature.rs`,
    `crates/cobolt-semantic/tests/test_form_load_path.rs`
  - Do: signature-table rows `OPENSTANDALONEFORMSYNC` (7 params) /
    `OPENSTANDALONEFORMASYNC` (6); add both names to the
    `check_open_form_target` gate (same `allows_standalone` predicate).
  - Verify: `cargo test -p cobolt-semantic` green; new tests mirror the
    existing files: space-form arity/type errors, comma form exempt (AC14);
    `Embedded` literal target → error, `Both` → clean, data-item → clean
    (AC13 method half); zero edits to existing assertions (AC6).

- [x] **T5 — Menu model: standalone action parsing + two-kind validation**
  (R17, R26-menu-half)
  - Files: `crates/cobolt-forms/src/menu.rs`
  - Do: `open_standalone_target(item) -> Option<(&str, bool /*sync*/)>`
    parsing `open-standalone-sync:` / `open-standalone-async:` (trimmed,
    empty → None); `validate_menu_targets` walks both kinds and
    `MenuTargetError` gains the kind, so the caller can phrase each error;
    embedded actions keep `allows_embedded`, standalone actions require
    `allows_standalone`; unknown forms still skipped.
  - Verify: `cargo test -p cobolt-forms --lib --features render menu` green;
    table-test covers: `open-form:`×Standalone → error, standalone×Embedded →
    error, `Both` passes everywhere, unknown skipped, nested items walked.

- [ ] **T6 — Compiler: two-kind menu validation at the build gate** (R26,
  R14)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the sole
    `validate_menu_targets` call site)
  - Do: surface both violation kinds as `CompilerError::Semantic`, the
    standalone message mirroring the 049 R17 one ("…whose FormFormat is
    Embedded — a standalone open requires Standalone or Both").
  - Verify: `cargo test -p cobolt-compiler` green with a new test building a
    project fixture containing a mis-wired standalone item (AC13 menu half);
    existing 049 R17 test untouched (AC6).

- [ ] **T7 — Compiler: per-form programs embedded** (R1, R2)
  - Files: `crates/cobolt-compiler/src/lib.rs`
  - Do: after the main parse, parse every other project form's generated
    `.cbl` (same tokenizer path, no copybook expansion — plan D8); stage each
    as `assets/programs/<ID>.bin` (gz bincode, `write_if_changed`); emit
    `static PROGRAMS: &[(&str, &[u8])]` + `load_program_by_id()` in
    `generate_main_rs`, mirroring `forms_const`/`load_program`; a form whose
    generated `.cbl` is missing or unparseable is **omitted with a build
    warning** naming it (runtime R15 covers the open attempt). Single-form
    projects emit the empty table.
  - Verify: `cargo test -p cobolt-compiler` green — new template-text
    assertions (a `PROGRAMS` entry with
    `include_bytes!("../assets/programs/…")`; the empty-table form; the
    loader), `the_build_puts_the_main_form_first` and
    `generated_binary_source_actually_compiles` unchanged and green (AC4
    build half).

- [x] **T8 — Host: `FormBody` extraction (behaviour-preserving)** (plan D4;
  groundwork for R3/R6/R10)
  - Files: `crates/cobolt-form-host/src/host.rs`
  - Do: mechanically move the per-form fields (`controls`, `state`, `anim`,
    `hovered`, `form_size`, `form_name`, `form_object`, channel set,
    lifecycle one-shots) into `struct FormBody`; `FormHost` holds
    `root: FormBody`; `ui_impl`'s form-frame path becomes
    `body_frame(&mut FormBody, …)`. **No behaviour change, no new fields.**
  - Verify: `cargo test -p cobolt-form-host` green (042 parity suite);
    `cargo test -p cobolt-forms --lib --features render` green (paint
    baselines); `cargo build -p cobolt-ide -p cobolt-cli` green. Diff review:
    field moves only.

- [x] **T9 — Host: real `SpawnWindow` — child viewports, modal, fan-out
  wiring** (R3, R4, R6, R7, R8, R9, R15; plan D7)
  - Files: `crates/cobolt-form-host/src/host.rs`,
    `crates/cobolt-form-host/src/lib.rs` (exports),
    `crates/cobolt-runtime/tests/test_super_receiver.rs` (AC5 e2e)
  - Do: `FormSource` provider + `spawn_form_interpreter` helper in
    `FormHostConfig`; the `SpawnWindow` arm builds a channel set, spawns the
    interpreter (fan-out registration from T2), pushes
    `ChildWindow { handle, body, viewport_id, builder }`; per-frame
    `ctx.show_viewport_immediate` re-declaration with `close_requested` →
    `supervisor.try_close(handle)`; geometry overrides honoured (037 R21);
    spawn failure → error reply + visible runtime error line (R15), stub
    eprintln deleted; while `modal_children_of(root)` is non-empty the root
    body renders under `ui.disable()`. **Q1 ruling:** the spawn helper
    injects ONE shared `Arc<Mutex<RustBridge>>` into every interpreter
    (root included) so EXEC RUST blocks in any form share the process-wide
    object bridge; default construction keeps a private bridge.
  - Verify: `cargo test -p cobolt-form-host` green — headless tests through
    the frame driver: OpenForm → child body exists and renders; modal child →
    root input disabled; child close → NotifyClosed reaches **all**
    interpreters. `cargo test -p cobolt-runtime` green — AC5 e2e: two live
    interpreters, `handle::"SetProperty"` on B read back via B's `me::X`;
    AC2 shapes (Async continues, Sync blocks, handle NULL after close).
    `grep -rn "not hosted yet" crates/` returns nothing (AC7).

- [x] **T10 — rcrun glue: disk-backed `FormSource`** (R13 rcrun door)
  - Files: `crates/cobolt-cli/src/form_gui.rs`
  - Do: provider that resolves a form id against the project's forms
    (uppercased stem), loads the `.cfrm`, parses the sibling generated
    `.cbl`; wired into both `run-form` and the shell config; missing
    generated code → the R15 error path.
  - Verify: `cargo build -p cobolt-cli` green; `cargo test -p cobolt-cli`
    green; observable: `rcrun run-form` on a two-form fixture project opens
    the child (operator check, AC8 half).

- [ ] **T11 — Shell: standalone click arms + shell modality** (R18, R19)
  - Files: `crates/cobolt-form-host/src/shell.rs`
  - Do: two new arms in the menu-click match parsing the T5 encodings →
    `supervisor.handle_request(OpenForm { caller: ROOT_HANDLE, sync,
    modal: sync, … })` with a stored reply receiver (never blocking the UI
    thread); while a modal child lives, the MenuPane, breadcrumb and pane
    body render disabled; children close with the shell per the existing
    cascade.
  - Verify: `cargo test -p cobolt-form-host` green — harness tests: each
    action produces the right `sync`/`modal` flags (AC10 shape); modality
    disables shell input; `close-application` with a live modal child routes
    through the veto path.

- [ ] **T12 — Shell: embedded occupant swap — the `open-form:` door** (R10,
  R11, R12)
  - Files: `crates/cobolt-form-host/src/shell.rs`,
    `crates/cobolt-form-host/src/host.rs` (active-occupant pane render)
  - Do: occupant registry aligned with `NavChain`; `open-form:` click spawns
    (or reactivates a preserved) occupant `FormBody` + interpreter,
    registered via T1's `Kind::Embedded`; event order `onDeactivate(out)` →
    `onActivate(in)`; teardown + `onDestroy` when not preserved; preserved
    occupants keep their parked interpreter (plan D2); breadcrumb reflects
    `chain.segments()` after every swap; the `open-form:` stub eprintln is
    deleted. **Q2 ruling:** the shell TICKS parked occupants' enabled Timer
    controls itself (per-timer clocks, `onTick` over the occupant's `ev_tx`
    with backlog coalescing, `request_repaint_after` for the next due tick)
    — timer handlers keep running off-pane; a headless test proves a parked
    occupant's onTick count advances.
  - Verify: `cargo test -p cobolt-form-host` green — headless swap tests
    assert the event order, `onDestroy` iff not preserved, TextBox state
    surviving a preserve round-trip (AC3 shape), breadcrumb segments (AC1
    shape); `grep -rn "awaits the multi-form host" crates/` returns nothing
    (AC7).

- [ ] **T13 — Designer: menu-editor surface + Target filtering + i18n ×6**
  (R16, R17, R20, R25)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-ide/src/app.rs` (pass the control type at modal open),
    `crates/cobolt-ide/src/i18n.rs`
  - Do: `MenuEditorModal.is_side_menu`; Action combo shows 5 options for
    SideMenu, 3 for MenuBar; encode/decode arms for both new encodings
    (`action_type_of`, `sync_bufs_from_selection`, both write sites);
    `forms_under` returns (embeddable, standaloneable) from the extended
    `form-format` sniff (missing → Standalone; unreadable → both); Target
    picker filters per action and the orange "⚠ Standalone" advisory styling
    is removed; "Preserve previous form" stays `open-form`-only; new `Tr`
    fields `menu_action_open_standalone_sync` / `_async` in all six
    languages.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` green — new tests:
    encode/decode round-trip for both encodings (AC9), option-count gating
    (SideMenu 5 / MenuBar 3), filter table-test over
    Standalone/Embedded/Both/unreadable (AC12); i18n completeness tests
    green (no empty translations).

- [ ] **T14 — IDE Run Form parity** (R13)
  - Files: `crates/cobolt-ide/src/form_runtime.rs`,
    `crates/cobolt-ide/src/app.rs`
  - Do: IDE Run Form supplies the project-backed `FormSource` and the shared
    spawn helper; child viewports declared from the IDE's own viewport loop
    (its existing idiom); shell mode in Run Form gains the same occupant and
    standalone behaviour.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` green;
    `cargo build -p cobolt-ide` green; operator: the T10 fixture behaves
    identically under IDE Run Form (AC8).

- [ ] **T15 — Docs: System KB + Developer's Guide + chunked rebuild**
  (steering; R21 docs)
  - Files: `crates/cobolt-compiler/src/lib.rs` (KB constants),
    `docs/developers-guide-en.md`, `assets/knowledge/chunked.data`
  - Do: `control_method_docs` gains a SideMenu arm (both methods, signatures
    + descriptions); `methods_reference_doc` gains the SideMenu section;
    `control_purpose` SideMenu string and the 037/shell prose updated (child
    windows are real; three doors; filtering); guide: the three doors, the
    two methods, Sync-is-modal, and ⚠️ caveats (timers pause off-pane;
    same-INDEXED-file sharing between live forms; EXEC RUST state is
    per-form, `cobolt_windows` ids process-wide; `rcrun build` trusts
    on-disk generated code). Translations untouched. Rebuild the chunked KB.
  - Verify: `cargo run -p cobolt-ide --example build_chunked_kb` succeeds;
    `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb` green.

- [ ] **T16 — Finalize** (AC4, AC7, AC8; operator gate)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump the fix number `z`; CHANGELOG feature entry. Full sweep:
    `cargo test --workspace --no-fail-fast` — collect **every** "test result"
    line, list expected failures explicitly (never verdict from a grep).
    Manual checklist for the operator: AC1/AC2/AC3/AC10/AC11 visually in a
    compiled two-form app; AC8 under `rcrun run-form` and IDE Run Form; AC4
    single-form project unchanged; rebuild `--release` so the running IDE
    carries the change.
  - Verify: full sweep summary posted; version/changelog in the diff. Do
    **not** merge/push/announce without the operator's go-ahead (feature →
    f=96 post, drafted for approval).

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and
the change is a **feature-only** commit series on `feat/multi-form-host` per
the operator's rules (no fix/feature mixing; do **not** merge/push/announce
without the operator's explicit go-ahead).
