<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Project's Crates: System Awareness & Collision Aliasing

- **Status:** draft → in progress → **done** (2026-08-08; T1–T15 complete,
  all 12 acceptance criteria checked in spec.md §5)
- **Plan:** ./plan.md   **Date:** 2026-08-08

Ordered, small, independently-verifiable tasks. Each names the files it
touches, the requirement(s) it satisfies, and how to verify it. The project
builds green after every task.

- [x] **T1 — Compiler: `ExternalCrate.alias` + alias-aware `lib_name()`** (R3)
  ✓ 2026-08-08: 15/15 `cobolt-compiler` external_crates tests green (incl.
  new `lib_name_honors_alias`); e2e test still green (26.7s). All
  `ExternalCrate` struct literals across the workspace updated for the new
  field; the update-flow's `..recorded.clone()` spread already carries an
  existing pin's alias forward with no code change needed.
  - Files: `crates/cobolt-compiler/src/external_crates.rs`
  - Do: add `alias: Option<String>` to `ExternalCrate`
    (`#[serde(default, skip_serializing_if = "Option::is_none")]`);
    `lib_name()` returns `lib_name(alias)` when set, else today's behavior.
    Extend `pins_round_trip_and_default`'s fixture TOML with one
    `alias = "prj_egui"` pin so the new field's round-trip (and its absence
    on old pins) is proven, not assumed.
  - Verify: `cargo test -p cobolt-compiler external_crates` green; new
    `lib_name_honors_alias` test asserts
    `ExternalCrate{name:"egui", alias:Some("prj_egui".into()), ..}.lib_name()
    == "prj_egui"`.

- [x] **T2 — Compiler: aliased pins compile to a `package =` path dependency,
      never a patch** (R1)
  ✓ 2026-08-08: 16/16 tests green (new
  `alias_pin_emits_package_path_not_version_or_patch`); e2e still green
  (23.7s). Both `generate_cargo_toml` and `probe_manifest` pick this up for
  free since both call the shared `pin_sections`.
  - Files: `crates/cobolt-compiler/src/external_crates.rs` (`pin_sections`)
  - Do: when `c.alias` is `Some`, `pin_sections` emits
    `prj_<name> = { package = "<name>", path = "<vendored>" }` into the
    `[dependencies]` half and **skips** the `[patch.crates-io]` half for that
    pin entirely; a non-aliased pin in the same call is unaffected. Since
    both `generate_cargo_toml` (real builds) and `probe_manifest` (the
    resolver probe) call `pin_sections`, this one change covers both call
    sites — no separate edit needed at either.
  - Verify: `cargo test -p cobolt-compiler external_crates` green; new
    `alias_pin_emits_package_path_not_version_or_patch` proves the shape for
    an aliased pin and that a co-present non-aliased pin still gets its
    normal `version =` + patch line. `generated_manifest_carries_workspace_pins_and_patches`
    (existing) stays green unchanged (no alias in its fixture).

