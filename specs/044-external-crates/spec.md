<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Project's Crates for EXEC RUST

- **Status:** draft → approved → **implemented** (1.60.45, 2026-08-07)
- **Folder:** specs/044-external-crates/
- **Author:** Emerson Lopes (design) · drafted with Claude Fable 5   **Date:** 2026-08-07

> **Naming (operator, 2026-08-07):** the user-facing name of this feature is
> **Project's Crates** — the tree category, the dialog, every message and all
> documentation. "External Crates" survives only in internal Rust identifiers
> (`Category::ExternalCrates`, `external_crates.rs`, this folder's slug),
> exactly as the internal `cobolt-*` crate names sit behind the
> PowerRustCOBOL brand. Requirement text below keeps the original wording;
> read every user-visible occurrence as "Project's Crates".

> **Search results (operator, 2026-08-07):** R6's results are **not** a short
> list. The dialog shows a **rendered markdown table** — crate · version ·
> downloads · description — **50 rows per page** with page controls and the
> registry's total match count, so the developer browses every match until
> they find the one they need; clicking a crate name picks it. The earlier
> `N × "query"` log line was debug output and is gone: the table is the
> feedback.

## 1. Overview

Spec 041 gave `EXEC RUST` real, compiled Rust — but deliberately stopped at the
crates every generated program already links: `std`, `egui`/`eframe`,
`egui_extras`, `cobolt-forms`, `cobolt-runtime`. A block that names any other
crate is refused by `unlinked_crates()`
([`exec_rust.rs:212`](../../crates/cobolt-semantic/src/exec_rust.rs)) with
"arbitrary crates are not yet supported" — 041's R16 called that the deferred
"manifest story". This spec is that story.

The developer registers third-party crates **per project**, and finds them
**without leaving PowerRustCOBOL**: the add dialog searches the registry —
crates.io by default; the endpoint is an IDE setting, so a team mirror plugs in
by changing one field — and presents matching crates to pick from. Each
registered crate appears in the project tree under a new fixed category,
**External Crates**, and its source lives in `crates/` inside the project
folder, so the project owns what it links. A crate is *resolved once, at
inclusion*: the IDE pins the exact version, downloads the source into
`crates/`, and every later build uses exactly that — until the developer
explicitly updates one crate or all of them. During `build_project` the
registered crates are linked into the program's single self-contained binary at
the same optimization level as everything else, so a block simply writes
`use csv::Reader;` and it works. Every successful build also emits
`rust_manifest.md` into the destination folder (`dist/` by default, delivered
alongside the binary in build step 11c) recording the name, URL, and exact
version of every external crate in the binary. A crate that cannot coexist
with what PowerRustCOBOL already links is refused **at add time**, with the
reason — never discovered as a mysterious build failure three weeks later.

## 2. Goals / Non-goals

**Goals**

- Search the configured registry from inside the IDE — type a query, pick from
  the results — then add the crate with an optional version requirement and an
  optional feature list. Finding and adding a crate never requires leaving
  PowerRustCOBOL.
- A pluggable registry: search, resolution, and download all go through one
  endpoint configured in the IDE's settings, defaulting to crates.io.
- Deterministic builds: the version is pinned at inclusion and the source is
  stored project-locally in `crates/`; builds never silently re-resolve.
- Explicit updates — one crate, or all of them — with an old → new report.
- Registered crates usable from any `EXEC RUST` block (statement-level and
  item-level, hand-written sources and form event handlers alike) with no new
  COBOL syntax: a plain `use` line, exactly as `egui` works today.
- Linked into the program's own binary, same profile as the rest of the build
  (release unless the project chose debug compilation); no sidecar artefacts;
  the end user still installs nothing (041 R3 is preserved).
- `rust_manifest.md` in the destination folder: name, URL, exact version of
  every external crate linked into the delivered binary.
- Conflicts with PowerRustCOBOL's own dependency tree are detected when the
  developer adds or updates a crate: coexistence is accepted (with a warning
  where it is subtle), impossibility is refused with the resolver's reason.

**Non-goals**

- **Sources other than the configured registry.** Git dependencies and local
  path crates are deferred. The pluggable endpoint (R4) must be a
  crates.io-compatible registry, and there is **one** active registry at a
  time — federated search across several is out. (The manifest's URL column is
  well-defined because of this: every crate has a page on the registry it came
  from.)
- **Registry authentication.** v1 assumes the registry is anonymously readable
  — true of crates.io and public mirrors. Token-authenticated private
  registries are deferred (Q8).
- **Patching / forking vendored source.** The copy under `crates/` is
  third-party input, read-only; editing it is not supported and an update
  overwrites it. A developer who needs a modified crate is out of scope for v1.
- **Browsing vendored source in the project tree.** v1 shows one node per
  crate (name + pinned version), not the crate's file tree.
- **Fully offline builds.** The registered crate itself never re-downloads,
  but its transitive dependencies follow cargo's normal registry/cache
  behaviour, exactly as the base dependency tree (eframe, …) does today.
- **Per-crate `default-features = false`.** The add dialog accepts *extra*
  features; switching default features off is deferred (see Q2).
- **No new COBOL syntax.** No `IMPORT CRATE` verb, no manifest DIVISION;
  availability is project configuration, not language.

## 3. User stories

- As a COBOL developer, I want to type "csv" in the add dialog and pick from
  the matching crates it shows me, so that I never open a browser to hunt for
  a crate's exact name.
- As a COBOL developer, I want to add `csv` to my project from the tree and
  then `use csv::Reader;` inside a block, so that I can read a CSV without
  writing a parser — and without learning cargo.
- As a team lead, I want to point the IDE at our internal registry mirror, so
  that every crate my team adds comes from a source we control.
- As a COBOL developer, I want my project to build the same way next month as
  it does today, so a crate release on the internet never changes my program
  behind my back.
- As a COBOL developer, I want to press *Update* (on one crate, or on all)
  when *I* decide to take new versions, and see exactly what changed.
- As a COBOL developer, I want to be told **when I add** a crate that it
  cannot live alongside PowerRustCOBOL's own libraries — not by a wall of
  cargo errors at build time.
- As someone shipping an application, I want `rust_manifest.md` next to my
  binary in `dist/`, so I can tell anyone — auditors included — exactly which
  third-party code is inside, at which version, from where.

## 4. Requirements (EARS)

### The External Crates category

- **R1 (ubiquitous):** The project tree shall show a fixed top-level category
  **External Crates**, positioned after *Generated Code* and before *Assets*,
  whose on-disk root is `crates/` inside the project folder. Its label, like
  every category label, is localized in all six languages.
- **R2 (ubiquitous):** The category shall list one node per registered crate,
  showing the crate's name and its pinned exact version (e.g. `csv 1.3.1`).
- **R3 (constraint):** The vendored source under `crates/` shall not be
  editable through the IDE, and the category shall not offer the file-tree
  operations of file categories (rename, move, folders); its operations are
  add, update (one/all), and remove.

### The registry (pluggable)

- **R4 (ubiquitous):** The IDE's settings shall expose the **registry** this
  feature uses — a single endpoint compatible with crates.io's interface,
  defaulting to crates.io — and all crate search, resolution, and download
  shall go through the configured registry. The setting is **IDE-wide**, not
  per-project: it applies to every project the IDE opens and is not stored in
  `cobolt.toml`.
- **R5 (event):** When the developer changes the configured registry,
  subsequent searches, adds, and updates shall use the new registry; crates
  already registered keep their pin, their vendored source, and their recorded
  URL untouched until their next explicit update.

### Adding a crate (search, then resolve at inclusion)

- **R6 (event):** When the developer types a query in the category's add
  dialog, the system shall search the configured registry and present the
  matching crates as a **paged table** — crate name, newest version,
  downloads, and short description — **50 rows per page**, with controls to
  move between pages and the registry's **total match count**, so that every
  match is reachable rather than a truncated sample. Choosing a result shall
  fill in the crate name. Finding a crate shall never require leaving
  PowerRustCOBOL: no browser, no command line.
- **R6a (constraint):** A completed search shall write **nothing** to the
  dialog's log — the results table is the feedback. (The log is for actions
  that change something: resolve, probe, download, update, remove.)
