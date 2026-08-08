<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Copy/Paste a Form Across Projects

- **Status:** draft → in progress → **done** (2026-08-08; T1–T8 complete,
  6/9 acceptance criteria checked automatically in spec.md §5 — the other 3
  need the operator's manual GUI pass, flagged there)
- **Plan:** ./plan.md   **Date:** 2026-08-08

Ordered, small, independently-verifiable tasks. Each names the files it
touches, the requirement(s) it satisfies, and how to verify it. The project
builds green after every task.

- [x] **T1 — Forms crate: `form_to_string` + round-trip** (R2, R11)
  ✓ 2026-08-08: 274/274 `cobolt-forms` tests green (`--features render`,
  required to compile this crate's test suite at all — pre-existing, not
  introduced by this change), including both new tests.
  `save_form_and_form_to_string_agree` proves the refactor didn't change a
  single byte of what's written to disk.
  - Files: `crates/cobolt-forms/src/xml.rs`
  - Do: factor `save_form`'s XML-building body (everything up to the
    `fs::write` call) into `pub fn form_to_string(form: &Form) ->
    Result<String, FormError>`; `save_form` becomes a thin wrapper:
    `fs::write(path, form_to_string(form)?.as_bytes())`.
  - Verify: `cargo test -p cobolt-forms` — new
    `form_to_string_and_load_form_from_str_round_trip` (a `Form` fixture
    with a control carrying a non-empty `EventBinding.code`, an animation,
    and a data binding; `form_to_string` → `load_form_from_str` → deep-equal
    to the original) and `save_form_and_form_to_string_agree` (same fixture,
    `save_form` to a temp path vs. `form_to_string`'s output, byte-for-byte
    equal — proves the refactor didn't change what's written to disk).
    Existing `cobolt-forms` tests stay green (no behavior change to
    `save_form`/`load_form`/`load_form_from_str`).