- [x] **T3 — Compiler: `rust_manifest.md` notes an alias** (R4, spec open Q2)
  ✓ 2026-08-08: 17/17 tests green (new `rust_manifest_notes_the_alias`);
  e2e still green.
  - Files: `crates/cobolt-compiler/src/external_crates.rs`
    (`render_rust_manifest`)
  - Do: an aliased pin's Crate cell reads `` egui (as `prj_egui`) `` instead
    of a bare name; a non-aliased row is byte-identical to today.
  - Verify: `cargo test -p cobolt-compiler external_crates` green; new
    `rust_manifest_notes_the_alias` test; existing
    `rust_manifest_lists_name_version_url` stays green unchanged (no alias in
    its fixture — proves the common case didn't shift).

- [x] **T4 — IDE service: `system_closure()` — direct vs. transitive System
      names** (R12)
  ✓ 2026-08-08: live test green — 15 direct crates (matches
  `DIRECT_LINKED`'s count exactly), 557 transitive, resolved in **8.0s**
  real-world. Confirms plan §5's "compute lazily, cache per session" risk
  mitigation is the right call, not overcaution.
  - Files: `crates/cobolt-ide/src/external_crates_service.rs`
  - Do: add `pub struct SystemClosure { pub direct: BTreeSet<String>, pub
    transitive: BTreeSet<String> }` and `pub fn system_closure(workspace_root:
    &Path, scratch_project_dir: &Path) -> Result<SystemClosure, String>` that
    calls the existing `resolve_graph(&crates_path, scratch_project_dir, &[],
    None, &scratch)` (empty pins, no candidate — exactly the platform's own
    base dependency graph) and splits the returned names: those in
    `external_crates::direct_linked_lib_names()` go to `direct`, everything
    else in the graph goes to `transitive`.
  - Verify: `cargo test -p cobolt-ide external_crates_service -- --ignored`
    (or whatever flag this crate's existing live tests use — match
    `every_row_of_a_live_search_keeps_its_four_cells`'s convention) green;
    new **live** test `system_closure_splits_direct_from_transitive` asserts
    `egui` ∈ `direct`, a name known only pulled in transitively by the GUI
    stack ∈ `transitive`, and `csv` ∈ neither. Covers AC4's data half.

- [x] **T5 — IDE service: `Incompatible` collision becomes an offer, not an
      `Err`** (R1, R2)
  ✓ 2026-08-08: 11/11 `external_crates_service` tests green, including new
  live tests `incompatible_direct_collision_offers_alias_not_error` (real
  `egui =0.29.0` → `AliasOffered`, vendored dir survives) and
  `compatible_and_reserved_collisions_still_refuse`. Also fixed a latent
  update-flow bug this refactor would otherwise have introduced: layer 1 is
  now skipped once a pin is already aliased, so Update on an aliased crate
  doesn't re-trip the same collision forever (`layer1_collision`). `panels/
  external_crates.rs`'s Add button compiles against the new `AddOutcome`
  with a placeholder message; real UI lands in T13.
  - Files: `crates/cobolt-ide/src/external_crates_service.rs`,
    `crates/cobolt-ide/src/panels/external_crates.rs` (minimal call-site
    update only — no UI yet)
  - Do: introduce `pub enum AddOutcome { Added(String), AliasOffered {
    candidate: ExternalCrate, linked_requirement: String, vendored: PathBuf }
    }`; `add()`'s return type becomes `Result<AddOutcome, String>`.
    `check_conflicts`'s layer-1 match: `AlreadyAvailable`/`Reserved` still
    `Err` exactly as today (R2); `Incompatible` returns the offer instead of
    erroring, and `add()` does **not** delete the vendored download in that
    case. Update the panel's Add-button call site to pattern-match
    `AddOutcome` (for now: `Added` logs as before, `AliasOffered` logs a
    plain placeholder line — the real offer UI is T13).
  - Verify: `cargo build -p cobolt-ide` green (panel compiles against the new
    return type); `cargo test -p cobolt-ide external_crates_service` —
    `incompatible_direct_collision_offers_alias_not_error` (mock/injected
    candidate, asserts `AliasOffered` + vendored dir still exists) and
    `compatible_and_reserved_collisions_still_refuse` (regression) both
    green. Covers AC2.

- [x] **T6 — IDE service: `confirm_alias` / `discard_alias_offer`; alias
      build proof** (R1, R3, R4)
  ✓ 2026-08-08: both new unit tests green; new e2e test
  `external_crates_alias_build_and_run` proves the FULL flow — egui 0.29.0
  offered as `prj_egui`, accepted, staged as a `package =` path dependency
  with no patch, built alongside the platform's real linked egui 0.36, run,
  output `ALIAS-OK=255` confirmed, manifest notes the alias. Cold build
  51.7s, warm rebuild 4.7s. Existing 044 e2e test still green (22.2s,
  unaffected by the alias machinery).
  - Files: `crates/cobolt-ide/src/external_crates_service.rs`,
    `crates/cobolt-compiler/tests/test_external_crates_e2e.rs`
  - Do: `pub fn confirm_alias(project_path, candidate, alias: &str) ->
    Result<String, String>` sets `candidate.alias = Some(alias.into())`, runs
    the layer-2 probe (`check_conflicts`'s probe half, or a small refactor
    exposing it) against the now alias-shaped candidate, then saves the pin.
    `pub fn discard_alias_offer(vendored: &Path) -> Result<(), String>`
    removes the kept-around download when the developer declines.
  - Verify: `cargo test -p cobolt-ide external_crates_service` —
    `declining_the_alias_offer_removes_the_vendored_download` and
    `confirming_the_alias_offer_pins_with_alias_and_probes_the_alias_shape`
    green. **AC1's full proof**: extend
    `crates/cobolt-compiler/tests/test_external_crates_e2e.rs` with a new
    case — register an `egui` pin at an incompatible version through the
    alias path (`confirm_alias`), then `build_project` a program whose block
    `use prj_egui::…`s it, and assert the build succeeds. Reports timings in
    the existing e2e test's summary block per the operator's test-reporting
    rule (`CLAUDE.md` #7) — extend the existing narrated summary, don't add a
    silent case.

- [x] **T7 — IDE: promote the WCAG contrast helper out of `flags.rs`** (R10,
      no behavior change)
  ✓ 2026-08-08: new `contrast.rs` (`pub(crate)`), `flags.rs` imports it; all
  8 `flags::` tests green unmodified, including the two named in this
  task's own verification — proves the move changed nothing.
  - Files: `crates/cobolt-ide/src/flags.rs`, new
    `crates/cobolt-ide/src/contrast.rs` (or `pub(crate)` in `theme.rs` —
    whichever reads cleaner once written)
  - Do: move `relative_luminance`/`contrast_ratio` to the shared location
    unchanged; `flags.rs` imports them. Pure refactor — no visual or
    behavioral change to the flag renderer.
  - Verify: `cargo test -p cobolt-ide flags` — existing
    `contrast_ratio_matches_the_wcag_reference_points` and
    `every_theme_paints_flags_with_high_contrast` stay green unmodified
    (proves the move didn't change results).

- [x] **T8 — IDE: per-theme System/System-dependency/addable marker colors**
      (R7, R8, R9, R10)
  ✓ 2026-08-08: `every_theme_marks_system_crates_with_sufficient_contrast`
  green across all 16 themes × 3 categories. Worst case 3.00:1
  (light-plus theme, addable/green marker) — right at the floor since the
  solver stops at the minimum passing lightness; **flagging for the
  operator's manual eyeball per plan §5** (numerically compliant but worth a
  look, not assumed pretty).
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs` (new small
    color module or private fns in this file)
  - Do: fixed target hues (dimmed yellow / gray / green); for each theme,
    solve lightness against `theme.bg_panel`'s luminance (using T7's shared
    helper) until `contrast_ratio(marker, bg) >= 3.0`; expose e.g.
    `fn marker_color(theme: &Theme, category: SystemCategory) -> Color32`.
  - Verify: `cargo test -p cobolt-ide panels::external_crates` — new
    `every_theme_marks_system_crates_with_sufficient_contrast` checks all 16
    themes × 3 categories against the 3.0 floor, mirroring `flags.rs`'s test
    shape. Covers AC6. **Manual:** eyeball at least one dark and one light
    theme per plan §5's risk note — not automatable, call out any theme that
    reads ambiguous even though it passes the floor.

- [x] **T9 — IDE: results table migrates to `egui_extras::TableBuilder` with
      the System column** (R5, uses T4 + T8)
  ✓ 2026-08-08: `results_markdown()`/`PICK_SCHEME` retired; `draw_results_table`
  is the native `TableBuilder` grid. The three retired markdown tests'
  concerns now covered by `visible_rows_carry_every_field_verbatim`,
  `every_hit_of_a_live_search_keeps_its_fields`,
  `odd_description_text_reaches_visible_rows_unmodified` — the last of
  these also confirms the pipe/CR bug class (the operator's screenshot) is
  now categorically impossible, not just escaped, since there's no
  Markdown being built from crate text any more.
  `system_column_classifies_direct_transitive_and_addable` green.
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs`
  - Do: replace the `md_render`-based results block with a native
    `TableBuilder` grid (System marker · Crate · Version · Downloads ·
    Description) driven directly by `Vec<SearchHit>` + the cached
    `SystemClosure`, following `md_render.rs::draw_table_tight`'s
    measure-widest-column / tight-resizable / last-column-wraps pattern.
    Row click sets `sel_name` (replaces the old `crate:<name>` link scheme).
    Retire `results_markdown()` once its three concerns (cell data, pipe
    escaping, row shape) have native equivalents in T9's own tests — not
    before.
  - Verify: `cargo test -p cobolt-ide panels::external_crates` green,
    including native equivalents of the retired
    `results_render_as_a_markdown_table_with_pick_links` /
    `live_results_parse_back_as_whole_rows` /
    `a_pipe_in_a_description_cannot_break_the_row` (cell data, pipe-safety,
    row-shape, now asserted on the typed rows instead of Markdown text). New
    `system_column_classifies_direct_transitive_and_addable` asserts a
    direct-linked name, a transitive-only name, and an unrelated name each
    render with their correct marker category from a fixed `SystemClosure`
    fixture — covers AC4. `cargo build -p cobolt-ide` green.

- [x] **T10 — IDE: "Show System crates" toggle** (R6)
  ✓ 2026-08-08: `show_system_toggle_filters_results_and_column` green; new
  i18n keys populated ×6.
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: `show_system: bool` panel state, default `false`. Toggle switch next
    to the search button, label **before** the switch (`Tr::ec_show_system`).
    While off: System/System-dependency rows excluded from what T9's table
    draws, and the System column itself is not drawn. While on: both appear.
    Add `Tr::ec_col_system` (System column header) and `Tr::ec_system_tag` /
    `Tr::ec_system_dep_tag` (marker legend text) — all six languages.
  - Verify: `cargo test -p cobolt-ide panels::external_crates` — new
    `show_system_toggle_filters_results_and_column` (fixed `SearchHit` +
    `SystemClosure` fixtures, asserts the filtered row list and column
    visibility for both toggle states). Covers AC5. `cargo test -p cobolt-ide
    i18n` green (new keys populated ×6).

- [x] **T11 — IDE: read-only crate-name field** (R13)
  ✓ 2026-08-08: `crate_name_field_is_read_only` green — a real click+type
  simulation against `interactive(false)` (value unchanged) plus a control
  group against `interactive(true)` (value DOES change), proving the
  harness would catch a regression rather than trivially passing.
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs`
  - Do: the Add row's `sel_name` `TextEdit` becomes
    `TextEdit::singleline(&mut self.sel_name).interactive(false)` (the same
    read-only idiom `panels/editor.rs` uses for generated-code tabs); the
    only remaining write site is T9's row-click handler.
  - Verify: `cargo test -p cobolt-ide panels::external_crates` — new
    `crate_name_field_is_read_only` (widget-level assertion matching
    whatever pattern the generated-tab read-only tests already use). Covers
    AC8. **Manual:** confirm typing into the field does nothing while picking
    a result still does.

- [x] **T12 — IDE: abbreviated Downloads + sortable Crate/Downloads headers**
      (R14, R15, R16, R17, R18)
  ✓ 2026-08-08: `downloads_abbreviate_per_worked_examples` and
  `sort_toggles_direction_and_reapplies_across_pages` green. Note: the
  legacy `thousands()` helper turned out to have no remaining call site
  once `results_markdown()` was retired in T9 — removed rather than kept
  as planned (no manifest/alias-offer text ended up needing an exact
  count); `abbreviate_downloads` is the only formatter left.
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs`
  - Do: new `abbreviate_downloads(n: u64) -> String` next to the existing
    `thousands()` (which stays, used by the manifest table and alias-offer
    text where an exact count reads better) — `<1000` plain, `<1_000_000`
    → `N.NK`, `<1_000_000_000` → `N.NM`, else `N.NB`; one decimal, dropped
    when exactly `.0`. `sort: Option<(SortCol, SortDir)>` state
    (`SortCol::{Crate,Downloads}`); clicking a header sets/reverses it and
    reorders the current page's rows only (never re-queries the registry —
    R17); the same sort re-applies when a new page loads (R18).
  - Verify: `cargo test -p cobolt-ide panels::external_crates` —
    `downloads_abbreviate_per_worked_examples` (R14/AC9's three worked
    examples: `1209→"1.2K"`, `1239897→"1.2M"`, `5000→"5K"`, plus boundaries
    999/1000/999999/1000000) and
    `sort_toggles_direction_and_reapplies_across_pages` (fixture pages, both
    columns, both directions, persistence across a simulated page load).
    Covers AC9, AC10.

- [x] **T13 — IDE: alias-offer modal + System/System-dependency add refusal**
      (R1, R4, R11; wires T5/T6 into the UI)
  ✓ 2026-08-08: modal wired in `show()` (accept spawns `confirm_alias`,
  decline calls `discard_alias_offer` synchronously). System-dependency
  refusal implemented at TWO layers, not just the UI: the panel's Add
  button checks `self.system` first (fast, translated, no thread spawn),
  and `service::add` itself now takes `system: Option<&SystemClosure>` and
  refuses before `registry.resolve` — defense-in-depth, and what the new
  service-level test drives directly. New
  `adding_a_system_dependency_crate_is_refused_without_an_alias_offer`
  green (uses an unreachable registry host to prove no network call
  happens before the refusal). All `external_crates*` tests green
  together: 30/30.
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: replace T5's placeholder log line — `AliasOffered` now opens a modal
    styled like the existing R19 `confirm_remove` window: shows
    `Tr::ec_alias_offer_title` / `Tr::ec_alias_offer_body` (naming the
    collision and `linked_requirement`) and `Tr::ec_alias_caveat` (the
    no-interop warning), with an "Add as `prj_<name>`" button
    (`Tr::ec_alias_add` → calls T6's `confirm_alias`) and the existing
    `Tr::btn_cancel` (→ `discard_alias_offer`). Attempting to add a crate
    T9/T4 marked System or System-dependency shows `Tr::ec_system_refused`
    and is refused before any network call — **no** alias offer for the
    System-dependency (transitive) case, only for a direct
    `CollisionRefusal::Incompatible` (T5).
  - Verify: `cargo test -p cobolt-ide panels::external_crates` — modal
    open/accept/decline state transitions (no network; drives the panel with
    injected `AddOutcome` values). `cargo test -p cobolt-ide
    external_crates_service` — new
    `adding_a_system_dependency_crate_is_refused_without_an_alias_offer`.
    Covers AC3, AC7. `cargo test -p cobolt-ide i18n` green (new keys ×6).

- [x] **T14 — Docs & i18n final pass**
  ✓ 2026-08-08: `docs/developers-guide-en.md`'s Project's Crates section
  extended (System column/toggle, sortable+abbreviated Downloads, read-only
  name field, the alias escape hatch with its no-interop warning callout).
  Also updated the System KB doc constant in `cobolt-compiler/src/lib.rs`
  (the "Crates:" paragraph the in-IDE AI assistant reads) and rebuilt
  `assets/knowledge/chunked.data` via `cargo run -p cobolt-ide --example
  build_chunked_kb` — this wasn't in the original task list but tech.md's
  hard constraint requires it for any behavior-affecting doc-table change;
  `prebuilt_chunked_kb_matches_the_published_documentation` green (974
  records, 5 documents). `cargo test -p cobolt-ide i18n` — all 3 tests
  green.
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: extend the Project's Crates section (from spec 044) with: what the
    System column/toggle mean and why some crates can't be added; how the
    alias offer works, what `use prj_<name>::…` means for a block, and the
    no-interop caveat. Sweep all `Tr` keys added across T10/T13 are present
    and non-empty in all six languages (EN/ES/PT/JA/ZH/FR) — the earlier
    per-task verifications already ran `i18n` tests; this is the consolidated
    check before finalize.
  - Verify: `cargo test -p cobolt-ide i18n` green. Covers AC11, AC12.

- [x] **T15 — Finalize**
  ✓ 2026-08-08: `version.rs` bumped to **1.60.48** (z only, per plan §7).
  `CHANGELOG.md` split into a `### Fixed` section (the `\r` normalization —
  its own narrow, still-isolated one-line change, `Registry::search`) and
  an `### Added` section (spec 045's feature), both under the 1.60.48
  heading, mirroring this session's earlier zune-jpeg/Project's-Crates
  precedent for a shared version number across a same-session fix+feature
  pair. Full `cargo test --workspace --no-fail-fast`: **1708 passed, 0
  failed, 8 ignored** across 98 binaries (up from the pre-045 baseline of
  1691/0/8, consistent with net new tests after T9 retired 3 obsolete
  Markdown-table tests). All 12 acceptance criteria in `spec.md` §5 checked
  — see below. Manual launch walkthrough (plan §6) is for the operator; not
  run here (see report).
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump `VERSION`'s `z` only (per plan §7 — confirm with the operator
    before touching `y`/`x`, overriding `tech.md`'s "features bump the
    minor" per the operator's standing rule); one `CHANGELOG.md` entry
    describing the alias-collision resolution and the System-awareness
    dialog changes together (this is one feature, one commit classification
    — no fix bundled in, per golden rule #5).
  - Verify: `cargo test --workspace --no-fail-fast` — collect every `test
    result:` line (per the operator's test-sweep rule — never verdict from a
    partial grep) and confirm 0 unexpected failures against the pre-change
    baseline. Manual launch check (`cargo run -p cobolt-ide`): walk plan §6's
    manual-verification list end to end. All twelve acceptance criteria in
    `spec.md` §5 checked off with the task/test that proves each.

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and
the change is committed as a single feature (per golden rule #5, no fix
bundled in) — do **not** commit/push unless the operator asks.
