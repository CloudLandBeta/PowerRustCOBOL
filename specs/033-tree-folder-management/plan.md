<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Project tree folder management

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-23

## 1. Approach

The six categories today split into two renderers: flat `cobolt.toml`-list
(Forms, Indexed, Common Code, Generated) and disk-walked (Assets, Documentation).
We **unify them behind one recursive folder renderer** and add a small, well-
tested **project-fs module** for the mutating operations. Nothing about the six
top categories or the `.cfrm`/generated contracts changes; only their *contents*
gain a folder hierarchy.

**Rendering (R1, Q2 resolved → disk-walk ∪ membership).** For every category we
build, each frame, a virtual folder tree from the **union of** (a) the tracked
relative paths in that category's `cobolt.toml` list and (b) the directories that
actually exist on disk under the category root subdir (`forms/`, `src/`,
`generated/`, `Assets/`, `Knowledge Base/`; Indexed uses its stored dir). The
union preserves **empty** folders across reloads (they exist on disk) and keeps
membership authoritative for files. Folder nodes are `CollapsingState` headers
(reusing the existing pattern in `show_asset_path`/`show_knowledge_path`); at each
folder we dispatch to the **existing** leaf renderers (`show_form_item`,
`show_indexed_item`, `file_row`, asset/doc rows) filtered to that folder's direct
children. This keeps the rich form→controls→events and indexed→fields subtrees
intact.

**Mutations (R1–R14, R20–R21).** A new `project_fs` module in `cobolt-ide`
provides path-safe, project-relative operations: `create_folder`,
`rename_folder`, `delete_folder`, `move_path`, and `import_os_files`. They
generalise the proven logic in `cobolt_agents::project_knowledge`
(`normalize_knowledge_folder`, `create/delete_knowledge_subfolder`): reject
traversal/`..`/absolute/leading-dot/reserved/duplicate names, forbid touching a
category root (R8), forbid moving into a descendant or a no-op (R13), reject
name collisions (R12) and incompatible categories (R14). All returned and stored
paths are **project-relative** (R21).

**Membership + view reconciliation (R4, R6, R9).** New `CoboltProject` helpers
rewrite/drop path entries across the six lists:
- `rename_prefix(old_dir, new_dir)` — rewrite every entry under `old_dir`.
- `move_entry(old_rel, new_rel)` — single-file path rewrite.
- `drain_under(dir) -> Vec<String>` — remove and return every entry under `dir`.
The app then rewrites/closes affected **editor tabs**, **designer** and
**indexed-editor/inspector** views (Q3 → rewrite the bound path where the view
keys on a `PathBuf`; otherwise close), then `do_save_project()`.

**Drag-and-drop (R9–R14).**
- *Internal:* file rows become drag sources via `egui::DragAndDrop::set_payload`
  (payload = source rel path, mirroring `toolbox.rs`); folder headers and category
  roots are drop targets that read the payload on release and emit
  `MoveInternal { src_rel, dest_dir_rel }`. A drop indicator uses
  `ui.painter()` on the hovered valid target (R11); invalid targets are not armed.
- *OS drop (R10):* in the main `update`, read
  `ctx.input(|i| (i.raw.dropped_files.clone(), i.raw.hovered_files.len()))`. The
  panel records, each frame, the rel path + rect of the folder row currently
  under the pointer (into a panel field); on `dropped_files` with a path present
  we emit `ImportOs { paths, dest_dir_rel }`, which copies into the project and
  reuses `add_file_to_project_path` rules (duplicate/kind checks). Files-only in
  v1 (Q5).

**Keyboard navigation (R15–R18, Q4).** The panel builds an ordered
`Vec<VisibleRow { key, depth, is_folder, expanded, parent_key, activation }>`
during render (the same walk that draws rows). After rendering, **when the tree
region has focus / the pointer is over it**, it consumes Up/Down/Left/Right/Enter
against the current `selected` key:
- Down/Up → next/previous visible row; `scroll_to_me` on the newly selected row
  next frame.
- Right → if selected folder is collapsed, set its `CollapsingState` open; if
  already open, select first child.
- Left → if selected folder is open, collapse it; else select `parent_key`.
- Enter → push the row's stored activation event (same as a click).
Expansion is toggled by writing the folder's `CollapsingState` open flag in egui
memory (keyed by the same persistent id used to render it). A visible focus ring
is drawn on the selected row.

**Generated safety (R7).** Moving/renaming generated `.cbl` must survive
regenerate. We change the generated-path resolvers (`generated_cbl_path` and the
indexed variant) to **first consult the tracked `generated` membership** for an
existing entry whose stem matches, falling back to the default
`generated/<stem>.cbl`. So a relocated generated file keeps regenerating in place
instead of reappearing at the default path.

## 2. Affected crates / files

- `crates/cobolt-ide/src/project_fs.rs` — **new**: path-safe folder/move/import
  ops (create/rename/delete/move + validation), all project-relative. Unit-tested.
- `crates/cobolt-ide/src/panels/project.rs` — unified recursive folder renderer;
  drag sources on file rows, drop targets on folders/roots; per-frame
  `VisibleRow` list + keyboard handling; context-menu items (New/Rename/Delete
  folder); drop-indicator painting.
- `crates/cobolt-ide/src/project_model.rs` — `rename_prefix`, `move_entry`,
  `drain_under` on `CoboltProject`; keep all writes relative.
- `crates/cobolt-ide/src/app.rs` — handle new `ProjectPanelEvent`s: create/rename
  name dialogs + delete confirm (generalise the knowledge-folder dialogs), call
  `project_fs`, reconcile membership/tabs/designer/indexed views, save; OS-drop
  input read in `update`; make generated-path resolvers consult membership.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields ×6 (menu items, dialog
  titles/labels, conflict/error/drop-hint strings).
