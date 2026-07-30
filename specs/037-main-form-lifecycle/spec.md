<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Main form, window lifecycle & Sync/Async form invocation

- **Status:** draft
- **Folder:** specs/037-main-form-lifecycle/
- **Author:** Emerson Lopes (with Grace)   **Date:** 2026-07-29

## 1. Overview

A multi-form application needs one designated **main form** — the form shown
first, the app's single identity in the OS taskbar/dock, and the anchor of the
application's lifetime. Today no such designation exists, forms have no
lifecycle state, and one form cannot open another. This spec adds: the
MainForm designation (exactly one per project, crown-marked in the RAD),
taskbar identity (single entry + icon, main form only), main-form singleton
semantics, configurable window controls, programmatic WindowState, fullscreen
mode and title-bar visibility, a programmatic FormState (Ready/Waiting) that
guards against closing unsaved work, and two form-invocation methods —
`OpenFormSync` / `OpenFormAsync` — returning a `windowHandler` usage object
with defined close-cascade semantics.

## 2. Goals / Non-goals

- **Goals:**
  - Exactly-one main form per project, designer-enforced, visually marked;
    the first form created holds the role by default.
  - Runtime window identity: only the main form in the taskbar/dock; its
    icon is the app's identity.
  - Main-form singleton; unrestricted concurrent instances of other forms.
  - Per-form window controls (minimize/maximize/restore), programmatic
    WindowState, fullscreen mode (property + method + event), and title-bar
    visibility (property + method).
  - FormState (Ready/Waiting) with close-veto semantics and event.
  - Sync/Async invocation with optional-parameter rules, compile-time
    signature checking, and deterministic close cascades.
- **Non-goals:**
  - Inter-form data passing / return values beyond the windowHandler (a
    later spec; LINKAGE-style parameters are out of scope here).
  - Cross-process forms (all forms run inside one rcrun process).
  - Taskbar progress/badges/jump-lists.
  - Changing the existing single-form Run/Debug flow for projects that never
    call OpenForm*.

## 3. User stories

- As a COBOL developer, I want to mark one form as the main form with a
  checkbox, so the app starts there and shows a single taskbar identity.
- As a COBOL developer, I want `invoke me::"OpenFormSync"` /
  `"OpenFormAsync"` with optional parameters, so I can open child forms with
  the geometry and state I designed in the RAD, overriding only what I need.
- As a COBOL developer, I want to set a form's FormState to Waiting while it
  has unsaved edits, so the user cannot close it and lose work until my logic
  saves and sets it back to Ready.
- As a COBOL developer, I want kiosk-style fullscreen forms and chromeless
  (title-less) forms, switchable at runtime, so I can build dashboards and
  point-of-sale style screens.
- As an end user, I want secondary windows to not clutter the taskbar, and
  the app's single taskbar entry to carry a proper icon.

## 4. Requirements (EARS)

### Main-form designation

- **R1 (ubiquitous):** Every form shall have a boolean **MainForm** property,
  editable in the RAD Form properties as a checkbox and persisted in the
  `.cfrm`.
- **R2 (event):** When MainForm is set on a form, the system shall unset
  MainForm on every other form of the project in the same operation (one
  undoable action).
- **R3 (ubiquitous):** Each project shall have **exactly one** main form:
  the **first form created** in a project takes the role automatically and
  keeps it until the developer sets a different form as main; the designer
  shall prevent unchecking the last MainForm (the current main form's
  checkbox is read-only until another form takes the role); and opening a
  project with zero or multiple main forms shall normalise to exactly one —
  the **first form in the list** wins — with a status notice.
- **R4 (ubiquitous):** The Forms tree shall show a king's-crown icon in place
  of the plain form icon for the main form.
- **R5 (event):** When the application starts (Run/Debug/built binary), the
  system shall show the main form first.

### Taskbar / dock identity

- **R6 — removed** (ShortName dropped on 2026-07-29; the taskbar entry
  keeps the OS-default label — the main form's window title. Number kept so
  later references stay stable.)
