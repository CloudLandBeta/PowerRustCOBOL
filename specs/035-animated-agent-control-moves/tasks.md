<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Animated agent control moves

- **Status:** done
- **Plan:** ./plan.md   **Date:** 2026-07-24

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

## Phase A — pure animation math (headless, no UI wiring)

- [x] **T1 — `MoveAnim` type + `eased` + `anim_offset` helpers** (R2, R3, R5)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: add `struct MoveAnim { id: String, from: egui::Pos2, to: egui::Pos2 }`.
    Add a pure `eased(t: f32) -> f32` (ease-in-out via egui `emath::easing::
    cubic_in_out` or local smoothstep `t*t*(3-2t)`, clamped `[0,1]`) and a pure
    `move_offset(from, to, t) -> egui::Vec2` = `lerp(from,to,eased(t)) - to`.
  - Verify: `cargo test -p cobolt-ide designer` — `eased(0)=0`, `eased(1)=1`,
    `eased(0.5)≈0.5`, monotonic; `move_offset` at `t=0` = `from-to`, at `t≥1` =
    `(0,0)`, at `t=0.5` = eased midpoint (AC1 math).

- [x] **T2 — Before/after move diff** (R1, R7, R8, Q3)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: a pure `diff_moves(before: &HashMap<String,(i32,i32)>, form: &Form) ->
    Vec<MoveAnim>` — for each control that (a) existed in `before`, (b) still
    exists, (c) has the **same parent**, and (d) whose `(x,y)` changed, emit a
    `MoveAnim { from: old, to: new }`. Created/deleted/unmoved/reparented → none.
  - Verify: `cargo test -p cobolt-ide designer` — a fixture form asserts only the
    moved-same-parent controls yield anims; created/deleted/unmoved/reparented
    yield none (AC5 core).

## Phase B — designer state + capture

- [x] **T3 — Designer animation state + capture in `apply_agent_change_set`** (R1, R5, R6, R7)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: add fields `move_anims: Vec<MoveAnim>` and `move_anim_start: Option<f64>`.
    In `apply_agent_change_set`, snapshot `id→(x,y)` for existing controls **before**
    `self.apply(AgentBatch)`; after apply, set `move_anims = diff_moves(...)` and
    `move_anim_start = None` (armed; the first paint stamps the start). For a
    retarget (R6), rebuild `from` using the control's **current on-screen** position
    (its live `move_offset` + final rect) so the new motion continues smoothly.
  - Verify: `cargo build -p cobolt-ide`; a test: after `apply_agent_change_set`
    moving a control, `form.controls` hold the **final** coords (R5/AC3) and
    `move_anims` contains that control.

## Phase C — paint + drive

- [x] **T4 — Interpolate on paint + repaint while active** (R2, R3, R4, R5)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: in the canvas control-paint loop (where `c.rect.x/y` → on-canvas rect),
    add `self.anim_offset(&c.id, now)` to the drawn origin only. On the first paint
    after capture, set `move_anim_start = Some(now)`. Compute `t = (now-start)/1.0`;
    while any `t < 1`, `ctx.request_repaint()`; when all `t ≥ 1`, clear `move_anims`.
    **Offset feeds paint position only — never any container/panel/Resize size**
    (egui self-inflation guard, plan §5).
  - Verify: `cargo build -p cobolt-ide`; existing designer tests still pass;
    manual: agent-moved controls glide together ~1 s, IDE stays responsive (AC1,
    AC2, AC4).

## Phase D — finalize

- [x] **T5 — Docs (English guide)** (R1–R4)
  - Files: `docs/developers-guide-en.md`
  - Do: one line in the Grace/AI-agent or designer section noting that when the
    agent repositions controls they animate into place (~1 s). English only; do
    **not** touch the translations. (No i18n `Tr` keys — the effect adds no text.)
  - Verify: line reads correctly.

- [x] **T6 — Version + CHANGELOG** (feature)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: minor bump; changelog entry describing animated agent control moves.
  - Verify: `cargo build -p cobolt-ide`.

- [x] **T7 — Finalize & AC sweep**
  - Do: full crate test run; tick every acceptance criterion in `spec.md`; list
    the operator's manual/visual checks (move several controls → simultaneous ~1 s
    ease-in-out glide; IDE responsive; save right after shows final positions;
    second move mid-animation retargets smoothly; light + dark).
  - Verify: `cargo test -p cobolt-ide` green; AC1–AC6 satisfied.

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, the English guide is
updated (translations untouched), version/CHANGELOG bumped as a **feature**, and
the change is left uncommitted for the operator to commit/push per their rules
(do **not** commit/push unless asked).
