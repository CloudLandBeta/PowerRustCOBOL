<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Main form, window lifecycle & Sync/Async form invocation

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-07-29

## 1. Approach

Grounding facts (verified in code):

- Form-level state is **struct fields** on `Form`
  (`cobolt-forms/src/model.rs:3983`), serialized as XML attributes on
  `<Form>` by `cobolt-forms/src/xml.rs` — not the controls' property map.
- A run-form is **one external `rcrun run-form` process**
  (`app.rs:1305`, `cobolt-cli/src/form_gui.rs`): one eframe window
  (`FormApp`), the interpreter on its own thread, joined by
  `event_tx`/`state_rx`/`display` mpsc channels.
- Inline methods are `Stmt::Invoke`/`Expr::Member` →
  `Interpreter::exec_method(object, method, args)`
  (`interpreter.rs:5823`); objects are seeded via `seed_objects`.
- The classic `INVOKE obj "Method" USING a b c` form (space-separated,
  `stmt.rs:2563`) and the inline `obj::"Method"(a, b)` form (comma-
  separated) are **already two distinct syntaxes** — they map 1:1 onto the
  spec's required/optional parameter rules (R21/R22).

Design per requirement group:

- **Main form (R1–R5).** New `Form` fields (§3). The properties panel Form
  section gains a MainForm checkbox; ticking it emits a new
  `InspectorAction` variant handled at **app level** (the app owns every
  form), which sets the flag on the target and clears it on the previous
  holder, saving both `.cfrm`s as one undoable unit (§4 D2). Project open
  and form creation run a `normalize_main_form(&mut project)` helper in
  `cobolt-ide`: first form created ⇒ main; zero/many mains on load ⇒ first
  in the project's `forms = [...]` list wins + status notice. The Forms
  tree (`panels/project.rs`, `tree_icon`) draws a new `draw_crown_icon`
  vector stroke for the main form. Run/Build starts with the main form
  (R5) — the project run action targets it by default.
- **Taskbar identity (R7–R9; R6 removed).** New field `taskbar_icon`.
  The main form's process keeps today's taskbar presence and sets its icon
  from TaskbarIcon (`ViewportBuilder::with_icon`, form_gui.rs:566 already
  loads one); the entry's label is the OS default (the main form's window
  title). Child viewports are created with `with_taskbar(false)`
  (Windows/Linux skip-taskbar; macOS child viewports of one process never
  get their own Dock entry) — R7/R8. TaskbarIcon row rendered only when
  `form.main_form` (R9).
- **Instances (R10–R11).** All forms opened via OpenForm* run **inside the
  caller's rcrun process** as egui viewports (tech.md:
  `show_viewport_immediate`). A `WindowRegistry` in the form host maps
  `handle id → (form name, viewport id, interpreter thread, kind
  Sync/Async, caller handle, modal, FormState mirror)`. The registry
  enforces the main-form singleton: an OpenForm* targeting the main form
  returns the existing handle + `ViewportCommand::Focus`.
- **Window controls & state (R12–R15).** Map to egui viewport API:
  CanMinimize/CanMaximize → `ViewportBuilder::with_minimize_button` /
  `with_maximize_button`; WindowState → `ViewportCommand::Minimized` /
  `Maximized(bool)`; FullScreen → `ViewportCommand::Fullscreen(bool)`
  (orthogonal per R14 — restore previous WindowState on exit);
  TitleVisible → `with_decorations` / `ViewportCommand::Decorations`.
  Runtime changes flow interpreter → `StateUpdate` (form-scope props) →
  host applies the viewport command; `onFullScreenChanged` fires from the
  host on the **actual** viewport state change (egui reports it in
  `ViewportInfo`), not on the request.
