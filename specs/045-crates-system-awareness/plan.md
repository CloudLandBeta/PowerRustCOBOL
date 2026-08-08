<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Project's Crates: System Awareness & Collision Aliasing

- **Status:** draft → **approved → implemented** (1.60.48, 2026-08-08)
- **Spec:** ./spec.md   **Date:** 2026-08-08

## 1. Approach

Two mostly-independent pieces of work, sharing one insight: everything needed
already exists in spec 044's resolver-probe machinery — nothing here talks to
`cargo metadata` in a new way, it reuses `resolve_graph`/`probe_manifest` with
different (empty) inputs.

**A. Collision aliasing (R1–R4).** Today `check_conflicts` turns any
`name_collision` hit into a hard `Err`. This plan splits that: `Incompatible`
becomes a **reactive two-step offer** instead of an immediate refusal —
`add()` already resolves and downloads *before* checking collisions, so by the
time the collision is known the crate is already vendored and disk-cheap to
keep around while the developer decides. `ExternalCrate` gains an
`alias: Option<String>` field; when set, `lib_name()` returns the alias's lib
name (so R3's `use prj_egui::…` flows through the existing semantic-allowlist
and live-analysis wiring with **zero** changes there — both call sites already
just do `c.lib_name()`). The generated manifest's alias pins become plain
`path` dependencies with `package = "<name>"` — not a `[patch.crates-io]`
entry — because patch entries key on the crates.io source name and would
force the platform's own compatible copy and the incompatible alias onto one
resolved version, defeating the point (this is exactly the boundary the
handoff's experiment found: aliasing only works when cargo is left to resolve
two genuinely incompatible versions as two separate packages, which is its
ordinary behavior for an *unpatched* path dependency).

**B. System awareness + table UX (R5–R18).** The "is this crate part of the
app" question is answered once per IDE session by running the *existing*
`resolve_graph` with an **empty pin list and no candidate** — that's exactly
the platform's own base dependency closure, computed the same way 044's
resolver probe computes everything else, just with nothing added. The result
(a set of package names) splits into System (`DIRECT_LINKED`, already a
static table) and System dependency (everything else in the closure). This is
cached in `ExternalCratesPanel` for the life of the IDE session — no need to
recompute per search or per dialog reopen.