- **R7 (ubiquitous):** At runtime, only the **main form's** window shall
  appear in the OS taskbar/dock — regardless of how many form windows are
  open.
- **R8 (constraint):** Non-main form windows shall not create taskbar/dock
  entries on any supported OS.
- **R9 (optional):** Where a **TaskbarIcon** image property is set on the
  main form, the taskbar/dock entry shall use it; the property is editable
  only on the main form (hidden/inert on other forms).

### Instances

- **R10 (constraint):** The main form shall be a **singleton**: while an
  instance is running, invoking OpenFormSync/Async targeting the main form
  shall not spawn a second instance — it shall focus the running instance and
  return its existing windowHandler.
- **R11 (ubiquitous):** Non-main forms shall support multiple concurrently
  running instances, each with its own windowHandler and independent state.

### Window controls & state

- **R12 (ubiquitous):** Every form shall have boolean **CanMinimize** and
  **CanMaximize** properties (default true) controlling the presence/enabled
  state of the native minimize and maximize/restore title-bar controls.
- **R13 (ubiquitous):** Every form shall have a **WindowState** property with
  values `Normal` | `Minimized` | `Maximized` (default Normal): the
  designer-set value is the state the window opens in, and runtime logic can
  set it at any time to minimize/maximize/restore the window
  programmatically.
- **R14 (ubiquitous):** Every form shall have a boolean **FullScreen**
  property (default false): the designer-set value makes the window open in
  fullscreen; runtime logic can enter/leave fullscreen at any time via the
  inline method `me::"SetFullScreen"(bool)` (also on the windowHandler,
  R23); every actual change fires an **onFullScreenChanged** event on the
  form carrying the new state. FullScreen is **orthogonal** to WindowState
  (R13): it is a flag over the current state, and leaving fullscreen
  returns the window to its previous WindowState.
- **R15 (ubiquitous):** Every form shall have a boolean **TitleVisible**
  property (default true — title shown): when false the window renders
  without its title bar (chromeless); runtime logic can show/hide it at any
  time via the inline method `me::"SetTitleVisible"(bool)` (also on the
  windowHandler, R23). While the title bar is hidden, the R12 title-bar
  controls are naturally unavailable; close protection (R17) still applies
  to every remaining close path.

### FormState lifecycle

- **R16 (ubiquitous):** Every form shall have a **FormState** property with
  values `Ready` | `Waiting`; the design-time value is always Ready; it is
  settable only programmatically at runtime.
- **R17 (state):** While FormState = Waiting, the system shall refuse every
  close attempt on the form — user close (title-bar button, keyboard),
  windowHandler Close, and cascade closes — and shall fire an
  **onCloseRejected** event on the refused form.
- **R18 (state):** While FormState = Ready, the form shall be closable at
  any time.
- **R19 (state):** While any of a Sync caller's open Sync children has
  FormState = Waiting, the caller itself shall refuse to close and fire
  onCloseRejected on the caller.

### Invocation

- **R20 (ubiquitous):** Forms shall expose the inline methods
  `me::"OpenFormSync"("form id", [windowState], [x], [y], [width], [height],
  [modal]) returning windowHandler` and `me::"OpenFormAsync"("form id",
  [windowState], [x], [y], [width], [height]) returning windowHandler`
  (Async is never modal). `windowHandler` is a data-item with USAGE OBJECT.
- **R21 (event):** When the argument list is **comma-separated**, trailing
  parameters may be omitted; each omitted parameter shall default to the
  target form's RAD-designed property (WindowState, X, Y, Width, Height),
  and `modal` shall default to **true**.
- **R22 (event):** When the argument list uses **space-separated**
  COBOL-standard syntax, all parameters shall be required, and the compiler
  shall report a compile-time error when the call does not conform to the
  method signature (count or type).
