<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Grace target disambiguation modal

- **Status:** draft → approved
- **Folder:** specs/034-grace-target-disambiguation/
- **Author:** Claude (agent) for CloudLandBeta   **Date:** 2026-07-24

## 1. Overview

Spec 033 gave every project-tree category a folder hierarchy, so an element can
now live in any of several folders (e.g. `forms/customers/order.cfrm` vs.
`forms/suppliers/order.cfrm`). When the AI agent **Grace** is asked in chat to
**create** or **edit** an element *by name*, its destination or target is no
longer obvious. This feature makes Grace stop and ask the developer to pick the
target on the project tree before it acts, via a **modal window showing the
project tree**:

- For a **create**, Grace **always** shows the modal so the developer chooses the
  destination **folder** — and may **create a new folder inline** during
  selection (reusing spec 033's folder creation).
- For an **edit**, Grace shows the modal only when the requested name matches
  **more than one** element, so the developer picks which one.

The modal carries a localized prompt whose grammar matches the operation. For an
edit with a single match, Grace proceeds silently as today.

## 2. Goals / Non-goals

- **Goals:**
  - When Grace is asked to **create** an element, **always** show a project-tree
    modal to pick the destination **folder** before creating, with an option to
    **create a new folder inline** and select it.
  - When Grace is asked to **edit** an element by name and **more than one**
    element of that name exists, show a project-tree modal to pick the specific
    **element** before editing.
  - A localized modal prompt: *"Select the folder for creating `<element>`"* /
    *"Select the element for editing `<element>`"*, grammatically matched to the
    operation.
  - An **edit** with a single match runs with no modal — behavior unchanged.
  - Cancelling the modal aborts the operation cleanly and Grace reports it did
    nothing.
- **Non-goals:**
  - The IDE's **manual** create/edit flows (the category `+` buttons, New
    dialogs, double-click-to-edit) — out of scope; they already target a node or
    a fixed folder.
  - Resolving collisions by renaming, moving, or merging — the modal only
    **selects** an existing target.
  - Multi-select (picking several targets at once).
  - Changing how edits themselves are applied (the change-set / tool contracts in
    `tool_exec.rs` are unchanged apart from target resolution).

## 3. User stories

- As a developer, when I tell Grace "create a form called order" and an `order`
  form already exists in another folder, I want Grace to ask **which folder** to
  put the new one in, so I don't get a surprise duplicate or overwrite.
- As a developer, when I tell Grace "edit the order form" and two `order` forms
  exist, I want to pick **which one** on the tree, so Grace edits the right file.
- As a developer, when there is only one `order`, I want Grace to just do it —
  no needless prompt.

## 4. Requirements (EARS)

- **R1 (event):** When Grace is asked to **create** a project-tree element by
  name, the system shall **always** present a modal showing the project tree and
  require the developer to select the destination **folder** before the create
  proceeds.
- **R2 (event):** When Grace is asked to **edit** a project-tree element by name
  and **more than one** element of that name exists in the tree, the system shall
  present a modal showing the project tree and require the developer to select
  the specific **element** before the edit proceeds.
- **R3 (state):** While the disambiguation modal is open, the system shall show a
  localized prompt reading *"Select the folder for creating `<element>`"* for a
  create and *"Select the element for editing `<element>`"* for an edit, where
  `<element>` is the requested name and the wording matches the operation.
- **R4 (event):** When Grace is asked to **edit** an element by name and exactly
  **one** element matches, the system shall perform the edit on that element
  **without** showing the modal.
- **R5 (event):** When the developer **confirms** a selection in the modal, the
  system shall carry out the original create/edit against the chosen target;
  when the developer **cancels**, the system shall abort the operation and Grace
  shall report that nothing was created/edited.
- **R6 (constraint):** In **create** mode the modal shall offer only folders
  within the element's own category as valid destinations, and shall let the
  developer **create a new folder inline** (spec 033 folder creation) and select
  it as the destination; in **edit** mode it shall offer only the existing
  elements that match the requested name and kind.
- **R6a (constraint):** When the developer selects a destination folder that
  already contains an element of the requested name, the create shall be refused
  with the same localized collision message as a manual create (spec 033, R12) —
  the modal never silently overwrites.
- **R7 (constraint):** The disambiguation shall apply to **AI-agent
  (Grace) tool-driven** create/edit only; the IDE's manual create/edit flows are
  unaffected.
