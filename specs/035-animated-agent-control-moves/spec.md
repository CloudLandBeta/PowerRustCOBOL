<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Animated agent control moves

- **Status:** draft → approved
- **Folder:** specs/035-animated-agent-control-moves/
- **Author:** Claude (agent) for CloudLandBeta   **Date:** 2026-07-24

## 1. Overview

When the AI agent repositions controls on a form in response to a developer's
request, the change is applied instantly — controls simply **pop** to their new
coordinates. This spec makes those moves **animated**: each affected control
**glides** from its previous position to the agent's new one, all controls moving
**at once** over **1000 ms** with a smooth easing curve. The effect makes the
agent's layout work visible and legible ("watch it arrange the form") instead of
a jarring jump. It is a **purely visual** designer effect — the persisted `.cfrm`
and the generated COBOL are unaffected; the controls' logical positions are the
agent's final coordinates the instant the change-set is applied.

## 2. Goals / Non-goals

- **Goals:**
  - When an agent change-set moves one or more existing controls, animate each
    from its old position to its new one, **simultaneously**, over **1000 ms**.
  - Use a pleasant easing (ease-in-out) so the motion accelerates and settles,
    not a linear slide.
  - Keep the IDE **responsive** during the animation (non-blocking; the designer
    repaints each frame until the motion completes).
  - Purely visual: the form model, saved file, and generated code carry the
    final positions immediately; only the on-screen rendering interpolates.
- **Non-goals:**
  - Animating **manual** (developer drag) moves — only agent-applied moves.
  - Animating non-position changes (caption, colour, font, visibility, etc.).
  - A general animation framework or per-control configurable timing.
  - Persisting or exporting the animation; it never affects runtime forms.
  - Undo/redo of the animation (the underlying change-set undo is unchanged).

## 3. User stories

- As a developer, when I ask the agent to "line these buttons up along the
  bottom", I want to **see** them slide into place, so I can follow what it did.
- As a developer, I want the whole rearrangement to feel like one coordinated
  motion (all controls together, ~1 s), not a stutter of individual jumps.
- As a developer, I don't want the animation to block me — the IDE stays usable
  and the final layout is correct the moment the change is applied.

## 4. Requirements (EARS)

- **R1 (event):** When an agent change-set is applied and it changes the
  position (X and/or Y) of one or more existing controls, the system shall
  animate each such control from its **pre-change** position to its **new**
  position.
- **R2 (constraint):** All controls moved by the same change-set shall animate
  **simultaneously** (one shared timeline), not sequentially.
- **R3 (constraint):** The animation shall last **1000 ms** and use an
  **ease-in-out** curve (smooth start and stop).
- **R4 (state):** While the animation is running, the system shall keep
  repainting the designer each frame and shall not block editing or other IDE
  interaction; the run shall complete on its own after 1000 ms.
- **R5 (constraint):** The animation shall be **visual only** — the form model,
  the saved `.cfrm`, and any generated COBOL shall already hold the **final**
  coordinates when the change-set is applied; the interpolation affects only what
  is drawn.
- **R6 (event):** When a new agent change-set arrives while an animation is still
  running, the system shall retarget to the newest positions (start a fresh
  1000 ms motion from wherever each control currently appears) rather than queue
  or ignore it.
- **R7 (constraint):** Only **agent-applied** moves shall animate; a developer's
  manual drag/nudge of a control shall not trigger the animation.
- **R8 (constraint):** A control that the change-set **creates** (deploy_control)
  or **deletes**, or whose position does **not** change, shall not be given a
  move animation (its handling is unchanged from today).

## 5. Acceptance criteria

- [x] AC1 — Applying an agent change-set that moves ≥1 existing control makes each
      moved control glide from its old to its new position over ~1 s, all at once
      (R1, R2, R3). Verifiable by an interpolation unit test (position at t=0 =
      old, t=500 ms ≈ eased midpoint, t≥1000 ms = new) plus a manual visual check.
- [x] AC2 — During the animation the designer keeps repainting and the IDE stays
      interactive; the animation ends by itself at 1000 ms (R4).
- [x] AC3 — The saved `.cfrm` and generated COBOL contain the **final** positions
      immediately after apply, regardless of animation progress (R5). A test
      asserts the model holds final coords at t=0.
- [x] AC4 — A second change-set mid-animation restarts the motion toward the new
      targets from the controls' current on-screen positions (R6).
- [x] AC5 — A manual drag does not animate; created/deleted/unmoved controls do
      not get a move animation (R7, R8).
- [x] AC6 — `cargo build -p cobolt-ide` and `cargo test -p cobolt-ide` pass,
      including the easing/interpolation test.

## 6. Constraints & steering check

- **i18n (6 languages):** None expected — the effect has no new user-facing text.
  If any label is added, it must be a `Tr` field ×6.
- **Generated-code / regenerate contract:** Unaffected — the animation never
  changes model coordinates; the generated COBOL and regenerate-on-action
  contract are untouched (R5).
- **Docs (English guide):** A one-line mention in the Grace/AI-agent or designer
  section that agent moves animate. English only.
- **Fix vs feature:** **Feature** — minor version bump + `CHANGELOG.md` entry.
- **egui / resize safety:** The animation is drawn by interpolating paint
  positions only; it must **not** derive any container/panel size from animated
  values (that is the self-inflation trap — see the egui-resize guidance). The
  canvas layout is unchanged; only control draw offsets interpolate.

## 7. Open questions

- **Q1:** Should control **size** changes (W/H from set_property) animate too, or
  **position only** in v1? *Recommendation: position only (matches the request
  "changing controls' place"); note size as a follow-up.*
- **Q2:** Should **newly created** controls (deploy_control) get an entrance
  effect (fade/scale-in), or is that out of scope? *Recommendation: out of scope
  (R8); moves only.*
- **Q3:** Should a control that is **reparented** (its container changes) animate
  across containers, or only animate when its coordinates change within the same
  parent? *Recommendation: animate on coordinate change within the same parent in
  v1; cross-container reparents just apply. Resolve in `/plan`.*
- **Q4:** Exact easing function (e.g. cubic ease-in-out vs. egui's
  `emath::easing`), confirmed in `/plan`.
- **Q5:** Does this apply to the **full Grace chat** apply path, the **contextual
  designer chat** apply path, or **both**? *Recommendation: both — anywhere
  `apply_agent_change_set` runs.*
