<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Form-host parity (Run Form and the built application behave as one)

- **Status:** draft
- **Folder:** specs/042-form-host-parity/
- **Author:** Claude (operator: Emerson Lopes)   **Date:** 2026-08-06
- **Classification:** **FIX** (f=97) — documented behaviour that does not happen
  (operator ruling, 2026-08-06). Technical debt, not new capability.

## 1. Overview

A designed form can be hosted by three hand-maintained implementations of the
same "form host":

| Host | Where | Reaches the user as |
|------|-------|--------------------|
| **A** — `rcrun run-form` | `crates/cobolt-cli/src/form_gui.rs` (~1 370 code lines) | Run Form (external process), CLI `rcrun run-form` |
| **B** — built application | Rust source **template** in `crates/cobolt-compiler/src/lib.rs` (`form_runtime_code`, ~430 code lines) | Every shipped binary; also what Run launches for a form with `EXEC RUST` blocks |
| **C** — IDE in-process viewport | `crates/cobolt-ide/src/form_runtime.rs` + `show_running_form_window` in `app.rs` | **Nothing** — dead code (see below) |

They have drifted, and the copy every end user receives — the built
application — is the poorest. The trigger report (operator, 2026-08-06) was
*"entrance/exit effects are not working in any case for any theme"*: spec 038
window effects exist only in host A, while the compiler parses the `[forms]`
effect settings into `#[allow(dead_code)]` fields whose own comment defers them
to "037/038 parity (tracked work item)". The full inventory (§ 8) is much
wider: in a built application the spec-037 window lifecycle is entirely absent
(no `onClose`/`onShow`/`onActivate`, no close vetoes, no `me::` window
methods), the window never closes when the program ends, the object registry is
never seeded (a property read before the first write returns nothing instead of
the designed value), the designed window title/icon/state/position are
ignored, and repeating-group events lose their instance routing.

Host C is a special case: since Run Form became an external process, **nothing
instantiates it** — `CoboltApp::form_runtimes` is created empty and never
pushed to, and `FormRuntime` is never constructed (verified 2026-08-06 at
1.60.36). It is an unreachable third copy of the state logic, kept alive only
by its tests. Duplication is why the 1.60.33 caption fix originally missed
host B: a fix lands in one copy and the others drift.

This spec closes the gap three ways: **one behavioural contract and one shared
implementation** for everything hosts A and B have in common, **a parity test
suite** that runs the same assertions against both, and **retirement of the
dead host C** so no unreachable copy is left to drift (operator decision,
2026-08-06).

## 2. Goals / Non-goals

**Goals**

- G1 — A built application behaves like Run Form for the same form and project
  settings, across the whole parity set enumerated in § 4.
- G2 — The behaviour the hosts share exists **once** in the codebase; each
  surface supplies only its genuinely different pieces (§ 4.7).
- G3 — A parity test suite exercises the shared behaviour identically for
  hosts A and B, so drift becomes a failing test instead of a field report.
- G4 — The dead in-process host (C) is removed; every behavioural test it
  carried is preserved against the shared implementation.
- G5 — The System KB and the Developer's Guide say only things that are true in
  every live host once this ships.

**Non-goals**