The results table currently renders through `panels/md_render.rs`'s
Markdown-table pipeline (`TableLayout::TightResizable`), which has no notion
of per-cell color or a clickable, stateful header — it exists to render
*prose* tables, and stretching it to carry row metadata would out-scope this
change. `md_render`'s tight-table layout already wraps `egui_extras::
TableBuilder` internally (`draw_table_tight`, `md_render.rs:706`), so the
concrete move is: **the crate-search results table stops going through
`md_render` and becomes its own `egui_extras::TableBuilder` in
`panels/external_crates.rs`**, built directly against `Vec<SearchHit>` with
per-row System classification, following the exact same
measure-widest-column / tight-resizable-with-wrapping-last-column pattern
`draw_table_tight` already established (same crate, same widget, same look —
just driven by typed data instead of parsed Markdown). This one table is the
only thing that moves; every other `md_render` consumer (docs, event help,
etc.) is untouched.

Marker colors (R7–R10) are derived per-theme rather than hard-coded once:
fixed target hues (yellow/gray/green) at a "dimmed" saturation, with the
*lightness* pushed toward or away from the theme's actual panel luminance
until the pair clears the WCAG AA graphical-object minimum (3.0:1) — the same
check `flags.rs` already runs for its two-tone flags
(`every_theme_paints_flags_with_high_contrast`), extended rather than
duplicated: `relative_luminance`/`contrast_ratio` move out of `flags.rs` into
a small shared `contrast.rs` (or a `pub(crate)` home in `theme.rs`) both
modules import.

## 2. Affected crates / files

- `crates/cobolt-compiler/src/external_crates.rs`
  - `ExternalCrate.alias: Option<String>` (serde default, `skip_serializing_if`
    so non-aliased pins serialize byte-identical to today).
  - `ExternalCrate::lib_name()` — honor `alias` when set (R3).
  - `pin_sections()` — an aliased pin emits `prj_<name> = { package = "<name>",
    path = "<vendored>" }` in `[dependencies]`; no `[patch.crates-io]` entry
    for it (R1). Shared by `generate_cargo_toml` (real builds) and
    `probe_manifest` (the resolver probe), so both stay in lockstep as today.
  - `render_rust_manifest()` — an aliased row's Crate cell reads
    `` egui (as `prj_egui`) `` instead of a bare name; non-aliased rows
    unchanged byte-for-byte (resolves spec's open question 2).
  - New: `system_closure_manifest()` or reuse `probe_manifest(crates_path,
    project_dir, &[], None)` directly (it already produces exactly this) —
    likely no new function needed here at all, just a new call site in the
    IDE crate.
- `crates/cobolt-ide/src/external_crates_service.rs`
  - `check_conflicts()` — split `CollisionRefusal::Incompatible` out of the
    immediate-`Err` path; `AlreadyAvailable`/`Reserved` unchanged (R2).
  - `add()` — return type becomes an outcome enum (`Added(String)` |
    `AliasOffered { .. }`) instead of bare `Result<String, String>`; on offer,
    the vendored download is kept (not deleted) pending the developer's
    choice.
  - New: `confirm_alias(project_path, candidate, alias)` — runs the layer-2
    probe against the *alias-shaped* candidate (via the now alias-aware
    `pin_sections`/`probe_manifest`), then saves the pin with `alias` set.
  - New: `discard_alias_offer(vendored_dir)` — cleans up the kept-around
    download when the developer declines (mirrors the existing cleanup
    already used elsewhere in this file for other refusal paths).
  - New: `system_closure(workspace_root, scratch_project_dir) -> Result<SystemClosure,
    String>` — calls the existing `resolve_graph` with `pins = &[]`,
    `candidate = None`; splits the returned names into `direct` (checked
    against `external_crates::direct_linked_lib_names()`) and `transitive`
    (everything else in the graph). `SystemClosure { direct: BTreeSet<String>,
    transitive: BTreeSet<String> }`.
- `crates/cobolt-ide/src/panels/external_crates.rs`
  - Results table: replace the `md_render`-based block with a native
    `egui_extras::TableBuilder` grid (System marker · Crate · Version ·
    Downloads · Description), row click (not a link) sets `sel_name` and is
    the *only* way it changes (R13 — the field itself becomes
    `TextEdit::interactive(false)`, the same "read-only" idiom
    `panels/editor.rs` already uses for generated-code tabs).
  - New state: `show_system: bool` (default `false`, R6), `sort: Option<(SortCol,
    SortDir)>` where `SortCol` is `Crate | Downloads`, `system: Option<SystemClosure>`
    (lazily computed, cached for the panel's lifetime).
  - "Show System crates" label + toggle switch next to the search button.
  - Alias-offer modal, styled like the existing R19 `confirm_remove` window
    (own `egui::Window`, centered, non-collapsible): shows R4's caveat, an
    "Add as `prj_<name>`" button and a `btn_cancel` button.
  - Downloads abbreviation (`1.2K`/`1.2M`/`1.2B`) as a small pure function
    next to the existing `thousands()` helper (which stays, for the manifest
    table and the alias-offer text where an exact count still reads better).
- `crates/cobolt-ide/src/flags.rs` — `relative_luminance`/`contrast_ratio`
  become `pub(crate)` (or move to a new `contrast.rs`) so
  `panels/external_crates.rs` can reuse them instead of re-deriving WCAG math.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields (all six languages):
  `ec_col_system`, `ec_show_system`, `ec_system_tag`, `ec_system_dep_tag`,
  `ec_system_refused`, `ec_alias_offer_title`, `ec_alias_offer_body`,
  `ec_alias_caveat`, `ec_alias_add`. (`btn_cancel` already exists — reused for
  the offer's decline action, no new key.)
- `docs/developers-guide-en.md` — extend the Project's Crates section (added
  by spec 044) with: what the System column/toggle mean, why some names can
  only be added as an alias and what that changes about the block's `use`
  line, and the caveat about aliased values not interoperating with the
  platform's own copy.
- `CHANGELOG.md` / `crates/cobolt-ide/src/version.rs` — new entry, `z` bump
  only (see §7 — the operator's standing rule overrides `tech.md`'s "features
  bump the minor" here; confirm before any `y`/`x` change).

## 3. Data / model changes

- **`cobolt.toml` `[[crates]]` schema** gains one optional field:
  ```toml
  [[crates]]
  name    = "egui"
  version = "0.29.0"
  alias   = "prj_egui"     # absent for every pin added before this feature
  ```
  `#[serde(default, skip_serializing_if = "Option::is_none")]` — old projects
  parse unchanged, and a project that never hits the alias path serializes
  identically to today (the existing `pins_round_trip_and_default` test
  extends to cover this, doesn't change shape).
- **Generated `Cargo.toml`** — one new shape for an aliased pin's
  `[dependencies]` line (`package =` + `path =`, no version requirement,
  since the vendored source pins it implicitly); every other pin's line is
  unchanged. No new `[patch.crates-io]` line for an aliased pin (see §1).
- **`rust_manifest.md`** — the Crate cell grows an optional
  `` (as `prj_<name>`) `` suffix; column count/headers unchanged.
- No `.cfrm` impact — this feature is build/dialog only.

## 4. Key decisions & alternatives

- **Decision:** the alias offer is *reactive* — surfaced only after `add()`
  actually hits an `Incompatible` collision — rather than a proactive
  checkbox the developer ticks before knowing whether they'll need it.
  **Why:** matches R1's wording ("when the candidate's name collides... the
  system shall offer") and the ordinary case (compatible version, no alias
  needed) never shows the extra control. **Rejected:** a pre-emptive "Add as
  alias" checkbox shown whenever the picked name is in `DIRECT_LINKED` —
  simpler to implement (no outcome-enum plumbing through `add()`) but forces
  the developer to predict a collision before trying, and shows UI clutter
  for the common case where the version turns out compatible anyway.