- **R8 (constraint):** The target the modal yields shall be recorded and used as
  a **project-relative** path (consistent with spec 033, R21) — never absolute.
- **R9 (constraint):** Every new user-facing string shall be a `Tr` field in all
  six languages (EN/ES/PT/JA/ZH/FR).
- **R10 (constraint):** The modal shall appear **centered on the IDE window**
  (anchored center, matching the IDE's other modal dialogs) and be modal — it
  takes focus until the developer confirms or cancels.

## 5. Acceptance criteria

- [x] AC1 — Asking Grace to create an element **always** opens the modal with the
      project tree and the create prompt; the new element is created only in the
      folder the developer picks, and the developer can create a new folder inline
      and pick it (R1, R3, R6). Selecting a folder that already holds that name is
      refused with the collision message (R6a).
- [x] AC2 — Asking Grace to edit an element whose name matches two or more tree
      elements opens the modal with the edit prompt; the edit is applied only to
      the element the developer picks (R2, R3, R6).
- [x] AC3 — Editing a name with exactly one match runs with no modal, as before
      (R4). A test asserts the single-match no-modal path.
- [x] AC4 — Cancelling the modal aborts the operation; nothing is created or
      edited and Grace's reply says so (R5).
- [x] AC5 — The prompt text is grammatically correct for each operation and comes
      from the `Tr` table in all six languages (R3, R9).
- [x] AC6 — The IDE's manual `+`/New flows are unchanged (R7). The modal renders
      centered on the IDE window and holds focus until confirm/cancel (R10).
- [x] AC7 — The resolved target is stored/used as a project-relative path; no
      absolute path is introduced (R8). Covered by a unit test on the collision/
      resolution logic.
- [x] AC8 — `cargo build -p cobolt-ide` and `cargo test -p cobolt-ide` pass,
      including a test of the collision-detection ("more than one same-name")
      logic.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — the modal title/prompt (create vs edit variants),
  and any buttons/labels, need `Tr` fields across EN/ES/PT/JA/ZH/FR (R9).
- **Generated-code / regenerate contract:** Unaffected — this only resolves
  *which* target a create/edit acts on; the generated banner and
  regenerate-on-action contract are untouched.
- **Docs (English guide):** Yes — a short note in `docs/developers-guide-en.md`
  under the Grace/AI-agent section describing the disambiguation prompt. English
  only; translations user-maintained.
- **Fix vs feature:** **Feature** — minor version bump + `CHANGELOG.md` entry; not
  mixed with unrelated fixes in one commit.
- **Agent architecture:** Grace already has a **confirmation-required tool-result**
  handshake (a locked/finalized indexed-file write returns a "confirm
  destroy-and-recreate" result that Grace relays; the developer decides; the op
  re-runs). Target disambiguation should reuse this pattern: the tool detects the
  collision, returns a "select a target" result carrying the candidate folders/
  elements, the IDE shows the tree modal, and the operation completes against the
  chosen project-relative target. (Mechanism detail — resolved in `/plan`.)

## 7. Open questions

- **Q1:** For **edit**, what counts as the **same name** matching more than one
  element — the file **stem** (`order`) or the full filename (`order.cfrm`), and
  within the element's **category** only or across the whole tree? *Recommendation:
  same category, by stem (case-insensitive), matching how the IDE already de-dups
  COBOL ids.* (For **create** the modal always shows, so this only governs edit.)
  Resolve in `/plan`.
- **Q2:** Which agent operations count as "**edit by name**"? Form-designer edits
  usually target a form already named in the task context; the ambiguity mainly
  arises when Grace must first **resolve** which form/indexed/source a request
  refers to. Enumerate the exact tool entry points in `/plan`.
- **Q3:** Is the modal **synchronous** (blocks the agent turn until the developer
  picks) or does it use the **relay** handshake (tool returns "needs selection",
  turn ends, developer picks, Grace re-runs)? *Recommendation: the relay
  handshake, matching the existing confirm-recreate flow and the async I/O model
  (spec 032).* Resolve in `/plan`.
- **Q4:** Should the modal **reuse the existing project-tree renderer**
  (`panels/project.rs`) in a selectable mode, or be a **dedicated lightweight
  picker**? Resolve in `/plan`.
- **Q5:** (Resolved) Create now **always** prompts for a folder, so cross-category
  name matches no longer gate the modal. A same-name collision *within the chosen
  folder* is still refused at confirm time (R6a).
