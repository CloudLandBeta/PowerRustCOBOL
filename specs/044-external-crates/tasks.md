<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — External Crates for EXEC RUST

- **Status:** draft → in progress → **done** (2026-08-07; T1–T15 complete,
  all 15 acceptance criteria checked in spec.md §5 with named evidence)
- **Plan:** ./plan.md   **Date:** 2026-08-07

Ordered, small, independently-verifiable tasks. Each names the files it
touches, the requirement(s) it satisfies, and how to verify it. The project
builds green after every task. Prototype modules
(`./prototype/src/*.rs`) transplant per plan §1 — copy, then adapt.

- [x] **T1 — Compiler: pin model + name-collision layer** (R8, R10, R12, R20)
  ✓ 2026-08-07: 6/6 unit tests, workspace builds. `CollisionRefusal` is a
  structured enum (IDE localizes by variant); `cobolt-*` reserved by prefix.
  - Files: `crates/cobolt-compiler/src/external_crates.rs` (new),
    `crates/cobolt-compiler/src/lib.rs` (mod decl; `#[serde(default)]
    crates: Vec<ExternalCrate>` on the compiler's `CoboltProject`).
  - Do: transplant prototype `project.rs` (`ExternalCrate`, `lib_name`,
    vendor-dir helper) and `conflict.rs` layer 1 (`DIRECT_LINKED` — mirrors
    `generate_cargo_toml`'s direct deps — plus reserved `cobolt-*` names,
    `name_collision()` with the already-available / clash / reserved
    verdicts). TOML round-trip of `[[crates]]` incl. absent-field defaults.
  - Verify: `cargo test -p cobolt-compiler external_crates` green (round-trip,
    collision-both-ways, reserved, lib_name); `cargo build` workspace green.
    Covers AC6's unit half.

- [x] **T2 — Compiler: generated manifest with pins + patches; probe manifest**
      (R7, R10, R15)
  ✓ 2026-08-07: 11/11 module tests; full compiler suite 52/52 in 191 s.
  **Found & fixed a PRE-EXISTING breakage** (fails on clean main too): fresh
  lock resolutions pick zune-jpeg ≥0.5.15 whose `log` feature nobody unions
  in → every generated GUI build failed to compile. Fix = one feature-union
  line in `base_dependency_block` + `zune-jpeg` joins the reserved table.
  **This hunk is a FIX — separable commit on the operator's rules (GR#5).**
  - Files: `crates/cobolt-compiler/src/lib.rs` (`generate_cargo_toml` gains
    `pins: &[ExternalCrate]`; call site passes `proj.crates`; missing-vendor
    validation before staging), `external_crates.rs` (`probe_manifest()`).
  - Do: each pin → exact-versioned dep (`= "=X.Y.Z"`, features) +
    `[patch.crates-io]` path entry to `<project>/crates/<name>-<version>`;
    paths **absolute and forward-slashed**; emit an empty `[workspace]`
    table (prototype-found defects). A pin whose vendor dir is missing fails
    the build naming the dir and the fix (re-add / update). `probe_manifest`
    = same generator + a candidate, for add/update-time probing.
  - Verify: `cargo test -p cobolt-compiler` — string asserts on pin lines,
    patch paths (incl. a backslash-bearing input rendered with `/`),
    `[workspace]` presence, features; missing-vendor error message test.

- [x] **T3 — Compiler: rust_manifest.md in delivery step 11c** (R24, R25, R26)
  ✓ 2026-08-07: 13/13 module tests (writer columns/banner/sort, stale-delete);
  wired beside the license notices with the step's warn-don't-fail posture.
  - Files: `crates/cobolt-compiler/src/external_crates.rs`
    (`write_rust_manifest`), `crates/cobolt-compiler/src/lib.rs` (step 11c
    call beside the license notices).
  - Do: transplant prototype `manifest.rs` — banner, `| Crate | Version |
    URL |` sorted table when pins exist; delete a stale manifest when none.
  - Verify: `cargo test -p cobolt-compiler` (columns/banner test,
    stale-delete test). AC10/AC11's unit halves.

- [x] **T4 — Semantic: `analyze_with` + allowlist + message split** (R20, R21, R22)
  ✓ 2026-08-07: 5 new tests + full semantic suite green (41 across binaries);
  options thread through nested-program recursion (handlers covered); parity
  test landed as **subset** (deliberate plan §4d revision: semantic must never
  allow an unlinked crate; linked-but-unadvertised flate2/bincode stay so).
  - Files: `crates/cobolt-semantic/src/lib.rs` (`AnalyzeOptions`,
    `analyze_with`; `analyze` delegates with `None`),
    `crates/cobolt-semantic/src/exec_rust.rs` (`unlinked_crates` takes the
    allowlist; two refusal texts), `crates/cobolt-compiler/tests/` (parity
    test: semantic `LINKED_CRATES` ≡ `DIRECT_LINKED` lib names — plan §4d).
  - Do: `Some(list)` accepts registered lib names and refuses others with
    "…add it under External Crates"; `None` refuses with "external crates
    require a project"; existing callers/behaviour unchanged.
  - Verify: `cargo test -p cobolt-semantic` (accept, both message variants,
    `analyze()` regression) + `cargo test -p cobolt-compiler` parity green.
    AC3/AC4 unit halves.

- [x] **T5 — Wire the build + CLI check** (R20, R21, R22)
  ✓ 2026-08-07: `build_core` gains `has_project` (build_project=true,
  build_single_file=false) and passes pins' lib names. **CLI unchanged, on
  purpose:** rcrun's check/run/run-form are single-file surfaces, so plain
  `analyze()`'s `None` is already the correct R22 context — no speculative
  plumbing. The pins-suppress-diagnostic proof lands with T6's e2e build.
  - Files: `crates/cobolt-compiler/src/lib.rs` (build passes pins' lib names
    into `analyze_with`; `build_single_file` passes `None`),
    `crates/cobolt-cli/src/main.rs` (`check` with/without `cobolt.toml`).
  - Do: plumbing only; no behaviour beyond T4's.
  - Verify: `cargo test -p cobolt-compiler -p cobolt-cli`; a compiler test
    proves a pinned project with `use csv…` produces **no** unlinked-crate
    diagnostic and an unpinned one produces the R21 text.

- [x] **T6 — Compiler e2e: build, run, manifest, determinism, one-copy,
      no-toolchain** (R7, R10, R15, R20, R23, R24, R25)
  ✓ 2026-08-07: 1/1 green, 76.4 s total — vendor csv+serde 2.5 s; build #1
  (cold, pins) 52.3 s → binary printed `MAIN=rows=2 qty=5` +
  `HANDLER-ROWS=3` (nested-program = handler equivalent, 041 precedent),
  same output with PATH/RUSTUP_HOME/CARGO_HOME cleared; manifest rows for
  csv+serde; lock has exactly ONE serde and both pins resolve as **path**
  (vendored) packages; build #2 (warm) 0.6 s with byte-identical
  manifest+lock; build #3 (crates removed) 20.2 s deleted the stale
  manifest and ran PLAIN-OK. Covers AC2/AC3(handler)/AC5/AC9/AC11/AC12.
  - Files: `crates/cobolt-compiler/tests/test_external_crates_e2e.rs` (new)
    + fixture project (cobolt.toml with `[[crates]]` csv + serde/derive; a
    console program and a form whose **event handler** block uses csv);
    `ureq`/`tar` as **dev-dependencies** only (test vendors like the IDE
    does; shipped compiler stays network-free).
  - Do: 041-style real builds, timings reported (golden rule #7): build →
    binary runs, output proves the csv parse (console + handler variants);
    `dist/rust_manifest.md` rows; build twice → manifest byte-identical and
    lockfile stable; lockfile has exactly one `serde`; run binary with
    emptied `PATH` + cleared `RUSTUP_HOME`/`CARGO_HOME` (041 AC3 harness).
  - Verify: `cargo test -p cobolt-compiler --test test_external_crates_e2e
    -- --nocapture` green, summary block lists cases + timings.
    **Covers AC2 (build+run halves), AC3's handler clause, AC5, AC9,
    AC11, AC12.**

- [x] **T7 — IDE model: project pins + the category** (R1, R2, R8)
  ✓ 2026-08-07: 23/23 project_model tests (3 new: category shape, pin
  round-trip incl. pre-044 load, not-a-file-list). `cat_external_crates`
  ×6 added (needed to compile the label match). One pre-existing test
  (`indexed_category_tree_order`) updated for the deliberate TOP change.
  The user-`crates/`-dir guard moved to T9's service (where the add flow
  lives) — noted deviation.
  - Files: `crates/cobolt-ide/src/project_model.rs`.
  - Do: `#[serde(default)] crates: Vec<ExternalCrate>` (type from
    `cobolt_compiler`); `Category::ExternalCrates` — `TOP` = 7, ordered
    after `Generated`; `root_subdir()="crates"`; `is_addable()` true;
    explicit arms in every exhaustive match (never a file list); add-time
    guard: an existing unexpected `<project>/crates` dir → refusal message,
    never overwrite (plan §5, user code sacred).
  - Verify: `cargo test -p cobolt-ide` (round-trip through the IDE save
    path, `TOP` order, guard test); workspace builds.

- [x] **T8 — IDE tree: rows, icon, events** (R1, R2)
  ✓ 2026-08-07: category renders under a hand-drawn crate/box icon; rows are
  `name version` (probe-verified `csv 1.4.0` rendered); synthesized click on
  the row emits `OpenExternalCrates` and no file events (1/1 behavioural
  test, real SidePanel/ScrollArea/CollapsingState wrappers); `[+]` routes to
  the dialog; folder-button and OS-drop targeting excluded for the category;
  `app.rs` flag `show_external_crates` wired.
  - Files: `crates/cobolt-ide/src/panels/project.rs`,
    `crates/cobolt-ide/src/app.rs` (event routing stub),
    `crates/cobolt-ide/src/i18n.rs` (`cat_external_crates` ×6).
  - Do: category renders `name version` per pin (R2); hand-drawn
    package/cube `tree_icon` (Q6); `[+]` and row activation emit
    `ProjectPanelEvent::OpenExternalCrates` → `app.rs` sets the dialog-open
    flag (dialog lands in T10).
  - Verify: `cargo test -p cobolt-ide` render/behavioural test — tree shows
    the category and a `csv 1.4.0` row (AC2's tree half); i18n test green.

- [x] **T9 — IDE service: registry client, settings, actions** (R4–R9,
      R11–R19)
  ✓ 2026-08-07: `external_crates_service.rs` — full transplant (client with
  UA `PowerRustCOBOL/<version>`, R9 error taxonomy; IDE-wide settings file
  via the debug_settings pattern; add/update/remove on-disk through
  `load_project`/`save_project`; probe = compiler's `probe_manifest` +
  `cargo metadata`, baseline diff, R15 guard; `resolve_workspace_root`
  exported from the compiler — build_core now uses the same helper).
  4/4 unit tests incl. the plan-§5 foreign-`crates/` guard. `ureq`/
  `native-tls`/`tar`/`flate2`/`semver` added to IDE deps.
  - Files: `crates/cobolt-ide/src/external_crates_service.rs` (new),
    `crates/cobolt-ide/Cargo.toml` (`ureq`, `native-tls` — runtime's
    versions/features).
  - Do: transplant prototype `registry.rs` (search/resolve/download+unpack,
    User-Agent, 30 s timeout, R9 error taxonomy) and `ops.rs`
    (add/update/remove with progress sink; update = newest-within-
    requirement, download-before-delete ordering, R18); probe step calls
    `cobolt_compiler::probe_manifest` + `cargo metadata` + baseline diff
    (R14) + duplicate-copy guard (R15); workspace via the compiler's
    exported `find_workspace_root`. `ExternalCratesSettings { registry }`
    at `llm::base_dir()/external_crates.toml` (debug_settings pattern),
    default `https://crates.io`.
  - Verify: `cargo test -p cobolt-ide external_crates_service` — transplanted
    version-picking/URL tests, settings round-trip, ops unit tests with a
    stubbed registry trait or the T12 mock (no live network in this task).

- [x] **T10 — IDE dialog** (R2, R4, R6, R7, R16–R19)
  ✓ 2026-08-07: `panels/external_crates.rs` — prototype transplant on the
  house worker/mpsc pattern; 24 new `Tr` keys ×6 (chrome; action progress
  stays diagnostic-stream per spec §6); registry field persists the IDE-wide
  setting on change; R19 confirm modal; app wiring saves the project before
  opening and reloads `cobolt.toml` when an action mutated it. 4/4
  state-machine tests (busy/log/reload lifecycle, refusal keeps project,
  hits→selection, removal waits for confirmation); 11/11 across the
  feature's IDE tests.
  - Files: `crates/cobolt-ide/src/panels/external_crates.rs` (new),
    `crates/cobolt-ide/src/app.rs` (state + show call),
    `crates/cobolt-ide/src/i18n.rs` (~20 dialog keys ×6).
  - Do: transplant prototype `ui.rs` (already egui 0.36): registry header
    field persisting the IDE-wide setting on change; search box + pickable
    results; name/requirement/features + Add; registered list with Update /
    Update All / Remove (+ ↗ URL); R19 confirm modal; log pane with
    warn/error colours; worker `mpsc` + spinner + disabled buttons while
    busy. All strings `Tr`.
  - Verify: `cargo test -p cobolt-ide` dialog state-machine tests with
    injected worker messages — search results render and fill the name
    (AC1's UI half), busy disables actions, refusal shows red, confirm
    gates removal (AC13's UI half); `cargo build -p cobolt-ide`.

- [x] **T11 — IDE analysis plumbing** (R20)
  ✓ 2026-08-07: all nine IDE `analyze` sites (runner ×2, app ×2,
  form_runtime ×3, designer ×1 + the wrapper) route through
  `external_crates_service::analyze_project`, fed by a per-frame published
  project-crates list (the `theme::set_active`/debug-switch sync pattern —
  RwLock'd for the worker threads). Targeted test proves published-crate
  passes and no-project yields the R22 message. 12/12.
  - Files: `crates/cobolt-ide/src/{runner.rs, form_runtime.rs, app.rs,
    agent_lint.rs, panels/designer.rs}`.
  - Do: every `analyze` call in a project context becomes
    `analyze_with(Some(pins' lib names))`; no-project surfaces pass `None`.
  - Verify: `cargo test -p cobolt-ide` green; targeted test: Check on a
    project with a pinned crate reports no unlinked-crate diagnostic
    (AC3's Check half).

- [x] **T12 — IDE integration tests: mock registry + live probe verdicts**
      (R4, R5, R10 update-paths, R11, R13, R14, R16–R18, R24)
  ✓ 2026-08-07: in-src `flow_tests` (bin crate — tests/ cannot import it).
  Mock (std TcpListener, crates.io API shape): search-from-mock, add+vendor
  with mock URL recorded + in manifest, switch-back untouched, update
  0.1.0→0.2.0, broken release leaves pin+source (6.5 s total; add incl.
  probe 4.3 s). Live: egui refused instantly; rusqlite =0.24.2 links clash
  refused in 6.1 s with cargo's reason + download cleaned; itoa =1.0.10
  warned in 8.5 s. **Design correction found by the mock:** actions now
  vendor BEFORE the probe (candidate enters via its patch, like the real
  build) — required for non-crates.io registries, cleanup on refusal.
  Covers AC6/AC7/AC8/AC10/AC14.
  - Files: `crates/cobolt-ide/tests/test_external_crates_flows.rs` (new; a
    std-`TcpListener` mock registry serving the crates.io API shape: search
    JSON, versions JSON, `.crate` tarball built in-test).
  - Do + Verify (`cargo test -p cobolt-ide --test
    test_external_crates_flows -- --nocapture`, quantified summary):
    - mock: search hits come from the mock; add vendors from it; recorded
      URL points into it; switching the setting back leaves pin/source/URL
      untouched — **AC14** (its manifest clause via `write_rust_manifest`
      on the recorded pins).
    - mock: add at an old version, bump the mock's index, Update → old→new
      + summary counts; a mock-injected failure leaves pin+source untouched
      — **AC10**.
    - live (network, 041 precedent): `egui` refusal both ways (**AC6**),
      `links`-conflict refusal with the resolver's reason (**AC7**),
      coexistence warning for a disjoint same-major pin (**AC8** — crate/
      version pair chosen at implement time for stability).

- [x] **T13 — Docs & System KB** (steering)
  ✓ 2026-08-07: guide gains "External Crates" at the end of the EXEC RUST
  chapter (PowerCOBOL/OCX framing, COBOL example, pin/update/remove,
  conflicts in plain words, manifest, notes+caveats, screenshot placeholder
  `external-crates-dialog.png`); `cobol85-supported-syntax.md` extension
  line updated. Translations untouched.
  **Correction, same day:** the first `build_chunked_kb` run left
  `chunked.data` byte-identical — the KB is built from
  `cobolt_compiler::publish_system_documentation`'s Rust constants, **not**
  from `docs/*.md`, so a green freshness test was false comfort. The
  constant's "Crates" bullet still said *"a `use` of anything else is
  rejected"*, which this feature makes false and which is exactly what
  Grace would have told a developer. Rewrote that bullet (registration,
  `serde-json`→`serde_json`, both refusal messages, conflict outcomes,
  manifest, "do not say third-party crates are unsupported") and re-ran the
  build: **969 → 972 records**, `chunked.data` now genuinely modified,
  freshness test green, release IDE rebuilt to embed it.
  - Files: `docs/developers-guide-en.md` (External Crates section: search,
    add, pin, update, remove, conflict rules in plain words, the manifest,
    network at add time, older-IDE note — COBOL/prose only, no Rust),
    `docs/cobol85-supported-syntax.md` (EXEC RUST crate rule: linked +
    registered), `assets/knowledge/chunked.data` (rebuilt).
  - Do: write the docs; run `cargo run -p cobolt-ide --example
    build_chunked_kb`; commit the regenerated data. Translations untouched.
  - Verify: KB freshness test green
    (`prebuilt_chunked_kb_matches_the_published_documentation`); guide
    section renders in the IDE doc viewer.

- [x] **T14 — i18n audit** (R1, R4; AC15)
  - Files: `crates/cobolt-ide/src/i18n.rs` (fill any gap found).
  - Verify: `cargo test -p cobolt-ide i18n` — every new key present and
    non-empty in EN/ES/PT/JA/ZH/FR. **Covers AC15.**
  ✓ 2026-08-07: 25 keys ×6 (1 category + 24 dialog); audit green, no gaps.

- [x] **T15 — Finalize** (all)
  ✓ 2026-08-07: version 1.60.43 → **1.60.44** (z only), CHANGELOG entry
  written with the feature and the zune-jpeg fix in **separate sections**
  (they must ship as separate commits, GR#5). Sweep
  `cargo test --workspace --no-fail-fast`: **98 binaries, 1683 passed,
  0 failed, 8 ignored** — every ignore pre-existing and annotated (2 native
  store suspended until 1.90.0, 2 live DB servers, 1 micro-benchmark, 1
  scale smoke test, 1 rounded-rect dump helper, 1 spec-009 INITIALIZE
  out-of-scope). Release IDE built (optimized, 1 m 26 s). Manual checklist
  below is the operator's.
  - Files: `crates/cobolt-ide/src/version.rs` (z bump — operator owns x/y),
    `CHANGELOG.md`.
  - Do: full sweep `cargo test --workspace --no-fail-fast` — collect
    **every** "test result" line, list expected failures explicitly (memory
    rule; no failure-grep verdicts); `cargo build --release`; manual
    checklist (plan §6): open IDE → External Crates → search "csv" → add →
    probe narration → Build → run the app → read `dist/rust_manifest.md` →
    flip registry to a mirror and search (AC14 live half) → walk the six
    languages. Implementation happened on the **features** branch (merged
    from main first, GR#5); commits are feature-only (no fix mixing); do
    **not** push in the São Paulo window; f=96 [Noticia] announcement only
    after the operator merges to main, text confirmed first.
  - Verify: sweep summary attached to the task; release IDE launches; every
    AC box in spec.md §5 checked with its evidencing test/check named.

- [x] **T16 — Operator revisions (post-implementation, 2026-08-07)**
      (R1, R4, R6, R6a)
  - Files: `panels/md_render.rs`, `panels/external_crates.rs`,
    `external_crates_service.rs`, `i18n.rs`, `project_model.rs` (unchanged
    internals), `docs/developers-guide-en.md`,
    `docs/cobol85-supported-syntax.md`, compiler KB constant, `CHANGELOG.md`,
    `version.rs`.
  - Do: (1) **removed the `N × "query"` log line** — debug output the
    operator spotted in the log pane; a search now finishes silently
    (`Msg::Finished{result: Ok(None)}`). (2) **Results are no longer capped
    at 10**: `Registry::search(query, per_page, page)` returns a `SearchPage`
    with the registry's `meta.total`, and the dialog renders **50 per page**
    as a **rendered markdown table** (crate · version · downloads ·
    description) with `◀`/`▶` and a "Page 2/3 — 120 results" counter.
    Clicking a crate name picks it, which needed `md_render::draw_table` to
    stop flattening cells to plain text: cells now keep their link and report
    it via the new `RenderOutput::clicked_link` (header cells stay inert).
    Descriptions are pipe-escaped so a `|` cannot shear a row. (3) **Renamed
    the feature to "Project's Crates"** everywhere it is user-visible —
    category label ×6, dialog, diagnostics, guide, syntax reference, KB
    constant, changelog — leaving internal identifiers alone per the
    project's own internal-vs-brand convention.
  - Verify: `cargo test -p cobolt-ide` 17/17 on the feature's tests + 2/2 new
    md_render tests; mock registry now serves 120 matches so paging is real
    (page 1 = 50, page 2 = 50 with no repeats, page 3 = 20); KB rebuilt and
    freshness green. ✓ 2026-08-07, version → **1.60.45**.
    Re-swept after the rework: **98 binaries, 1687 passed, 0 failed, 8
    ignored** (the same 8 pre-existing, annotated ignores); release IDE
    rebuilt (39.3 s).

- [x] **T17 — Results-table layout (operator, 2026-08-07)** (R6)
  - Files: `panels/md_render.rs` (+`TableLayout` opt-in, `draw_table_tight`),
    `panels/external_crates.rs` (opts into it), the three other `RenderOpts`
    call sites (explicitly `Equal`), guide, KB constant, `CHANGELOG.md`,
    `version.rs`.
  - Do: the operator's screenshot showed four equal columns — three of them
    mostly empty — with the description crammed into a quarter of the width,
    **and a row whose crate/version/downloads cells were blank**. Added
    `TableLayout::TightResizable`, opted into by this dialog only (the doc
    viewer, the editor preview and the leaderboard keep `Equal`, so no doc
    table changes): every column but the last is measured against its own
    widest cell and never wraps, the last takes the remaining width and is
    the only wrapping one, and `egui_extras` supplies draggable boundaries
    that paint a line at each column edge. Row heights are computed from the
    last column's width as measured on the previous frame, so a drag settles
    in one frame.
  - Verify: **the blank-cell bug is a rendering artifact, proven not a data
    one** — live-search markdown parses back to 51 rows × 4 cells with 50
    pick links (`every_row_of_a_live_search_keeps_its_four_cells`,
    `live_results_parse_back_as_whole_rows`), and the new paint-level test
    `no_row_loses_its_cells_to_a_tall_description` asserts every cell of
    every row is actually painted when one row's description wraps long —
    which is the case that was failing. `tight_columns_give_the_description_more_room`
    measures wrapped galley rows and proves the tight layout wraps less than
    the equal one. 4/4 md_render, 19/19 feature tests, KB rebuilt (972
    records), sweep below. ✓ version → **1.60.46**.

## Done criteria

All 15 acceptance criteria in spec.md §5 are checked with named evidence,
the full-workspace sweep is green (expected failures listed explicitly),
docs + KB are current, and the change sits in feature-only commit(s) on the
features branch per the operator's rules (do **not** commit/push/announce
unless the operator asks).

## AC → task map

| AC | Covered by |
|----|-----------|
| AC1 search | T10 (UI state) + T15 (manual live) |
| AC2 add/pin/vendor/tree | T6 (build+run) + T8 (tree) + T9 (ops) + T15 |
| AC3 csv block everywhere | T6 (console + handler) + T11 (Check) |
| AC4 unregistered messages | T4 + T5 |
| AC5 serde one-copy | T6 |
| AC6 egui refusal | T1 (unit) + T12 (live) |
| AC7 links refusal | T12 |
| AC8 coexistence warning | T12 |
| AC9 determinism | T6 |
| AC10 update old→new / fail-safe | T12 (mock) |
| AC11 manifest present/removed | T3 (unit) + T6 (e2e) |
| AC12 no-toolchain run | T6 |
| AC13 confirmed removal | T9 (ops) + T10 (modal) + T15 (manual) |
| AC14 pluggable registry | T12 (mock) + T15 (manual live) |
| AC15 i18n ×6 | T14 |
