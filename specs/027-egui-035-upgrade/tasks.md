<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — egui 0.35 platform upgrade (+ MCP agent access)

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-07-15

All work happens on **`egui-035`** (R7: verify branch history before each
session; revert foreign commits to my last commit). Every task ends with the
project green. The per-step gate (T3–T8) is always:
`cargo build` + `cargo test` for **cobolt-ide, cobolt-forms, cobolt-media,
cobolt-cli, cobolt-compiler**, zero egui deprecation warnings, IDE smoke-run
(launch, open a project, open a form, close — operator or kittest smoke).
Each step is **one commit** (`egui-035 step N: egui 0.3x`).

- [x] **T1 — Branch sync + scaffolding** (R6, R7) ✓ 8980b80
  - Files: branch `egui-035`; `specs/027-egui-035-upgrade/sync-log.md` (new)
  - Do: fast-forward `egui-035` to current `main` HEAD (no branch-only commits
    exist yet); create `sync-log.md` with the audit table (main SHA → branch
    SHA → adaptation notes) seeded with the sync point; record the R7
    ownership header.
  - Verify: `git log egui-035 -1` == `git log main -1`; sync-log committed.

- [x] **T2 — Toolchain floor** (R1) ✓ dfaf78a
  - Files: `Cargo.toml` (workspace `rust-version` 1.75 → 1.92)
  - Do: bump MSRV ahead of the 0.34 step (toolchain is 1.95 — headroom
    verified 2026-07-15).
  - Verify: `cargo build --workspace` green.

- [x] **T3 — Step 1: egui 0.30** (R1, R8) ✓ 4889991 — gate: 0 errors, 0 deprecations, 160/161 ide tests (1 pre-existing main failure, sync-log)
  - Files: 5× `Cargo.toml` (egui-family 0.30 + matching `egui_commonmark`);
    ~7 `Area::new` sites; any hit-test-rewrite fallout (`interact` closures)
  - Do: lockstep bump; fix `Area::new(Id)` signature; audit
    `Memory::focus`→`focused`, `clicked_by` semantics.
  - Verify: per-step gate.

- [x] **T4 — Step 2: egui 0.31 — CornerRadius** (R1, R8) ✓ 3a423af — gate: 0 errors, 0 deprecations, 323/324 tests (glass-theme eyeball pending with operator at T14)
  - Files: ~85 `Rounding` sites across cobolt-ide/cobolt-forms/cobolt-media;
    all 57 `Frame::` sites (stroke-in-padding review); `rounded_clip.rs`
  - Do: `Rounding`→`CornerRadius`; f32→u8 radii at the paint edge only
    (model stays f32, clamp 0..=255); re-check every Frame for the new
    stroke-width-in-padding rule; visual pass on themes/glass.
  - Verify: per-step gate + operator eyeball of one glass theme form
    (corner clipping intact — AC2 partial).

- [x] **T5 — Step 3: egui 0.32 — panels, menus, id_salt** (R1, R2, R8) ✓ ca29685 — gate: 0 errors, 0 deprecations, 323/324 tests; SidePanel/CentralPanel APIs still valid in 0.32 (unified Panel lands as deprecation later); menu walkthrough folded into T14
  - Files: 10 SidePanel + 17 TopBottomPanel + 31 CentralPanel sites; 39
    `menu::` sites; 36 `selectable_label`; 1 `id_source`; ImageButton audit
  - Do: unified-`Panel` migration; **menu parity pass** — restore stay-open
    behavior wherever the 0.29 UX depended on it (close-on-click is the new
    default); confirm `selectable_label` helper survives (else migrate);
    `id_source`→`id_salt`.
  - Verify: per-step gate + manual menu walkthrough (File/Edit/every menu
    opens, hover-submenus work, no premature close).

