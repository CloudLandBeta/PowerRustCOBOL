<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Copy/Paste a Form Across Projects

- **Status:** draft → approved → **implemented, pending operator manual
  verification** (1.60.49, 2026-08-08 — AC1/AC3/AC6 need a real `CoboltApp`
  GUI session and, for AC6, two separate running IDE processes; see §5)
- **Folder:** specs/046-form-clipboard/
- **Author:** Emerson Lopes (requirements) · drafted with Claude Sonnet 5   **Date:** 2026-08-08

## 1. Overview

A developer building a second application often wants a form they already
built — a login screen, a customer picker, a settings dialog — without
rebuilding it control by control. Today a form can only be copied *within*
one open project, and only at the control-selection level, inside the
Designer canvas (`DesignerClipboard`/`copy_selected`/`paste_from_clipboard`
in `panels/designer.rs`). This spec adds a **whole-form** copy that goes
through the **OS clipboard**, so it survives across projects and across
separate running instances of the IDE — right-click a form in the project
tree, Copy Form; open (or switch to) a different project, right-click the
Forms category, Paste Form.

The form model already stores everything the copy needs to be self-
contained: `Form.controls`, their properties, animations and data bindings,
*and* every event handler's full COBOL body (`EventBinding.code`) — none of
that lives in a separate hand-written source file. The existing `.cfrm` XML
serialization (`cobolt_forms::xml::{save_form, load_form_from_str}`) already
round-trips 100% of this. Copy therefore only needs a string-returning
sibling of `save_form`; Paste already has a pure `load_form_from_str`. Unlike
the Designer's own cross-form paste (which merges a control *selection* into
an already-open form and must dedupe IDs against that form's own controls),
pasting a whole new form needs no ID or paragraph remapping at all — each
form compiles to its own outer `PROGRAM-ID` and runs as its own process with
its own object registry (verified against `cobolt-codegen` and
`form_runtime.rs`), so nothing about a pasted form's control IDs can collide
with an unrelated form already in the target project. The only real design
work is the OS-clipboard round trip itself and the form-name collision
prompt (R7).

## 2. Goals / Non-goals

- **Goals:**
  - Copy a form, complete with every control's properties, every bound
    event's full COBOL handler body, animations and data bindings, to the
    OS clipboard from the project tree.
  - Paste it into a different project (a different project open in the same
    IDE session, or a different project open in a separate running IDE
    instance) as a new, fully working form — no missing handler code, no ID
    collisions with what the target project already has.
  - Handle a form-name collision at paste time explicitly (rename or
    replace), never silently.

- **Non-goals:**
  - Not a live link — paste is a one-time snapshot; later edits to either
    copy do not propagate.
  - Not a replacement for the Designer's existing in-canvas
    selection copy/paste (same-session, control-level) — that stays as-is.
  - Not copying resources the form's blocks merely *reference* — a
    `Project's Crates` pin an `EXEC RUST` block needs, an asset image path,
    an indexed file a data binding names. The pasted form carries the
    *reference* (it's part of a control's properties/code, faithfully
    copied), but not the referenced resource itself; if the target project
    doesn't already have a matching one, the pasted form may fail Check
    until the developer adds it there — exactly as if they'd typed that
    reference by hand.
  - Not tolerating clipboard content from a materially incompatible/future
    `.cfrm` schema beyond whatever `load_form_from_str` already handles for
    on-disk forms today.

## 3. User stories

- As a developer starting a new project, I want to copy a form I already
  built in another project — controls, layout, event handlers and all — so
  I don't have to rebuild it from scratch.
- As a developer, if the project I'm pasting into already has a form with
  that name, I want to be asked what to do, not have it silently overwritten
  or silently renamed without my knowing.
- As a developer, if I try to paste something that isn't actually a copied
  form (I copied some other text by mistake), I want a clear message, not a
  broken tree entry.

## 4. Requirements (EARS)

- **R1 (event):** When the developer right-clicks a form row in the project
  tree, the system shall offer a **Copy Form** action.
- **R2 (event):** When Copy Form is invoked, the system shall serialize the
  complete form — every control's properties, every bound event's full
  COBOL handler body, animations, and data bindings — to the existing
  `.cfrm` XML text representation and write it to the OS clipboard.
- **R3 (event):** When the developer right-clicks the Forms category (or an
  empty area within it), the system shall offer a **Paste Form** action.
- **R4 (state):** While the OS clipboard's current text does not parse as a
  valid copied form, Paste Form shall be disabled (or, if invoked anyway,
  refuse immediately — see R9) rather than attempt a partial paste.
- **R5 (event):** When Paste Form is invoked with clipboard text that parses
  as a valid form, the system shall create a new form in the current
  project from it.
- **R6 (constraint):** Control IDs and event/user-procedure paragraph names
  inside the pasted form need **no** remapping against the target project.
  Each form compiles to its own outer `PROGRAM-ID` with its own nested
  event programs (verified: `cobolt-codegen` gives every form a wholly
  separate `PROGRAM-ID`/`END PROGRAM` pair, and a running form is its own
  OS process with its own object registry) — a control named `BUTTON1` in
  the pasted form cannot collide with a same-named control in some other,
  unrelated form already in the target project. The pasted form's IDs stay
  exactly as copied; this is *not* the same operation as the Designer's
  existing cross-form paste, which merges a selection into an **already
  open** form and genuinely must dedupe against that form's own controls
  (unchanged, out of scope here).
- **R7 (event):** When the pasted form's own name collides with a form
  already in the target project, the system shall prompt the developer to
  either rename the incoming form before it's created, or replace the
  existing one — nothing is written until the developer chooses.
