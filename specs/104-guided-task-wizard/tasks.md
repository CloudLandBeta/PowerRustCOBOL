# Tasks — Task Wizard

- **Status:** implemented — awaiting the operator's look and the full sweep (T27)
- **Plan:** ./plan.md   **Date:** 2026-10-17

Ordered, small, independently verifiable tasks. Each names the files it touches, the
requirement(s) it satisfies, and how to verify it. Checked off as completed. All paths are in
`crates/cobolt-ide/src/` unless noted; tests run with `cargo test -p cobolt-ide --bin cobolt-ide`.

## Phase 1 — The Walkthrough, frozen, and the shared spotlight

- [x] **T1 — Characterise the Walkthrough** (R38, AC27)
  - Files: `panels/walkthrough.rs` (test module).
  - Do: a test that paints each of the six steps on a fixed screen and theme and records the
    shapes (kind, rect, colour, text) into a fingerprint; the expected fingerprints are captured
    from the current code.
  - Verify: the test passes on the untouched code.
- [x] **T2 — Extract `panels/spotlight.rs`** (R38)
  - Files: `panels/spotlight.rs` (new), `panels/walkthrough.rs`, `panels/mod.rs`.
  - Do: move `scrim_bands`, `place_balloon`, `Balloon`, `Side`, `balloon_palette`, `Palette`, `DIM`
    and the geometry constants; `walkthrough.rs` re-exports them.
  - Verify: T1 and every walkthrough test unchanged and green.
- [x] **T3 — `blockers` and `place_balloon_avoiding`** (R12, R13, R16)
  - Files: `panels/spotlight.rs`.
  - Verify: property tests — disjoint pieces, union = screen − holes, count bound; balloon off the
    avoided rect, tail inside the aim.

## Phase 2 — The switch, the catalogue, persistence

- [x] **T4 — `UiPrefs` fields** (R5, R29–R31)
  - Files: `ui_prefs.rs`.
  - Verify: round-trip test extended; an older `ui.toml` reads them empty and off.
- [x] **T5 — IDE toolbar split toggle** (R1–R4)
  - Files: `panels/toolbar.rs`, `aurora.rs`, `app.rs`.
  - Verify: engaged look contrast on painted colours over the 32 themes and the Aurora pill.
- [x] **T6 — Designer toolbar split toggle** (R1–R4)
  - Files: `panels/designer.rs`, `panels/designer_grid_layout_edit_tests.rs`, `app.rs`.
  - Verify: the engaged icon differs from the idle one; the existing toolbar tests still pass.
- [x] **T7 — Help item** (R6)
  - Files: `menu_layout.rs`, `app.rs`.
  - Verify: the menu-layout tests updated for 18 items; the item sits after *IDE Walkthrough*.
- [x] **T8 — Catalogue popup** (R3, R7–R9)
  - Files: `panels/task_wizard/catalogue.rs`.
  - Verify: render test — eight tasks, states, disabled with reason when no project.

## Phase 3 — Engine

- [x] **T9 — Facts, conditions, task table types** (R17, R19, R28, R37)
  - Files: `panels/task_wizard/{facts,conditions,tasks}.rs`.
- [x] **T10 — Engine state machine** (R12, R17–R24, R29)
  - Files: `panels/task_wizard/mod.rs`.
  - Verify: unit tests for every route; a fixture task (AC26).

## Phase 4 — Overlay, anchors, input

- [x] **T11 — Anchors sink** (R35)
  - Files: `panels/task_wizard/anchors.rs`.
- [x] **T12 — Overlay painter** (R12–R16, R33, R34)
  - Files: `panels/task_wizard/overlay.rs`.
  - Verify: render tests AC7, AC8, AC9, AC23, AC24.
- [x] **T13 — Input filter** (R22, R23)
  - Files: `panels/task_wizard/input.rs`.
  - Verify: render tests AC14, AC15.

## Phase 5 — App glue

- [x] **T14 — Glue module and call sites** (R4, R11, R15, R24, R35)
  - Files: `app/task_wizard_glue.rs` (new), `app.rs`.
  - Verify: AC25 (off: no sink, no layer); AC16 (pause).