- **Decision:** an aliased pin is a bare `path` dependency with `package =`,
  never routed through `[patch.crates-io]`. **Why:** proven necessary by the
  handoff's own experiment — a patch entry replaces *every* consumer of that
  crates.io source name, so patching in the alias's incompatible version
  would also silently rewrite the platform's own (compatible) dependency edge
  onto it, unifying exactly what the alias exists to keep separate. **Rejected:**
  reusing the normal pin path (`version = "=X"` + patch) for aliases too —
  doesn't compile; this is the mechanism, not a style choice.
- **Decision:** System-dependency collisions (R11) are an unconditional
  refusal with no alias offered, even though they're discovered the same way
  as a direct collision. **Why:** the operator's instruction was explicit
  ("cannot be added either to prevent conflicts") and a transitive dependency
  isn't something a block would `use` directly anyway — aliasing it wouldn't
  serve the workflow the alias exists for. **Rejected:** extending R1's offer
  to transitive collisions too, for symmetry — adds alias-offer plumbing to a
  path that has no real use case and no requirement asking for it.
- **Decision:** compute the System closure by calling the *existing*
  `resolve_graph` with empty pins/no candidate, cached once per `ExternalCratesPanel`
  instance (i.e., once per IDE session — the panel is a persistent `app.rs`
  field, not recreated per dialog open). **Why:** this is precisely 044's
  resolver-probe machinery already proven to mirror a real build; no new
  `cargo metadata` call shape, and computing it once avoids repeating a
  multi-hundred-package resolve on every keystroke/search. **Rejected:**
  computing it fresh per search (wasteful, adds latency to every query) or
  hard-coding the transitive set as a static list next to `DIRECT_LINKED`
  (would silently drift from the real dependency tree the moment `egui`/
  `eframe` bump a transitive dependency — exactly the staleness 044 avoided
  for `DIRECT_LINKED` by pairing it with a parity test).
- **Decision:** move only the crate-search results table off `md_render` onto
  a direct `egui_extras::TableBuilder`. **Why:** per-cell color, click-to-pick
  rows, and sortable headers are outside what a Markdown-table renderer
  should grow to support; the underlying widget is already
  `egui_extras::TableBuilder` one layer down, so this isn't a new dependency
  or a new visual language, just cutting out the Markdown layer for this one
  data-driven table. **Rejected:** extending `md_render` with a bespoke
  colored-cell / sortable-header Markdown syntax — would leak
  external-crates-dialog-specific concepts into a shared prose renderer used
  by docs and event help.
- **Decision:** marker colors are computed per-theme (fixed hue, lightness
  solved for contrast) rather than 16 hand-picked triples. **Why:** matches
  how `flags.rs` already guards contrast — a derived-and-tested approach
  catches a *future* 17th theme automatically, where a hand-authored table
  would silently need updating. **Rejected:** a static 16×3 lookup table —
  more predictable to eyeball once, but exactly the kind of table 044/the
  flags module already learned drifts out of sync.

## 5. Risks & mitigations

