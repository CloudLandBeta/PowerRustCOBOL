<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Project window entrance & exit effects

- **Status:** draft
- **Folder:** specs/038-window-effects/
- **Author:** Emerson Lopes (with Grace)   **Date:** 2026-07-30

## 1. Overview

Form windows currently appear and disappear instantly. This spec adds
**window-level entrance and exit effects configured once per project** —
effect, duration and easing for each direction, set in the project settings,
persisted in the project file, and played by the run-form host for **every
form of the project**. Forms carry a single boolean: play the project's
effects or not. The entrance effect plays on a window's **first opening**,
and — when a project option enables it — again when the window is
**restored after being minimized**. The catalogue spans classic transitions
(fade, dBASE-style zoom, slide), masked reveals (radar/clock wipe, iris,
blinds, checkerboard), the signature **Matrix falling-code** reveal (the
default for new projects), and a genie approximation. Effects compose with
spec 037's lifecycle: close vetoes fire before any exit animation.

## 2. Goals / Non-goals

- **Goals:**
  - **Project-level** entrance and exit effects (effect, duration, easing
    each), set in the project settings, persisted in the project file,
    applied uniformly to all the project's forms.
  - Per-form boolean **WindowEffects** (default on) — a form only chooses
    whether it plays the project's effects, never which.
  - Entrance plays on first open; a project option additionally plays it on
    restore-after-minimize.
  - Deterministic choreography: control load-time animations start
    immediately after the entrance effect finishes — window first, then
    the controls.
  - The effect catalogue of §R4, implemented inside the egui/eframe host —
    no OS compositor dependencies. Matrix rain uses **classic glyphs only**
    (katakana/digits) — no easter eggs.
  - New projects default to the Matrix entrance; existing projects load
    with effects = None (unchanged behaviour).
  - An IDE-wide kill-switch that disables all window effects at runtime
    without modifying any project or form.
  - Designer/settings preview of the selected effect.
  - Correct interplay with spec 037: vetoes before exit animations; exit
    effects delay the actual close until the animation completes.
- **Non-goals:**
  - Per-form effect selection (deliberately rejected: one look per
    project).
  - Hooking the OS minimize animation itself (macOS plays its own genie;
    not interceptable — we only play OUR entrance on restore when the
    option is on).
  - True render-to-texture genie warp (pseudo-genie first; textured warp a
    later upgrade, out of scope).
  - Control-level animations (already exist) — this spec is the WINDOW.
  - Transition sounds.

## 3. User stories

- As a COBOL developer, I want to pick my application's window transition
  once, in the project settings, so every form opens and closes with the
  same polished identity without per-form fiddling.
- As a COBOL developer, I want a specific form (say, a modal alert) to opt
  out of the animation with one checkbox.
- As a COBOL developer, I want the entrance to optionally replay when the
  user restores a minimized window, so the app feels alive — or not, if I
  find that annoying.
- As an end user on a weak machine (or sensitive to motion), I want one
  switch that turns all window effects off.

## 4. Requirements (EARS)

### Project settings & model

- **R1 (ubiquitous):** The project shall have **EntranceEffect** and
  **ExitEffect** settings, each comprising an effect id, a duration in
  milliseconds (bounded, e.g. 100–3000), and an easing; editable in the
  project settings UI and persisted in the project file. They apply to
  every form of the project.
- **R2 (ubiquitous):** The project shall have a boolean
  **PlayEntranceOnRestore** option (default off): when on, the entrance
  effect also plays when a form window is restored after being minimized;
  when off, the entrance plays only on the window's first opening.
- **R3 (ubiquitous):** Every form shall have a boolean **WindowEffects**
  property (default true), persisted in the `.cfrm`: when false the form
  opens and closes instantly, ignoring the project's effects. Forms never
  select effects — only this on/off.
- **R4 (ubiquitous):** The effect catalogue shall comprise, for both
  directions unless noted: **None**, **Fade**, **Zoom** (dBASE-IV-style box
  zoom from a point), **Slide** (from a chosen edge), **ExpandFromTitleBar**,
  **RadarWipe** (angular sweep), **IrisWipe** (circle), **Blinds**,
  **Checkerboard**, **MatrixRain** (glyph rain — classic katakana/digit
  glyphs only — condensing into the form face on entrance / dissolving it
  on exit), and **Genie** (approximation: vertical squash with a
  corner-directed bend).
- **R5 (ubiquitous):** A newly created project shall default to
  EntranceEffect = MatrixRain (tasteful default duration), ExitEffect =
  None, PlayEntranceOnRestore = off; a project file saved before this spec
  shall load with both effects = None and behave exactly as today.
- **R6 (event):** When an effect is selected in the project settings, the
  UI shall offer a **preview** of that effect without running a form.

### Runtime playback

- **R7 (event):** When a form window opens for the first time (Run, Run
  Form, and — once the multi-window host lands — OpenFormSync/Async
  children), and the form's WindowEffects is true, the system shall play
  the project's EntranceEffect; the form must be fully interactive no later
  than the animation's end.
- **R8 (event):** When an entrance effect plays, the form's control-level
  **load-time animations** (the existing form-load animation engine) shall
  be deferred and start **immediately after the entrance effect finishes**
  — the window materialises first, then the controls come alive. When no
  entrance effect plays (effect None, form opted out, kill-switch on), they
  start with the window exactly as today. Only the visual sequencing moves:
  the COBOL onLoad event still fires per R13.
- **R9 (event):** When PlayEntranceOnRestore is on and a form window is
  restored after being minimized, the system shall play the EntranceEffect
  again; when off, restore is instant (OS default). Restore replays do NOT
  replay the control load-time animations.