- **FormState (R16–R19).** `FormState` lives on the form object in the
  interpreter (`obj_set/obj_get`), mirrored to the host via `StateUpdate`.
  All close paths converge on one host function `try_close(handle)`:
  Waiting ⇒ veto + queue `onCloseRejected` UiEvent to that form's
  interpreter; Sync caller check walks its registry children (R19).
  `close_requested()` in each viewport calls `try_close` instead of
  closing directly (same pattern as form_gui.rs:789's quit guard).
- **Invocation (R20–R24).** `exec_method` gains `OPENFORMSYNC` /
  `OPENFORMASYNC` on the form's own object (`me` — §4 D4). The
  interpreter does not own windows; it sends a `FormRequest` over a new
  channel to the host, which loads the target `.cfrm` + its generated
  program, spawns the child interpreter thread, creates the viewport, and
  returns the handle id. The returning data-item stores an object
  reference (`USAGE OBJECT` exists: `parser.rs:33`,
  `interpreter.rs:301`); handle methods (R23) dispatch through the
  existing `object_refs` route in `exec_method`. On child close the host
  broadcasts handle-NULL to every interpreter (R24). Space-form INVOKE
  signature checking (R22) lands in `cobolt-semantic` against a small
  built-in signature table for the two methods (count + literal/numeric
  type of each arg).
- **Cascades (R25–R28).** All in the host registry: caller close ⇒
  `try_close` each Sync child first (any veto aborts the whole close);
  Async children detach; main-form close ⇒ `try_close` everything, veto
  aborts; modal Sync child ⇒ host swallows input to the caller viewport
  while the child lives (egui: don't forward events / overlay blocker in
  the caller viewport), and the caller's interpreter thread blocks in
  `OPENFORMSYNC` with `modal=true` until the child closes (R28 "flow
  resumes").

## 2. Affected crates / files

- `crates/cobolt-forms/src/model.rs` — new `Form` fields + defaults.
- `crates/cobolt-forms/src/xml.rs` — save/load the new attributes
  (backward-compatible defaults for old `.cfrm`).
- `crates/cobolt-cli/src/form_gui.rs` — multi-viewport host:
  `WindowRegistry`, `try_close`, viewport spawn/command plumbing, modal
  input blocking, `FormRequest` channel, fullscreen-change detection.
- `crates/cobolt-runtime/src/interpreter.rs` — `OPENFORMSYNC/ASYNC`,
  handle methods, `FormState` set/get + veto events, handle-NULL
  propagation, form-request channel seam (host-agnostic trait or channel
  pair so tests can run headless).
- `crates/cobolt-semantic/src/lib.rs` — space-form INVOKE signature check
  for the two methods (compile-time errors, R22).
- `crates/cobolt-codegen/src/…` — emit the new event paragraphs
  (onCloseRejected, onFullScreenChanged) like onLoad/onClose; form
  property constants into generated WS where applicable.
- `crates/cobolt-compiler/src/lib.rs` — property-docs table entries for
  the new properties/methods/events (KB text).
- `crates/cobolt-ide/src/panels/properties.rs` — Form section: MainForm
  checkbox (read-only when already main), TaskbarIcon (main-only),
  CanMinimize/CanMaximize, WindowState, FullScreen, TitleVisible rows.
- `crates/cobolt-ide/src/panels/project.rs` — `draw_crown_icon` +
  main-form branch in the tree.
- `crates/cobolt-ide/src/app.rs` — MainForm reassignment action (+ undo
  pair), `normalize_main_form` on open/create/delete, run starts at main
  form.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields ×6.
- `crates/cobolt-agents/…` knowledge docs — regenerate chunked KB after
  compiler property-docs change (existing pipeline).
- `docs/developers-guide-en.md` — new "Multi-form applications" section.
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` — minor bump at
  /implement completion.

## 3. Data / model changes

`Form` gains (with serde/XML defaults preserving old files):

| Field | Type | Default | .cfrm attribute |
|---|---|---|---|
| `main_form` | bool | false | `main-form` |
| `taskbar_icon` | String | "" | `taskbar-icon` |
| `can_minimize` | bool | true | `can-minimize` |
| `can_maximize` | bool | true | `can-maximize` |
| `window_state` | enum Normal/Minimized/Maximized | Normal | `window-state` |
| `full_screen` | bool | false | `full-screen` |
| `title_visible` | bool | true | `title-visible` |

`FormState` is **not** persisted (design-time always Ready, R16) — runtime
object property only. `windowHandler` is a `USAGE OBJECT` reference whose
string form is an opaque handle id; NULL is the existing null object value.
Old `.cfrm` files load with all defaults ⇒ rendering identical; projects
with no `main-form` normalise per R3 (write-back happens on next save, not
on open).

## 4. Key decisions & alternatives

- **D1 — Children as viewports in the caller's process**, not one process
  per form. Why: handles, modality, cascades, and the singleton need
  shared state; cross-process window control is OS-specific pain. Rejected:
  spawning `rcrun` per child (today's IDE↔run-form model) — no shared
  registry, no modal blocking, taskbar control impossible. The IDE's own
  "Run form" flow is unchanged (non-goal).
- **D2 — MainForm reassignment is an app-level action**, not a designer
  `Cmd`. Why: it touches two forms; the designer undo stack
  (`designer.rs:1213`) is per-form. The action records `(previous_main,
  new_main)` and registers an undo entry that swaps back both — one
  undoable operation (R2). Rejected: per-designer Cmd (can't see the other
  form); no undo (violates R2/AC1).
- **D3 — Two syntaxes reuse the existing parse forms.** Comma-optional =
  inline `me::"OpenFormSync"(…)` member call; space-required = classic
  `INVOKE me "OpenFormSync" USING …`. Why: both parsers exist; the
  distinction is exactly the spec's rule; zero new grammar. The semantic
  layer only validates the INVOKE form (R22); the inline form fills
  defaults at runtime (R21).
- **D4 — `me` is a seeded object alias** for the form's own object (the
  host seeds `ME → <form name>` per interpreter). Why: `exec_method`
  dispatches by object name already; no parser change. Rejected: parser
  keyword (needless grammar).
- **D5 — Fullscreen event from viewport state, not from the setter.** Why:
  R14/AC8 require events only on actual changes (OS can refuse); egui
  exposes the real state in `ViewportInfo::fullscreen`. Rejected: firing in
  `exec_method` (double/false events).
- **D6 — no custom taskbar label** (ShortName removed from the spec,
  2026-07-29). The single taskbar entry keeps the OS-default label — the
  main form's window title — which every platform supports natively.

## 5. Risks & mitigations

- **Risk: egui multi-viewport limits** — `show_viewport_immediate` renders
  children inside the parent's callback; deep Sync chains nest closures.
  → Mitigation: registry drives a flat iteration from the root each frame
  (children of closed callers are reparented per R24 before the next
  frame); spike task first (T1) proves 3-deep nesting + modal on macOS.
- **Risk: interpreter thread blocking on modal Sync** could deadlock if the
  child needs the caller's channel. → Mitigation: each child interpreter
  has its own channel set to the host; the caller blocks only on the
  host's `FormClosed(handle)` reply — the host never blocks.
- **Risk: taskbar platform variance** (Windows labels entries with window
  titles; macOS Dock shows the app/bundle name regardless). → Mitigation:
  implement the guaranteed parts (single entry, icon, no child entries)
  and document the exact per-OS behaviour in the guide (spec Q4 — answer
  lands in the docs task).
- **Risk: cross-form undo confusion** (undo pressed in form B's designer
  reverting a MainForm change made from form A). → Mitigation: the undo
  entry lives in the app-level action history tied to the properties panel
  (same surface where the change was made), with a status message naming
  both forms on undo.
- **Risk: handle-NULL propagation** across interpreters referencing a
  closed form. → Mitigation: host broadcast + interpreters nulling their
  `object_refs` entries on receipt; runtime test asserts NULL reads and
  the standard error on invoking through NULL (AC13).

## 6. Test strategy

- `cobolt-forms` (unit): `.cfrm` round-trip of all 7 new fields; old-file
  load ⇒ defaults (reports each field's persisted/parsed value).
- `cobolt-parser`/`cobolt-semantic` (unit): inline comma form with 1–7
  args parses; INVOKE space form with missing/typed-wrong arg produces the
  signature diagnostic naming the method (reports the diagnostic text).
- `cobolt-runtime` (integration, headless via the channel seam):
  OpenFormSync/Async return distinct handles; singleton returns the same
  handle; comma-form defaults filled from seeded form properties; modal
  Sync blocks the caller thread until child-close; FormState Waiting vetoes
  Close and fires onCloseRejected; Sync-caller veto with Waiting child;
  cascade close NULLs handles; Async survival; main-form close-all. Each
  test prints the observed event/handle sequence.
- `cobolt-ide` (unit): `normalize_main_form` — zero mains ⇒ first; two
  mains ⇒ first; first-created default; reassignment action undo restores
  both forms.
- **Manual/visual:** crown icon in the tree (and it moves on reassignment);
  MainForm checkbox read-only on the holder; TaskbarIcon row only on main;
  run a 3-form project — one taskbar entry, correct icon; CanMin/CanMax
  buttons; fullscreen in/out fires the event once per change; TitleVisible
  chromeless window; Waiting form refuses the title-bar close. (Operator
  drives the app; agent verifies via build + tests only.)

## 7. Steering compliance

- [ ] i18n: all new UI strings (property labels, tooltips, normalisation
  notice, undo status) as `Tr` fields in EN/ES/PT/JA/ZH/FR.
- [ ] Generated-code banner + regenerate-on-action contract preserved (new
  event paragraphs go through `cobolt-codegen`; `write_header` untouched).
- [ ] English dev guide updated; translations untouched.
- [ ] Feature → **minor** bump (`1.42.0`) + CHANGELOG; commits keep fixes
  and this feature separate; forum f=96 `[Noticia]`, ≤ 50 chars, push
  window respected.
- [ ] No "cobolt" in user-facing text; COBOL identifiers/properties/events
  stay English (`Ready`/`Waiting`, method names).

Open spec questions carried into tasks: Q1 (main-form deletion → proposed
auto-assign + notice), Q2 (TaskbarIcon formats → reuse image pipeline),
Q3 (Focus restores minimized → proposed yes), Q4 (per-OS taskbar caveats →
documented in the guide task).
