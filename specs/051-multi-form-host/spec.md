<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Multi-form host

- **Status:** **approved** (operator, 2026-08-15)
- **Folder:** specs/051-multi-form-host/
- **Author:** Claude (operator: Emerson Lopes)   **Date:** 2026-08-15

## 1. Overview

Two ways of opening a second form exist in the product's surface today, and
neither performs. A sidebar/menu item whose action is `open-form:<NAME>`
(spec 049) should load that form into the application shell's ContentPane; a
COBOL `INVOKE me "OpenFormSync"/"OpenFormAsync"` (spec 037) should open it as
its own window. Both routes are fully parsed, semantically checked, routed —
and both converge on `HostAction::SpawnWindow`, which logs "child windows are
not hosted yet" and releases the handle so callers never deadlock.

Everything around the gap is built and is reused, not rebuilt: the
`FormSupervisor` (handles, the main-form singleton, modal/veto/close rules,
cross-form property publishing), the semantic load-path checks over
`FormFormat` (`Standalone` / `Embedded` / `Both`), the shell's menu-click
routing, the ContentPane with breadcrumbs, `preserve_previous_form` and the
`onActivate`/`onDeactivate` lifecycle, and the embedded `FORMS` table (every
form's design is already inside the compiled binary).

What is genuinely missing, and what this feature builds: the compiled binary
carries only the MAIN form's **program** (one `program.bin`), so a second
form's event handlers do not exist at runtime; nothing ever creates a second
interpreter; and `SpawnWindow` performs nothing for either surface.

**Settled by the operator (2026-08-15):**

1. Both doors in one spec.
2. Each loaded form runs as its **own program with its own WORKING-STORAGE on
   its own interpreter** — forms communicate only through the supervisor's
   published property surface, never by reading each other's data items.
3. A **third door**: the sidebar menu gains two new actions — **"Open Stand
   Alone Form (Sync)"** and **"Open Stand Alone Form (Async)"** — that open
   the target form in its own window, in the same process, with the shell as
   its parent.
4. The SideMenu control gains two **methods** — `OpenStandAloneFormSync` and
   `OpenStandAloneFormAsync` — the programmatic twin of those actions.
5. Target lists are **filtered, not just warned**: the "Open form" action
   offers only embeddable forms (`Embedded`/`Both`); the standalone actions
   and methods accept only `Standalone`/`Both` forms.
6. **Sync is implicitly modal** — from a menu click as from COBOL, a
   Sync-opened child is modal over its parent, no separate switch.

## 2. Goals / Non-goals

- **Goals:**
  - A compiled application (and every host that routes through the shared form
    host) can hold more than one live form at a time.
  - The shell's `open-form:<NAME>` menu action loads the named form into the
    ContentPane, with the 049 lifecycle honoured.
  - `OpenFormSync`/`OpenFormAsync` open a standalone child window, with the
    037 handle/modal/veto/close semantics honoured.
  - The two new menu actions and the two new SideMenu methods open standalone
    child windows parented to the shell, from a menu click or from COBOL.
  - The menu editor's Target picker shows only forms whose `FormFormat`
    matches the chosen action.
  - Each loaded form is an isolated program instance: own storage, own
    interpreter, own event loop.
- **Non-goals:**
  - No cross-form *data-item* visibility (`FORM-B`'s handlers reading
    `FORM-A`'s WORKING-STORAGE) — communication is the supervisor surface
    only.
  - No new COBOL verbs; the only language-surface additions are the two
    SideMenu methods.
  - **MenuBar** items do not gain the standalone actions in this spec — the
    menu editor is shared between MenuBar and SideMenu (one modal, keyed by
    control id), so the editor must show the new options **only when editing
    a SideMenu's menu**. A MenuBar equivalent is future work.
  - Multi-form support in the designer **Preview** surface (canvas preview
    stays single-form; Run Form and the compiled binary are the target
    hosts).
  - Out-of-process forms, remote forms, or more than one OS *process*.

## 3. User stories

- As a COBOL developer who built shell applications in PowerCOBOL, I want a
  sidebar item marked `open-form:CRM` to actually display the CRM form in the
  content area, so my application navigates the way the designer promised.
- As a COBOL developer, I want `INVOKE me "OpenFormSync" USING "DETAIL" …` to
  open the detail window and block until the user closes it, so my
  master–detail flow works the way spec 037 documents it.
- As an application designer, I want a sidebar item that opens a report form
  in its **own window** (not the content pane), so reference windows can stay
  open while the user keeps navigating the shell.
- As a COBOL developer, I want
  `INVOKE SideMenu-1 "OpenStandAloneFormAsync" USING "MONITOR"` to open a
  monitor window programmatically, so window management isn't limited to menu
  clicks.
- As a form designer, I want the Target picker to offer only forms that can
  legally serve the action I chose, so I cannot wire a navigation that the
  build will reject.
- As a developer with a costly search screen, I want
  `preserve_previous_form` to keep that screen's state alive when the user
  navigates away and back, so it does not rebuild from scratch each time.

## 4. Requirements (EARS)

### Build — the programs must exist in the binary

- **R1 (ubiquitous):** The compiler shall embed, alongside each form's design,
  that form's **program** (its generated `.cbl`, parsed and serialised the
  same way as the main program's), for every project form whose `FormFormat`
  permits it to be loaded at all — so a target form's event handlers exist in
  the compiled binary. The main program remains the entry point.
- **R2 (constraint):** Embedding shall not change the single-form fast path: a
  project with one form produces a binary whose startup, behaviour and
  observable output are unchanged.

### Runtime — isolation model

- **R3 (ubiquitous):** Each loaded form shall run as its **own program
  instance**: its own WORKING-STORAGE, its own interpreter, its own event
  loop, seeded from its own design exactly as the main form is today.
- **R4 (constraint):** A form's handlers shall not read or write another
  form's data items. Cross-form communication is only the supervisor surface
  already specified: published form properties (049 R30/R33), `super::X`,
  `handle::"SetProperty"`/`"GetProperty"`, and windowHandler methods
  (037 R20/R23).
- **R5 (ubiquitous):** File and database access from two live forms shall
  behave as two independent programs in one process — the existing in-process
  record-locking and status rules apply unchanged; nothing is newly shared.

### Standalone door (037)

- **R6 (event):** When the supervisor emits `SpawnWindow` for a target whose
  `FormFormat` allows standalone, the host shall create a child window hosting
  that form and its interpreter, honouring the caller's
  `window_state`/`x`/`y`/`width`/`height` overrides (037 R21) and the target's
  designed window properties otherwise.
- **R7 (state):** While a Sync child opened with `modal` true is open, the
  caller shall remain blocked and the reply held until the child closes
  (037 R28); Async is never modal (037 R20).
- **R8 (event):** When a child form closes, every interpreter's windowHandler
  data-items referring to it shall become NULL (037 R24); close vetoes
  (037 R17/R19) still apply, including to children at application close.
- **R9 (event):** When `OpenForm*` targets a form that is already open, the
  system shall keep the supervisor's existing registry semantics — focus the
  running instance where the singleton rule applies, rather than spawning a
  duplicate.

### Embedded door (049)

- **R10 (event):** When a menu item whose action is `open-form:<NAME>` is
  activated and `NAME`'s `FormFormat` allows embedding, the shell shall load
  that form — its own program instance per R3 — into the ContentPane as its
  sole occupant (049 R11), replacing the current occupant per 049 R25 and
  firing `onDeactivate` on the outgoing and `onActivate` on the incoming form
  (049 R26).
- **R11 (state):** While an outgoing occupant's menu item carried
  `preserve_previous_form`, that form's interpreter and storage shall stay
  resident and be reattached — not re-initialised — when navigated back to
  (049 R24/R25); without it, the outgoing form is torn down and fires
  `onDestroy`.
- **R12 (event):** When the pane occupant changes, the breadcrumb shall
  reflect the shell's navigation chain (049 R14), exactly as the chrome
  already draws it for the main form.

### Both doors

- **R13 (ubiquitous):** The behaviour shall be the same in every host that
  routes through the shared form host (042 R1/R3): the compiled binary,
  `rcrun run-form`, and the IDE's Run Form.
- **R14 (constraint):** The system shall not change compile-time gating: a
  menu item loading a `Standalone` form, or `OpenForm*` targeting an
  `Embedded` form, remains the compile error it is today (049 R17).
- **R15 (constraint):** The stub's honesty rule survives in reverse: once this
  ships, no path may silently drop an open request. A request that cannot be
  satisfied (unknown form id at runtime, spawn failure) shall produce a
  visible runtime error, not a log line and a released handle.

### Third door — menu-driven standalone (new, 2026-08-15)

- **R16 (ubiquitous):** The menu editor's Action combo shall offer, **when
  the edited menu belongs to a SideMenu control**, two additional options:
  **"Open Stand Alone Form (Sync)"** and **"Open Stand Alone Form (Async)"**
  (English labels; all six languages via new `Tr` fields). The existing three
  options are unchanged, and a MenuBar's editor shows only those three.
- **R17 (ubiquitous):** The new options shall persist as distinct action
  encodings in the menu YAML, mirroring the existing scheme:
  `open-standalone-sync:<NAME>` and `open-standalone-async:<NAME>`, where
  `<NAME>` is the target form's file stem exactly as `open-form:` records it.
- **R18 (event):** When a sidebar item carrying a standalone action is
  activated at runtime, the shell shall open `<NAME>` as a standalone child
  window — the same spawn path as R6, with the **shell (root form) as the
  caller/parent** — running as its own program instance per R3.
- **R19 (state):** While a **Sync**-opened child window is open, it shall be
  modal over the shell (shell input blocked until the child closes), matching
  037's Sync default; an **Async**-opened child is modeless and the shell
  stays interactive. In both cases the child closes with the shell at
  application close, subject to the 037 veto rules (R8).
- **R20 (constraint):** The "Preserve previous form" option shall remain
  exclusive to the `open-form` (embedded) action — a standalone open replaces
  no pane occupant, so the editor shall not offer it for the new actions.

### SideMenu methods (new, 2026-08-15)

- **R21 (ubiquitous):** The SideMenu control shall expose two new methods,
  `OpenStandAloneFormSync` and `OpenStandAloneFormAsync`, invocable from
  COBOL on the control (e.g.
  `INVOKE SideMenu-1 "OpenStandAloneFormAsync" USING "MONITOR"`), with the
  same parameter surface as `OpenFormSync`/`OpenFormAsync` (037 R21: form id,
  then optional windowState/x/y/width/height, Sync adding modal; the comma
  form accepts the form id alone). The opened window's parent is the shell,
  exactly as R18.
- **R22 (ubiquitous):** `OpenStandAloneFormAsync` shall support `RETURNING` a
  windowHandler that participates fully in the 037 handle lifecycle
  (methods, property access, NULL on close). `OpenStandAloneFormSync` blocks
  until the child closes and returns no live handle (037 R28/R24 precedent).
  *Note for `/plan`: the interpreter registers RETURNING handles only for
  method names starting `OPENFORM` — that gate must be widened.*
- **R23 (constraint):** The new method names shall be dispatched as
  **methods, never as property access**. The interpreter's unknown-method
  fallback currently reinterprets unrecognised names as property writes; the
  new names must be recognised on every dispatch path (statement `INVOKE`
  and the inline `::` member form) before that fallback can swallow them.
- **R24 (ubiquitous):** The semantic pass shall validate the new methods
  exactly as it validates `OpenForm*`: full arity and literal types for the
  space form (037 R22 table extended with the two new signatures), the comma
  form exempt, and the load-path check applied (R26).

### Target filtering (new, 2026-08-15)

- **R25 (ubiquitous):** The menu editor's Target picker shall **filter, not
  warn**: for "Open form" it lists only forms whose `FormFormat` allows
  embedding (`Embedded`/`Both`); for the two standalone actions it lists only
  forms whose format allows standalone (`Standalone`/`Both`). A form whose
  format cannot be determined (unreadable `.cfrm`) appears in **both** lists
  — a guess never hides a form. The existing orange "⚠ Standalone" advisory
  styling is retired along with the mis-wiring it warned about.
- **R26 (ubiquitous):** Compile-time checks shall mirror the filter, both
  ways: a menu item whose standalone action targets a form that does not
  allow standalone is a build error (the mirror of 049 R17's embedded check,
  same error style); `OpenStandAloneForm*` with a literal target that does
  not allow standalone is a compile error (the existing `OpenForm*` gate
  extended to the new names); dynamic (data-item) targets remain a runtime
  concern, failing per R15.

## 5. Acceptance criteria

- [ ] AC1 — In a compiled two-form shell app, clicking a sidebar item with
      `open-form:CRM` displays the CRM form in the ContentPane, and clicking a
      button on CRM runs **CRM's** handler (observable: the handler writes to
      a CRM label). The outgoing form fires `onDeactivate`, CRM fires
      `onActivate`. (R1, R3, R10)
- [ ] AC2 — `OpenFormAsync("DETAIL")` opens the DETAIL window and the caller
      continues; `OpenFormSync("DETAIL")` with modal default blocks the
      caller's handler until DETAIL closes; after close, the stored handle is
      NULL and invoking through it raises the 037 R24 runtime error. (R6–R8)
- [ ] AC3 — Navigating away from a `preserve_previous_form` occupant and back
      finds its edited TextBox content intact; the same round trip without the
      flag finds the design-time defaults and a fresh `onCreate`/`onDestroy`
      pair. (R11)
- [ ] AC4 — A single-form project builds and runs byte-identically to
      1.61.49 behaviour: same startup window, same close path, same effects.
      (R2)
- [ ] AC5 — Form A sets a published property on form B through its handle
      (`handle::"SetProperty"`), and B's own `me::X` read returns the new
      value; B's data items are not visible to A by name in any scope. (R4)
- [ ] AC6 — The existing compile errors for format violations are unchanged
      (semantic test suites stay green with no assertions edited). (R14)
- [ ] AC7 — Killing neither door: with the feature present, the old stderr
      stub lines are gone, and a runtime open failure surfaces as a visible
      error. (R15)
- [ ] AC8 — The same two-form scenario as AC1 runs identically under
      `rcrun run-form` and IDE Run Form. (R13)
- [ ] AC9 — Editing a **SideMenu** menu shows five Action options; editing a
      **MenuBar** menu shows the original three. Choosing "Open Stand Alone
      Form (Async)" and a target persists
      `open-standalone-async:<stem>` in the menu YAML, and re-opening the
      editor restores the selection. (R16, R17)
- [ ] AC10 — At runtime, a sidebar item with `open-standalone-async:REPORT`
      opens REPORT in its own window while the shell stays interactive; the
      Sync variant blocks shell input until the child closes; closing the
      shell closes surviving children. (R18, R19)
- [ ] AC11 — `INVOKE SideMenu-1 "OpenStandAloneFormAsync" USING "MONITOR"
      RETURNING WS-H` opens the window and `WS-H` drives `Focus`/`Close`;
      after close `WS-H` is NULL. The Sync method blocks the handler until
      the child closes. The names are never misread as property writes — a
      project with a property of the same name is unaffected. (R21–R23)
- [ ] AC12 — With a project holding an `Embedded`-only form E and a
      `Standalone`-only form S: the "Open form" Target picker lists E but not
      S; the standalone pickers list S but not E; a `Both` form appears in
      each. (R25)
- [ ] AC13 — A menu YAML hand-edited to `open-standalone-sync:E` (E is
      `Embedded`) fails the build with a format-mismatch error naming the
      item and the form; `INVOKE SideMenu-1 "OpenStandAloneFormSync" USING
      "E" …` fails semantic analysis the same way. (R26)
- [ ] AC14 — The space-form signature check applies to the new methods: a
      call with a wrong-typed literal argument is a compile error listing the
      expected signature; the comma form with only the form id passes. (R24)

## 6. Constraints & steering check

- **i18n (6 languages):** **required.** Two new Action-combo labels ("Open
  Stand Alone Form (Sync)" / "(Async)") are new `Tr` fields in all six
  languages. The existing combo labels are already `Tr` fields, so the
  pattern is established. *(Noted in passing: the menu editor holds a few
  pre-existing hard-coded literals — "(select form)", "Item properties" — not
  in this spec's scope.)*
- **Generated-code contract:** unchanged. Forms' `.cbl` files are already
  generated and regenerated on Build/Run/Debug/Check; the compiler consumes
  more of them (R1) but their content and banner are untouched.
- **System KB:** **required.** The SideMenu control currently publishes *no*
  type-specific methods table — it gains one (the two new methods), plus the
  methods-reference section, plus updates to the 037 prose and any "not yet
  hosted" notes; `chunked.data` rebuilt in the same change
  (`cargo run -p cobolt-ide --example build_chunked_kb`). The KB's
  closed-vocabulary rule makes R23 doubly important: an undocumented method
  is treated as a property.
- **Docs (English guide):** required. The guide's form-lifecycle and shell
  sections must document all three doors and the two methods; translations
  are user-maintained and not touched.
- **Fix vs feature:** **feature** — capability beyond the current scope
  (the stub was an honest "not yet", not a regression). Announced on **f=96**
  after merge to main; version bump is the fix number `z` only, per the
  operator's standing rule.
- **Tech constraints:** egui 0.36 multi-viewport via
  `ctx.show_viewport_immediate`; MSRV 1.92; the "one interpreter per process"
  comment in the compiled template is retired by design (see Q1).

## 7. Open questions

- **Q1 — EXEC RUST object bridge scope.** Spec 041/042 promise "one object
  bridge per process, so every block sees the same state". With one
  interpreter per form, is the bridge (a) still process-wide — blocks in any
  form share state, preserving the 041 contract verbatim — or (b)
  per-interpreter, matching the storage-isolation model? **Recommendation:
  (a) process-wide**, because it keeps the documented 041 contract and gives
  EXEC RUST users a deliberate shared channel that COBOL storage no longer
  provides. To be settled in `/plan`.
- **Q2 — Interpreter thread lifetime for preserved occupants.** Does a
  `preserve_previous_form` occupant's interpreter thread keep running (able
  to process timers) while off-pane, or is it parked and only its state kept?
  049 R26 implies the form object survives; the thread model is a `/plan`
  decision.
- **Q3 — Handle surface for embedded occupants.** 037 handles were specified
  for windows. Does an embedded occupant get a windowHandler-compatible
  handle (so `super`/handle property access is uniform, per 049 R30), with
  window-only methods erroring? **Recommendation: yes, uniform surface.**
- ~~**Q4 — Sync-from-menu modality.**~~ **Resolved (operator, 2026-08-15):
  Sync is implicitly modal** — R19 stands as written.