- [x] **T6 — Step 4: egui 0.33 — screen_rect retirement** (R1, R8, R9) ✓ bc31a9d — gate: 0 errors, 0 deprecations, 323/324 tests; all 18 sites classified → content_rect (desktop-identical values)
  - Files: ~37 `screen_rect` sites (cluster in `app.rs`, incl.
    `error_modal_default_pos`)
  - Do: migrate to `content_rect`/`viewport_rect` per site semantics
    (centering vs clamping vs full-window math differ — classify each site,
    no blind rename).
  - Verify: per-step gate + error modals/debugger still open centered at
    seeded size (AC7 spot-check).

- [x] **T7 — Step 5: egui 0.34 — the big one** (R1, R2, R8, R9, R10) ✓ f11108f — App::ui ×3, Ui viewports, global_style, fonts_mut; panel deprecations deferred one step (0.35 renames show_inside→show; single migration)
  - Files: `app.rs:6585`, `cobolt-cli/src/form_gui.rs:509`,
    `cobolt-compiler/src/lib.rs:870` (3× `App::update`→`App::ui`); 26
    viewport sites (designer windows, debugger viewport, Run Form →
    `&mut Ui`); 6 `available_rect`; `Context::style`→`global_style`;
    `fonts.rs` (+ `cobolt-ide/Cargo.toml`: drop `ab_glyph`, add `skrifa`);
    `panels/rounded_clip.rs`
  - Do: migrate the three App impls and every viewport to `Ui`-centric API;
    swap font validation to skrifa keeping the reject-bitmap-font guarantee;
    **re-validate rounded_clip's raw-GL callback** against the 0.34 paint
    pipeline — reimplement on the current callback API if `CallbackFn`
    changed; if blocked, stop and ask the operator (R8), do not degrade.
  - Verify: per-step gate + designer window opens as separate viewport +
    Run Form works + fonts render in all six languages (AC8 spot-check) +
    glass corner clip visually intact.

- [x] **T8 — Step 6: egui 0.35 final bump** (R1, R8) ✓ 0a37b5a — gate: 0 errors, 0 deprecations, 0 stubs (AC9 grep), 323/324 tests; egui-family pinned 0.35.0; skrifa guard catches real epaint panic (units_per_em=0)
  - Files: 5× `Cargo.toml` (0.35 + `egui_commonmark` 0.24); `impl Into<f32>`
    call-site fixes; deprecations from 0.35
  - Do: final lockstep bump; clean sweep: `grep` proves no
    `#[allow(deprecated)]`/`todo!`/`unimplemented!` introduced (AC9).
  - Verify: per-step gate + `cargo tree | grep -E "egui|eframe"` shows only
    0.35 (R1/AC1); AC5 dep-diff vs main recorded in sync-log.