- **R8 (constraint):** A "Replace" choice shall delete the existing form's
  file only after this confirmation, never as a side effect of an
  unconfirmed paste (mirrors the existing form-delete confirmation already
  in the tree).
- **R9 (event):** When Paste Form is invoked and the clipboard text does not
  parse as a valid copied form (arbitrary text, empty, unrelated data), the
  system shall show a clear message naming the problem and change nothing.
- **R10 (event):** When paste completes, the new form's COBOL shall be
  regenerated immediately, so its Generated Code entry exists without
  waiting for the next Build/Run/Debug/Check.
- **R11 (ubiquitous):** Copy shall read the form's last-**saved** on-disk
  `.cfrm` state — the same source of truth the tree's other per-form actions
  (delete, open) already use. If the form is open in a Designer with unsaved
  edits, Copy shall not silently include those; this is a stated caveat, not
  a defect (a plan may choose to save-then-copy, but that is a design
  choice, not a requirement here).

## 5. Acceptance criteria

- [ ] AC1 — Copy Form on a form row, then Paste Form in a different project
      open in the same running IDE, produces a new form whose controls,
      properties, animations, and data bindings match the source exactly.
      **Needs the operator's manual pass** (plan §6, step 1) — the full
      flow runs through `CoboltApp`, which requires a real
      `eframe::CreationContext` and cannot be constructed in an automated
      test in this codebase (confirmed while implementing T4/T5/T6: the
      exact precedent function, `save_new_form_to`, has zero test coverage
      for the same reason).
- [x] AC2 — Every event handler on the pasted form's controls carries its
      full original COBOL body verbatim (verified by content, not just
      presence) — including every reference inside that body to a control
      ID, since IDs are never renamed (R6). Verified at the mechanism level:
      `form_to_string_and_load_form_from_str_round_trip` (T1) round-trips a
      control's full event body through the exact serialization
      `copy_form`/`register_pasted_form` use, byte-for-byte content checked.
- [ ] AC3 — Pasting into a project that already has a same-named form
      prompts for rename-or-replace; choosing rename creates the form under
      the new name without touching the existing one; choosing replace
      requires its own confirmation and then replaces it. **Needs the
      operator's manual pass** (plan §6, step 4) — same `CoboltApp`
      constraint as AC1.
- [x] AC4 — Pasting a form whose controls reuse IDs an unrelated,
      already-present form in the target project also uses internally
      (e.g. both have a `BUTTON1`) causes no conflict — the pasted form's
      generated COBOL and its running behavior are unaffected by that other
      form's use of the same names (each form is its own `PROGRAM-ID`/own
      process). Verified by construction: confirmed via `cobolt-codegen`
      (each form emits its own outer `PROGRAM-ID`/`END PROGRAM`) and
      `form_runtime.rs` (each running form is its own OS process with its
      own `ObjectRegistry`) that no cross-form namespace exists to collide
      in, and `register_pasted_form` passes the parsed `Form` straight to
      `save_form` with no ID-touching code in between.
- [x] AC5 — Pasting arbitrary clipboard text (not a copied form) shows a
      clear refusal and creates nothing.
      `invalid_clipboard_text_is_refused_and_changes_nothing` (T3).
- [ ] AC6 — Copy → close the IDE entirely → reopen a different project in a
      fresh IDE process → Paste Form succeeds (proves the OS clipboard, not
      just in-app state, carries the copy). **Needs the operator's manual
      pass** (plan §6, step 2) — requires two real, separate running IDE
      processes and the actual OS clipboard; not reproducible in this
      environment at all, automated or otherwise.
- [x] AC7 — After a successful paste, the new form's Generated Code entry
      exists immediately, without requiring a manual Build/Run/Debug/Check
      first. Verified by construction: `register_pasted_form` calls
      `write_generated_for(&path, &form)` unconditionally right after
      `save_form` succeeds — the same, already-established regeneration
      function every other form-creating action uses.
- [x] AC8 — All new UI strings (context-menu items, the rename/replace
      prompt, the invalid-clipboard message) exist as `Tr` fields in all six
      languages. `cargo test -p cobolt-ide i18n` — 3/3 green.
- [x] AC9 — `docs/developers-guide-en.md` describes Copy Form / Paste Form
      and the rename/replace prompt. New "Copying a form between projects"
      subsection in §6.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — new context-menu labels, the rename/replace
  modal's text, and the invalid-clipboard refusal message. All as `Tr`
  fields, all six languages.
- **Generated-code / regenerate contract:** R10 explicitly requires an
  immediate regeneration after paste, consistent with the standing
  regenerate-on-action contract (`tech.md`).
- **Docs (English guide):** Yes — required per AC9.
- **Fix vs feature classification:** **Feature** — new capability beyond
  today's scope, no COBOL-85 conformance involved. Per the operator's
  standing rule, implementation bumps the version's `z` only; ask before
  touching `y`/`x`.
- **Commit/announce:** Its own commit, separate from any incidental fix;
  announced on forum f=96 after merge to `main`, exact post text approved
  first — per golden rules #4b/#5.

## 7. Open questions

- Q: R11 leaves open whether Copy should offer to save first when the form
  is open in a Designer with unsaved edits, rather than silently copying
  the stale on-disk version. Leaning toward: if the form is open and dirty,
  save it as part of Copy (least surprising — "copy" should mean "copy what
  I'm looking at"), but this is a `/plan`-level UX call, not fixed here.
- Q: Should "Copy Form" also be reachable from inside an *open* Designer
  (e.g. a toolbar/menu action on the form itself), not only from the
  project-tree row? The tree entry point satisfies every acceptance
  criterion above; an additional entry point is a nice-to-have `/plan` can
  add without changing any requirement.
