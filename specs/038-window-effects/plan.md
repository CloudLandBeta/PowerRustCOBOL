<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Project window entrance & exit effects

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-07-30

## 1. Approach

**Core idea — animate the static face, hand off to the live UI.** Every
effect renders the form's *designer face* (the shared
`cobolt_forms::paint::draw_control` pipeline both the designer canvas and
the run form already use) transformed by the effect's progress `t ∈ 0..1`:
scaled for Zoom/Genie, clipped through mesh masks for the wipes, glyph-rain
overlaid for MatrixRain, alpha-blended for Fade. While `t < 1` the host
paints only this animated face; at `t = 1` it swaps to the normal live UI.
This sidesteps render-to-texture entirely (egui repaints the face each frame
at the transformed geometry), guarantees pixel parity with the designer, and
gives one code path for entrance (t: 0→1) and exit (t: 1→0). It also
satisfies R7's interactivity bound: the live UI takes over exactly at the
animation's end.

- **Effect engine (R4)** — new module `cobolt-forms/src/window_fx.rs`:
  `WindowEffect` enum (the R4 catalogue), `Easing`, and
  `paint_window_fx(painter, rect, t, effect, face: impl FnOnce(&Painter,
  Rect))` where `face` paints the form face into an effect-chosen
  geometry/clip. Living in `cobolt-forms` lets the run-form host, the IDE
  preview, and (later) the packaged-binary template share it. MatrixRain
  keeps a deterministic per-window seed, classic katakana/digit glyphs from
  the bundled CJK-capable fonts (`fonts.rs`), column count capped
  (≈ width/14 px, hard max 160) for the perf constraint.
- **Project settings (R1, R2, R5)** — extend
  `project_model.rs::FormsConfig` (IDE) with serde-defaulted fields:
  `entrance_effect`, `entrance_ms`, `entrance_easing`, `exit_effect`,
  `exit_ms`, `exit_easing`, `entrance_on_restore`. Absent in old
  `cobolt.toml` ⇒ None/None/off (R5 back-compat); the IDE's project-new
  path writes MatrixRain defaults for NEW projects only. (The compiler's
  own minimal `FormsConfig` gains the same fields for the packaged binary,
  serde-defaulted.)