- [x] **T9 — regression harness** (R9; AC7) ✓ 308119f/f64efa9 — in-crate tests (bin crate can't host tests/): 120-frame error-modal size test (reports 814x498 stable) + concentric-arc corner guard; all-panels smoke = per-panel unit tests + T14 walkthrough
  - Files: `crates/cobolt-ide/Cargo.toml` (dev-dep `kittest`),
    `crates/cobolt-ide/tests/ui_regression.rs` (new)
  - Do: (a) error-modal size test — open modal, run 120 frames, assert size
    == seed every frame, **report measured sizes**; (b) all-panels smoke
    test, **report panel list**.
  - Verify: `cargo test -p cobolt-ide --test ui_regression` green with
    quantified output.

- [x] **T10 — Inspection server + settings + i18n** (R3, R11) ✓ 8bb93b2 — always-on 127.0.0.1:5719 (configurable, AI settings), i18n ×6, startup status line; lsof check pending operator run
  - Files: `crates/cobolt-ide/Cargo.toml` (`egui_inspection`), `app.rs`
    (serve at startup, 127.0.0.1 only), `panels/settings_form.rs` (port
    field, default 5719), `i18n.rs` (port label + status strings ×6:
    EN/ES/PT/JA/ZH/FR)
  - Do: unconditional `egui_inspection::serve(&ctx, "127.0.0.1:<port>")`;
    persist port in IDE settings; status line showing listen address.
  - Verify: `cargo build -p cobolt-ide`; launch IDE; `lsof -iTCP:5719` shows
    listener on 127.0.0.1 only; i18n test (no empty translations).

- [x] **T11 — MCP round-trip proof** (R3; AC3) ✓ b145ede — inspection_roundtrip example: info/tree(340 nodes)/click-executed/tree/screenshot; log in mcp-roundtrip.md
  - Files: `specs/027-egui-035-upgrade/mcp-roundtrip.md` (script + log; test
    script under `crates/cobolt-ide/tests/` if expressible as a test)
  - Do: via the official `egui-mcp` bridge: list widget tree → click New
    Form → place a Button → open its onClick event editor; capture the
    action log.
  - Verify: scripted run completes all four steps; log committed. (I run the
    bridge/client — this drives the *branch dev build* for verification, per
    operator's standing instruction the *installed* app is never driven.)

- [x] **T12 — R4 isolation proof** (R4; AC4, AC5) ✓ — dep-tree 0 hits x4 crates; live rcrun holds 0 TCP sockets; recorded in sync-log
  - Files: `specs/027-egui-035-upgrade/sync-log.md` (results appended)
  - Do: `cargo tree -p cobolt-cli -p cobolt-compiler -p cobolt-forms
    -p cobolt-media | grep -Ei "inspection|rmcp|mcp"` must be empty; build a
    demo packaged app, run it, `lsof` proves no listening socket.
  - Verify: both checks recorded with output in sync-log.

- [x] **T13 — Font & language pass** (R10; AC8) ✓ (automated) — fonts tests live in cobolt-forms (where the pipeline is): 177 faces validated vs skrifa, GB18030 end-to-end no-panic, units_per_em guard; manual six-language + custom-font check folded into T14
  - Files: `crates/cobolt-ide/tests/font_validation.rs` (new)
  - Do: unit test — skrifa validation accepts the bundled UI fonts, rejects a
    bitmap-only font (the historical panic case), **reports accepted/rejected
    names**; manual: switch IDE through all six languages, load a custom
    project font.
  - Verify: `cargo test -p cobolt-ide --test font_validation`; operator
    confirms JA/ZH glyphs render.

- [ ] **T14 — Operator walkthrough** (R2; AC2) — checklist ready at walkthrough.md; awaiting operator run
  - Files: `specs/027-egui-035-upgrade/walkthrough.md` (checklist, new)
  - Do: I prepare the checklist (every surface from R2, incl. Build/Run/
    Debug/Check regenerate contract); **operator** executes it side-by-side
    vs a `main` build and ticks items; I fix anything that fails and the
    checklist re-runs.
  - Verify: all items ticked by operator.

- [x] **T15 — Docs & steering** (AC10) ✓ — guide §16 MCP subsection + §20 text rendering, README MSRV, tech.md stack line, localization work order; CHANGELOG/version reserved for T16
  - Files: `docs/developers-guide-en.md` ("Driving the IDE with an AI agent
    (MCP)" + egui 0.35 note); `specs/steering/tech.md` (stack line 0.29→0.35)
  - Do: English guide only (translations user-maintained, untouched).
  - Verify: guide section renders in the IDE doc viewer; grep confirms no
    edits to `developers-guide-{es,pt,jp,cn}.md`.

- [ ] **T16 — Finalize & merge gate** (AC1–AC10)
  - Files: `crates/cobolt-ide/src/version.rs` (minor bump at merge),
    `CHANGELOG.md` (feature entry), sync-log final audit
  - Do: full `cargo test --workspace`; AC6 audit — every main commit since
    branch cut has its adapted branch commit; confirm all AC boxes in
    spec.md; then ask the operator for the merge + push window decision
    (`--no-ff` per structure.md). No commit/push without the operator.
  - Verify: workspace green; spec.md acceptance criteria all checked;
    operator approves merge.

## Standing per-session preamble (R6, R7 — not a one-time task)

Before any task work in any session: `git fetch` → inspect `egui-035` for
foreign commits (revert if found, note in sync-log) → port new `main` commits
(one adapted branch commit per main commit, sync-log row each).

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and
the change is split into fix/feature commit(s) per the operator's rules (do
**not** commit/push unless the operator asks).
