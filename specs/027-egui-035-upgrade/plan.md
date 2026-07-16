<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — egui 0.35 platform upgrade (+ MCP agent access)

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-15

## 1. Approach

Migrate stepwise, one egui minor at a time (0.29 → 0.30 → 0.31 → 0.32 → 0.33 →
0.34 → 0.35), compiling **and running** the IDE at each step, entirely on the
`egui-035` branch (R1, R8). Stepping matters because egui 0.34 **removed
everything previously deprecated**: jumping straight to 0.35 would turn
guided deprecation warnings into hard errors with no migration hints. At each
step: bump all egui-family crates together, fix errors, fix every deprecation
warning (never suppress, R8), build all five egui-consuming crates, run the
crate test suites, and smoke-run the IDE.

Verified migration surface per step (counts from `grep` inventory of this
codebase, 2026-07-15):

| Step | Breaking change | Our exposure |
|------|-----------------|--------------|
| 0.30 | `Area::new(Id)` signature; hit-test rewrite | 7 `Area::new` sites |
| 0.31 | `Rounding` → `CornerRadius`; radius/margin/shadow become `u8`/`i8`; Frame padding includes stroke | **85 `Rounding` sites**, all `Frame::` (57) reviewed |
| 0.32 | `id_source`→`id_salt` (1); panels deprecated for unified `Panel`; menus close-on-click; `SelectableLabel` removed; `ImageButton` deprecated | 10 SidePanel + 17 TopBottomPanel + 31 CentralPanel; 39 `menu::` sites; 36 `selectable_label` calls (verify helper survives) |
| 0.33 | `screen_rect` deprecated → `content_rect`/`viewport_rect`; MSRV 1.86 | **37 `screen_rect` sites** |
| 0.34 | `App::update`→`App::ui`; viewports get `&mut Ui` not `&Context`; `Context::style`→`global_style`; `available_rect` removed; **skrifa font backend**; deprecated APIs removed; MSRV 1.92 | 3 `eframe::App` impls (ide `app.rs:6585`, cli `form_gui.rs:509`, compiler `lib.rs:870`); 26 viewport sites; 6 `available_rect`; `fonts.rs` + ab_glyph validation |
| 0.35 | classes system; `impl Into<f32>` args removed; inspection protocol | adopt `egui_inspection` + `egui_mcp` (R3) |