- **R10 (event):** When a form window is allowed to close and the form's
  WindowEffects is true, the system shall play the project's ExitEffect and
  perform the actual close when it completes.
- **R11 (state):** While spec 037 lifecycle vetoes apply (FormState =
  Waiting, Sync-child Waiting), the close shall be refused **before** any
  exit animation starts — a vetoed close plays nothing.
- **R12 (constraint):** Effects shall render inside the OS window's bounds.
  Where an effect wants to exceed the final frame, it shall only do so when
  the form opens chromeless (TitleVisible = false, spec 037 R15) with
  window transparency; with native decorations the effect applies to the
  window's content only.
- **R13 (constraint):** Effect playback shall not alter the form's logical
  state or generated COBOL: onLoad fires once per open and onClose once per
  actual close, exactly as without effects; restore-replays (R9) fire no
  form events at all.

### Kill-switch

- **R14 (ubiquitous):** The IDE shall provide a machine-wide **"Disable
  window effects"** setting (same scope as the 1.36 debug switches); when
  on, every entrance/exit effect is skipped (instant open/close/restore)
  for forms run from this IDE and for `rcrun run-form` processes it
  spawns, without modifying any project or `.cfrm`.

### Documentation & agents

- **R15 (ubiquitous):** The System KB docs tables shall document the
  project settings, the form boolean, the effect catalogue, and the
  kill-switch (steering System-KB rule — docs tables updated in the same
  change; store reindex only on operator request).
- **R16 (ubiquitous):** Grace's validators shall accept the new form
  boolean (form-property list agreement, 1.42.2 test extended) and the
  project-level settings through whatever project-settings surface Grace
  already uses.

## 5. Acceptance criteria

- [x] AC1 — EntranceEffect/ExitEffect (effect, duration, easing) and
  PlayEntranceOnRestore round-trip through the project file; a pre-038
  project loads with None/None/off and no behavioural change. (R1, R2, R5)
- [ ] AC2 — A newly created project carries MatrixRain entrance / None
  exit / restore off; the settings UI shows them. (R5)
- [ ] AC3 — Every catalogue effect is selectable and plays in the run-form
  host on first open; visual check per effect against its description;
  MatrixRain shows only katakana/digit glyphs. (R4, R7)
- [ ] AC4 — The settings preview plays the selected effect on demand
  without launching a form. (R6)
- [ ] AC5 — A form with WindowEffects=false opens/closes instantly while
  the rest of the project's forms animate. (R3)
- [ ] AC6 — On a form with load-time control animations, the entrance
  effect plays to completion FIRST and the control animations begin
  immediately after it ends; with effects disabled (any path) the control
  animations start with the window as before. (R8)
- [ ] AC7 — With PlayEntranceOnRestore on, minimize→restore replays the
  entrance (but not the control animations) and fires NO form events; with
  it off, restore is instant. (R9, R13)
- [ ] AC8 — With an ExitEffect set, the window closes only after the
  animation completes; onClose fires exactly once, at the actual close.
  (R10, R13)
- [ ] AC9 — A Waiting form refuses the close with onCloseRejected and plays
  NO exit animation; once Ready, the close plays the effect then closes.
  (R11)
- [ ] AC10 — With native decorations the effect stays within the content
  area; with TitleVisible=false + transparency the effect may use the full
  window rectangle. (R12)
- [ ] AC11 — The IDE-wide kill-switch makes every open/close/restore
  instant without touching any file; turning it off restores the effects.
  (R14)
- [ ] AC12 — KB docs tables document settings/catalogue/boolean/kill-switch;
  Grace can toggle a form's WindowEffects via SetProperty (validator
  accepts). (R15, R16)

## 6. Constraints & steering check

- **i18n (6 languages):** yes — project-settings labels (Entrance effect,
  Exit effect, Duration, Easing, effect names, restore option, preview
  button), the form's WindowEffects label, and the kill-switch label as
  `Tr` fields ×6. COBOL-facing identifiers stay English.
- **Generated-code contract:** no change to generated `.cbl` semantics
  (R13); banner/regeneration contract untouched.
- **Docs:** English developers-guide section (project effects, catalogue,
  per-form opt-out, restore option, control-animation sequencing,
  kill-switch, the R12 chromeless note). Translations untouched.
- **Fix vs feature:** **feature** → minor bump, own commit(s), forum
  announcement f=96 `[Noticia]`, title ≤ 50 chars, push-window rules.
- **Tech:** egui/eframe 0.35 only — masks via mesh clipping, glyph rain via
  text painting; no compositor/OS APIs. Minimize/restore detection via the
  viewport state already mirrored for spec 037 (WindowState). Respect the
  egui resize/self-inflation lessons (effects must never feed the window's
  own size).
- **Perf:** effects run at the host's frame budget; MatrixRain glyph count
  bounded; the kill-switch is also the escape hatch for weak GPUs (R14).

## 7. Open questions

- Q1: Zoom origin — fixed window centre, or from the invoking control's
  screen position when opened via OpenForm* (nice "spawned from here"
  reading)? *(Proposed: centre now; caller-origin when the multi-window
  host lands.)*
- Q2 — resolved: MatrixRain uses classic katakana/digit glyphs only; no
  easter eggs (operator, 2026-07-30).
- Q3: Should the kill-switch live in Help → Debug Settings (existing
  machine-wide switch home) or a new Appearance settings row? *(Proposed:
  Debug Settings — same scope and plumbing as 1.36 switches.)*
- Q4: Exit default for NEW projects stays None — confirm, or mirror the
  entrance (MatrixRain dissolve)?
- Q5: Where in the project settings UI do the effect rows live — the
  existing project Settings form's appearance section? *(Proposed: yes,
  next to the form-theme default.)*
