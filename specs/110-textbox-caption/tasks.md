<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Caption do TextBox

- **Status:** in progress
- **Plan:** ./plan.md   **Date:** 2026-10-31

Ordered, small, independently-verifiable tasks. References are spec.md sections (§n) and acceptance bullets (AC). Branch `features-2x-caption` (worktree). On 2.x.x never touch `README.md` or `docs/`.

**Standing verification, every task that changes Rust:** `cargo test -p cobolt-forms --features render --no-fail-fast`, every `test result:` line read; `cobolt-ide` with `--bin`. Known reds on 2.x.x are reported as pre-existing.

- [x] **T1 — Pure core** (§3, §4, §5, §8)
  - Files: `crates/cobolt-forms/src/caption.rs`, `lib.rs`.
  - Do: types, `caption_rect`, `outer_bounds`, `FontOverride::resolve`, `parse_mnemonic`, tests of the §8 table and overflow.
  - Verify: `cargo test -p cobolt-forms --features render --lib caption::` (8 passed).

- [x] **T2 — Model properties** (§3, §4)
  - Files: `crates/cobolt-forms/src/model.rs`.
  - Do: `Caption*` names in `property_names_for`, get/set by name (a key is present exactly when set; never seeded in `Control::new`), `reset_caption(attr)`, `caption_is_set(attr)`, a `Caption` view resolved against the TextBox font/ForeColor.
  - Verify: unit tests: set-equal-to-host marks set (AC), only-Bold keeps the rest inherited, reset restores, inherited read returns the resolved value.

- [x] **T3 — `.cfrm` round-trip** (§4, AC save/reopen, AC old form)
  - Files: `crates/cobolt-forms/src/xml.rs`, `crates/cobolt-forms/tests/textbox_caption_roundtrip.rs`.
  - Do: write only set attributes; a form without `Caption*` serialises byte-identical.
  - Verify: test saves, reopens, compares set/inherited per attribute; golden of an old form unchanged. The result block reports cases and timings (rule #7).

- [x] **T4 — Measurement and painting, both paths** (done via one hook: paint_textbox_caption after draw_control / draw_control_face / draw_control_capturing_label; no pixel-level render test yet, to be checked by eye) (§5, §6, AC move/resize/overflow/visible/disabled)
  - Files: `crates/cobolt-forms/src/paint.rs`, `render.rs`, `textmetrics.rs`.
  - Do: one measuring function shared by designer and runtime; draw the caption (mnemonic underline, back colour, disabled colour); Visible follows the box; relayout when font/geometry/Caption* changes.
  - Verify: tests for the four positions moving with the box, FontSize change updating an inherited caption in the same frame; guard `engine_reference_form_parity_static_vs_faces` green.

- [x] **T5 — Interaction** (focus by click and Alt+mnemonic done and tested; accessible name NOT yet) (§6, AC focus/tab)
  - Files: `crates/cobolt-forms/src/render.rs`, `model.rs`.
  - Do: caption rect joins the hit area (not the edit area) and focuses the box; Alt+mnemonic; accessible name; tab order unchanged.
  - Verify: tests extending `set_focus_moves_the_keyboard_focus.rs` style.

- [x] **T6 — COBOL access and System KB** (§6) (ResetCaption/CaptionIsSet in cobolt-ast + interpreter, inherited reads, INHERIT marker merged in render::merge_props, compiler doc tables, chunked.data rebuilt; cobolt-project-tools reference.rs, cobolt-codegen and cobolt-wasm NOT yet checked: T7)
  - Files: tables in `cobolt-compiler`, `cobolt-project-tools/src/reference.rs`, `cobolt-codegen` (only if a static table lists TextBox members), `assets/knowledge/chunked.data`.
  - Do: register `Caption*`, `ResetCaption`, `CaptionIsSet` (`is_known_method`); regenerate with `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: a sample handler using the §6 forms compiles (`check`); `prebuilt_chunked_kb_matches_the_published_documentation` green.

- [ ] **T7 — Hosts parity** (interpreter rule)
  - Files: `cobolt-runtime`, `run_form_app` in `cobolt-compiler`, `cobolt-wasm`, `cobolt-web-player` (skill `interpreter-binary-parity`).
  - Do: the property/method behaviour lands in every host that runs a form.
  - Verify: each host runs the sample and reads back a resolved inherited value.

- [ ] **T8 — Designer** (§7)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`, property grid, `i18n.rs`.
  - Do: Caption group (inherited grey, set bold), Restore default, select/drag by caption, snap on inner bounds, rubber band on outer, in-place edit, warnings, one undo step per change; every string a `Tr` field in six languages.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n`; designer unit tests for snap/outer-bounds; the §7 checklist is by hand (operator).

- [ ] **T9 — Subform and outlet** (§5, AC subform)
  - Files: tests in `cobolt-forms/tests/`.
  - Verify: TextBox with caption inside a subform and an outlet renders at the translated place.

- [ ] **T10 — Docs** — deferred: the Developer's Guide is not touched on 2.x.x; written afresh when 2.x.x merges to main (memory: 2.x-only docs wait for main merge). Record this in CHANGELOG.

- [ ] **T11 — Finalize**
  - Do: bump `z` in `version.rs` (collision with the shared tree's uncommitted 2.0.111: decide at commit), top CHANGELOG entry, credential scan of the staged diff, move stray non-Rust files to quarantine.
  - Verify: full sweep of the touched crates; every AC in spec.md §8 has a passing test or a named manual check.

## Done criteria
Every acceptance bullet in spec.md §8 is covered, tests pass, one `features` commit per slice, merge into 2.x.x and push only as the operator asked (outside Brazilian work hours).