- `docs/developers-guide-en.md` — new "Organising the project tree" section
  (folders, drag-and-drop, keyboard navigation). English only.
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — minor bump (feature).

## 3. Data / model changes

- **No `cobolt.toml` schema change.** Folder structure is implicit in the
  relative path strings already stored per file; empty folders live on disk only.
  `[files]` lists simply start carrying subfolder-prefixed paths
  (`forms/customers/order.cfrm`). Backward compatible: existing flat projects
  render as today (everything at the category root).
- New `CoboltProject` methods (pure, no format change). New `project_fs` types
  (`MoveError`, etc.). New `ProjectPanelEvent` variants. New panel state fields
  (hovered-folder rel/rect, keyboard focus flag).
- **Migration:** none required; no on-disk format changes.

## 4. Key decisions & alternatives

- **Q2 — disk-walk ∪ membership** for folder rendering.
  Why: preserves empty folders across reloads, unifies all six categories on one
  renderer, keeps membership authoritative for files.
  Rejected: *pure path-prefix derivation* (empty folders vanish on reload —
  breaks "create folder then add files"); *pure disk-walk* (would show untracked
  junk files that aren't project members).
- **Reuse leaf renderers, wrap in folder nodes** rather than rewriting the form/
  indexed subtrees. Why: lowest risk, keeps controls/events/fields intact.
- **New `project_fs` in `cobolt-ide`** rather than extending
  `cobolt_agents::project_knowledge`. Why: these ops are IDE/project concerns, not
  agent-knowledge; agents crate stays focused. We port and generalise its proven
  validation.
- **Q3 — rewrite bound `PathBuf` on rename/move where the view keys on it; else
  close** the affected designer/indexed view. Why: cheap for path-keyed views,
  avoids stale handles for the rest.
- **Q4 — explicit visible-row model + focus ring** driving arrow keys, toggling
  the existing `CollapsingState` memory. Why: reuses egui's own expand state; no
  parallel tree structure to keep in sync.
- **Q5 — files-only OS import in v1.** Folder-subtree import noted as follow-up.
- **Generated stays editable (R7)** with membership-aware regenerate paths.
  Rejected: excluding Generated (operator chose all six); auto-only folders
  (inconsistent with the other five).

## 5. Risks & mitigations

- **Generated regenerate vs. moved output** → resolvers consult `generated`
  membership before the default path; test covers move-then-regenerate.
- **egui `CollapsingState` id stability** (expand toggled from keyboard) → derive
  folder ids deterministically from the rel path (as Assets/KB already do) so the
  keyboard handler and renderer agree on the id.
- **Drop-target hit-testing** for OS drops (eframe gives dropped paths, not a
  target) → panel records the hovered folder's rel/rect each frame; fall back to
  the category under the pointer, else the category root the pointer is within.
- **Path-safety / traversal / absolute leak (R20, R21)** → single choke-point in
  `project_fs`; unit tests assert rejection and relative-only output; a test greps
  a mutated `cobolt.toml` for absolute paths.
- **i18n breadth** (largest surface) → collect every new literal into `Tr` up
  front; a task gates on all six languages present.
- **Rename touching many tabs/views** → centralise reconciliation in one
  `app` helper invoked by rename, move, and delete.

## 6. Test strategy

- **`project_fs` unit tests** (in `cobolt-ide`): reject empty/`..`/absolute/
  leading-dot/duplicate/category-root names (R3, R8); `move_path` rejects
  self-descendant, no-op, name-collision, incompatible-category (R12–R14);
  create/rename/delete perform the disk effect in a tempdir; **all returned paths
  are relative** (R21). Report pass/fail counts.
- **Model tests**: `rename_prefix`, `move_entry`, `drain_under` rewrite/return the
  right entries across lists; assert no absolute paths written (AC10/R21).
- **Headless egui panel tests** (extend the existing `#[cfg(test)]` harness in
  `project.rs`): (a) create-folder then the folder node appears; (b) delete-folder
  reconciliation — after confirm, membership drained + tabs closed (drive through
  app helper); (c) **keyboard sequence** Down/Right/Right/Left asserts the
  resulting `selected` key and expand state (AC8); (d) an internal move updates
  the tracked path (AC6).
- **Generated test**: move a generated `.cbl`, regenerate, assert it rewrites the
  moved path, not the default (R7).
- **Manual/visual** (operator, not agent — see memory *never-drive-the-app*):
  launch IDE, create/rename/delete folders in each category, drag a file between
  folders, drop files from Finder, arrow-key through the tree; confirm drop
  indicator + focus ring render in light and dark themes.

## 7. Steering compliance

- [ ] **i18n:** all new UI strings (menu items, dialogs, conflict/error/drop
      hints) added to `Tr` in all six languages (EN/ES/PT/JA/ZH/FR).
- [ ] **Generated-code banner + regenerate-on-action** preserved; resolvers made
      membership-aware so moved generated output stays stable (R7).
- [ ] **English dev guide** gains an "Organising the project tree" section;
      `-es/-pt/-jp/-cn/-fr` translations untouched (user-maintained).
- [ ] **Fix vs feature:** **feature** → minor bump in `version.rs` + `CHANGELOG.md`
      entry; not mixed with unrelated fixes in one commit.
- [ ] **No "cobolt" in user-facing text; COBOL identifiers/source stay English.**
- [ ] **Paths project-relative** everywhere persisted (R21).
