<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Copy/Paste a Form Across Projects

- **Status:** draft → approved → **implemented** (1.60.49, 2026-08-08)
- **Spec:** ./spec.md   **Date:** 2026-08-08

## 1. Approach

**Copy (R1–R2, R11)** is a small, self-contained change: `cobolt_forms::xml`
currently has `save_form(form, path)` (writes to a file) and
`load_form_from_str(xml)` (parses from a string) — the string-in half of the
round trip exists, the string-out half doesn't. Factor `save_form`'s body
(it already builds a `Vec<u8>` before calling `fs::write`) into a new
`form_to_string(form: &Form) -> Result<String, FormError>`, with `save_form`
becoming a two-line wrapper around it. The project tree's Copy Form action
calls it and hands the result to `ctx.copy_text(...)` (egui's existing
write-to-OS-clipboard path, already used throughout this codebase). If the
form is open in a Designer with unsaved edits, Copy uses that live `Form`
(not the stale on-disk one) — resolving spec §7's open question in favor of
"copy what's on screen."

**Paste (R3–R5, R9–R10)** needs a read path this codebase has never used:
egui's `Event::Paste(String)` only arrives in `RawInput.events`, generated
either by the user's own Cmd/Ctrl+V keystroke or by the app requesting one
via `ctx.send_viewport_cmd(ViewportCommand::RequestPaste)` — confirmed
present in this workspace's egui 0.36 (`viewport.rs`) and wired through to a
real OS clipboard read in `egui-winit` (`State::clipboard_text`, backed by
`arboard`, already a transitive dependency via `eframe`). There is no
synchronous "read the clipboard right now" call — clicking **Paste Form**
sends the `RequestPaste` command and sets a `pending_form_paste: bool` flag
on `CoboltApp`; the very next frame(s) carry the resulting `Event::Paste`,
which the app's top-level input scan consumes **only** while that flag is
set (so an ordinary Cmd+V into some other focused text field is untouched).
The received text is handed to `load_form_from_str`; a parse failure clears
the flag and shows R9's refusal — nothing else changes.

