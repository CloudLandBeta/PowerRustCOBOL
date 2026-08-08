<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Project's Crates: System Awareness & Collision Aliasing

- **Status:** draft → approved → **implemented** (1.60.48, 2026-08-08)
- **Folder:** specs/045-crates-system-awareness/
- **Author:** Emerson Lopes (requirements) · drafted with Claude Sonnet 5   **Date:** 2026-08-08

> **Builds on spec 044.** This closes the open design question left in
> [`specs/044-external-crates/HANDOFF.md`](../044-external-crates/HANDOFF.md)
> §5 (collision aliasing) and adds four related improvements to the
> **Project's Crates** search dialog that surfaced while using it: knowing at
> a glance which results are already part of the app, hiding them on demand,
> a safer name field, and a more readable/sortable results table. Internal
> identifiers keep the `external_crates`/`ExternalCrates` naming from 044
> (the brand stays "Project's Crates" everywhere user-facing) — see 044 §
> "Naming" for why.

## 1. Overview

Spec 044 shipped Project's Crates (Beta): search a registry, pick a crate,
pin+vendor it, build it into the binary. Two rough edges came out of using
it:

1. **Collisions with the platform's own crates are always a hard refusal**
   today (`name_collision`/`CollisionRefusal`, R12) — even when the developer
   genuinely wants a different, incompatible version of something like `egui`
   for a block's own use. The handoff investigated a `package =` rename alias
   as a narrow escape hatch and recommended it but left it unimplemented.
2. **The search dialog can't tell the developer what's already there.** A
   result for `egui` or `serde` looks exactly like a result for `csv` — the
   developer only learns a name collides after clicking Add and reading a
   refusal. There is also no way to filter those out, the crate-name field can
   be hand-typed (bypassing the pick-a-result flow the dialog is built
   around), download counts are hard to scan at a glance, and the results
   table has no sort.

This spec resolves the alias question narrowly (only where a refusal happens
today) and adds "System" awareness to the dialog: a column, a visibility
toggle, transitive-dependency detection, a read-only name field, abbreviated
download counts, and column sorting.

## 2. Goals / Non-goals

- **Goals:**
  - Turn today's hard refusal for a **name-colliding, version-incompatible**
    direct platform crate into an offered alias, without changing any other
    refusal path.
  - Let the developer see, before clicking Add, whether a result is already
    part of the running application (directly linked, or a dependency of
    something directly linked) — and hide those results on request.
  - Make the crate-name field foolproof: only a picked search result can set
    it.
  - Make the results table easier to scan: abbreviated download counts,
    sortable by crate name or downloads.

- **Non-goals:**
  - Aliasing/isolation does **not** become the general default for adding
    crates — plain names + `[patch.crates-io]` unification (044's existing
    behavior) stays the path for every add that isn't today's narrow refusal
    case.
  - No aliasing for **transitive** System-dependency collisions — those stay
    a hard, unconditional refusal (see R11). Only a *direct* platform-linked
    name at an incompatible version gets the alias offer (R1).
  - Not re-litigating any other part of 044 (pinning, updates, removal,
    manifest, R11–R15 conflict machinery beyond what R1 changes).
  - Not sorting/filtering across the full registry result set — sort and the
    System toggle act on what's already fetched (see R14–R18).

## 3. User stories

- As a developer, when the crate I want to add collides by name with a
  platform crate at an incompatible version, I want the dialog to offer me an
  aliased copy instead of just refusing, so I can still use it from a block.
- As a developer browsing search results, I want to see which crates are
  already part of my app (directly or as a dependency of something the
  platform links) so I don't waste a click finding out the hard way.
- As a developer who only wants to see what I can actually add, I want to
  hide System crates from the results.
- As a developer, I don't want to be able to typo a crate name into the Add
  row — I want it settable only by picking a result.
- As a developer scanning a page of 50 results, I want download counts I can
  read in one glance and the ability to sort by name or popularity.

## 4. Requirements (EARS)

### Collision aliasing (extends 044 R12)