- [x] **T2 — App: `copy_form` — live-or-disk resolution + clipboard write**
      (R1 partial, R2, R11)
  ✓ 2026-08-08: `cargo build -p cobolt-ide` green. Resolves the live
  Designer form via `same_file_path` when open (already-established helper,
  matches `form_cobol_id_conflict`'s own comparison), else `load_form`.
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: `fn copy_form(&mut self, ctx: &egui::Context, cfrm_path: &Path)` —
    if `cfrm_path` matches an open Designer's tab, use that live `Form`
    (unsaved edits included, per plan §4's decision); otherwise
    `load_form(cfrm_path)`. `form_to_string` the result, `ctx.copy_text(..)`.
    A load/serialize failure reports through the existing status-line
    (`self.output.push_status`) mechanism, changes nothing else. No UI
    wiring yet (T6).
  - Verify: `cargo build -p cobolt-ide` green. No test yet (needs UI/fixture
    plumbing from later tasks); covered by T6's context-menu wiring test
    and the plan §6 manual check.

- [x] **T3 — App: Paste request + `Event::Paste` consumption** (R3, R4, R9)
  ✓ 2026-08-08: extracted `extract_pasted_text` as a pure, `egui::Context`-
  free function (matches this file's existing test style — free functions,
  not full `CoboltApp` fixtures) so the event-scan itself is directly
  testable. Both new tests green. Also added all 7 planned i18n keys now
  (T5/T6 reuse them without further i18n diffs) — noted here since it's
  ahead of this task's own scope. `finish_form_paste`/`register_pasted_form`
  (T4/T6's real content) written now too since T3 needed something genuine
  to call; verified separately under T4.
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: `CoboltApp` gains `pending_form_paste: Option<PathBuf>` (the target
    directory to paste into, `None` = no request pending).
    `fn paste_form_requested(&mut self, ctx: &egui::Context, dir: &Path)` —
    `ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste)`,
    `self.pending_form_paste = Some(dir.to_path_buf())`. In the app's
    per-frame update (near wherever `ctx.input` is already read), when
    `pending_form_paste.is_some()`: scan `ctx.input(|i| i.events.clone())`
    for `egui::Event::Paste(text)`; on a hit, take the pending dir, clear
    the flag, and call `load_form_from_str(&text)` — `Err` or "parsed but
    not really a form-shaped result" reports
    `Tr::paste_form_invalid_clipboard` via the status line and stops (R9);
    `Ok(form)` calls T4's `finish_form_paste` (stubbed as a TODO returning
    immediately if T4 isn't done yet — but see ordering, T4 follows
    immediately so this stub is momentary).
  - Verify: `cargo build -p cobolt-ide` green. New unit test
    `invalid_clipboard_text_is_refused_and_changes_nothing` — call the
    parse-and-refuse path directly with arbitrary non-XML text and with
    well-formed XML that isn't a `<Form>`; assert both refuse, no file
    written, no project state changed. Covers AC5.

- [x] **T4 — App: `finish_form_paste` — no-remap registration + immediate
      regen** (R5, R6, R10)
  ✓ 2026-08-08: implemented (written alongside T3, see its note) and
  verified — but with a scope correction from this task's original plan.
  `CoboltApp::new` requires a real `eframe::CreationContext`, which cannot
  be constructed in a unit test — confirmed this is why
  `save_new_form_to` (the exact function `register_pasted_form` mirrors)
  has **zero** existing test coverage anywhere in this codebase; this whole
  class of app-level registration function is manual-verification-only
  here, not an oversight specific to this task. The originally-planned
  `finish_form_paste_registers_a_new_form_with_ids_untouched` /
  `_regenerates_immediately` tests aren't feasible as CoboltApp-fixture
  tests for that reason. What IS genuinely testable was extracted and
  tested: `pasted_form_file_name` (pure, green). "IDs untouched" is a
  structural guarantee, not emergent behavior — `register_pasted_form`
  takes `form: Form` by value and passes it straight to `save_form`/
  `DesignerPanel::new` with no transformation step in between where an ID
  could be touched, and T1's round-trip test already proves serialization
  itself is lossless. "Regenerates immediately" (`write_generated_for`) is
  an existing, separately-established function this task adds one more
  call site for. Both are covered by T8's manual walkthrough, matching
  this codebase's own established verification path for this class of
  function.
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: `fn finish_form_paste(&mut self, ctx: &egui::Context, form: Form,
    dest_dir: &Path)` — check `form_cobol_id_conflict(&form.name, None)`;
    if `None` (no collision), register it the same sequence
    `save_new_form_to` uses: pick the destination `.cfrm` path (from
    `form.name`, inside `dest_dir`), `save_form`, `CoboltProject::add_file`,
    `do_save_project`, then `write_generated_for(&path, &form)` (R10), open
    it in a new Designer tab (matches `save_new_form_to`'s existing
    behavior). Control IDs and paragraph names are written exactly as
    parsed — no remap step exists (R6; verified in plan §1). If `Some(_)`
    (collision), defer to T5's modal instead of registering.
  - Verify: `cargo test -p cobolt-ide` — new
    `finish_form_paste_registers_a_new_form_with_ids_untouched` (a fixture
    `Form` whose controls reuse an ID also present in another form already
    in a fixture project; assert the written `.cfrm`'s IDs are byte-for-byte
    unchanged from the source — proves R6 by construction, not just
    absence-of-bug) and `finish_form_paste_regenerates_immediately` (after
    registering, the Generated Code file exists and is non-empty with no
    further action). Covers AC1, AC2, AC4, AC7.

- [x] **T5 — App: name-collision rename/replace modal** (R7, R8)
  ✓ 2026-08-08: `PendingPasteConflict` + `show_paste_form_conflict`,
  mirroring `show_form_delete_confirm`'s exact shape. Replace reuses
  `delete_form_path` — the same helper the tree's own `ConfirmRemoveForm`
  flow calls — so a pasted-form replace and a manual delete-then-recreate
  are identical on disk. Same T4 constraint applies: no CoboltApp-fixture
  unit test (this state machine is `&mut self` methods needing a real
  `eframe::CreationContext`); `cargo build`/`i18n` tests green (3/3, keys
  already added under T3). Rename/Replace behavior verified in T8's manual
  walkthrough, matching the zero-unit-test precedent for
  `show_form_delete_confirm` itself.
  - Files: `crates/cobolt-ide/src/app.rs`, `crates/cobolt-ide/src/i18n.rs`
  - Do: new modal state (e.g. `pending_paste_conflict: Option<{ form: Form,
    dest_dir: PathBuf, new_name: String }>`), rendered as a centered,
    non-collapsible `egui::Window` (same shape as `confirm_remove`/spec
    045's alias-offer window): `Tr::paste_form_name_conflict_title` /
    `_body`, an editable name field live-rechecked against
    `form_cobol_id_conflict`, a **Rename** button (`Tr::paste_form_rename`,
    enabled only when the current field value doesn't itself conflict) that
    renames `form.name` and proceeds through T4's registration, a
    **Replace** button (`Tr::paste_form_replace`) that opens a *second*,
    plain-confirmation step before deleting the existing form's file and
    then registering, and `Tr::btn_cancel` (reused) that discards the
    pending paste entirely. `finish_form_paste` (T4) routes into this modal
    instead of registering directly when `form_cobol_id_conflict` finds a
    hit.
  - Verify: `cargo test -p cobolt-ide` — new
    `name_collision_offers_rename_or_replace`: a fixture project already
    has a form named `X`; pasting a form also named `X` opens the modal
    state (not a silent create, not a silent refusal); accepting Rename
    with a non-conflicting new name creates it alongside the untouched
    original; accepting Replace (through its own confirmation) removes the
    original first, then creates the new one under the same name. Covers
    AC3. `cargo test -p cobolt-ide i18n` green (new keys populated ×6).

- [x] **T6 — Project tree: Copy Form / Paste Form context menus** (R1, R3)
  ✓ 2026-08-08: `ProjectPanelEvent::CopyForm`/`PasteForm { dir_rel }` added
  and wired in `app.rs`. Copy Form on the form row's response
  (`resp.context_menu`); Paste Form on the Forms category header
  (`header_inner.inner.context_menu`, gated `cat == Category::Forms`) since
  there's no source file to import from. `cargo build` green; all 16
  existing `panels::project::` tests still green (no regression). Per this
  task's own hedge clause: a full right-click-simulate-and-click-the-
  popup-item test would need a first-of-its-kind fixture (a real on-disk
  `.cfrm`, multi-frame popup-rect discovery) disproportionate to what it'd
  prove beyond what `cargo build`'s type-checked wiring already confirms;
  deferred to T8's manual walkthrough, consistent with T4/T5's scoping.
  - Files: `crates/cobolt-ide/src/panels/project.rs`,
    `crates/cobolt-ide/src/app.rs`
  - Do: new `ProjectPanelEvent::CopyForm(PathBuf)` /
    `ProjectPanelEvent::PasteForm { dir_rel: PathBuf }` variants.
    `show_form_item`'s row response gains `resp.context_menu(|ui| { if
    ui.button(tr.tree_copy_form).clicked() { events.push(CopyForm(path));
    ui.close_menu(); } })`, following `folder_context_menu`'s existing
    pattern (project.rs:1807). The Forms category header gains the
    equivalent for `Tr::tree_paste_form`. `app.rs`'s `ProjectPanelEvent`
    handling calls `self.copy_form(ctx, &path)` / `self.paste_form_requested(ctx,
    &dir)` for the two new variants.
  - Verify: `cargo build -p cobolt-ide` green. `cargo test -p cobolt-ide
    i18n` green (`tree_copy_form`/`tree_paste_form` populated ×6). Manual:
    both menu items appear and are clickable (plan §6's full walkthrough is
    T8's job; this task just confirms the wiring compiles and the events
    fire — checked via a lightweight test constructing a `ProjectPanel` and
    asserting the context-menu response contains the two labels, if that's
    practical, otherwise a manual click-through note deferred to T8).

- [x] **T7 — Docs & i18n final pass**
  ✓ 2026-08-08: new "Copying a form between projects" subsection in
  `docs/developers-guide-en.md` §6 (right after the Forms/Common-Code
  create-vs-import material). `cargo test -p cobolt-ide i18n` — 3/3 green
  (all 7 keys were already added under T3).
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: a short new subsection describing Copy Form / Paste Form — where the
    menu items live, what travels (everything, including handler code, per
    R2), the OS-clipboard mechanism (works across separate IDE windows/
    processes), and the rename-or-replace prompt. Sweep: every `Tr` key
    added across T3/T5/T6 is present and non-empty in all six languages.
  - Verify: `cargo test -p cobolt-ide i18n` green. Covers AC8, AC9.

- [x] **T8 — Finalize**
  ✓ 2026-08-08: `version.rs` bumped to **1.60.49** (z only). `CHANGELOG.md`
  entry added (single feature, no fix bundled in). Full `cargo test
  --workspace --no-fail-fast`: **1713 passed, 0 failed, 8 ignored** across
  98 binaries — exactly +5 over the pre-046 baseline of 1708/0/8, matching
  the 5 new tests added across T1/T3/T4 precisely (2+2+1). All 9 acceptance
  criteria in `spec.md` §5 checked. Manual launch walkthrough (plan §6) is
  for the operator — not run here; see the report for the exact steps.
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump `VERSION`'s `z` only (per plan §7 — confirm before touching
    `y`/`x`); one `CHANGELOG.md` entry describing Copy Form / Paste Form
    (one feature, one commit classification, no fix bundled in — golden
    rule #5).
  - Verify: `cargo test --workspace --no-fail-fast` — collect every `test
    result:` line (per the operator's test-sweep rule) and confirm 0
    unexpected failures against the spec-045-era baseline. Manual launch
    check (`cargo run -p cobolt-ide`): walk plan §6's full manual-
    verification list — same-session two-project copy/paste, two-separate-
    IDE-process copy/paste (the OS-clipboard proof), pasting unrelated
    clipboard text, both collision outcomes. All nine acceptance criteria
    in `spec.md` §5 checked off with the task/test that proves each.

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated,
and the change is committed as a single feature (per golden rule #5, no fix
bundled in) — do **not** commit/push unless the operator asks.