- N1 — **Real child windows for `OpenForm`.** Host A today accepts an
  `OpenForm` and immediately releases the handle with a message ("multi-window
  host pending"); parity means B does the same. Hosting real child viewports
  is the separate 037 follow-on feature, not this fix.
- N2 — **The IDE debugger in a built application.** Intentional host-A-only
  capability (a shipped binary has no IDE attached).
- N3 — **`cobolt_windows` / EXEC RUST block windows under interpreted
  `rcrun run-form`.** Blocks are compiled artefacts; the interpreter already
  raises a hard, catchable error when an uncompiled block is invoked. B-only by
  design.
- N4 — **New effects, new lifecycle events, or any capability neither live
  host has today.** This is parity, not features.
- N5 — Changing how themes are delivered (A discovers packs on disk; B embeds
  them). The *resolution rule* is shared; the *source* stays per-host.
- N6 — Re-introducing an in-process IDE run path. C is retired, not rebuilt.

## 3. User stories

- As a COBOL developer, I want the application I ship to behave exactly like
  the form I tested with Run Form — window effects, lifecycle events, close
  behaviour, property reads — so that "works in the IDE" means "works for my
  users".
- As a COBOL developer coming from PowerCOBOL, I expect `me::Close`, close
  vetoes and `onClose` to work in the delivered `.exe` the way the guide says,
  because that is what I designed against.
- As the maintainer, I want one implementation of the shared host behaviour,
  so a bug fixed once is fixed everywhere and cannot silently survive in the
  copy users actually run.

## 4. Requirements (EARS)

### 4.1 Shared implementation and host C retirement (the root cause)

- **R1 (ubiquitous):** The system shall implement the behaviour common to
  hosts A and B (control state, state/event plumbing, backdrop, window
  properties, effects, lifecycle, diagnostics) in **one** shared
  implementation consumed by both surfaces; per-host code shall be limited to
  the genuinely different pieces listed in R30.
- **R2 (ubiquitous):** The system shall have exactly **one** definition of the
  control-state type (today `CtrlState` exists three times); property writes
  shall replace case-variant keys (the 1.60.33 rule) in every host by
  construction.
- **R3 (constraint):** The generated application's host code shall not be
  maintained as a divergent copy of host A's logic: the template shall contain
  only the thin entry glue around the shared implementation.
- **R4 (event):** When the unreachability of the in-process host is confirmed
  (no `FormRuntime` construction, `form_runtimes` never populated), the system
  shall remove the dead in-process run path (`FormRuntime`,
  `show_running_form_window` and its viewport glue); every behavioural test
  that exercised its logic (e.g. the `run_interaction_tests` family) shall be
  preserved, re-pointed at the shared implementation, and remain green. If any
  reachable use is discovered instead, work shall stop and the finding be
  reported to the operator before anything is removed.

### 4.2 Window effects in the built application (spec 038 — the reported bug)

- **R5 (event):** When a built application's window first opens, and the
  project configures an entrance effect, the system shall play that entrance
  exactly as `rcrun run-form` does — same effect catalogue, durations, easing,
  MatrixRain duration floor, and deterministic seed rule.
- **R6 (event):** When a built application's window is allowed to close, and
  the project configures an exit effect, the system shall play the exit and
  perform the real close when it completes, as host A does.
- **R7 (state):** While an entrance or exit effect plays, the built
  application's window shall wear no chrome, and face-moving effects shall
  play on a see-through window while masked reveals keep an opaque one — the
  same `plays_over_desktop` rule as host A, including the `clear_color`
  contract.
- **R8 (optional):** Where the project sets `entrance-on-restore`, the built
  application shall replay the entrance when the window is restored after a
  minimize, without re-firing form events.
- **R9 (optional):** Where the form's `WindowEffects` designer attribute is
  false, or the `PRC_NO_WINDOW_FX` kill switch is set, the built application
  shall play no effects — the same resolution rule (project × form opt-out ×
  kill switch) as the IDE applies for Run Form.
- **R10 (state):** While an entrance plays, control load-time animations shall
  not start; they shall begin when the entrance completes (038 R8), in every
  host that plays effects.
- **R11 (ubiquitous):** The built application shall obtain the effect settings
  without reading `cobolt.toml` at run time (a shipped binary has no manifest
  beside it); the settings shall be resolved at build time from the same
  values the IDE writes.

### 4.3 Window lifecycle in the built application (spec 037)

- **R12 (ubiquitous):** The built application shall run the same window
  lifecycle machinery as host A (`FormSupervisor` semantics, `ROOT_HANDLE`
  wiring into the interpreter), so lifecycle behaviour cannot differ by host.
- **R13 (event):** When the operator attempts to close a built application's
  window while the form vetoes closing, the system shall cancel the OS close
  and fire `onCloseRejected`, as host A does.
- **R14 (event):** When a built application's window actually closes, the
  system shall fire `onClose` exactly once; when the window first appears
  (after the warm-up), it shall fire `onShow` then `onActivate` once.
- **R15 (event):** When the COBOL program ends (`STOP RUN` or a runtime
  error), the built application's window shall close — through the exit effect
  when one is configured — instead of remaining open and unresponsive.
- **R16 (event):** When a handler invokes a `me::` window method
  (`SetWindowState`, `SetFullScreen`, `SetTitleVisible`, `Focus`, `Close`),
  the built application shall honour it as host A does.
- **R17 (ubiquitous):** The built application's window shall honour the
  designed window properties host A honours: icon, `title_visible`,
  `can_minimize` / `can_maximize`, fullscreen, opening `WindowState`
  (Maximized / Minimized), start position (the eight edge/corner positions,
  Center, Custom, System), and the exact designed inner size (no `+4 px`
  slack). The window title shall be the designed `form.title`, falling back to
  `"{AppName} v{Version}"` only when the designed title is blank (operator
  decision, 2026-08-06).
- **R18 (event):** When the window's fullscreen state actually changes, the
  built application shall fire `onFullScreenChanged` on real transitions only,
  mirroring the state onto the form object first, as host A does.
- **R19 (event):** When a handler calls `OpenForm`, the built application
  shall behave exactly as host A does today: accept, immediately release the
  handle with the "multi-window host pending" notice, and never deadlock the
  caller (real child windows are non-goal N1).

### 4.4 Object registry and control state

- **R20 (ubiquitous):** The built application shall seed the object registry
  at startup exactly as host A does: every control's designed properties, the
  standard geometry/identity properties (`Name`, `Visible`, `Enabled`, `X`,
  `Y`, `Width`, `Height`, `TabOrder`), the `_Binding*` seeds data binding
  reads, and the resolved Maps / Custom Search API keys where the respective
  env vars are provided — so a property read before the first write returns
  the designed value in every host.
- **R21 (ubiquitous):** Control-state reads shall match control ids
  case-insensitively in every host (host B today requires a byte-exact match
  on merge).
- **R22 (ubiquitous):** Repeating-group (control-array) behaviour shall be
  identical in every host: instance writes routed to per-instance state seeded
  from the designed template control, and instanced events dispatched to the
  base control id with the instance index (`CONTROL-ARRAY-INDEX`), as host A
  does.

### 4.5 Input, output, pacing

- **R23 (event):** When a `DISPLAY` line is written by any host, the system
  shall flush stdout so piped output appears live (host B today buffers).
- **R24 (state):** While the event queue holds a backlog, every host shall
  coalesce timer ticks (host A's rule) and shall not enter the long idle
  repaint interval with events still queued.
- **R25 (ubiquitous):** File-dialog capability shall be equal in every host:
  open **and** save dialogs, filters, initial directory and suggested name
  (host B today has open-only, no filters).
- **R26 (ubiquitous):** The scroll area shall use floating scrollbars in every
  host (no reserved gutter strip on the right/bottom edges).

### 4.6 Diagnostics

- **R27 (optional):** Where `COBOLT_FRAME_DIAGNOSTICS` is enabled, **every**
  host shall emit the launch preamble (form, control roster) and the
  per-property-update trace with "NO SUCH CONTROL" reporting (today host B
  only); the file-based diagnostic dumps host A writes shall keep working.
- **R28 (ubiquitous):** Diagnostic environment flags shall parse with one
  truthiness rule (`1` / `true` / `on`, case-insensitive) everywhere (host B
  today disagrees with itself between two reads).

### 4.7 Testing, documentation, and the seam

- **R29 (ubiquitous):** A parity test suite shall run the **same** behavioural
  assertions against hosts A and B (state merge, seeding, effects gating,
  lifecycle event order, close veto, program-end close, control-array
  routing); a behaviour present in one host and absent in the other shall be a
  failing test, not a code review hope. The generated template shall gain
  behaviour-level coverage (today the compiler tests only assert it
  *compiles*).
- **R30 (constraint):** The following shall remain per-host and shall be the
  **only** per-host behaviour: host A's debugger channel and `@DBG` protocol;
  host B's `cobolt_windows` replay, compiled-block registration and headless
  fallback; theme-pack *source* (disk discovery vs embedded). Each shall be
  listed in the shared host's documentation as a named seam.
- **R31 (ubiquitous):** The System KB text and `docs/developers-guide-en.md`
  shall be updated in the same change so every statement about window effects
  and window lifecycle is true in every live host, and the
  "consumed once the packaged host reaches 037/038 parity" placeholder is
  retired; the prebuilt chunked KB store shall be regenerated.

## 5. Acceptance criteria

- [ ] AC1 — Building the operator's PowerDemo3 (entrance `matrix-rain`, exit
  `fade`) and launching the binary plays the MatrixRain entrance on first
  open and the fade exit on close; `PRC_NO_WINDOW_FX=1` suppresses both.
  (R5, R6, R9)
- [ ] AC2 — In a built application, a masked-reveal entrance opens an opaque
  window and a face-moving entrance opens a see-through one with no chrome
  until the effect ends — matching `rcrun run-form` at the rule level.
  (R7)
- [ ] AC3 — With `entrance-on-restore = true`, minimizing and restoring the
  built application replays the entrance and fires no form events on the
  replay. (R8)
- [ ] AC4 — A control fly-in configured `OnFormLoad` starts only after the
  entrance completes, in Run Form and in the built binary alike. (R10)
- [ ] AC5 — `STOP RUN` closes the built application's window (through the exit
  effect when configured); a runtime error does the same after reporting.
  (R15)