**MCP architecture (R3/R4):** `cobolt-ide` calls
`egui_inspection::serve(&ctx, "127.0.0.1:<port>")` unconditionally at app
start (always-on per Q1), binding **localhost only**. The inspection protocol
exposes the AccessKit widget tree, input injection, and screenshots over TCP
(MessagePack, request→response). Agents connect through the official
`egui-mcp` bridge (stdio MCP ↔ TCP inspection, built on `rmcp`); we document
the client config and ship nothing extra in-process. Port default **5719**
(egui_inspection's documented default), overridable in AI settings.
`cobolt-cli` (rcrun), `cobolt-compiler` (packaged FormApp), `cobolt-forms`,
and `cobolt-media` never depend on `egui_inspection`/`egui_mcp` and never
enable eframe's `inspection` feature — R4 holds by construction; AC4 verifies
via `cargo tree` per-crate + a runtime listener check.

**Branch sync (R6/R7):** before any migration work each session: fetch, check
`egui-035` history for foreign commits (revert to my last commit if found,
standing authorization), then port any new `main` commits — re-implemented
against 0.35 APIs, one branch commit per main commit, message referencing the
main SHA. First action of implementation: sync the branch with current `main`
HEAD (it was cut at 20d18aa; main has moved).

## 2. Affected crates / files

- **`Cargo.toml` (workspace)** — `rust-version` 1.75 → 1.92.
- **`crates/cobolt-ide/Cargo.toml`** — egui/eframe/egui_extras/egui_glow
  0.29→0.35; `egui_commonmark` 0.18→**0.24** (released 2026-06-26 in lockstep
  with egui 0.35); replace `ab_glyph` with `skrifa` (the parser epaint ≥0.34
  actually uses — keeps R10's "validate with the same parser" guarantee);
  add `egui_inspection` 0.35.
- **`crates/cobolt-cli/Cargo.toml`, `cobolt-forms`, `cobolt-media`,
  `cobolt-compiler`** — egui-family bump only; **no** inspection/MCP deps.
- **`crates/cobolt-ide/src/app.rs`** — `App::update`→`App::ui`; panel API;
  37 `screen_rect` sites cluster here; menus (close-on-click parity);
  inspection `serve()` call + port setting.
- **`crates/cobolt-ide/src/panels/designer.rs`** — largest file: viewports
  (`show_viewport_immediate` → `Ui`-based), Rounding renames, event modal.
- **`crates/cobolt-ide/src/panels/rounded_clip.rs`** — egui_glow raw-GL
  callback (spec 017 rounded clip): re-validate `CallbackFn` API and stencil
  approach against 0.35's paint pipeline (see Risks).
- **`crates/cobolt-ide/src/fonts.rs`** — skrifa-based validation replacing
  ab_glyph; verify CJK fallback under the new hinting-enabled renderer.
- **`crates/cobolt-ide/src/panels/debugger.rs`, `editor.rs`, `output.rs`,
  `properties.rs`, `settings_form.rs`, `theme*.rs`, `welcome.rs`,
  `form_runtime.rs`, `inspector.rs`** — mechanical renames + behavior parity.
- **`crates/cobolt-forms/src/…` (render feature), `cobolt-media`** —
  CornerRadius/paint API renames.
- **`crates/cobolt-cli/src/form_gui.rs`, `cobolt-compiler/src/lib.rs`** —
  `App::ui` migration for the runtime/packaged form window.
- **`crates/cobolt-ide/src/i18n.rs`** — new Tr keys ×6: MCP port setting
  label, MCP status line (R11).
- **`docs/developers-guide-en.md`** — at merge: "Driving the IDE with an AI
  agent (MCP)" section + egui 0.35 note (AC10).
- **`specs/steering/tech.md`** — stack line 0.29→0.35 at merge.

## 3. Data / model changes

- **None to `.cfrm` / project files / generated COBOL.** Border radii etc.
  stay `f32` in the model; converted at the paint edge (`CornerRadius` is
  `u8` since 0.31 — clamp 0..=255, document rounding).
- **IDE settings:** one new persisted field — inspection port (default 5719).
- **Generated-code contract untouched** (banner, regenerate-on-action).

## 4. Key decisions & alternatives

- **Stepwise minor-by-minor migration** — Why: 0.34 removed all deprecated
  APIs; stepping keeps compiler-guided renames (deprecation messages name the
  replacement); each step is a committable, runnable checkpoint. — Rejected:
  one-hop 0.29→0.35 (hundreds of "no such method" errors with no hints —
  pure guesswork, violates R8).
- **Inspection served in-process; MCP via official `egui-mcp` bridge** — Why:
  `serve()` is the documented always-on path; localhost-only TCP; zero extra
  in-process protocol code; the bridge is the officially supported client
  path. — Rejected: embedding an rmcp MCP server inside the IDE (more code,
  duplicate of the maintained bridge); env-var gating via
  `EGUI_INSPECTION` (Q1 says always on — no gate).
- **`skrifa` replaces `ab_glyph` for font validation** — Why: R10's guarantee
  is literally "same parser as epaint", and epaint ≥0.34 uses skrifa; keeping
  ab_glyph would validate with the *wrong* parser. skrifa is already in the
  tree transitively via epaint (deps rule: bump/promote of existing, not new
  ecosystem). — Rejected: keep ab_glyph (guarantee silently broken).
- **`egui_commonmark` 0.24** — lockstep release for 0.35; Q5 closed.
- **Sync = re-implement per main commit** (not `git merge`) — Why: R6 demands
  adaptation to 0.35 semantics; a merge would auto-take 0.29-idiom code.
  One branch commit per main commit keeps AC6 auditable. — Rejected: periodic
  bulk merges (loses per-commit audit, mixes concerns).

## 5. Risks & mitigations

- **Risk: `rounded_clip.rs` raw-GL backdrop blit breaks** (paint pipeline and
  text rasterizer changed in 0.34/0.35; egui_glow callback API may differ).
  → Mitigation: re-validate at the 0.34 step specifically; if `CallbackFn` is
  gone, reimplement on the current callback API; acceptance = visual parity
  on glass/rounded themes. If truly stuck: ask operator (R8) — never ship a
  degraded clip silently.
- **Risk: skrifa renders our fonts differently** (hinting on by default in
  0.35, CJK fallback, bitmap-font rejection behavior). → Mitigation: AC8
  six-language visual pass + custom-font load test; keep `fontdb` enumeration
  unchanged.
- **Risk: menu close-on-click (0.32) changes IDE menu UX** (39 sites). →
  Mitigation: parity pass over every menu; restore stay-open behavior where
  the 0.29 UX depended on it (egui exposes opt-outs).
- **Risk: anti self-inflation contract under new layout code** (R9; `Resize`
  internals may have changed across 6 versions). → Mitigation: re-test
  debugger + error modals at each step (AC7); the seeded-box pattern is
  content-independent, so it should survive — verify, don't assume.
- **Risk: viewport→`Ui` refactor destabilizes designer windows / Run Form**
  (26 sites, immediate viewports). → Mitigation: migrate one viewport kind at
  a time; manual matrix in AC2 covers designer window, debugger viewport,
  Run Form.
- **Risk: `main` keeps moving while the branch matures** (already true:
  1.30.x fixes landed since cut). → Mitigation: R6 same-session ports; sync
  audit table in the branch (`specs/027-egui-035-upgrade/sync-log.md`).
- **Risk: inspection port always open worries** → bound to 127.0.0.1 only;
  documented; port configurable. (Operator explicitly chose always-on.)

## 6. Test strategy

- **Per-step gate (each egui minor):** `cargo build` + `cargo test` for
  cobolt-ide, cobolt-forms, cobolt-media, cobolt-cli, cobolt-compiler; zero
  deprecation warnings from egui APIs; IDE smoke-run.
- **New automated UI tests (egui `kittest`, dev-dependency, ecosystem-OK):**
  in `cobolt-ide` — (a) error modal opens at 800×450 and size is identical
  across 120 simulated frames (R9/AC7, reports measured sizes); (b) main
  window builds all panels without panic (smoke, reports panel list).
- **MCP round-trip test (AC3):** scripted client via the `egui-mcp` bridge:
  list widget tree → click "New Form" → place a Button control → open its
  onClick event editor; asserts each step's tree state; reports the action
  log. Run against a debug IDE instance.
- **R4 check (AC4):** `cargo tree -p cobolt-cli -p cobolt-compiler | grep -c
  inspection` == 0, plus runtime assertion that a packaged demo app opens no
  listening socket (`lsof` during test, reported).
- **Font tests (AC8):** load each of the six languages' UI + a bundled custom
  `.ttf` project font; skrifa-validation unit test rejects a bitmap-only font
  (the original panic case) and reports accepted/rejected names.
- **Manual/visual (AC2 walkthrough):** operator checklist — designer
  (multi-viewport), editor, event editor, output, debugger, project tree,
  file dialogs, Run Form, themes/glass, AI assistant, error modals; compared
  against `main` build side-by-side.
- **Sync audit (AC6):** `sync-log.md` table main-SHA → branch-SHA, checked
  before merge.

## 7. Steering compliance

- [ ] i18n: MCP port/status strings ×6 in `i18n.rs` (R11).
- [ ] Generated-code banner + regenerate-on-action contract preserved
      (no codegen changes; AC2 re-verifies Build/Run/Debug/Check).
- [ ] English dev guide updated at merge (MCP section); translations
      untouched.
- [ ] Fix vs feature: **feature** — minor bump + CHANGELOG at merge; interim
      main-fix ports keep their fix classification; migration commits never
      mixed with ported fixes.
- [ ] No "cobolt" in user-facing text; COBOL identifiers English.
- [ ] Push window respected for branch pushes; no forum post until merge.