**No ID/paragraph remapping (R6).** Verified against `cobolt-codegen`
(`crates/cobolt-codegen/src/lib.rs`): every form generates its own outer
`PROGRAM-ID`/`END PROGRAM` pair, with event handlers as COBOL-85 nested
programs scoped inside it — and at runtime, `form_runtime.rs`'s "Run Form"
spawns each running form as a **separate OS process** with its own
`ObjectRegistry` (`cobolt-runtime/src/objects.rs`). Two unrelated forms in
one project sharing a control ID like `BUTTON1` is already normal and
harmless. This is a materially different operation from the Designer's
existing `DesignerClipboard`/`paste_from_clipboard` (which merges a
*selection* into an **already-open** form and must dedupe against that
form's own controls) — Paste Form creates a brand-new form, so the copied
`Form`'s IDs travel unchanged.

**Registering the new form (R5, R10)** reuses the exact sequence
`save_new_form_to` already uses for a hand-created form: `save_form` to
disk, `CoboltProject::add_file`, `do_save_project`, then (new, per R10)
`write_generated_for(&cfrm_path, &form)` so its Generated Code exists
immediately rather than waiting for the next Build/Run/Debug/Check.

**Name collision (R7–R8)** reuses `form_cobol_id_conflict(name,
exclude_path) -> Option<PathBuf>` (already the exact check `create_new_form`
uses) to detect the collision *before* writing anything. On a hit, a new
modal (same shape as the existing R19-style `confirm_remove`/spec-045
alias-offer windows: a centered, non-collapsible `egui::Window`) offers
**Rename** (an editable name field, defaulting to `<name> (2)`,
re-checked live against the same conflict function) or **Replace**, which
requires its own second confirmation before deleting the existing form's
file (mirrors `ConfirmRemoveForm`'s existing delete-confirmation, never a
one-click overwrite).

## 2. Affected crates / files

- `crates/cobolt-forms/src/xml.rs` — new `pub fn form_to_string(form: &Form)
  -> Result<String, FormError>`; `save_form` becomes `fs::write(path,
  form_to_string(form)?.as_bytes())`.
- `crates/cobolt-ide/src/app.rs`
  - `CoboltApp` gains `pending_form_paste: Option<PasteFormTarget>` (`None`
    = no pending request; `Some` set when Paste Form is clicked, cleared
    once an `Event::Paste` is consumed or the developer navigates away).
  - Top-level input scan (wherever the app already inspects `ctx.input(|i|
    &i.events)` per frame, or a new small one near `update()`'s start):
    when `pending_form_paste.is_some()` and an `Event::Paste(text)` arrives,
    consume it — parse, then either open the R7 collision modal or register
    the form directly.
  - `copy_form(&mut self, cfrm_path: &Path)` — resolves the live Designer's
    `Form` if open, else `load_form(cfrm_path)`; `form_to_string`;
    `ctx.copy_text`.
  - `paste_form(&mut self, ctx: &egui::Context, target_dir: &Path)` — sends
    `RequestPaste`, sets `pending_form_paste`.
  - `finish_form_paste(&mut self, form: Form, dest_dir: &Path)` — the
    collision check + either the rename/replace modal state or the direct
    `save_form` + `add_file` + `do_save_project` + `write_generated_for`
    sequence (mirrors `save_new_form_to`).
  - New modal state + rendering, alongside the existing `confirm_remove`-
    style windows.
- `crates/cobolt-ide/src/panels/project.rs` — `show_form_item` gains
  `resp.context_menu(...)` with **Copy Form**; the Forms category header
  gains one with **Paste Form**, following `folder_context_menu`'s existing
  pattern. New `ProjectPanelEvent::CopyForm(PathBuf)` /
  `ProjectPanelEvent::PasteForm { dir_rel: PathBuf }` variants, handled in
  `app.rs` next to the other `ProjectPanelEvent` arms.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields (six languages):
  `tree_copy_form`, `tree_paste_form`, `paste_form_invalid_clipboard`,
  `paste_form_name_conflict_title`, `paste_form_name_conflict_body`,
  `paste_form_rename`, `paste_form_replace` (`btn_cancel` reused for the
  modal's cancel action, no new key).
- `docs/developers-guide-en.md` — a short new subsection near the forms/
  project-tree material describing Copy Form / Paste Form and the
  rename-or-replace prompt.
- `CHANGELOG.md` / `crates/cobolt-ide/src/version.rs` — new entry, `z` bump
  only per the operator's standing rule.

## 3. Data / model changes

- No `.cfrm` schema change — the clipboard payload *is* today's `.cfrm` XML,
  verbatim (`form_to_string`/`load_form_from_str` are exact inverses of the
  file-based `save_form`/`load_form`).
- No project-file (`cobolt.toml`) schema change — a pasted form registers
  through the same `CoboltProject::add_file` path any other new form does.
- Runtime-only addition to `CoboltApp`: `pending_form_paste` and the new
  rename/replace modal's state (a small struct, not persisted).

## 4. Key decisions & alternatives

- **Decision:** use `ViewportCommand::RequestPaste` + a pending-flag scan
  for `Event::Paste`, not a direct `arboard` dependency for a synchronous
  read. **Why:** this is the mechanism `egui-winit` itself already uses
  internally for Cmd+V (confirmed in its source) — going through it keeps
  clipboard access inside egui's own event/platform abstraction (already
  correct on macOS/Windows/Linux via the existing `eframe` dependency) and
  needs no new crate. **Rejected:** adding `arboard` directly for a
  synchronous "read now" call — `arboard` is already pulled in transitively
  and duplicating that dependency (and its platform-specific clipboard
  backends) explicitly would fight the framework instead of using it, for
  a capability egui already exposes.
- **Decision:** the pasted form's control IDs and paragraph names are never
  remapped. **Why:** verified — each form is its own `PROGRAM-ID`/own
  runtime process; there is no cross-form namespace to collide in.
  **Rejected:** reusing `DesignerPanel::paste_from_clipboard`'s remap
  machinery wholesale (the spec's original R6, written before this was
  verified) — would add real complexity (ID generation needs a live
  `DesignerPanel`-like context; paragraph dedup would need scanning every
  other form in the target project) to solve a collision that cannot
  actually occur.
- **Decision:** Copy reads the *live* Designer form when the source form is
  open with unsaved edits, not the stale on-disk file. **Why:** resolves
  spec §7's open question — "copy" should mean "copy what I'm looking at";
  silently copying a stale save would be a surprising, hard-to-notice
  paste-comes-back-wrong bug. **Rejected:** always reading disk (simpler,
  but wrong the moment a developer copies mid-edit) and forcing a save
  first (extra friction for a read-only operation, and the developer might
  not want to save yet).
- **Decision:** collision handling is a dedicated rename-or-replace modal,
  not `reject_duplicate_form_cobol_id`'s existing reject-only behavior.
  **Why:** the operator's answer was explicit ("prompt to rename or
  replace"); the existing helper only refuses. **Reused, not replaced:**
  the modal's live conflict re-check while the developer edits the rename
  field calls the same underlying `form_cobol_id_conflict`
  `reject_duplicate_form_cobol_id` wraps, so both paths agree on what counts
  as a conflict.
- **Decision:** Paste Form always creates the new form on disk immediately
  (R10's regeneration included) rather than opening it in an unsaved
  Designer tab first. **Why:** matches `save_new_form_to`'s existing
  behavior for hand-created forms — a pasted form is exactly as "real" as
  one built by hand, and leaving it unsaved would contradict R2's promise
  that copy/paste doesn't lose anything. The Designer still opens
  afterward (same as `save_new_form_to`) so the developer immediately sees
  what landed.

## 5. Risks & mitigations

- **Risk:** `Event::Paste` could theoretically be consumed by a focused
  `TextEdit` before the app-level scan sees it, if egui's widget-level
  paste handling runs first and the event doesn't remain in `ctx.input`'s
  event list for later inspection. **Mitigation:** `ctx.input(|i| &i.events)`
  exposes the full raw event list for the whole frame regardless of what
  any widget does with it (confirmed by existing patterns elsewhere in this
  codebase reading `ctx.input` for keys/pointer state alongside normal
  widget interaction) — the app-level scan does not need to "grab" the
  event first, only read the same list widgets do. Verify empirically
  during T2's implementation with a manual paste-while-a-field-is-focused
  check; if there's any interference, gate `pending_form_paste` consumption
  to only fire when no `TextEdit` has focus at request time (Paste Form is
  invoked from a tree context menu, not while editing text, so this should
  never actually trigger).
- **Risk:** a developer pastes a `.cfrm` XML copied from a **much older**
  PowerRustCOBOL version whose schema `load_form_from_str` can't fully
  parse. **Mitigation:** out of scope per spec §2's non-goals — R9's
  refusal already covers "does not parse," which includes this case; no
  special-cased partial-recovery is attempted.
- **Risk:** pasting a form whose blocks reference a Project's Crates pin,
  an asset, or an indexed file the target project doesn't have leaves a
  form that fails Check. **Mitigation:** explicitly a non-goal (spec §2) —
  the manual verification step should include pasting such a form and
  confirming the failure is a normal, clear Check error at the right line,
  not a crash or a silent partial paste.

## 6. Test strategy

- **`cobolt-forms/src/xml.rs`** (unit): `form_to_string_and_load_form_from_str_round_trip`
  — build a `Form` with at least one control carrying a bound event (non-
  empty `EventBinding.code`), an animation, and a data binding;
  `form_to_string` → `load_form_from_str` → assert deep equality with the
  original. `save_form_and_form_to_string_agree` — for the same `Form`,
  `save_form` to a temp path and read it back, compare byte-for-byte
  against `form_to_string`'s output (proves the refactor didn't change the
  file-writing path's output).
- **`cobolt-ide/src/app.rs`** (unit, no real clipboard — inject text
  directly into the paste-handling function rather than going through a
  real OS clipboard round trip, which a headless test can't reliably do):
  - `finish_form_paste_registers_a_new_form_with_ids_untouched` — a `Form`
    whose controls reuse an ID also present in another form already in a
    fixture project; assert the new `.cfrm` is written with those IDs
    completely unchanged (proves R6's "no remap" design, not just its
    absence of a bug).
  - `finish_form_paste_regenerates_immediately` — after registering, assert
    the corresponding Generated Code file exists and is non-empty without
    any further action (R10).
  - `name_collision_offers_rename_or_replace` — a fixture project already
    containing a form named `X`; pasting a form also named `X` opens the
    modal state (not a silent create, not a silent refusal); accepting
    Rename with a new name creates it alongside the original untouched;
    accepting Replace (with its own confirmation) removes the original
    first.
  - `invalid_clipboard_text_is_refused_and_changes_nothing` — arbitrary
    non-XML text (and separately, well-formed XML that isn't a `<Form>`)
    both refuse cleanly; assert no file was written and no project state
    changed.
- **i18n:** the existing repo-wide `i18n_tests` catches any missing
  translation automatically.
- **Manual/visual** (the operator, two ways per plan §1's read-path risk):
  1. **Same IDE session, two projects**: open project A, right-click a form
     with at least one control with a real event handler, Copy Form; open
     project B (same running IDE), right-click Forms, Paste Form; open the
     new form in the Designer and confirm every control, property and event
     handler matches; Build/Run it and confirm the handler actually runs.
  2. **Two separate IDE processes**: launch a second `cobolt-ide` instance
     against a different project, Copy in the first, Paste in the second —
     proves the OS clipboard (not just in-process state) carries the data.
  3. Paste with the clipboard holding unrelated text (e.g. copied from a
     browser) — confirm the clear refusal message, not a crash or a garbage
     tree entry.
  4. Trigger the name-collision prompt both ways (rename, replace) and
     confirm Replace's own confirmation step is a genuine, separate click.

## 7. Steering compliance

- [ ] i18n: `tree_copy_form`, `tree_paste_form`, `paste_form_invalid_clipboard`,
      `paste_form_name_conflict_title`, `paste_form_name_conflict_body`,
      `paste_form_rename`, `paste_form_replace` — all six languages
      (`btn_cancel` reused, no new key).
- [ ] Generated-code banner + regenerate-on-action contract: preserved and
      extended — R10 adds "paste" to the set of actions that trigger
      immediate regeneration for the affected form, via the same
      `write_generated_for` every other action already uses.
- [ ] English dev guide updated; `-es/-pt/-jp/-cn` translations untouched,
      `-fr` continues not to exist (user-maintained per standing rule).
- [ ] Fix vs feature: **feature** (new capability, no COBOL-85 conformance
      involved). Bump `z` only per the operator's standing rule; ask before
      touching `y`/`x`.
- [ ] No "cobolt" in user-facing text; COBOL identifiers/generated source
      stay English and untouched by this feature (it moves an already-valid
      `Form` between projects, never edits COBOL text itself beyond what
      the developer's own handler code already was).