- **R7 (event):** When the developer confirms an add — the crate chosen from
  search results (R6) or its name typed exactly — with an optional **version
  requirement** (empty = newest stable) and an optional **feature list**, the
  system shall resolve the request against the configured registry.
- **R8 (event):** When resolution succeeds and the conflict check (R11–R15)
  passes, the system shall record in the project the crate's name, the
  developer's requirement, the exact resolved version, the requested features,
  and the crate's URL on the registry it came from; shall store the crate's
  source under `crates/`; and the tree shall show the new node (R2).
- **R9 (event):** When resolution fails — unknown crate, no version matching
  the requirement, or the registry unreachable — the system shall refuse the
  add, state which of those it was, and leave the project unchanged.
- **R10 (ubiquitous):** Every build shall use each registered crate at its
  recorded exact version, from its project-local source. The system shall not
  re-resolve, upgrade, or re-download a registered crate except through the
  explicit update actions (R16–R18) — a build gives the same answer tomorrow
  as today.

### Conflicts with PowerRustCOBOL's own tree

- **R11 (ubiquitous):** Conflict checking shall run at **add and update time**,
  against the complete dependency graph of a generated program — the workspace
  crates, the GUI stack, and the other registered external crates — so the
  developer learns the outcome while the decision is still in their hands.