- **R23 (ubiquitous):** The windowHandler shall support: `Close`, `Focus`,
  `SetWindowState(state)`, `SetFullScreen(bool)`, `SetTitleVisible(bool)`,
  and reading the child's `FormState`; all subject to the same lifecycle
  rules (e.g. Close on a Waiting child is refused, R17).
- **R24 (event):** When a form closes, the windowHandler data-items referring
  to it shall be set to **NULL** automatically.

### Close cascades

- **R25 (event):** When a Sync caller closes, all its Sync-opened child forms
  shall close with it (subject to R17/R19), and their handles become NULL
  (R24).
- **R26 (ubiquitous):** Async-opened forms shall survive their caller's
  close.
- **R27 (event):** When the **main form** closes, every open form —
  Sync or Async — shall close and the application shall exit (Waiting forms
  still veto per R17; the main-form close is then refused with
  onCloseRejected on the blocking form).
- **R28 (state):** While a Sync child is open with `modal` true, the caller's
  window shall not accept user input (modal blocking); the caller's COBOL
  flow resumes when the modal child closes.

## 5. Acceptance criteria

- [x] AC1 — Checking MainForm on form B unchecks it on form A in the same
  undoable action; undo restores A. (R1, R2) *(machine-verified: designer
  claim/undo/redo + file-settlement tests, 2026-07-29)*
- [x] AC2 — The first form created in a fresh project is automatically the
  main form; the current main form's checkbox cannot be unchecked directly;
  a project opened with zero/two main forms normalises to the first form in
  the list and reports it in the status/output. (R3) *(machine-verified:
  4 normalize_main_form tests, 2026-07-29; checkbox read-only by
  construction)*
- [ ] AC3 — The Forms tree shows the crown icon exactly on the main form,
  and it moves immediately when the designation changes. (R4)
  *(implemented; awaiting operator visual check)*
- [ ] AC4 — Run and built binary open the main form first. (R5)
  *(OPEN GAP: the IDE's Run Form action is per-designer today and the
  compiled binary's entry-form selection was not changed — R5 needs a
  follow-up task alongside the multi-viewport host)*
- [ ] AC5 — With three forms open, the OS taskbar/dock shows one entry;
  setting TaskbarIcon changes its icon; the TaskbarIcon row is absent/inert
  on non-main forms. (R7–R9) *(TaskbarIcon property + main-only row +
  root-window icon done; the three-windows scenario rides the pending
  multi-viewport host)*
- [x] AC6 — OpenFormAsync on the main form while it runs focuses it and
  returns the existing handle (instance count stays 1); the same call on a
  normal form spawns a second independent instance. (R10, R11)
  *(machine-verified headless: supervisor + end-to-end invoke tests,
  2026-07-29; visible-window half rides the pending multi-viewport host)*
- [ ] AC7 — CanMinimize=false / CanMaximize=false remove/disable the
  corresponding title-bar buttons; setting WindowState from COBOL minimizes,
  maximizes, and restores the window. (R12, R13) *(implemented end-to-end;
  awaiting operator visual check)*
- [ ] AC8 — A form with FullScreen=true opens fullscreen;
  `me::"SetFullScreen"` enters/leaves fullscreen at runtime and each actual
  change fires onFullScreenChanged with the new state (no event when the
  state didn't change). (R14) *(implemented — event from ACTUAL
  ViewportInfo transitions, tracker seeded with designed value; awaiting
  operator visual check)*
- [ ] AC9 — A form with TitleVisible=false opens without a title bar;
  `me::"SetTitleVisible"` hides/shows it at runtime; with the title hidden a
  Waiting form still cannot be closed through any remaining path. (R15,
  R17) *(implemented; awaiting operator visual check)*
- [x] AC10 — Setting FormState=Waiting makes the title-bar close, handle
  Close, and cascade close all no-ops that fire onCloseRejected; setting
  Ready lets the same close succeed. (R16–R18) *(machine-verified:
  form_state_lifecycle supervisor tests; title-bar path wired through
  try_close + CancelClose in the run-form host, 2026-07-29)*
