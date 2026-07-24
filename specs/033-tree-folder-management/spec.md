<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Project tree folder management

- **Status:** draft → approved
- **Folder:** specs/033-tree-folder-management/
- **Author:** Claude (agent) for CloudLandBeta   **Date:** 2026-07-23

## 1. Overview

The project explorer (`crates/cobolt-ide/src/panels/project.rs`) presents six
fixed top-level categories — **Forms, Indexed Files, Common Code, Generated,
Assets, Documentation**. Today only Assets and Documentation render nested
folders (by walking the disk); Documentation alone can create/delete folders,
and nothing supports rename. The other four categories are flat lists tracked as
relative paths in `cobolt.toml`. To build **enterprise-grade** applications,
developers need to organize every category into an arbitrary folder hierarchy —
**create, rename, and delete** folders — plus **move files** between folders
(drag from another folder, or drop in from the OS file manager) and **navigate
the whole tree with the keyboard**. Deleting a folder must recursively remove its
on-disk contents, drop the affected files from project membership, and close any
editor tabs pointing at deleted files.

## 2. Goals / Non-goals

- **Goals:**
  - Uniform **create / rename / delete folder** actions in all six top-level
    categories, including Generated.
  - Folder **delete is recursive on disk** and reconciles project membership
    (`cobolt.toml`) and open editor tabs.
  - **Drag-and-drop** to move files: folder→folder within the tree, and from the
    **OS file manager** into a category/folder.
  - **Keyboard navigation** of the tree: Up/Down move between visible rows;
    Right expands a collapsed folder (or descends if already expanded); Left
    collapses an expanded folder (or ascends to its parent).
  - Consistent behaviour, icons, and i18n across categories; theme-aware.
- **Non-goals:**
  - Reorganizing the six top-level categories themselves (they remain fixed and
    IDE-owned; only their *contents* become foldered).
  - Hand-editing generated `.cbl` content (the regenerate contract is unchanged;
    Generated folders only relocate/rename existing artifacts).
  - Multi-select drag of many files at once (single-file move is the baseline;
    multi-select may be a later enhancement).
  - Cross-category moves that change a file's kind (e.g. dragging a `.cfrm` into
    Common Code). Moves are constrained to compatible destinations (see R14).
  - Cut/copy/paste clipboard semantics, and undo/redo of folder operations.

## 3. User stories

- As a COBOL developer, I want to group related forms/programs into folders
  (e.g. `customers/`, `billing/`), so a large project stays navigable.
- As a developer, I want to rename a folder and have every tracked file and open
  tab follow the change, so I don't hand-fix `cobolt.toml` or broken tabs.
- As a developer, I want deleting a folder to also remove its files from disk and
  from the project, so there are no orphaned entries or stale artifacts.
- As a developer, I want to drag a file from one folder to another, or drop files
  in from Finder/Explorer, so importing and reorganizing is fast.
- As a keyboard-first developer, I want to move through the tree and expand/
  collapse folders with the arrow keys, without reaching for the mouse.

## 4. Requirements (EARS)

### Folder create / rename / delete

- **R1 (ubiquitous):** The system shall let the developer create a new folder
  inside any of the six top-level categories and inside any existing folder
  within them, via a context menu and/or a category header affordance.
- **R2 (event):** When the developer creates a folder, the system shall create
  the directory on disk under the category's root subdirectory and show it in the
  tree without requiring a project reload.
- **R3 (constraint):** The system shall reject an invalid folder name (empty,
  containing a path separator or `..`, a leading `.`, a reserved name, or one
  that collides with an existing sibling) and surface a localized error, leaving
  disk and project state unchanged.
- **R4 (event):** When the developer renames a folder, the system shall rename
  the directory on disk and update every affected `cobolt.toml` membership path
  (prefix rewrite) and every open editor tab / inspector path to the new
  location.
- **R5 (event):** When the developer deletes a folder, the system shall present a
  confirmation dialog stating that the folder and all its contents will be
  permanently removed from disk, and perform nothing until confirmed.