- **Risk:** the System-closure `cargo metadata` resolve (base dependency
  block, no pins) touches ~800+ transitive packages (observed in the 044 e2e
  build log) — first computation could add a few seconds the first time the
  dialog is used in a session. **Mitigation:** compute lazily (first dialog
  open or first search, not app startup), run it on the existing worker
  thread with the existing `busy`/spinner UI, and cache for the rest of the
  session; a stale cache across a workspace-root change is not a concern in
  practice (the workspace root doesn't change within one running IDE
  process).
- **Risk:** the alias flow keeps a *downloaded* crate on disk while the
  developer decides — if they close the dialog (or the IDE) without choosing,
  the vendored directory is orphaned. **Mitigation:** `foreign_crates_dir`'s
  existing check (refuses an add when `crates/` holds something not in
  `cobolt.toml`) already treats an unregistered vendor directory as a
  problem to surface, not silently ignore — an orphaned alias-offer download
  surfaces the same way an interrupted 044 add already could; no new failure
  mode, same existing guard.
- **Risk:** deriving marker colors at runtime (vs. hard-coded) could produce
  a contrast-passing but visually *ugly* or ambiguous color on some theme.
  **Mitigation:** the WCAG check is a floor, not the whole design — the
  implementation phase should eyeball all 16 themes (the same manual pass
  `every_theme_paints_flags_with_high_contrast`'s author presumably did) before
  calling this done; flag any theme that looks off for a targeted override
  rather than blocking on a perfect formula.
- **Risk:** replacing the results table's rendering path could regress
  existing 044 table tests (`results_render_as_a_markdown_table_with_pick_links`,
  `live_results_parse_back_as_whole_rows`, `a_pipe_in_a_description_cannot_break_the_row`)
  since they assert against `results_markdown()`'s Markdown output, which this
  plan removes as the *rendering* path. **Mitigation:** those tests currently
  verify: (a) correct data in each cell, (b) pipe-escaping, (c) parse-back
  shape — all three concerns still need coverage under the new TableBuilder
  path, just asserted against the typed row data instead of Markdown text;
  `results_markdown()` can be deleted once its assertions have native
  equivalents, not before.

## 6. Test strategy

- **`cobolt-compiler/src/external_crates.rs`** (unit):
  - `alias_pin_emits_package_path_not_version_or_patch` — an aliased
    `ExternalCrate` in `pin_sections` produces a `package = "…", path = "…"`
    dependency line and **no** corresponding `[patch.crates-io]` entry; a
    non-aliased pin in the same call is unaffected (regression guard for the
    shared function).
  - `lib_name_honors_alias` — `ExternalCrate { name: "egui", alias:
    Some("prj_egui".into()), .. }.lib_name() == "prj_egui"`.
  - `rust_manifest_notes_the_alias` — an aliased pin's row reads
    `` egui (as `prj_egui`) ``; a non-aliased row is byte-identical to today
    (extends `rust_manifest_lists_name_version_url`).
  - `pins_round_trip_and_default` (existing) extended with one alias-bearing
    pin in the fixture TOML, so the new field's round-trip is proven, not
    assumed.
- **`cobolt-ide/src/external_crates_service.rs`** (unit + live, following the
  existing `flow_tests`/mock-registry pattern for the former, the existing
  live-crates.io pattern for the latter):
  - `incompatible_direct_collision_offers_alias_not_error` — `add()` against
    a mock/injected candidate whose version collides incompatibly with a
    `DIRECT_LINKED` entry returns `AddOutcome::AliasOffered`, not `Err`; the
    vendored directory still exists afterward.
  - `compatible_and_reserved_collisions_still_refuse` — R2 regression: the
    other two `CollisionRefusal` variants still produce `Err` exactly as in
    044.
  - `declining_the_alias_offer_removes_the_vendored_download` —
    `discard_alias_offer` leaves no trace under `crates/`.
  - `confirming_the_alias_offer_pins_with_alias_and_probes_the_alias_shape` —
    `confirm_alias` saves a pin with `alias` set and runs the layer-2 probe
    against the alias-shaped candidate (asserted via the probe manifest text,
    same style as `probe_manifest_stages_the_real_dependency_set`).
  - `system_closure_splits_direct_from_transitive` — **live** (real
    `cargo metadata` against the real workspace, same tier as
    `every_row_of_a_live_search_keeps_its_four_cells`): a known
    `DIRECT_LINKED` name (`egui`) lands in `direct`; a name known only to be
    pulled in transitively by the GUI stack (e.g. one of the `windows-*`/
    `zbus` family observed in the 044 e2e log on this platform, or a
    platform-neutral pick if one exists) lands in `transitive`; an unrelated
    name (`csv`) lands in neither.
- **`cobolt-ide/src/panels/external_crates.rs`** (unit, no network, no
  screenshot — pure state/geometry like the existing `run_interaction_tests`
  style used elsewhere in this crate):
  - Marker contrast: `every_theme_marks_system_crates_with_sufficient_contrast`
    — for all 16 themes × 3 marker categories, `contrast_ratio(marker,
    row_background) >= 3.0`, mirroring `flags.rs`'s existing test structure.
  - `show_system_toggle_filters_results_and_column` — with a fixed
    `Vec<SearchHit>` + a fixed `SystemClosure` fixture, `show_system = false`
    excludes System/System-dependency rows from what gets drawn/considered
    and `show_system = true` includes them (assert on the filtered row list
    the draw call would receive, not pixels).
  - `crate_name_field_is_read_only` — the `TextEdit` widget backing
    `sel_name` reports `interactive(false)`/cannot gain a text-input response
    that changes its value except via the row-click code path (mirrors
    whatever assertion `panels/editor.rs`'s read-only generated-tab tests
    already use for the same idiom).
  - `sort_toggles_direction_and_reapplies_across_pages` — clicking "Crate"
    sorts a fixture page ascending, clicking again reverses; loading a new
    page (a second fixture `Vec<SearchHit>`) through the existing sort state
    applies the same order without a fresh click.
  - `downloads_abbreviate_per_worked_examples` — the three numbers from spec
    R14/AC9 (`1209→"1.2K"`, `1239897→"1.2M"`, `5000→"5K"`) plus a couple of
    boundary cases (999, 1000, 999999, 1000000).
  - Existing `results_render_as_a_markdown_table_with_pick_links` /
    `live_results_parse_back_as_whole_rows` /
    `a_pipe_in_a_description_cannot_break_the_row` — retired once their three
    concerns (cell data correctness, pipe-safety, row-shape) have native
    `TableBuilder`-path equivalents (§5's mitigation).
- **i18n:** the existing repo-wide `i18n_tests` (all `Tr` fields populated in
  all six languages) catches any missed translation automatically — no
  bespoke test needed beyond correctly filling in the six blocks.
- **Manual/visual** (the operator, IDE launched via `cargo run -p cobolt-ide`):
  open Project's Crates, search something broad, confirm the System column
  is hidden by default and appears with the toggle; spot-check that `egui`/
  `eframe` read System (yellow), something only transitively pulled in reads
  System dependency (gray), and an unrelated hit reads addable (green) —
  across at least one dark and one light theme; try adding an old,
  incompatible `egui` version and confirm the alias offer appears with the
  caveat text, and that accepting it lets a block `use prj_egui::…` and
  build; confirm the name field can't be typed into; confirm both column
  headers sort and that paging preserves the sort.

## 7. Steering compliance

- [ ] i18n: `ec_col_system`, `ec_show_system`, `ec_system_tag`,
      `ec_system_dep_tag`, `ec_system_refused`, `ec_alias_offer_title`,
      `ec_alias_offer_body`, `ec_alias_caveat`, `ec_alias_add` — all six
      languages (`btn_cancel` reused, no new key).
- [ ] Generated-code banner + regenerate-on-action contract: unaffected (this
      touches the build's *dependency* manifest, not generated `.cbl`).
- [ ] English dev guide updated (`docs/developers-guide-en.md`'s Project's
      Crates section); `-es/-pt/-jp/-cn` translations untouched, `-fr`
      continues not to exist (user-maintained per standing rule).
- [ ] Fix vs feature: **feature** (capability beyond 044's shipped scope, no
      COBOL-85 conformance involved). `tech.md` says bump the minor; the
      operator's standing memory rule ("never bump the minor without
      permission") overrides — implementation bumps `z` only and asks before
      touching `y`/`x`, per spec §6.
- [ ] No "cobolt" in user-facing text (internal identifiers keep
      `external_crates`/`ExternalCrates` per 044's precedent — see this
      spec's header note); COBOL identifiers/source unaffected (build/dialog
      feature only).
- [ ] Commit discipline: this whole feature is one classification (feature) —
      single commit or a tightly related set, kept separate from any
      incidental fix found along the way (golden rule #5); announced on forum
      f=96 after merge to `main`, exact post text approved first (golden rule
      #4b).
