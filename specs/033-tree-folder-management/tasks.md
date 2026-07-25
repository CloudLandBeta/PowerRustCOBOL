<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Project tree folder management

- **Status:** done
- **Plan:** ./plan.md   **Date:** 2026-07-23

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

## Phase A — foundation (headless, keeps the tree green)

- [x] **T1 — `project_fs` module + validation** (R3, R8, R12, R13, R14, R20, R21)
  - Files: `crates/cobolt-ide/src/project_fs.rs` (new), `crates/cobolt-ide/src/main.rs` (mod decl)
  - Do: path-safe, project-relative ops over a project root + category root:
    `validate_folder_name`, `create_folder`, `rename_folder`, `delete_folder`
    (recursive), `move_path`, plus `is_category_root`, self-descendant/no-op/
    collision/incompatible-kind guards. Port & generalise
    `project_knowledge::normalize_knowledge_folder`. Every returned path relative.
  - Verify: `cargo test -p cobolt-ide project_fs` green (reject empty/`..`/absolute/
    leading-dot/duplicate/category-root; move rejects self-descendant/no-op/
    collision/incompatible; create/rename/delete effect in a tempdir; all paths
    relative).

- [x] **T2 — `CoboltProject` membership helpers** (R4, R6, R9, R21)
  - Files: `crates/cobolt-ide/src/project_model.rs`
  - Do: `rename_prefix(old_dir, new_dir)`, `move_entry(old_rel, new_rel)`,
    `drain_under(dir) -> Vec<String>` across all six file lists; relative-only.
  - Verify: `cargo test -p cobolt-ide project_model` green; new tests assert the
    right entries rewritten/returned and no absolute path is ever stored (AC10).

- [x] **T3 — Membership-aware generated-path resolvers** (R7)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: `generated_cbl_path` (+ indexed variant) first consult the tracked
    `generated` list for an existing entry whose stem matches, else default
    `generated/<stem>.cbl`.
  - Verify: `cargo build -p cobolt-ide`; a unit test: a relocated generated entry
    resolves to the moved path, an unknown stem to the default.

## Phase B — i18n

- [x] **T4 — New `Tr` fields ×6 languages** (R19)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: add fields for New/Rename/Delete-folder menu items, dialog titles/labels/
    hints, and conflict/error/drop-hint messages, in EN/ES/PT/JA/ZH/FR.
  - Verify: `cargo test -p cobolt-ide i18n` green (no empty translations); build.

## Phase C — rendering

- [x] **T5 — Unified folder renderer** (R1, R2, Q2)
  - Files: `crates/cobolt-ide/src/panels/project.rs`
  - Do: for each category build a virtual folder tree from membership ∪ on-disk
    dirs under the category root; render folder nodes (deterministic
    `CollapsingState` id from rel path) wrapping the existing leaf renderers
    (`show_form_item`/`show_indexed_item`/`file_row`/asset+doc rows) filtered to a
    folder's direct children. Flat projects render unchanged.
  - Verify: `cargo build -p cobolt-ide`; existing `project.rs` panel tests still
    pass; new headless test: a folder present on disk appears as a node.

- [x] **T6 — Folder context menus + new events** (R1, R4, R5)
  - Files: `crates/cobolt-ide/src/panels/project.rs`
  - Do: add `ProjectPanelEvent` variants `CreateFolder{category,parent_rel}`,
    `RenameFolder{rel}`, `DeleteFolder{rel}`, `MoveInternal{src_rel,dest_dir_rel}`,
    `ImportOs{paths,dest_dir_rel}`; folder headers get New/Rename/Delete-folder
    context items; category headers get New-folder.
  - Verify: `cargo build -p cobolt-ide` green.

## Phase D — folder-op wiring

- [x] **T7 — App handlers + dialogs for create/rename/delete folder** (R2–R6, R8, R20)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: generalise the knowledge-folder create/delete dialogs into category-
    agnostic ones + a rename dialog; on confirm call `project_fs`, then a shared
    `reconcile_paths_after_move/delete` helper (rewrite/close editor tabs +
    designer + indexed views), then `do_save_project()`. Delete shows the
    recursive-removal confirmation before any disk mutation.
  - Verify: `cargo build -p cobolt-ide`; headless test: confirmed folder delete
    drains membership + closes matching tabs (AC4).

## Phase E — drag-and-drop

- [x] **T8 — Internal DnD move** (R9, R11, R12, R13)
  - Files: `crates/cobolt-ide/src/panels/project.rs`, `crates/cobolt-ide/src/app.rs`
  - Do: file rows become `DragAndDrop` sources (payload = src rel); folder headers
    + category roots are drop targets that emit `MoveInternal`; draw a drop
    indicator on the hovered valid target. App handler moves on disk via
    `project_fs::move_path` + reconciles + saves.
  - Verify: `cargo build -p cobolt-ide`; headless test: an internal move updates
    the tracked path (AC6); collision/self-descendant rejected.

- [x] **T9 — OS file drop import** (R10, R14, R21)
  - Files: `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/panels/project.rs`
  - Do: panel records the hovered-folder rel/rect each frame; `update` reads
    `i.raw.dropped_files`; emit `ImportOs{paths,dest_dir_rel}`; handler copies
    into the folder and reuses `add_file_to_project_path` (kind/duplicate checks),
    recording relative paths only.
  - Verify: `cargo build -p cobolt-ide`; a unit test on the import helper: an
    absolute source copies in and is tracked relative (AC10); incompatible kind
    rejected (AC7).

## Phase F — keyboard navigation

- [x] **T10 — Arrow-key navigation + focus ring** (R15–R18)
  - Files: `crates/cobolt-ide/src/panels/project.rs`
  - Do: build an ordered `VisibleRow` list during render; when the tree has
    focus, consume Up/Down (prev/next visible), Right (expand → descend), Left
    (collapse → ascend), Enter (activate = click). Toggle `CollapsingState` memory
    by the deterministic id; draw a focus ring; `scroll_to_me` the selection.
  - Verify: `cargo build -p cobolt-ide`; headless test drives Down/Right/Right/
    Left and asserts the resulting `selected` key + expand state (AC8).

## Phase G — finalize

- [x] **T11 — Docs (English guide)** (R1–R18)
  - Files: `docs/developers-guide-en.md`
  - Do: add an "Organising the project tree" section — folders per category,
    drag-and-drop (internal + OS), keyboard navigation. English only; do **not**
    touch `-es/-pt/-jp/-cn/-fr`.
  - Verify: section renders; screenshots left as placeholders per doc-shots.

- [x] **T12 — Version + CHANGELOG** (feature)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: minor bump; add a changelog entry describing tree folder management,
    drag-and-drop, and keyboard nav.
  - Verify: `cargo build -p cobolt-ide`.

- [x] **T13 — Finalize & AC sweep**
  - Do: run the full crate test suite; tick every acceptance criterion in
    `spec.md`; list manual/visual checks for the operator (create/rename/delete in
    each category, drag between folders, Finder drop, arrow-key walk; light+dark).
  - Verify: `cargo test -p cobolt-ide` green; AC1–AC10 satisfied.

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, the English guide is
updated (translations untouched), version/CHANGELOG bumped as a **feature**, and
the change is left uncommitted for the operator to commit/push per their rules
(do **not** commit/push unless asked).