- **R6 (event):** When a folder deletion is confirmed, the system shall
  recursively remove the folder from disk, drop every project-tracked file whose
  path is under the folder from `cobolt.toml`, and close any editor tabs /
  designer / inspector views bound to a removed file.
- **R7 (state):** While a category is **Generated** (IDE-owned, read-only
  output), the system shall still allow create/rename/delete of folders but shall
  not treat relocated `.cbl` files as hand-edited, and the regenerate-on-action
  contract shall continue to target the correct output paths.
- **R8 (constraint):** The system shall not delete or rename a category's root
  subdirectory itself (e.g. `forms/`, `src/`, `generated/`, `Assets/`,
  `Knowledge Base/`) through folder actions.

### Drag-and-drop file moves

- **R9 (event):** When the developer drags a file node onto a folder node (or
  onto a category root) within the tree and drops it, the system shall move the
  file on disk into that folder and update its `cobolt.toml` membership path and
  any open editor tab / inspector path.
- **R10 (event):** When the developer drops one or more files from the OS file
  manager onto a category or folder node, the system shall copy them into that
  location on disk and add them to the project under the appropriate category
  (reusing the existing import/add-file rules and duplicate checks).
- **R11 (state):** While a drag is in progress over the tree, the system shall
  show a drop indicator on the hovered valid target and visually reject invalid
  targets.
- **R12 (constraint):** The system shall not move a file onto a destination that
  would overwrite an existing sibling of the same name; it shall surface a
  localized conflict message instead.
- **R13 (constraint):** The system shall not move a folder or file **into its own
  descendant**, and shall reject a no-op drop (same source and destination).
- **R14 (constraint):** The system shall restrict a move/import to destinations
  whose category matches the file's kind (a `.cfrm` stays in Forms, `.cbl` in
  Common Code / Generated, `.cidx` in Indexed Files, docs in Documentation, other
  binaries in Assets); an incompatible drop is rejected with a localized message.

### Keyboard navigation

- **R15 (event):** When the project tree has focus and the developer presses
  Down/Up, the system shall move the selection to the next/previous **visible**
  tree row (respecting current expand/collapse state), **load that element's
  properties/editor as a single click would**, and keep the highlighted row
  visible — scrolling so it is never the first/last visible line (a one-row
  margin) unless it is genuinely the first/last row of the whole tree.
- **R16 (event):** When a **collapsed** folder is selected and the developer
  presses Right, the system shall expand it; when an **already-expanded** folder
  is selected and Right is pressed, the system shall move selection to its first
  child.
- **R17 (event):** When any row is selected and the developer presses Left, the
  system shall move the selection to its parent folder (it never collapses a
  folder; a selected top-level row with no parent stays put).
- **R18 (event):** When a row is selected and the developer presses Enter, the
  system shall perform that row's primary activation (open file / open designer /
  inspect), matching a single mouse click.

### Cross-cutting

- **R19 (constraint):** Every new user-facing string introduced by this feature
  shall be a `Tr` field translated in all six languages (EN/ES/PT/JA/ZH/FR); no
  hard-coded literals.
- **R20 (constraint):** All folder operations shall be path-safe: no traversal
  outside the project root, and all confirmations for destructive actions must
  precede any disk mutation.
- **R21 (constraint):** Every path this feature persists (`cobolt.toml`
  membership, indexed-file references, asset/image references, any folder path
  written to project files) shall be stored **relative to the project root** —
  never an absolute or user-home path. When an OS drop or import supplies an
  absolute path, the system shall copy the file into the project and record only
  the project-relative path, so the project remains portable across machines and
  users.

## 5. Acceptance criteria

- [ ] AC1 — In each of the six categories, a context menu offers **New folder…**,
      **Rename folder…**, and **Delete folder…**; each performs the disk +
      membership + tree update described in R1–R8.
- [x] AC2 — Creating a folder with an invalid or duplicate name shows a localized
      error and changes nothing on disk (R3).
- [ ] AC3 — Renaming a folder containing tracked files updates `cobolt.toml`
      paths and any open editor tab titles/paths to the new folder (R4).
