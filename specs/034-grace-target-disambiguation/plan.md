<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Grace target disambiguation modal

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-24

## 1. Approach

Grace runs on a **worker thread** (`GraceSession::spawn`) and talks to the UI
through an `mpsc` channel of `GraceMsg`. Gated git ops already use a **blocking
handshake**: the worker's `confirm` closure sends `GraceMsg::Confirm(req,
reply_tx)` and blocks on `reply_rx.recv()`; the UI stores it as
`pending_confirm`, renders Approve/Deny, and `respond_confirm(bool)` sends the
answer back, unblocking the worker (a dropped channel = deny). We reuse this
pattern verbatim for target selection (R1, R2, R5, R10).

**Handshake (Q3 resolved → blocking channel, supersedes the spec's relay note).**
Add `GraceMsg::SelectTarget(TargetRequest, Sender<Option<TargetChoice>>)` and a
`select_target: &mut dyn FnMut(TargetRequest) -> Option<TargetChoice>` closure,
threaded from `GraceSession` down the same path the `confirm` closure already
travels (`run_grace_workflow_* → orchestration → tool backends`). When a
create/edit resolves its target:
- **create** → always send a `TargetRequest::Folder { kind, name }` (R1);
- **edit** → send `TargetRequest::Element { kind, name, candidates }` **only when
  `candidates.len() > 1`** (R2); one candidate resolves silently (R4).
The worker blocks; the UI opens the centered modal; the developer picks a folder
(and may create one inline) or an element; `respond_select(Option<TargetChoice>)`
returns the **project-relative** path (R8) or `None` (cancel → abort, R5). A
dropped channel (session dismissed) = cancel.

**The modal (R3, R6, R10).** A new IDE modal renders the project tree in a
**select mode**, reusing `panels/project.rs`: a create request shows folders of
the target category with a selectable highlight plus a **📁+ "new folder"**
affordance wired to the spec-033 `project_fs::create_folder` (R6); an edit request
shows only the candidate elements. The window is `egui::Window` anchored
`Align2::CENTER_CENTER` (matching every other IDE dialog) and blocks interaction
until Select/Cancel (R10). The prompt is a localized `Tr` string formatted with
the element name, with distinct create/edit wordings (R3, R9).

**Collision / candidate detection.** A small helper over `CoboltProject` +
`FolderStructure` (spec 033): for **create**, gather the selectable folders of the
category; for **edit**, gather tracked entries in the category whose **stem**
matches the requested name **case-insensitively** (Q1). At confirm time, a create
into a folder that already holds that name is refused with the spec-033 collision
message (R6a) — reusing `project_fs`'s duplicate check.

**Scope (R7).** Only the AI-agent tool-driven create/edit chokepoints call
`select_target`; the IDE's manual `+`/New dialogs are untouched.

## 2. Affected crates / files

- `crates/cobolt-ide/src/grace_session.rs` — new `GraceMsg::SelectTarget`
  variant; `pending_select` state; `pending_select()` + `respond_select(...)`;
  a worker-side `select_target` closure mirroring `confirm`.
- `crates/cobolt-ide/src/grace_host.rs` — thread the `select_target` closure
  through `run_grace_workflow_*` into the orchestration / backend, alongside the
  existing `confirm`.
- `crates/cobolt-ide/src/tool_exec.rs` — the create/edit tool chokepoints call
  `select_target` to resolve the destination folder / target element; a new
  `TargetRequest` / `TargetChoice` type; collision/candidate detection helper.
- `crates/cobolt-ide/src/panels/` — new `target_picker.rs` (or a select-mode flag
  on the project panel) rendering the tree for selection + inline folder create.
- `crates/cobolt-ide/src/app.rs` — own the picker modal: when the session reports
  a pending target request, show the centered modal and route the choice back via
  `respond_select`.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields ×6 (create/edit prompts,
  Select/Cancel, "no candidates" edge text).
- `docs/developers-guide-en.md` — a note under the Grace/AI-agent section.
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — minor bump (feature).

## 3. Data / model changes

- **No `cobolt.toml` / on-disk format change.** New in-memory types only:
  `TargetRequest { op: Create|Edit, kind: FileKind, name: String, candidates:
  Vec<String> }` and `TargetChoice { rel_path: String }` (relative, R8), plus the
  `GraceMsg::SelectTarget` channel variant and picker UI state.
- Reuses spec-033 `FolderStructure`, `project_fs::create_folder`, and the
  category/stem helpers on `CoboltProject`.

## 4. Key decisions & alternatives

- **Q3 — blocking channel handshake** (not the relay). Why: the git-confirm path
  already proves it across the worker/UI thread boundary; it yields a true
  mid-workflow modal instead of ending the turn and re-running. Rejected: the
  `needs_confirmation` **relay** (turn ends, developer re-issues) — worse UX and
  it can't carry a structured candidate list cleanly.
- **Q1 — edit match = same category, by stem, case-insensitive.** Why: matches how
  the IDE de-dups COBOL ids; a `.cfrm`/`.cbl` extension is implied by kind.
- **Q4 — reuse `panels/project.rs` in a select mode** rather than a bespoke tree.
  Why: one tree renderer, consistent look, inline folder-create already exists
  (spec 033). Rejected: a separate mini-tree (drift + duplicated logic).
- **Create always prompts (R1)** with inline folder creation (R6). Rejected:
  collision-only prompting (the operator changed this after the spec draft).
- **Selection is per operation.** Rejected for v1: remembering a chosen folder for
  the rest of a workflow (possible later enhancement; noted under risks).

## 5. Risks & mitigations

- **Form / common-code target resolution is not a single tool call** (unlike
  `indexed_file.write` / `documentation.write`, which carry an explicit `path`).
  Forms are edited via change-sets against a form named in **task context**, so
  the create/edit chokepoint sits in orchestration, not one backend call. →
  Mitigation: enumerate each element type's resolution point in `/tasks`; land
  **indexed-file + documentation + form-create** first (clear chokepoints),
  then common-code. Flag any type whose resolution can't be cleanly hooked rather
  than forcing it.
- **Worker deadlock** if the UI never answers (session dismissed mid-request). →
  Mitigation: dropped reply channel = cancel/abort, exactly as git-confirm treats
  a dropped channel as deny.
- **Create-always in a multi-element request** could pop several modals in one
  workflow. → Accepted for v1 (one modal per created element); note a possible
  "reuse last folder for this category this run" follow-up.
- **Two pending prompts at once** (a git confirm and a target select). →
  Mitigation: the UI already serialises `pending_confirm`; add `pending_select`
  as a peer and render at most one modal at a time.
- **i18n grammar** across six languages (create vs edit wording). → One `Tr` field
  per operation variant; a task gates on all six present.

## 6. Test strategy

- **Detection unit tests** (`tool_exec`/model): edit candidates = same-category
  stem-insensitive matches (0/1/2+ cases; 2+ triggers the request, 1 resolves
  silently — R2/R4); create always yields a folder request (R1); the resolved
  `TargetChoice` path is **relative** (R8/AC7).
- **Handshake test** (`grace_session`): a `SelectTarget` message round-trips —
  `respond_select(Some(path))` unblocks the worker with the choice; a dropped
  channel yields `None` (cancel).
- **Headless modal test** (`panels`): the picker in create mode lists the
  category's folders + exposes the inline new-folder action; in edit mode lists
  only the candidates; Select returns the highlighted relative path.
- **Collision-at-confirm test**: selecting a folder that already holds the name is
  refused with the spec-033 message (R6a).
- **i18n test**: `cargo test -p cobolt-ide i18n` — no empty translations.
- **Manual/visual** (operator, per the never-drive-the-app rule): ask Grace to
  create a form → centered modal, pick/inline-create a folder; ask to edit an
  ambiguous name → element picker; confirm single-match edits show no modal;
  check light + dark.

## 7. Steering compliance

- [ ] **i18n:** create/edit prompts + Select/Cancel + edge text in all six
      languages (EN/ES/PT/JA/ZH/FR).
- [ ] **Generated-code banner + regenerate-on-action** unaffected (only target
      resolution changes).
- [ ] **English dev guide** updated (Grace section); translations untouched.
- [ ] **Fix vs feature:** **feature** → minor bump in `version.rs` + `CHANGELOG.md`;
      not mixed with unrelated fixes in one commit.
- [ ] **No "cobolt" in user-facing text; COBOL identifiers/source stay English.**
- [ ] **Targets stored/used project-relative** (spec 033 R21 upheld).