- **Form opt-out (R3)** — `Form::window_effects: bool` (default true),
  `.cfrm` attribute `window-effects`, written only when false (same
  additive pattern as 037's fields).
- **Runtime playback (R7–R10)** — in `cobolt-cli/src/form_gui.rs`:
  - Spawn args: `ExternalFormRun::spawn` (form_runtime.rs:828) grows the
    effect settings (args `--fx-entrance <id:ms:easing>`, `--fx-exit …`,
    `--fx-restore`), passed only when the form's `window_effects` is true
    and the kill-switch is off.
  - Entrance: while `fx_t < 1`, paint the animated face instead of the
    live UI; controls' load-time animations start when the entrance ends —
    the existing `if !self.anim_started { anim.start_form_load(...) }`
    gate simply also waits for `fx_done` (R8). onLoad dispatch is
    untouched (R13).
  - Restore replay (R9): the host already reads `ViewportInfo` each frame
    (fullscreen mirroring, 037); track `minimized` the same way and replay
    the entrance on a true→false transition when `--fx-restore` was given.
    No form events, no control-animation replay.
  - Exit (R10, R11): in the single close path 037 introduced (supervisor
    veto → close): after the veto check passes, enter `fx_exit` mode
    (t: 1→0 over the exit duration) and send the actual
    quit/`ViewportCommand::Close` when it completes. Program-end
    (STOP RUN) closes take the same path. A vetoed close never starts the
    animation.
- **Kill-switch (R14)** — a new switch in `debug_settings.rs` (Help →
  Debug Settings; resolves spec Q3 per the 1.36 pattern): IDE-wide,
  per-frame sync, and exported to children via `DebugSettings::child_env`
  (`PRC_NO_WINDOW_FX=1`), which the host checks before honouring any
  `--fx-*` args. The IDE-side preview respects the same flag.
- **Settings UI + preview (R1, R6)** — `panels/settings_form.rs` appearance
  section (resolves Q5), next to the form-theme default: two effect combo
  rows with duration/easing, the restore checkbox, and a **Preview** button
  that plays the chosen effect over a miniature form face (the project's
  main form when available, else a placeholder card) using the same
  `window_fx` module.
- **Validators & docs (R15, R16)** — `agent.rs::form_property_valid` gains
  `windoweffects`; the 1.42.2 list-agreement test extends; compiler docs
  tables document the two project settings, the form boolean, catalogue and
  kill-switch (docs update in-change; store reindex operator-gated).

## 2. Affected crates / files

- `crates/cobolt-forms/src/window_fx.rs` — NEW: effect enum, easing, paint.
- `crates/cobolt-forms/src/model.rs` + `xml.rs` — `window_effects` bool.
- `crates/cobolt-ide/src/project_model.rs` — FormsConfig effect fields +
  accessors; new-project defaults.
- `crates/cobolt-ide/src/panels/settings_form.rs` — settings rows + preview.
- `crates/cobolt-ide/src/panels/properties.rs` — form WindowEffects row.
- `crates/cobolt-ide/src/form_runtime.rs` — spawn args.
- `crates/cobolt-ide/src/debug_settings.rs` — kill-switch + child env.
- `crates/cobolt-ide/src/agent.rs` + `panels/designer.rs` — validator/lists.
- `crates/cobolt-cli/src/form_gui.rs` — arg parsing, entrance/exit/restore
  playback, control-animation gate.
- `crates/cobolt-compiler/src/lib.rs` — FormsConfig fields (packaged
  binary), KB docs tables.
- `crates/cobolt-ide/src/i18n.rs` — labels ×6.
- `docs/developers-guide-en.md` — feature section.

## 3. Data / model changes

| Where | Field | Default | Persisted as |
|-------|-------|---------|--------------|
| cobolt.toml `[forms]` | `entrance-effect` | "" (=None; new projects: "matrix-rain") | string id |
| cobolt.toml `[forms]` | `entrance-ms` / `entrance-easing` | 600 / "ease-out" | int / string |
| cobolt.toml `[forms]` | `exit-effect` / `exit-ms` / `exit-easing` | "" / 400 / "ease-in" | as above |
| cobolt.toml `[forms]` | `entrance-on-restore` | false | bool |
| `.cfrm` `<Form>` | `window-effects` | true (attr written only when false) | bool |
| Debug settings | `no_window_fx` | false | existing store + `PRC_NO_WINDOW_FX` child env |

Effect ids (stable, English): `none`, `fade`, `zoom`, `slide-left/right/
top/bottom`, `expand-title-bar`, `radar-wipe`, `iris-wipe`, `blinds`,
`checkerboard`, `matrix-rain`, `genie`.

## 4. Key decisions

- **D1 — static-face animation, not live-UI transformation.** egui cannot
  scale/warp a live widget tree without render-to-texture; the shared face
  renderer already draws every control identically to the designer.
  Rejected: offscreen texture capture (heavy plumbing, deferred with the
  true genie); overlay-masking the live UI (works only for masks, would
  split the code path per effect family).
- **D2 — effects as spawn ARGS, not read from cobolt.toml by the child.**
  The child process stays project-file-agnostic (it already receives theme
  and icon this way); the IDE resolves form opt-out + kill-switch before
  spawning. Rejected: child parses cobolt.toml (duplicates resolution
  logic, breaks for standalone `run-form` of a bare .cfrm).
- **D3 — kill-switch in Debug Settings** (spec Q3): machine-wide scope,
  per-frame sync and `child_env` plumbing already exist there (1.36
  pattern). Rejected: a new appearance-settings home (new plumbing for the
  same semantics).
- **D4 — zoom origin = window centre** (spec Q1): caller-position origin
  needs the multi-window host's caller geometry, which is T1-gated;
  recorded as a follow-up when 037's child windows land.
- **D5 — exit default stays None for new projects** (spec Q4, pending
  operator confirmation at the gate): an exit animation delays close on
  every window; opt-in feels right.
- **D6 — program-end closes play the exit effect too**: one close
  choreography regardless of why the window closes (user, handle, STOP
  RUN); the veto path remains the only animation-free refusal.

## 5. Risks & mitigations

- **Risk: minimized-state detection differs per OS** (some platforms
  report restore a frame late). → The replay triggers on the observed
  true→false edge only; worst case the effect starts a frame after the OS
  restore completes. Manual check on macOS; Windows/Linux noted for the
  operator's cross-platform pass.
- **Risk: MatrixRain cost on large windows.** → column cap + glyph pool
  reuse; the kill-switch is the documented escape hatch (R14); perf line
  in the guide.
- **Risk: exit animation delays close in automated/CI-like use.** →
  kill-switch env var also honoured by bare `rcrun run-form` (any caller
  can set `PRC_NO_WINDOW_FX=1`).
- **Risk: static face ≠ live UI for exotic controls (charts with pushed
  data).** → the face renderer already draws pushed chart data and current
  property state; any residual mismatch lasts < 1 s and ends in the live
  UI. Accepted.
- **Packaged binary parity**: the compiler template still predates 037
  chrome; effects there join the SAME parity work item (tracked from 037,
  not expanded here beyond the serde fields).

## 6. Test strategy

- `cobolt-forms` (unit): `window_fx` — effect id round-trip, easing curves
  monotonic, mask coverage at t=0/0.5/1 (mesh area sanity), MatrixRain
  column cap honoured; `.cfrm` round-trip of `window-effects=false` and
  absent⇒true (prints each).
- `cobolt-ide` (unit): FormsConfig serde — old toml ⇒ None/off; new-project
  defaults ⇒ matrix-rain/600 (prints parsed values); validator
  list-agreement extended (windoweffects); debug-settings child env
  includes `PRC_NO_WINDOW_FX` when on.
- `cobolt-cli` (unit where headless): spawn-arg formatting; arg parsing
  back to `WindowEffect` (round-trip printed).
- **Manual/visual (operator)**: each catalogue effect on open and close;
  R8 sequencing with a control-animation form; restore replay on/off;
  Waiting-veto plays nothing; kill-switch; settings preview. (I never
  drive the app — these are your checks, listed in tasks.)

## 7. Steering compliance

- i18n ×6 for every new label; COBOL identifiers English; no "cobolt" in UI
  text. Generated-code contract untouched (R13). English guide updated;
  translations untouched. Feature ⇒ minor bump + `[Noticia]` f=96 within
  the push window. System-KB docs tables in-change; reindex operator-gated.
  egui 0.35 only; effects never write window size (anti-self-inflation).