- [x] AC4 — Deleting a non-empty folder, after confirmation, removes the folder
      and all descendants from disk, removes those files from `cobolt.toml`, and
      closes their editor/designer/inspector views (R5, R6). A unit/integration
      test asserts disk removal + membership reconciliation.
- [x] AC5 — A folder action never removes/renames a category root subdirectory
      (R8), and never escapes the project root (R20).
- [x] AC6 — Dragging a file node onto another folder moves it on disk and updates
      its tracked path; dropping OS files onto a folder imports them there
      (R9, R10), with conflict and self-descendant drops rejected (R12, R13).
- [ ] AC7 — Incompatible-category drops are rejected with a localized message
      (R14).
- [x] AC8 — With the tree focused: Up/Down changes the selected row across the
      visible hierarchy; Right expands then descends; Left collapses then
      ascends; Enter activates the row (R15–R18). A headless test drives the
      arrow-key sequence and asserts the resulting selection/expansion.
- [x] AC9 — All new strings appear in the `Tr` table for all six languages (R19);
      `cargo build -p cobolt-ide` and `cargo test -p cobolt-ide` pass.
- [x] AC10 — After any create/rename/move/import, no absolute or user-home path
      appears in `cobolt.toml` or any project-persisted reference; every recorded
      path is project-relative (R21). A test greps a mutated `cobolt.toml` for an
      absolute-path leak and asserts none.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — new menu items, dialogs, and error/conflict
  messages each need a `Tr` field across EN/ES/PT/JA/ZH/FR (R19). This is the
  largest cross-cutting cost.
- **Generated-code / regenerate contract:** Folders may hold generated `.cbl`,
  but relocating/renaming them must keep `App::regenerate_all_forms` and the
  generated-path resolvers (`generated_cbl_path` and friends) pointing at the
  right files. Moving generated output does not make it hand-edited; the banner
  and regenerate-on-action contract are untouched. Open question Q1 on whether
  Generated folders are worth the risk vs. auto-managed.
- **Docs (English guide):** Yes — `docs/developers-guide-en.md` gains a section on
  organizing the project tree with folders, drag-and-drop, and keyboard
  navigation. English only; translations are user-maintained.
- **Fix vs feature:** **Feature** — bump the **minor** version in
  `crates/cobolt-ide/src/version.rs` and add a `CHANGELOG.md` entry. Do not mix
  with unrelated fixes in one commit.
- **Project portability:** All persisted paths stay project-relative (R21). The
  model already uses `relative_to` for `cobolt.toml`; this feature must uphold it
  for every new write path (moves, imports, folder records) so a project folder
  can be zipped/moved/shared without absolute-path leakage.
- **egui version:** Implemented on the `egui-035` branch (0.35 APIs), consistent
  with spec 027; drag-and-drop uses egui's `dnd`/drag payload APIs and the
  eframe `RawInput.dropped_files` / `hovered_files` for OS drops.

## 7. Open questions

- **Q1 (resolved):** Include the **Generated** category in folder editing? →
  **Yes**, per operator decision (all six categories). Plan must keep the
  regenerate/output-path contract intact (R7).
- **Q2:** Do the flat categories (Forms/Indexed/Common Code) render folders by
  **walking the disk** under the category root (like Assets/Documentation
  already do) or by **deriving folder nodes from tracked path prefixes**?
  Disk-walking preserves empty folders across reloads and unifies the renderer;
  path-prefix derivation loses empty folders. *Recommendation: disk-walk the
  category root, unioned with tracked membership.* Resolve in `/plan`.
- **Q3:** For **rename**, should open **designer/indexed-editor** viewports (not
  just plain editor tabs) also have their bound path rewritten live, or is
  closing-and-reopening acceptable? *Recommendation: rewrite live where cheap;
  otherwise close affected non-text views.* Resolve in `/plan`.
- **Q4:** Keyboard focus model — does the tree get an explicit focus/selection
  ring and a documented shortcut to focus it, or does it piggyback on the
  existing `selected` element? Resolve in `/plan`.
- **Q5:** Should OS drag-in of a **folder** (not just files) be supported (deep
  import of a directory subtree), or files only in v1? *Recommendation: files
  only in v1 (matches R10 wording); note folder-import as a follow-up.*