- [ ] AC6 — In the built application: closing while the form is Waiting fires
  `onCloseRejected` and the window stays; an allowed close fires `onClose`
  exactly once; `onShow` and `onActivate` fire once at open. (R13, R14)
- [ ] AC7 — `me::Close`, `me::SetWindowState`, `me::SetFullScreen`,
  `me::SetTitleVisible`, `me::Focus` work in the built application; fullscreen
  transitions fire `onFullScreenChanged` with the mirrored state. (R16, R18)
- [ ] AC8 — The built application's window shows the designed `form.title`
  (branded fallback only when blank), the designed icon, honours
  `title_visible`, min/max buttons, opening window state, start position, and
  opens at exactly the designed size. (R17)
- [ ] AC9 — In the built application, `DISPLAY Label-1::Caption` (and `MOVE`
  from a property) before any handler write yields the designed value, not an
  empty string; `RefreshBinding()` finds its `_Binding*` seeds. (R20)
- [ ] AC10 — A repeating-group card's button click reaches the handler with
  the correct `CONTROL-ARRAY-INDEX`, and a per-instance property write lands
  on that instance only — in both hosts. (R22)
- [ ] AC11 — `./app | cat` shows `DISPLAY` output live. (R23)
- [ ] AC12 — The parity suite runs one assertion set against hosts A and B and
  is green; deleting the 1.60.33 dedupe from the shared control-state type
  makes it fail for both. (R2, R29)
