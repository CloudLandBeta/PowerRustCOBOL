<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Animated agent control moves

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-24

## 1. Approach

All agent edits land through **one** method — `FormDesigner::apply_agent_change_set`
(designer.rs:1265), which builds a `Vec<Cmd>` and applies them atomically via
`self.apply(Cmd::AgentBatch { cmds })`. Both the full-Grace and contextual-designer
apply paths go through it (Q5 → both). That single choke-point lets us capture the
move animation cleanly:

1. **Snapshot before apply (R1, R7, R8).** Just before `self.apply(AgentBatch)`,
   record `id → (x, y)` for every control that **already exists** (created controls
   have no "before", so they never animate — R8). This runs only in the agent
   apply path, so manual drags never animate (R7).
2. **Diff after apply.** For each still-existing control whose `(x, y)` **changed**
   and whose parent is unchanged (Q3), push a `MoveAnim { id, from: old, to: new }`.
   Unmoved controls produce nothing (R8).
3. **Interpolate on paint (R2, R3, R5).** Store the animations plus a single shared
   `start: Option<f64>` on the designer. In the canvas paint loop, where each
   control's model rect (`c.rect.x/y`) becomes its on-canvas rect, add a per-control
   **draw offset** = `lerp(from, to, eased(t)) − to`, where `t = (now − start)/1.0`
   clamped to `[0,1]`. The model already holds `to`, so the offset is zero at
   `t=1` and the control rests at its final spot. **Only the drawn position moves;
   the model, `.cfrm`, and generated COBOL already hold the final coordinates**
   (R5).
4. **Drive + finish (R4).** While any animation is active, call
   `ctx.request_repaint()` so the designer keeps ticking; when `t ≥ 1` for all,
   clear the list. Nothing blocks — it is pure per-frame interpolation.
5. **Retarget mid-flight (R6).** A new change-set re-runs step 1–2, but the
   snapshot's "before" position for an already-animating control is its **current
   on-screen** position (the interpolated point), and `start` resets — so the new
   motion continues smoothly from where the control visually is.

**Easing (Q4).** One shared timeline, 1000 ms, ease-in-out. Use egui's
`emath::easing::cubic_in_out` if present on 0.35; otherwise a local smoothstep
`t*t*(3−2t)`. Chosen in a tiny pure helper so it is unit-testable.

## 2. Affected crates / files

- `crates/cobolt-ide/src/panels/designer.rs` —
  - new designer fields: `move_anims: Vec<MoveAnim>`, `move_anim_start: Option<f64>`;
  - snapshot-before / diff-after in `apply_agent_change_set`;
  - a pure `anim_offset(id, now) -> egui::Vec2` + `eased(t)` helper;
  - apply the offset in the canvas control-paint loop (where `c.rect.x/y` →
    on-canvas rect) and `request_repaint()` while active.
- `docs/developers-guide-en.md` — one line noting agent moves animate.
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — minor bump (feature).
- **No** `i18n.rs` change expected (no new user-facing strings).

## 3. Data / model changes

- **None persisted.** No `.cfrm` / `cobolt.toml` / control-model change. The
  animation is transient runtime state on `FormDesigner` only:
  `MoveAnim { id: String, from: egui::Pos2, to: egui::Pos2 }` + a start time.
- The control model's `rect` continues to hold the **final** coordinates from the
  moment the change-set applies (R5).

## 4. Key decisions & alternatives

- **Hook at `apply_agent_change_set`, snapshot/diff around `self.apply`.** Why: one
  agent-only choke-point → satisfies R1/R7/R8 without tagging individual ops.
  Rejected: animating inside the `Cmd::MoveControl`/`SetProperty` handlers — those
  also run for manual/undo actions (would violate R7) and lack the "all at once"
  shared timeline.
- **Interpolate a draw offset, keep the model final (R5).** Why: zero risk to the
  saved form / generated code; the effect is cosmetic. Rejected: animating the
  model's `rect` toward the target (pollutes undo, the file, and codegen; and a
  save mid-animation would persist a wrong position).
- **Single shared `start` (one timeline).** Why: R2 "all at once". Rejected:
  per-control start times (staggered — not requested).
- **Q1 → position only** (X/Y); size (W/H) not animated in v1.
- **Q2 → no entrance effect** for created controls (moves only, R8).
- **Q3 → animate coordinate changes within the same parent**; a control whose
  parent/container changes just applies (no cross-container tween in v1).
- **Q5 → both apply paths** (they share `apply_agent_change_set`).

## 5. Risks & mitigations

- **egui self-inflation trap** (tech.md / egui guidance): the offset must feed
  **only paint positions**, never any container/panel/`Resize` size or the canvas
  extent. → Mitigation: `anim_offset` returns a draw-only `Vec2` added at the
  screen-rect computation; no layout/available-size input is derived from it. Call
  out in review.
- **Save or regenerate mid-animation** could capture an interpolated position. →
  Mitigation: by design the *model* holds final coords immediately (R5); only the
  draw offset animates, so a save/generate at any instant is correct. A test
  asserts the model is final at `t=0`.
- **Hit-testing / selection during animation** — the control is drawn offset but
  its logical rect is final, so clicks would hit the final rect, not the moving
  glyph. → Acceptable for a 1 s cosmetic effect; note it. (Optionally suppress
  selection interaction while animating — deferred.)
- **Stale animations** if a control is deleted by a later change-set. →
  Mitigation: the diff only animates controls that exist after apply; `anim_offset`
  ignores ids not currently in `form.controls`.
- **Reduced-motion / preference**: none today; the fixed 1 s is per the request.
  Note a possible future toggle.

## 6. Test strategy

- **Easing unit test**: `eased(0)=0`, `eased(1)=1`, monotonic, `eased(0.5)≈0.5`
  and symmetric (ease-in-out).
- **Interpolation unit test**: for a `MoveAnim { from:(0,0), to:(100,40) }`, the
  offset at `t=0` = `from−to`, at `t≥1` = `(0,0)` (rests at final), and at
  `t=0.5` is the eased midpoint (R1, R3).
- **Diff unit test**: given a before-snapshot and an after form, only controls
  whose (x,y) changed and that existed before yield a `MoveAnim`; created / deleted
  / unmoved controls yield none (R7, R8).
- **Model-final test**: immediately after `apply_agent_change_set`, `form.controls`
  hold the target coordinates regardless of animation (R5/AC3).
- **Manual/visual** (operator, per never-drive-the-app): ask the agent to move a
  few controls → they glide together over ~1 s, ease-in-out; the IDE stays
  responsive; a save right after shows final positions; a second move mid-animation
  retargets smoothly (R6).

## 7. Steering compliance

- [ ] **i18n:** no new user-facing strings expected; if any are added, `Tr` ×6.
- [ ] **Generated-code banner + regenerate-on-action** preserved — animation is
      draw-only; model coords are final on apply (R5).
- [ ] **English dev guide** gains a one-line note; translations untouched.
- [ ] **Fix vs feature:** **feature** → minor bump + `CHANGELOG.md`; not mixed
      with unrelated fixes in one commit.
- [ ] **egui safety:** offset feeds paint only — never a container/panel size
      (self-inflation guard).
- [ ] **No "cobolt" in user-facing text; COBOL identifiers/source English.**