## Phase 6 — The eight tasks

Each: anchor marks, facts getters, the `TaskDef`, its strings in six languages, AC6/AC17/AC18.

- [x] **T15 — Configure a model provider** — `panels/models_modal.rs`, `panels/settings_form.rs`,
  `panels/project.rs`.
- [x] **T16 — Connect an agent to a model** — `panels/agents_modal.rs`.
- [x] **T17 — Bind a control to data** — `panels/properties.rs`, `panels/designer.rs`.
- [x] **T18 — Set up a DataGrid** — `panels/properties.rs`, `panels/grid_edit.rs`, `panels/designer.rs`.
- [x] **T19 — Fill a TreeView** — `panels/properties.rs`.
- [x] **T20 — Build a SideMenu** — `panels/designer.rs` (menu editor).
- [x] **T21 — Build a menu** — `panels/designer.rs` (menu editor).
- [x] **T22 — Add an animation** — `panels/properties.rs`, `panels/designer.rs`.

## Phase 7 — Offers

- [x] **T23 — Offers on DataGrid, TreeView, SideMenu, MenuBar** (R10, R30)
  - Files: `panels/task_wizard/offers.rs`, the glue.

## Phase 8 — Close-out

- [x] **T24 — i18n module and six-language probe** (R32, AC22)
- [x] **T25 — Secret-safety test** (R28, AC20) and mechanical-step audit (AC19)
- [x] **T26 — Developer's Guide section** (`docs/developers-guide-en.md`)
- [ ] **T27 — Finalize**: `version.rs` z bump, `CHANGELOG.md`, full `cargo test -p cobolt-ide --bin
  cobolt-ide --no-fail-fast` with every `test result:` line read, DataGrid Quality & Compatibility
  review, quarantine sweep, staged with a prepared commit message.

## Deviations from the plan, found while building (2026-10-17)

- **The surfaces were not quite as §3 assumed.** The DataGrid has no grid-edit toggle on the
  Designer toolbar (edit mode is a double-click on the canvas), its settings window has no
  Close button (only ✕), and the Data Binding editor has no mappings list (mappings are
  derived on Apply). So task 3 lights the source cards and the source's fields, and has no
  *Do this for me* (there was nothing empty to fill). Task 4's last steps light the grid on the
  canvas and the edit-mode toolbar.
- **A step's `via` is a list.** A Properties step can be closed in two ways — the pane hidden,
  or another tab up — so it names both what reopens the pane and the tab.
- **One target can be several widgets.** The animation's Trigger, Effect and Repeat rows are
  sorted A–Z by the pane; `Marks::find` unites every mark of one anchor in one window.
- **Glyphs.** 🎯, ✨, ▶ and ◀ are not in the IDE's base fonts (a test proves which are); the
  wizard uses 🗺, ★, ⏵ and ⏴ and draws a folded map on the Designer toolbar.
- **The tree's anchors** are read from `ProjectPanel::walkthrough_anchors()` — no change to the
  tree was needed.
- **The DataGrid guardrail review** found four ways a running step disturbed grid editing.
  All four are fixed, and each has a test that fails with its fix removed:
  - **R1:** `Esc` was swallowed, so a cell editor's cancel became a commit. R22 was
    amended: a field's `Esc` is passed on.
  - **R2:** the lit window was raised every frame and buried the windows it opens. It is now
    raised once per step.
  - **R3:** a press on the balloon over the canvas ended edit mode. Grid-edit now counts only
    a press on the canvas's own layer.
  - **R4:** the second `Esc` exited instead of leaving edit mode. Edit mode is now one of the
    `Esc` stages.

  Subjects are also keyed by the form's path, not its name.
- **AC15 (the target window's rect is stable for 120 frames in six languages)** is covered by
  construction (the wizard never sizes a window, and the balloon is sized from constants), not
  by a dedicated test.

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and the change is a
feature commit of its own (the operator commits; the commit hook blocks commits from this
worktree, and nothing is merged or pushed without the operator).