- [ ] AC13 — `FormRuntime` and `show_running_form_window` are gone; `grep`
  finds no construction of an in-process form runtime; the preserved
  behavioural tests (the `run_interaction_tests` family) run against the
  shared implementation and pass. (R4)
- [ ] AC14 — `grep` finds no `#[allow(dead_code)]` on the compiler's
  `[forms]` effect fields, and no "once the packaged host reaches 037/038
  parity" placeholder; the KB/guide describe effects and lifecycle as working
  in built applications; `prebuilt_chunked_kb_matches_the_published_documentation`
  is green. (R11, R31)
- [ ] AC15 — `cargo test` for `cobolt-cli`, `cobolt-compiler`, `cobolt-forms`
  and `cobolt-ide` passes; the four existing host-A tests still pass
  unchanged. (regression gate)

## 6. Constraints & steering check

- **i18n (6 languages):** No new IDE UI strings are expected (the work is in
  the hosts, not IDE chrome). If the plan surfaces any user-facing IDE string,
  it must be a `Tr` field in all six languages. Removing host C may retire
  `Tr` fields; unused keys are removed from all six languages together.
- **Generated-code contract:** The generated **Rust** template keeps its
  "auto-generated, do not edit" banner; generated **COBOL** is untouched by
  this spec. The regenerate-on-action contract is unaffected.
- **Docs (English guide):** Required — R31. Window effects and lifecycle
  sections must stop being Run-Form-only truths. Translations untouched
  (user-maintained).
- **System KB + chunked store:** Required — R31; reindex
  (`cargo run -p cobolt-ide --example build_chunked_kb`) in the same change.
- **Fix vs feature:** **Fix** (operator ruling 2026-08-06): documented
  behaviour that does not happen, i.e. technical debt — including the dead-code
  retirement. `z` bump in `version.rs`, CHANGELOG entry, f=97 announcement
  after merge+push to main. No feature (f=96) component: N1 keeps real child
  windows out.
- **Code removal:** Host C's removal is operator-approved (2026-08-06) and
  gated by R4's reachability check; it is IDE-internal code, not developer
  (user) code, so the user-code-is-sacred rule does not apply — but the
  check-then-remove discipline is kept anyway.
- **Tests:** Parity suite results must be quantified and human-readable
  (which hosts, which behaviours, how many assertions) per the operator's
  test-reporting rule.
- **MSRV / egui:** No new dependencies anticipated; must build on the
  workspace MSRV (1.92) and egui/eframe 0.36.

