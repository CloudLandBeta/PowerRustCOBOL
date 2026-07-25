<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Grace target disambiguation modal

- **Status:** done
- **Plan:** ./plan.md   **Date:** 2026-07-24

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

## Phase A — detection + types (headless, keeps the tree green)

- [x] **T1 — `TargetRequest` / `TargetChoice` types + detection** (R1, R2, R4, R8, Q1)
  - Files: `crates/cobolt-ide/src/tool_exec.rs` (or a small new
    `crates/cobolt-ide/src/target_select.rs` module used by it)
  - Do: define `TargetRequest { op: Create|Edit, kind: FileKind, name, candidates:
    Vec<String> }` and `TargetChoice { rel_path: String }`. Add a pure helper over
    `CoboltProject` + spec-033 `FolderStructure`:
    `edit_candidates(kind, name) -> Vec<String>` (same category, stem match,
    case-insensitive) and `create_folders(kind) -> Vec<String>` (selectable
    folders of the category). All paths **relative**.
  - Verify: `cargo test -p cobolt-ide target_select` — edit returns 0/1/2+
    candidates correctly; create lists category folders incl. the root; every
    returned path is relative (AC7).

## Phase B — worker↔UI handshake

- [x] **T2 — `GraceMsg::SelectTarget` + session plumbing** (R1, R2, R5)
  - Files: `crates/cobolt-ide/src/grace_session.rs`
  - Do: add `GraceMsg::SelectTarget(TargetRequest, Sender<Option<TargetChoice>>)`;
    `pending_select` state; `pending_select()` accessor and
    `respond_select(Option<TargetChoice>)`; a worker-side `select_target` closure
    that sends the request and blocks on the reply (dropped channel → `None` =
    cancel), mirroring the existing `confirm` closure.
  - Verify: `cargo test -p cobolt-ide grace_session` — a round-trip test:
    `respond_select(Some(choice))` unblocks the worker with the choice; a dropped
    reply channel yields `None`.

- [x] **T3 — Thread `select_target` through the workflow** (R1, R2)
  - Files: `crates/cobolt-ide/src/grace_host.rs` (+ signatures of
    `run_grace_workflow_*`), `crates/cobolt-ide/src/tool_exec.rs`
  - Do: pass the `select_target` closure alongside the existing `confirm` closure
    from session → `run_grace_workflow_*` → orchestration / tool backend, so the
    create/edit chokepoints can call it. No behaviour change yet (closure unused
    at call sites until T5).
  - Verify: `cargo build -p cobolt-ide` green; existing Grace tests still pass.

## Phase C — i18n

- [x] **T4 — New `Tr` fields ×6 languages** (R3, R9)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: add fields for the create prompt (*"Select the folder for creating {name}"*),
    edit prompt (*"Select the element for editing {name}"*), the modal title,
    Select/Cancel buttons, and a "no candidates" edge string — EN/ES/PT/JA/ZH/FR.
    Grammar matched per operation.
  - Verify: `cargo test -p cobolt-ide i18n` green (no empty translations); build.

## Phase D — the picker modal

- [x] **T5 — Centered project-tree select modal** (R3, R6, R6a, R10)
  - Files: `crates/cobolt-ide/src/panels/target_picker.rs` (new),
    `crates/cobolt-ide/src/app.rs`
  - Do: render an `egui::Window` anchored `Align2::CENTER_CENTER` showing the tree
    in **select mode** — create: category folders with a selectable highlight + a
    **📁+ inline new-folder** action wired to `project_fs::create_folder` (R6);
    edit: only the candidate elements. Localized prompt (T4). Select returns a
    `TargetChoice` (relative), Cancel returns `None`. `app.rs`: when
    `session.pending_select()` is set, show the modal and route the result via
    `respond_select`; refuse a create into a folder already holding the name with
    the spec-033 collision message (R6a).
  - Verify: `cargo build -p cobolt-ide`; headless test: create mode lists folders +
    exposes inline-create; edit mode lists only candidates; Select yields the
    highlighted relative path.

## Phase E — the `project.select_target` agent tool

> **Design pivot (implement phase):** the write tools (`documentation.write`,
> `indexed_file.write`) take an explicit agent-supplied path per the coordination
> contracts, and forms use change-sets against a task-context form — so there is
> no correct write-time chokepoint to intercept. Instead the agent **calls a
> declared `project.select_target` tool** to resolve the target (which drives the
> modal via the threaded `select_target` callback) and uses the returned path.

- [x] **T6 — `project.select_target` tool backend** (R1, R2, R4, R5, R6, R7, R8)
  - Files: `crates/cobolt-ide/src/tool_exec.rs`
  - Do: add `exec_project` handling `project.select_target` with args
    `{op, kind, name}`. **create** → `create_request` (always a folder pick);
    **edit** → 0 candidates = error, 1 = resolve silently, 2+ = element pick.
    Call `self.select_target(req)`; return the chosen **relative** path in the
    result for the agent to use, or a clean cancel (`needs_confirmation`) on
    `None` (R5). Loads the project to compute candidates.
  - Verify: `cargo test -p cobolt-ide exec_project` — with a stub picker: create
    yields a folder request + returns the choice; ambiguous edit yields an element
    request; a single-match edit resolves with no request; cancel returns a clean
    stop.

- [x] **T7 — Declare the tool + contract text** (R1, R2, R7)
  - Files: `crates/cobolt-ide/src/agents_db.rs`, `crates/cobolt-ide/src/tool_exec.rs`
    (contract appendix)
  - Do: add `project.select_target` to the declared tools of the agents that
    create/edit named elements (Form Designer, Data Indexed File, Documentation);
    add a short contract instructing the agent to call it before creating/editing
    a named element and to use the returned path. IDE manual flows untouched (R7).
  - Verify: `cargo build -p cobolt-ide`; an agents_db test asserts the tool is in
    the declared set for those agents.

## Phase F — finalize

- [x] **T8 — Docs (English guide)** (R1–R7, R10)
  - Files: `docs/developers-guide-en.md`
  - Do: add a short note under the Grace/AI-agent section describing the
    disambiguation modal (create always asks for a folder incl. inline create;
    edit asks only when the name is ambiguous; centered, cancellable). English
    only; do **not** touch `-es/-pt/-jp/-cn/-fr`.
  - Verify: section reads correctly.

- [x] **T9 — Version + CHANGELOG** (feature)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: minor bump; changelog entry describing Grace target disambiguation.
  - Verify: `cargo build -p cobolt-ide`.

- [x] **T10 — Finalize & AC sweep**
  - Do: full crate test run; tick every acceptance criterion in `spec.md`; list
    the manual/visual checks for the operator (create → centered folder modal +
    inline create; ambiguous edit → element modal; single-match edit → no modal;
    cancel aborts; light + dark).
  - Verify: `cargo test -p cobolt-ide` green; AC1–AC8 satisfied (noting any
    element type deferred by T7).

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, the English guide is
updated (translations untouched), version/CHANGELOG bumped as a **feature**, and
the change is left uncommitted for the operator to commit/push per their rules
(do **not** commit/push unless asked).
