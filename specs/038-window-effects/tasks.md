<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Project window entrance & exit effects

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-07-30

Ordered, small, independently-verifiable tasks. Each names the files it
touches, the requirement(s) it satisfies, and how to verify it. Check off as
completed.

- [x] **T1 — window_fx engine** (R4; plan D1)
  - Files: `crates/cobolt-forms/src/window_fx.rs` (new), `lib.rs` export
  - Do: `WindowEffect` enum (id round-trip: `none`, `fade`, `zoom`,
    `slide-*`, `expand-title-bar`, `radar-wipe`, `iris-wipe`, `blinds`,
    `checkerboard`, `matrix-rain`, `genie`), `Easing`, and
    `paint_window_fx(painter, rect, t, effect, face)` painting the
    transformed/masked face; MatrixRain with deterministic seed, classic
    katakana/digit glyphs, column cap (max 160).
  - Verify: `cargo test -p cobolt-forms window_fx` — id round-trip, easing
    monotonicity, mask area sanity at t=0/0.5/1, column cap (each printed).

- [x] **T2 — Form opt-out flag** (R3, AC5 model half)
  - Files: `crates/cobolt-forms/src/model.rs`, `xml.rs`
  - Do: `Form::window_effects: bool` default true; `.cfrm` attr
    `window-effects` written only when false (additive, 037 pattern).
  - Verify: `cargo test -p cobolt-forms --lib` — round-trip false, absent
    ⇒ true, default-valued attr not written (printed).

- [x] **T3 — Project settings model** (R1, R2, R5, AC1)
  - Files: `crates/cobolt-ide/src/project_model.rs`,
    `crates/cobolt-ide/src/app.rs` (new-project path),
    `crates/cobolt-compiler/src/lib.rs` (FormsConfig mirror)
  - Do: serde-defaulted `[forms]` fields per plan §3 + typed accessors;
    NEW projects written with matrix-rain/600/ease-out entrance, None
    exit, restore off.
  - Verify: `cargo test -p cobolt-ide forms_config_effects` — old toml ⇒
    None/None/off; defaults for new projects; value round-trip (printed).
    `cargo build -p cobolt-compiler`.

- [x] **T4 — Kill-switch** (R14, AC11 half)
  - Files: `crates/cobolt-ide/src/debug_settings.rs`, `i18n.rs` (label ×6)
  - Do: `no_window_fx` switch in Help → Debug Settings (plan D3);
    `child_env` exports `PRC_NO_WINDOW_FX=1` when on.
  - Verify: `cargo test -p cobolt-ide debug_settings` — env present
    when on, absent/0 when off (printed); i18n test green.

- [x] **T5 — Spawn plumbing** (R7 prereq; plan D2)
  - Files: `crates/cobolt-ide/src/form_runtime.rs`,
    `crates/cobolt-ide/src/app.rs` (spawn call sites)
  - Do: `--fx-entrance <id:ms:easing>` / `--fx-exit …` / `--fx-restore`
    args, passed only when the form's `window_effects` is true and the
    kill-switch is off; arg formatting helper shared with T6's parser.
  - Verify: `cargo test -p cobolt-ide fx_spawn_args` — args formatted /
    suppressed per opt-out and kill-switch (printed).

- [x] **T6 — Host entrance + control-animation gate** (R7, R8, AC3, AC6)
  - Files: `crates/cobolt-cli/src/form_gui.rs`
  - Do: parse `--fx-*`; while entrance `t < 1` paint the animated face via
    `window_fx` instead of the live UI; the `anim_started` gate for
    control load animations additionally waits for entrance completion;
    onLoad dispatch untouched (R13).
  - Verify: `cargo test -p cobolt-cli fx_args` (parse round-trip printed);
    `cargo build -p cobolt-cli`; operator visual: every catalogue effect
    plays on open (AC3); a control-animation form waits for the entrance
    then animates (AC6).

- [x] **T7 — Restore replay** (R9, AC7)
  - Files: `crates/cobolt-cli/src/form_gui.rs`
  - Do: track `ViewportInfo` minimized edge (alongside the 037 fullscreen
    mirror); on true→false with `--fx-restore`, replay the entrance —
    no form events, no control-animation replay.
  - Verify: `cargo build -p cobolt-cli`; operator visual: restore replays
    with the option on, instant with it off; Output shows no onLoad.

- [x] **T8 — Exit effect through the close path** (R10, R11, AC8, AC9;
  plan D6)
  - Files: `crates/cobolt-cli/src/form_gui.rs`
  - Do: after the 037 veto check passes (user close, handle close,
    STOP RUN), play exit (t 1→0) then send the actual close; vetoed close
    plays nothing; onClose fires once at the actual close (R13).
  - Verify: `cargo test -p cobolt-runtime form_state_lifecycle` still
    green (no veto regression); operator visual: exit plays then closes;
    Waiting form refuses with no animation; onClose once (Output).

- [x] **T9 — Settings UI + preview** (R1, R2, R6, AC2, AC4)
  - Files: `crates/cobolt-ide/src/panels/settings_form.rs`, `i18n.rs`
  - Do: appearance-section rows (entrance/exit combos + duration/easing,
    restore checkbox) beside the form-theme default (plan Q5); Preview
    button plays the chosen effect over a miniature face via `window_fx`;
    respects the kill-switch.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide i18n`;
    operator visual: rows shown, preview plays without running a form,
    new project shows matrix-rain defaults (AC2).

- [x] **T10 — Form properties row + validators** (R3, R16, AC5, AC12 half)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`, `agent.rs`,
    `panels/designer.rs` (list test), `i18n.rs`
  - Do: WindowEffects checkbox in the form section; `form_property_valid`
    gains `windoweffects`; list-agreement test extended both directions.
  - Verify: `cargo test -p cobolt-ide property_key_case` green; operator
    visual: opted-out form opens instantly while others animate (AC5).

- [x] **T11 — System KB docs tables** (R15, AC12 half; steering)
  - Files: `crates/cobolt-compiler/src/lib.rs`
  - Do: docs-table entries for the project settings, the form boolean,
    the effect catalogue, and the kill-switch. *(Store reindex NOT run —
    operator-request only; freshness test red is expected.)*
  - Verify: `cargo test -p cobolt-compiler` green; entries present in the
    published docs (grep).

- [x] **T12 — Docs & i18n sweep**
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: guide section (project effects, catalogue, opt-out, restore
    option, R8 sequencing, kill-switch, R12 chromeless note, MatrixRain
    perf note); verify every new label exists ×6.
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations).

- [ ] **T13 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: minor bump → next `1.45.0` + feature CHANGELOG entry; full sweep
    `cargo build --workspace` + `cargo test --workspace`
    (`--exclude cobolt-bench`); walk AC1–AC12 with the operator for the
    visual ones (AC3, AC4, AC5, AC6, AC7, AC8-visual, AC9, AC10).
  - Verify: workspace green; every AC checked in spec.md; feature isolated
    in its own commit(s) — do **not** commit/push unless the operator
    asks (push-window + forum rules apply at release time).

## Done criteria
All acceptance criteria in spec.md are checked, tests pass, docs updated, and
the change is split into fix/feature commit(s) per the operator's rules (do
**not** commit/push unless the operator asks).