## 7. Open questions

- **Q1 — Where does the shared host live?** `cobolt-forms` (already the shared
  renderer's home, but it would gain interpreter-channel types) or a new
  small crate (e.g. `cobolt-form-host`) that host A and B's generated Cargo
  project both depend on. This is a `/plan` decision; flagged here because it
  changes the generated project's dependency list.

*(Resolved during specification, 2026-08-06: built-app window title = designed
`form.title` with branded fallback when blank → R17. Host C = retire in this
spec → R4/G4.)*

---

## 8. Appendix — drift inventory (evidence, 2026-08-06)

Verified against the tree at 1.60.36 (`fixes` branch). "A" =
`crates/cobolt-cli/src/form_gui.rs`, "B" = the `form_runtime_code` template in
`crates/cobolt-compiler/src/lib.rs`, "C" = `crates/cobolt-ide/src/form_runtime.rs`
(+ `show_running_form_window` in `app.rs`).

**Host C is dead code:** `form_runtimes` is initialised empty
(`app.rs:1056`) and never pushed to; `FormRuntime` is never constructed
anywhere (only its `impl Drop` mentions it outside the definition). Its render
path survives only through tests.

| Area | A | B | Notes |
|------|---|---|-------|
| 038 effects (FxSpec, playback, transparency, chrome, restore, kill switch, clear_color, MatrixRain seed) | ✅ | ❌ | zero `window_fx` references in B |
| 037 supervisor / `set_form_host` / `ROOT_HANDLE` | ✅ | ❌ | B never wires the interpreter to a host |
| Close veto + `CancelClose` + `onCloseRejected` | ✅ | ❌ | B never reads `close_requested()` |
| `onShow` / `onActivate` / `onClose` | ✅ | ❌ | B fires no form-level lifecycle events |
| Program end closes the window | ✅ | ❌ | B's window outlives `STOP RUN`, looks alive, answers nothing |
| `me::` window methods | ✅ | ❌ | |
| `OpenForm` accepted-and-released stub | ✅ | ❌ | B: nothing (deadlock risk) |
| Designed icon / decorations / min-max / fullscreen / window state / start position / exact size | ✅ | ❌ | B: default icon, `+4 px` slack |
| Designed window title | ✅ | ❌ | B: `APP_NAME vVERSION` always (R17 keeps it as blank-title fallback) |
| `onFullScreenChanged` | ✅ | ❌ | |
| `CtrlState` 1.60.33 case-variant dedupe | ✅ | ✅ | in all three copies now (B via 021ebb6); the three copies are the hazard |
| State entries seeded from designed template (`state_entry_mut`) | ✅ | ❌ | B: `entry().or_default()` |
| Case-insensitive state read on merge | ✅ | ❌ | B requires byte-exact id |
| Control-array alias + instance routing + `CONTROL-ARRAY-INDEX` dispatch | ✅ | ❌ | |
| `seed_objects` (designed props, geometry, `_Binding*`, API keys) | ✅ | ❌ | C also lacks it (0 references) |
| Backdrop (colour/gradient/image/theme art/window_size) | ✅ | ✅ | equal |
| Control animations (AnimRuntime, triggers) | ✅ | ✅ | equal — but B starts load anims on frame 1 (no entrance gate) |
| Timer-tick coalescing + backlog-aware repaint | ✅ | ❌ | |
| `DISPLAY` stdout flush | ✅ | ❌ | |
| File dialogs | open+save+filters | open only | |
| Floating scrollbars | ✅ | ❌ | |
| `COBOLT_FRAME_DIAGNOSTICS` live trace | ❌ | ✅ | the one place B is richer; A has file dumps instead |
| env-flag truthiness | `1/true/on` | inconsistent (`"1"` vs `1/true`) | |
| Debugger `@DBG` channel | ✅ | — | intentional (N2) |
| `cobolt_windows` replay / block registration / headless fallback | — | ✅ | intentional (N3) |
| Theme source | disk packs | embedded | intentional (N5) |
| Host tests | 4 unit tests | 0 behaviour tests (compile-only) | why 1.60.33 survived in B |

Sizes: A ≈ 1 370 code lines; B ≈ 430; ~62 % of B's `ui()` is byte-identical to
A's after whitespace normalisation. Three `struct CtrlState` definitions:
`form_gui.rs:74`, compiler `lib.rs:1392`, `form_runtime.rs:137`.