- [x] AC11 — A Sync caller with a Waiting Sync child refuses to close and
  fires onCloseRejected on the caller; once the child is Ready, closing the
  caller closes the child too and the child's handle reads NULL. (R19,
  R24, R25) *(machine-verified: supervisor cascade tests, 2026-07-29)*
- [x] AC12 — Comma form: `invoke me::"OpenFormSync"("F2") returning H`
  opens F2 with RAD geometry/state, modal true. Comma form with overrides
  honours them. Space form with a missing parameter fails compilation with
  a signature error naming the method. (R20–R22) *(machine-verified:
  3 invoke + 4 signature tests, 2026-07-29)*
- [x] AC13 — Handle methods Close/Focus/SetWindowState/SetFullScreen/
  SetTitleVisible work and respect lifecycle rules; after child close the
  handle is NULL and invoking through NULL raises the standard runtime
  error. (R23, R24) *(machine-verified: window_commands + NULL-invoke
  tests, 2026-07-29)*
- [x] AC14 — Closing an Async caller leaves its Async children open;
  closing the main form closes everything and exits (unless a Waiting form
  vetoes). (R26, R27) *(machine-verified: form_cascades tests, 2026-07-29)*
- [ ] AC15 — A modal Sync child blocks mouse/keyboard input to the caller
  window until closed; the caller's paragraph resumes after the child
  closes. (R28) *(flow-blocking machine-verified — measured ≥120 ms block
  until child close; INPUT blocking rides the pending multi-viewport
  host)*

## 6. Constraints & steering check

- **i18n (6 languages):** yes — new RAD property labels (MainForm,
  TaskbarIcon, CanMinimize, CanMaximize, WindowState, FullScreen,
  TitleVisible), the normalisation status notice, and any designer tooltips
  must be `Tr` fields translated in EN/ES/PT/JA/ZH/FR. COBOL-facing names
  (property/method/event identifiers, `Ready`/`Waiting`) stay English per
  steering.
- **Generated-code contract:** yes — generated `.cbl` gains the new form
  properties and event paragraphs (onCloseRejected, onFullScreenChanged);
  banner + regenerate-on-Build/Run/Debug/Check contract unchanged.
- **Docs:** English developers-guide section required (main form, FormState,
  fullscreen and title visibility, OpenFormSync/Async with both syntaxes and
  defaulting rules, windowHandler, close cascades). Translations untouched
  (user-maintained).
- **Fix vs feature:** **feature** → minor version bump (`y`), own commit(s),
  forum announcement in f=96 with `[Noticia]` prefix, title ≤ 50 chars,
  posted within the operator's push window.
- **Compile-time check (R22):** lands in the parser/semantic layer
  (`cobolt-parser` / `cobolt-semantic`); runtime dispatch in
  `cobolt-runtime`; window/viewport behaviour in `cobolt-forms` +
  `cobolt-ide` run-form host; multi-viewport per egui 0.35
  (`show_viewport_immediate`) — plan must respect tech.md.

## 7. Open questions

- Q1: When the main form is **deleted** in the RAD, auto-assign the first
  remaining form (with notice) or block deletion until another form is made
  main? *(Proposed: auto-assign + notice, mirroring R3 normalisation.)*
- Q2: TaskbarIcon formats — reuse the existing image-asset pipeline
  (PNG/JPEG/SVG)? Any OS-specific size constraints worth documenting?
- Q3: Does `Focus` on a minimized child restore it first, or only request
  attention? *(Proposed: restore + focus.)*
- Q4: OS caveat to confirm during /plan: macOS gives one Dock entry per
  process by definition — R7/R8 are naturally satisfied; Windows/Linux need
  explicit skip-taskbar on child viewports. Any behaviour difference to
  document in the guide?
- ~~Q5: FullScreen × WindowState interplay~~ — **Resolved 2026-07-29:**
  FullScreen is orthogonal to WindowState (folded into R14).