- **R1 (event):** When a candidate's name collides with a crate the platform
  links **directly** (044's `DIRECT_LINKED` table) **at an incompatible
  version** — today's `CollisionRefusal::Incompatible` — the system shall, in
  place of a hard refusal, offer to add the candidate as a **renamed alias**:
  `prj_<name> = { package = "<name>", version = "…" }`, pinned and vendored
  exactly like any other registered crate.
- **R2 (constraint):** The other two 044 R12 refusal paths are unchanged:
  a **compatible** direct collision stays refused as *already available*
  (informational), and a `cobolt_*`-reserved name stays refused.
- **R3 (event):** A block using an aliased crate shall `use prj_<name>::…`
  (the alias's lib name), not the bare/original name; the semantic allowlist
  and generated manifest shall accept it under that name.
- **R4 (constraint):** When the dialog offers an alias, it shall show a plain
  caveat next to the offer: values from the aliased copy do not interoperate
  with the platform's own copy of the same crate (proven in the handoff's
  experiment — e.g. `expected egui::Color32, found a different egui::Color32`
  across the alias boundary). This is a warning, not a blocker — the
  developer decides.

### System awareness in the search dialog

- **R5 (ubiquitous):** The search results table shall show a **System**
  column classifying each result as one of: not part of the app (addable),
  directly linked by the platform (**System**), or a dependency of a crate
  the platform directly links (**System dependency**).
- **R6 (state):** A toggle switch, labeled **"Show System crates"**, sits next
  to the search button and defaults to **off**. While off, System and System
  dependency results are excluded from the list and the System column is
  hidden. While on, they are included and the column is shown.
- **R7 (event):** A **System** result (direct platform link) renders with a
  dimmed **yellow** marker.
- **R8 (event):** A **System dependency** result (transitive only, not itself
  directly linked) renders with a dimmed **gray** marker, visually distinct
  from R7.
- **R9 (event):** An addable (non-system) result renders with a dimmed
  **green** marker when the column is visible.
- **R10 (constraint):** In every one of the IDE's 16 themes, each of the three
  markers (R7–R9) shall meet at least WCAG AA's graphical-object contrast
  minimum (3.0:1) against the row's background — dark and light themes alike.
  Reuse/extend the existing WCAG contrast-ratio check
  (`crates/cobolt-ide/src/flags.rs`, `every_theme_paints_flags_with_high_contrast`)
  rather than inventing a second one.
- **R11 (event):** Attempting to add a **System** or **System dependency**
  crate is refused. The **System dependency** refusal is new (044's R12 only
  checked direct names); it is **unconditional** — no alias is offered for it
  (R1's alias path applies only to the direct, incompatible-version case).
- **R12 (ubiquitous):** The System/System-dependency classification is
  computed from the platform's own resolved dependency graph — the same
  resolver-probe machinery spec 044 built (`cargo metadata` against the base
  dependency block) — and is kept in sync with the generated manifest the
  same way `DIRECT_LINKED` is guarded today
  (`the_direct_linked_table_matches_the_generated_manifest`).

### Name field & results table

- **R13 (event):** The crate-name field in the Add row becomes **read-only**;
  its value is settable only by picking a crate from the search results
  (the existing click-a-result-row flow from 044 R6), never by direct typing.
- **R14 (ubiquitous):** The Downloads column shall render an **abbreviated**
  count: below 1,000 shown as the plain integer; from 1,000 shown as
  `N.NK`; from 1,000,000 as `N.NM`; from 1,000,000,000 as `N.NB` — one
  decimal place, dropped when it is exactly `.0` (e.g. `1209` → `1.2K`,
  `1239897` → `1.2M`, `5000` → `5K`).
- **R15 (event):** Clicking the **Crate** column header sorts the currently
  displayed page's rows alphabetically by crate name; clicking it again
  reverses the direction.
- **R16 (event):** Clicking the **Downloads** column header sorts the
  currently displayed page's rows numerically by (true, unabbreviated)
  download count; clicking it again reverses the direction.
- **R17 (constraint):** Sorting (R15–R16) reorders only the rows already on
  the current page — it never triggers a new registry query and never
  changes which crates are on which page.
- **R18 (state):** While a sort column/direction is active, loading a
  different page (◀/▶, or a new search) applies the same sort to the newly
  loaded rows.

## 5. Acceptance criteria

- [x] AC1 — Adding a candidate whose name matches a direct platform crate at
      an incompatible version offers the `prj_<name>` alias instead of
      refusing; accepting it pins and vendors the alias, and a block can
      `use prj_<name>::…` and build successfully. Proved end to end by
      `external_crates_alias_build_and_run` (real `egui` 0.29.0 → `prj_egui`,
      built alongside the platform's own linked 0.36, run, output verified).
- [x] AC2 — A compatible direct collision and a `cobolt_*` name are still
      refused exactly as in 044 (no alias offered). `compatible_and_reserved_collisions_still_refuse`.
- [x] AC3 — The alias offer displays the no-interop caveat text (R4). Wired
      in the modal (`Tr::ec_alias_caveat`); i18n presence proven by
      `cargo test i18n`.
- [x] AC4 — Search results include a System column; a known direct platform
      crate (e.g. `egui`) is marked **System**; a crate that is only a
      dependency of a platform crate is marked **System dependency**; an
      unrelated crate (e.g. `csv`) is marked addable.
      `system_column_classifies_direct_transitive_and_addable` (fixture) +
      `system_closure_splits_direct_from_transitive` (live: 15 direct, 557
      transitive).
- [x] AC5 — With "Show System crates" off (the default), System and System
      dependency rows are absent from the results and the System column is
      not drawn. Toggling it on brings them back with the column.
      `show_system_toggle_filters_results_and_column`.
- [x] AC6 — The three markers pass the WCAG AA graphical-object contrast
      check (≥3.0:1) in all 16 themes.
      `every_theme_marks_system_crates_with_sufficient_contrast` (worst case
      3.00:1, light-plus/addable — flagged for a manual eyeball, not assumed
      pretty).
- [x] AC7 — Attempting to add a System or System-dependency crate is refused;
      the System-dependency refusal never offers an alias.
      `adding_a_system_dependency_crate_is_refused_without_an_alias_offer`
      (service layer, proven with an unreachable registry host to confirm no
      network call happens first).
- [x] AC8 — The crate-name field cannot be typed into; it only changes when a
      result row is clicked. `crate_name_field_is_read_only` (real click+type
      simulation, plus an interactive-control-group check that the harness
      would catch a regression).
- [x] AC9 — Downloads render abbreviated per the R14 table, with at least the
      three worked examples verified exactly.
      `downloads_abbreviate_per_worked_examples`.
- [x] AC10 — Clicking "Crate" sorts the current page alphabetically (and
      reverses on a second click); clicking "Downloads" sorts numerically by
      the true count (and reverses on a second click); changing pages while a
      sort is active re-applies it.
      `sort_toggles_direction_and_reapplies_across_pages`.
- [x] AC11 — All new dialog strings exist as `Tr` fields in all six languages.
      `cargo test -p cobolt-ide i18n` — 3/3 green, including
      `non_english_is_actually_translated`.
- [x] AC12 — `docs/developers-guide-en.md`'s Project's Crates section is
      updated to describe the alias offer, the System column/toggle, and the
      read-only name field. Also updated the System KB doc constant
      (`cobolt-compiler/src/lib.rs`) and rebuilt `assets/knowledge/chunked.data`
      per `tech.md`'s hard constraint (not originally itemized in tasks.md —
      added during T14 since it applies here).

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — new strings: "Show System crates" toggle
  label, System column header, the two marker tooltips/legends (System /
  System dependency), the alias-offer prompt and its no-interop caveat, and
  any new refusal message text for System-dependency. All go in `i18n.rs` as
  `Tr` fields, all six languages, per `tech.md`.
- **Generated-code / regenerate contract:** No change to what's generated for
  existing (non-aliased) crates. A newly aliased crate adds a `package =`
  rename line to the generated `Cargo.toml`'s `[dependencies]` — additive,
  same mechanism `pin_sections` already writes for plain pins.
- **Docs (English guide):** Yes — required, per AC12 and the operator's
  standing rule (`CLAUDE.md` #3) to keep the Developer's Guide current in the
  same change.
- **System KB:** If the alias/System-awareness change is documented in the
  compiler's property/method/event doc tables (it likely isn't — this is
  dialog/build behavior, not a bindable control/property), no KB change is
  needed; otherwise the chunked KB must be rebuilt in the same change per
  `tech.md`.
- **Fix vs feature classification:** **Feature.** This adds capability beyond
  today's shipped Project's Crates behavior (aliasing, System awareness,
  sorting) — none of it is missing/non-conformant COBOL-85 behavior. Per
  `tech.md` this would bump the **minor** version; per the operator's
  standing memory rule ("never bump the minor without permission"), the
  implementation phase should bump only `z` and ask before touching `y`/`x` —
  flagging this now so `/plan`/`/implement` don't have to re-derive it.
- **Commit/announce:** New feature → its own commit(s), separate from any
  incidental fix found along the way (golden rule #5); announced on forum
  f=96 after merge to `main`, with explicit sign-off on the post text (golden
  rule #4b) — not before.

## 7. Open questions

- Q: Exact implementation of the results table (per-row color markers,
  sortable headers) likely can't stay the current rendered-Markdown table
  (`md_render`) — that pipeline has no notion of per-cell color or clickable
  headers. `/plan` should decide whether to move the results grid to a native
  `egui_extras::TableBuilder` (or extend `md_render`) — left open
  deliberately since it's a design/implementation call, not a requirement.
- Q: Should removing an aliased crate, or its entry in `rust_manifest.md`,
  say anything different from a normal pin (e.g. noting the alias/rename)?
  Leaning yes for manifest clarity, but not required by any acceptance
  criterion above — `/plan` can decide during design.