- **R12 (event):** When the candidate's name matches a crate the generated
  program links **directly** (`eframe`, `egui`, `egui_extras`, the `cobolt-*`
  crates, and the rest of the generated manifest's direct dependencies): if
  the requested version is semver-compatible with the linked one, the system
  shall refuse the add as *already available* (informational — the block can
  `use` it today); if incompatible, the system shall refuse and name the
  built-in version, since one `use` name cannot denote two crates.
- **R13 (event):** When the candidate (or its dependency tree) cannot form a
  single consistent dependency graph with the program's tree — for example two
  crates claiming the same native `links` library at incompatible versions —
  the system shall refuse the add and surface the resolver's reason.
- **R14 (event):** When the candidate resolves but drags in a second,
  semver-incompatible copy of a crate already in the program's tree, the
  system shall allow the add and warn: both copies will exist in the binary,
  and the candidate's types will not interoperate with the built-in copy's.
- **R15 (constraint):** When a registered crate's version unifies with a
  version already required elsewhere in the graph, the built binary shall
  contain exactly **one** copy of that crate. The project-local source must
  not introduce a duplicate of the same name and version (one `serde 1.x` in
  the lockfile, never a path copy *and* a registry copy).

### Updating (one or all)

- **R16 (event):** When the developer invokes *Update* on one crate, the
  system shall resolve, against the configured registry, the newest version
  satisfying that crate's recorded requirement; if newer than the pinned
  version, it shall re-run the conflict check, replace the vendored source and
  the recorded version, and report *old → new*; otherwise it shall report the
  crate is current.
- **R17 (event):** When the developer invokes *Update All* on the category,
  the system shall apply R16 to every registered crate and present one
  summary: updated (old → new), already current, and failed, per crate.
- **R18 (constraint):** An update that fails — resolution error or a new
  conflict — shall leave that crate's recorded version and vendored source
  exactly as they were.

### Removing

- **R19 (event):** When the developer invokes remove on a crate node, the
  system shall ask for confirmation and, on yes, delete the crate's record and
  its `crates/` source. Blocks still naming the crate then fail the check as
  unregistered (R21) — the system shall never remove or edit the developer's
  COBOL because of a crate removal.

### EXEC RUST availability and diagnostics

- **R20 (ubiquitous):** A block shall reference a registered crate by its Rust
  library name (crates.io `-` becomes `_`: `serde-json` → `serde_json`) in
  plain `use` lines, and the reference shall be accepted by every surface that
  checks blocks today — the IDE's *Check*, *Build*, *Run*, *Debug*, and
  `rcrun` — in statement-level and item-level blocks alike.
- **R21 (event):** When a block names a crate that is neither linked by
  default nor registered, the system shall keep failing the build at the
  developer's line — and, when the program belongs to a project, the message
  shall say the crate can be added under **External Crates**, replacing the
  blanket "arbitrary crates are not yet supported".
- **R22 (constraint):** A single-file build (`rcrun build file.cbl`, no
  `cobolt.toml`) has no External Crates; a block naming one shall fail with a
  message saying external crates require a project.

### Build and binary

- **R23 (ubiquitous):** While a project registers external crates, every build
  of it shall link all of them into the program's single output binary —
  compiled at the same optimization profile as the rest of the program
  (release unless the project chose debug compilation), statically, with no
  separate shared library or sidecar file — and the produced binary shall run
  on a machine with no Rust toolchain, no cargo cache, and no `crates/`
  folder (041 R2/R3 preserved).

### The manifest

- **R24 (event):** When a build of a project with at least one registered
  crate succeeds, the system shall write `rust_manifest.md` into the project's
  destination folder (the same delivery step that places the binary, assets,
  and license notices), listing **every** registered external crate with its
  name, the URL recorded at add time (its page on the registry it came from),
  and the exact version built into the binary.
- **R25 (event):** When a build of a project with **no** registered crates
  succeeds, the system shall not write a manifest and shall delete a stale
  `rust_manifest.md` left in the destination folder by an earlier build — the
  delivered folder never claims third-party code the binary does not contain.
- **R26 (constraint):** The manifest is a build artefact: regenerated on every
  successful build, never hand-edited, and carrying a generated-by note
  consistent with the project's generated-code banner convention.

## 5. Acceptance criteria

- [x] **AC1** — Typing "csv" in the add dialog lists matching crates from the
      configured registry as a paged table (crate, version, downloads,
      description; 50 per page, page controls, total count); clicking a crate
      name fills it in. The whole flow happens inside the IDE — no browser,
      and no truncation to a handful. *(R6, R6a)*
      — *`a_search_page_arrives_and_logs_nothing` (page/total state, and the
      log stays empty), `results_render_as_a_markdown_table_with_pick_links`,
      `a_pipe_in_a_description_cannot_break_the_row`,
      `a_link_in_a_table_cell_is_clickable` +
      `a_header_cell_is_never_a_link` (md_render), and the mock registry's
      3-page walk over 120 matches in
      `mock_registry_add_update_and_failure`; visual half is the operator
      check.*
- [x] **AC2** — Confirming the add of `csv` with no version requirement
      resolves and pins an exact version, stores its source under `crates/`,
      records it (name, requirement, version, features, URL) in the project
      file, and the tree shows `csv <version>` under External Crates.
      *(R1, R2, R7, R8)*
      — *build half: the T6 e2e (vendored csv 1.4.0, recorded pins); tree
      half: `a_pin_renders_as_a_row_and_a_click_opens_the_dialog` (probe-
      verified `csv 1.4.0` row); add flow: the mock-registry test.*
- [x] **AC3** — A program whose block has `use csv::ReaderBuilder;` and parses
      a two-row CSV passes *Check* with no unlinked-crate error, builds, runs,
      and the parsed value is visible to COBOL afterwards. The same succeeds
      from a form event handler's block. *(R20)*
      — *`external_crates_build_run_manifest_and_determinism`: `MAIN=rows=2
      qty=5` via INVOKE, `HANDLER-ROWS=3` from a nested program (= a form
      event handler, 041 precedent); Check half:
      `analyze_project_honours_the_published_crates`.*
- [x] **AC4** — A block with `use serde::Serialize;` while `serde` is **not**
      registered fails at the developer's line; in a project the message names
      External Crates as the remedy; in a single-file build it says external
      crates require a project. *(R21, R22)*
      — *`an_unregistered_crate_in_a_project_points_at_external_crates` and
      `a_single_file_build_says_a_project_is_required` (cobolt-semantic).*
- [x] **AC5** — Adding `serde` with feature `derive`, then deriving
      `Serialize` on a type in an item-level block, builds and runs — and the
      build's lockfile contains exactly **one** `serde` entry, at one version,
      even though the base tree already uses serde. *(R7, R15, R20)*
      — *the T6 e2e: `#[derive(Serialize)] Receipt` in an item-level block;
      lock asserted to hold exactly one serde, resolved as a path (vendored)
      package.*
- [x] **AC6** — Adding `egui` is refused: at a semver-compatible version the
      message says it is already available; at an incompatible one it names
      the linked version. Nothing is recorded either way. *(R12)*
      — *unit: `direct_link_collisions_are_refused_both_ways`; live add flow:
      `live_resolver_verdicts` (refused, nothing recorded, no vendor dir).*
- [x] **AC7** — Adding a crate whose tree cannot coexist with the program's
      (e.g. an old `rusqlite` pinning a `libsqlite3-sys` major incompatible
      with the one the SQL runtime bundles — two claimants for the native
      `sqlite3` `links` key) is refused at add time with the resolver's
      reason; the project is unchanged. *(R11, R13)*
      — *`live_resolver_verdicts`: rusqlite `=0.24.2` refused in 6.1 s with
      cargo's links reason; record empty and the download cleaned up.*
- [x] **AC8** — Adding a resolvable crate that brings a second,
      semver-incompatible copy of an in-tree crate succeeds **with** the
      coexistence warning. *(R14)*
      — *`live_resolver_verdicts`: itoa `=1.0.10` allowed in 8.5 s with the
      "will exist twice (1.0.18 and 1.0.10)" warning captured.*
- [x] **AC9** — With a crate recorded at a version older than the newest
      matching its requirement, *Update* replaces the vendored source, records
      the new pin, and reports old → new; *Update All* does it for every crate
      and shows one summary of updated / current / failed. A failed update
      leaves the previous pin and source untouched. *(R16, R17, R18)*
      — *`mock_registry_add_update_and_failure`: mock publishes 0.2.0 →
      Update All reports 0.1.0 → 0.2.0, old vendor dir gone; a broken 0.3.0
      release yields Failed with pin 0.2.0 and its source intact.*
- [x] **AC10** — Two consecutive builds with no update in between use the same
      pinned versions byte-for-byte in the generated manifest, even when the
      registry has newer releases. *(R10)*
      — *the T6 e2e: build #2 (0.6 s warm) with byte-identical
      `rust_manifest.md` AND `Cargo.lock`.*
- [x] **AC11** — After a successful build, `dist/rust_manifest.md` exists and
      lists every registered crate's name, URL, and exact built version; after
      removing all crates and rebuilding, the file is gone.
      *(R24, R25, R26)*
      — *the T6 e2e: csv+serde rows asserted; build #3 (crates removed)
      deleted the stale manifest. Unit: `rust_manifest_lists_name_version_url`,
      `empty_pin_set_removes_the_stale_manifest`.*
- [x] **AC12** — The built binary of AC3 runs with an emptied `PATH` and
      cleared `RUSTUP_HOME`/`CARGO_HOME` (the 041 AC3 harness), proving no
      toolchain or cache is needed at run time. *(R23)*
      — *the T6 e2e's bare-env run: same `MAIN=rows=2 qty=5` output.*
- [x] **AC13** — Removing a crate asks for confirmation, then deletes its
      record and its `crates/` source; a block still using it fails the next
      *Check* with the R21 message; no COBOL source was touched. *(R19)*
      — *`removal_waits_in_the_confirmation_slot` (nothing happens before
      confirm), `remove_deletes_record_and_vendored_source_only` (record +
      source and only them), the R21 message from AC4's test.*
- [x] **AC14** — With the registry setting pointed at a local mock registry
      implementing the crates.io interface, search results come from the mock,
      an added crate's source is fetched from it, and the manifest's URL
      column points into it; switching the setting back to crates.io leaves
      that crate's pin, source, and URL untouched. *(R4, R5, R24)*
      — *`mock_registry_add_update_and_failure` end to end (6.5 s), incl. the
      manifest row carrying the mock's URL and the switch-back assertion.*
- [x] **AC15** — Every new user-facing string (category label, add/search
      dialog, registry setting, warnings, update summary, confirmation) exists
      in all six languages, verified by the existing i18n completeness tests.
      *(R1, R4, steering)*
      — *25 new `Tr` keys ×6; `cargo test -p cobolt-ide i18n` green (the Tr
      struct is total per language by construction, plus the audit tests).*

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — the category label, the add/search dialog
  (query field, result rows, no-results text, version / features fields), the
  registry setting's label, every refusal and warning in R9/R12–R14, the
  update report, and the remove confirmation are `Tr` fields in
  EN/ES/PT/JA/ZH/FR. Compiler diagnostics (R21/R22) follow existing
  diagnostic conventions.
- **Generated-code / regenerate contract:** `rust_manifest.md` and the
  generated Cargo manifest are build artefacts — regenerated every build,
  never hand-edited (R26). Vendored crate source is third-party *input*, not a
  generated artefact: read-only, replaced only by update.
- **Docs (English guide):** `docs/developers-guide-en.md` gains an External
  Crates section — searching, adding, pinning, updating, removing, the
  registry setting, the conflict rules in plain words, the manifest, and the
  network requirement at add time. Translations are user-maintained; do not
  touch them.
- **System KB:** Compiler/semantic behaviour changes (the R21 message, the
  new availability rule), so the KB documentation tables are updated in the
  same change and `build_chunked_kb` is re-run with `chunked.data` committed.
- **Fix vs feature:** **Feature** — beyond COBOL-85 and beyond the IDE's
  existing scope. Features branch; announced on forum f=96 ([Noticia]) after
  merge to main; per the operator's standing rule the version bump is `z`
  only — `x`/`y` are the operator's.
- **User code is sacred:** a conflict or a removal never deletes or edits the
  developer's COBOL (R19); crate removal is explicit and confirmed.

## 7. Open questions

- ~~**Q1: crates.io only?**~~ **RESOLVED 2026-08-07 (operator):** the registry
  is **pluggable** — one endpoint configured in the IDE's settings, crates.io
  by default, searched from inside the IDE so the developer never leaves
  PowerRustCOBOL (R4–R6). The endpoint must speak crates.io's interface; git
  and local-path dependencies remain non-goals.
- **Q2: feature selection.** The add dialog accepts *extra* features (without
  `derive`, serde-class crates are unusable, so this is in). Turning
  **default** features off is deferred. Confirm both halves.
- **Q3: manifest depth.** Assumed the manifest lists the crates the developer
  registered — not the full transitive closure (often dozens of rows), and no
  license column. Both would suit an audit; say the word and either becomes a
  requirement.
- **Q4: exact pins and Update.** With a requirement like `=1.2.3`, *Update*
  finds nothing newer *within the requirement* and reports "current" — by
  design. Changing the requirement itself means remove + re-add in v1. OK?
- **Q5: tree position.** External Crates sits after Generated Code, before
  Assets (R1). Veto freely — one-line change.
- **Q6: category glyph.** The tree uses hand-drawn vector icons per category;
  a new one is needed. Any preference (a cube/package shape is the natural
  pick), or leave it to the plan?
- ~~**Q7: scope of the registry setting.**~~ **RESOLVED 2026-08-07
  (operator):** the setting belongs to the **IDE** — IDE-wide, like Debug
  Settings, applying to every project and not stored in `cobolt.toml`
  (recorded in R4). The manifest stays truthful across registry changes
  because each crate records its own URL at add time (R5, R8).
- **Q8: private registries.** v1 assumes the registry is anonymously readable
  (crates.io and public mirrors are). Token authentication is deferred — is
  that acceptable for your environment?
