<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Module architecture, memory model, threads and the JavaScript boundary (WASM-017)

- **Status:** draft (awaiting the operator's review, then `/implement`)
- **Plan:** ./plan.md   **Date:** 2026-10-22
- **Branch / line:** `features-2x`, 2.x.x. **No documentation task:** `README.md` and `docs/` are never touched on 2.x.x. What 2.x.x
  says about itself goes in `specs/` (`WASM-PROGRESS.md`, `WASM-ABI-v1.md` §12) and in the System KB.
- **Binding (operator):** Q1 one program module per project; R9 multi-memory by default, single-memory for iOS and any matrix with
  Safari; R19 threads and Rayon in (all 2026-10-20). On 2026-10-22: Q2 Workers = `hardwareConcurrency − 1`, capped at 8; Q4 browsers
  released from 2024 onwards; Q6 `wasm-opt` only in the player's build; Q7 revisit on native support in Chrome, Firefox and Safari, the
  developer checks; Q8 `opt-level = "s"`; Q9 `nightly-2026-10-22` with `build-std`; Q10 offer the isolation shim.
- **Open, with the working assumption each task uses (not decided here):** Q2 first half, the yield budget in ms (T12); Q5 isolation
  on the mobile shell — the Mobile kind runs the plain player on the main thread (T12, T13); Q-17a default `Instances` = 1 for the
  main form, 4 for the others (T2); Q-17b no product cap below the 32-bit limit (T5); Q-17c the Rayon pool lives with the page (T11);
  Q-17d the browser data (T1); WASM-016 Q-P1 `cargo-leptos` for the player (T14).

Plan D1 holds throughout: this list builds only what no other plan owns. Where a task depends on another spec's task, it says which,
and it is not started before that task is done. Every task leaves the workspace green. Every test prints a quantified line. `df -h`
before any multi-crate run. `PLAYER` stands for `--manifest-path crates/cobolt-web-player/Cargo.toml` (the player is its own Cargo
workspace). A task that changes the player's sources ends with `cargo run PLAYER --example build_player -- --out assets/web-player` and
`cargo test PLAYER --test freshness` green (WASM-016 tasks, "Freshness rule").

## Grounding (read for this task list, 2026-10-22, `79400bd9`, 2.0.29)

- `cobolt_forms::model::Form` (`crates/cobolt-forms/src/model.rs:8804`) has no `instances` field. Form attributes in
  `crates/cobolt-forms/src/xml.rs` are kebab-case and written only when not default (`dock-gap`: read at :408, written at :1797 when
  `!= 16`, round-trip test near :3920). `get_attr` (:101) looks up named attributes, so **an unknown attribute is ignored on read**:
  a 1.x.x IDE reading a 2.x.x form with `instances="…"` drops it without error (plan §3 asked `/tasks` to check this).
- No code in the tree names `.wcfrm` or `.mcfrm` (a grep over `crates/` finds none); every form goes through `xml.rs`.
- The Properties panel writes form properties as `action.form_props.push(("DockGap", …))` in `show_form_geometry`
  (`crates/cobolt-ide/src/panels/properties.rs:11421`, row at :11530, label `tr.lbl_dock_gap`); `designer.rs` applies them (:8609)
  and reads them back (:8772). Form-level labels are `Tr` keys, so `Instances` gets keys in six languages. `properties.rs` has no
  notion of the project kind today (no `ProjectKind` in the file).
- `cobolt_wasm::module`: `Arena` :32 (default :45), `FormSpec { instances: u32, .. }` :141, `compile_project` :161, `compile_forms`
  :178, regions `align(total,16) × instances.max(1)` :312, `end` :316, `data_pages = end.div_ceil(65536).max(1)` :333, memory
  `minimum == maximum` :790, manifest `frame_size` :885. Layout refusals: `crates/cobolt-wasm/src/layout.rs:766` ("needs a PICTURE"),
  :773 ("more than 38 digits").
- `cobolt_wasm_abi::scan`: `ModuleScan.has_memory_grow` :61, `scan_module` :102, `bundle_problems(bytes, single_memory)` :174.
- `cobolt_compiler::project_kind`: `WebSettings` :273 (`multi_memory` with `#[serde(default = "yes")]`), `module_variants` :310
  (`single_memory: !multi_memory || browsers.includes_webkit()`), `MobileSettings::new` :348, `MobileSettings::module_variants` :366;
  test at :517 asserts "no WebKit engine: multi-memory only". Non-test callers of `module_variants()`:
  `crates/cobolt-project-tools/src/web/mod.rs:81`, `:98`, `:103` and `crates/cobolt-ide/src/panels/web_settings.rs:311-312`.
  Mobile bundles use `Isolation::Off` (`web/mod.rs:95`).
- IDE: `pk_set_multi_memory`/`_hint` (`crates/cobolt-ide/src/i18n.rs:391-392`, drawn at `panels/web_settings.rs:314`, :324).
- Payload numbers: generator arena 64 KiB (`module.rs:44-47`), player `net::Config.max_frame` 384 KiB
  (`crates/cobolt-web-player/src/net.rs:83`), server `max_frame_bytes` default 1 MiB (`crates/cobolt-server/src/config.rs:494`).
  The bundle writes `config.json` in `crates/cobolt-web-bundle/src/lib.rs` (:671 the `backend` object, :691 the file).
- Player: `force_panic_for_test` is a plain `#[wasm_bindgen]` export (`crates/cobolt-web-player/src/api.rs:1142-1143`); features
  `threaded`, `rayon` only (`Cargo.toml:45-52`); `[profile.release] opt-level = "z"` (:108); `build_player.rs` `NIGHTLY =
  "nightly-2026-10-08"` (:35), `-Oz` (:154, :202), record `profile`/`tools` (:283-284). The parts `parts/{canvas,mermaid,pdf}/src/lib.rs`
  carry 4, 1 and 1 `#[wasm_bindgen]` attributes. Freshness test `the_build_record_names_the_tools_and_the_pinned_nightly`
  (`tests/freshness.rs:100`).
- Bundle tests: `tests/e2e_node.rs:64` `assert!(passed >= 13)`; `tests/support/harness.rs` core cells :288, scenarios from :294
  (probe, choose, isolation, below-baseline, integrity, ABI refusal, loading :368, parts :379/:416, sign-in). Isolation headers:
  `cobolt_web_bundle::csp::security_headers` (`src/csp.rs:138`, `require-corp` :149, `credentialless` :154); the shim is
  `Options.service_worker_shim` (`src/lib.rs:151`, written at :695). The loader's reasons: `assets/loader.js:114-130`.
- `crates/cobolt-wasm/tests/`: `calls.rs`, `common/mod.rs` (`run_wasm` :33, `run_interpreter` :39, `same_output` :58) — no `forms.rs`
  or `corpus.rs` yet (WASM-010 T9, T20 create them). `crates/cobolt-web-bundle/src/hostpage.rs`, `tests/generated_host_page.rs` and
  `crates/cobolt-project-tools/src/web/build.rs` do not exist yet (WASM-002 T10/T11/T13 create the first two and the `WasmBuild`).
- System KB: `system_documentation()` (`crates/cobolt-compiler/src/lib.rs:4983`); the `[web]`/`[mobile]` settings table near :6202;
  the form property table (`DockGap` at :8837); test `system_documentation_is_what_publishing_writes` (:13954).
- `crates/cobolt-ide/src/version.rs:22` `VERSION = "2.0.29"`; `CHANGELOG.md` top heading `## [PowerRustCOBOL 2.0.29] — 2026-10-22` (:20).

## Tasks

### Facts first

- [ ] **T1 — Confirm the browser baseline and matrix** (R9, R12, R18, R19; Q4, Q-17d)
  - Files: `specs/WASM-PROGRESS.md` (new section "WASM-017 baseline and matrix").
  - Do: from MDN and Can I Use, for Chrome/Edge, Firefox, Safari (macOS), the iOS web view and the Android WebView **released from
    2024 onwards** (Q4): the oldest such version, and per engine multi-memory, threads + `SharedArrayBuffer` under isolation, JS String
    Builtins, ES-module source imports, and every feature of `scan::baseline_features()`. Each cell cites its source URL and date.
  - **Stop gate:** if Safari or the iOS web view of 2024+ has multi-memory, R9's WebKit rule is put to the operator; T7 does not
    start, and nothing is changed silently.
  - Verify: the table has one row per engine and one column per feature, every cell sourced; the minimum versions are the ones T12
    and T13 test against.

### Form instances (R2)

- [ ] **T2 — `Form.instances` in the model and the XML** (R2; plan §1.1, §3; Q-17a assumption)
  - Files: `crates/cobolt-forms/src/model.rs` (`Form.instances: Option<u32>`, `Form::effective_instances(is_main: bool) -> u32`,
    `Form::new` sets `None`), `crates/cobolt-forms/src/xml.rs` (read and write the attribute `instances`, kebab-case like `dock-gap`;
    written only when `Some`), every `Form { … }` literal the compiler flags.
  - Do: `effective_instances` returns the set value, else **1 for the main form, 4 for the others** — the working reading of R2's
    "4 for a form `OpenFormSync`/`OpenFormAsync` can open" (Q-17a, still open; one line to change). An out-of-range stored value is
    kept as written (user code is sacred) and refused at Check (T3).
  - Verify: `cargo test -p cobolt-forms --no-fail-fast` and `cargo test -p cobolt-forms --features render --no-fail-fast` green; new
    tests: `instances` round trip (`Some(7)` back as `Some(7)`); a form without it saves byte-identical to its input (an existing
    fixture under `examples/` loaded and re-saved); `effective_instances` gives 1/4/the set value.

- [ ] **T3 — The range check, with the form named** (R2; plan §1.1, §2)
  - Files: `crates/cobolt-forms/src/model.rs` (`Form::instances_problem() -> Option<String>`: 1 to 64, the bound the plan chose, the
    operator may change it), `crates/cobolt-project-tools/src/web/mod.rs` (the Web/Mobile build path at :81/:98/:103 refuses with the
    existing `WebBuildError::Refused`, naming the form).
  - Do: the Check of WASM-002 T14 calls the same function when it lands; no second copy of the rule. Desktop projects are not checked
    (no fixed memory).
  - Verify: `cargo test -p cobolt-forms -p cobolt-project-tools --no-fail-fast` green; tests: 0 and 65 refused with the form's name, 1 and
    64 accepted; a Desktop project with `instances="0"` builds as before.

- [ ] **T4 — `Instances` in the Properties panel, six languages** (R2; plan §1.1, §7 i18n)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (a row in `show_form_geometry`, `property_row_keyed(ui, tr.lbl_instances,
    Some("Instances"), …)`, `DragValue` 1..=64, pushed to `action.form_props`; shown only when the inspector is told the project is
    Web or Mobile — a new field set by its caller in `app.rs`), `crates/cobolt-ide/src/panels/designer.rs` (`"Instances"` in the
    setter near :8609 and the getter near :8772), `crates/cobolt-ide/src/i18n.rs` (`lbl_instances`, `lbl_instances_hint`,
    `msg_instances_out_of_range` × EN/ES/PT/JA/ZH/FR; pt-BR spelling; no "cobolt").
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` (no empty translation); a panel test in the shape of
    `the_form_inspector_shows_responsive_design_and_its_breakpoints_056` (`properties.rs:15652`): the row is listed for a Web project,
    absent for a Desktop one; setting it marks the form dirty and saves `instances="…"`.

### Memory model (R7, R8, R9)

- [ ] **T5 — Fixed memory has a ceiling** (R7, AC5; plan §1.2; Q-17b assumption)
  - Files: `crates/cobolt-wasm/src/module.rs` (`compile_forms`, after `data_pages` at :333), new `crates/cobolt-wasm/tests/memory_limits.rs`.
  - Do: refuse when `data_pages + (single-memory ? arena pages : 0) > 65 536` with a `GenError` naming **the form, its largest item**,
    `region_size × instances` and the total. `MAX_DATA_PAGES` = 65 536 until the operator sets a lower product cap (Q-17b open).
  - Verify: `cargo test -p cobolt-wasm --test memory_limits`: an oversized `OCCURS` table refused naming form and item (both variants);
    the same table with `instances` 1 accepted where 4 was refused; the existing refusals ("needs a PICTURE", "more than 38 digits")
    name the item; `scan_module(..).has_memory_grow == false` for every module the file compiles (prints modules scanned / with
    `memory.grow`). When WASM-010 T20 lands, `tests/corpus.rs` gains the same assertion over the corpus.

- [ ] **T6 — One payload limit** (R8, AC6; plan §1.3 "Payload limit", D3)
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`max_payload_bytes: u32` on `WebSettings` and `MobileSettings`,
    `#[serde(default)]` → 384 KiB), `crates/cobolt-project-tools/src/web/mod.rs` (passes it to `Options.arena.frame` and to the
    bundle), `crates/cobolt-web-bundle/src/lib.rs` (`config.json` gains `max_frame`, additive), the player's configuration reader and
    `src/net.rs` (`Config.max_frame` taken from `config.json` when present, else the default at :83); player rebuilt.
  - Do: `Arena::batch`/`answer` stay 64 KiB (ABI §2). The backend keeps enforcing its own limit (WASM-005 R11); the value is handed to it
    where WASM-002 T16 links the backend. If an ABI rule proves incomplete, amend `WASM-ABI-v1.md` §12 in the same commit.
  - Verify: `cargo test -p cobolt-compiler -p cobolt-project-tools -p cobolt-web-bundle --no-fail-fast`, `cargo test PLAYER --no-fail-fast`,
    freshness green; tests: an old `cobolt.toml` reads 384 KiB; a project setting of 512 KiB reaches the module manifest's `frame_size`
    and `config.json`'s `max_frame`; the player refuses a 513 KiB frame and accepts a 512 KiB one.

- [ ] **T7 — "Multi-memory only", refused when WebKit is in the matrix** (R9, AC7; plan §1.3, D5) — after T1's stop gate
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`WebSettings.single_memory_fallback: bool`, serde default `true`;
    `module_variants` keeps its signature; new `WebSettings::variant_problem() -> Option<VariantProblem>` naming the WebKit browsers),
    `crates/cobolt-project-tools/src/web/mod.rs` (the build refuses on `variant_problem()`), `crates/cobolt-ide/src/panels/web_settings.rs`
    (checkbox beside the multi-memory row at :314; the refusal shown), `crates/cobolt-ide/src/i18n.rs` (`pk_set_single_memory_fallback`,
    `_hint`, `pk_err_multi_memory_only_with_webkit` × six languages).
  - Decision taken here (plan §2 left it to `/tasks`): a separate `variant_problem()` rather than `module_variants() -> Result`, because the
    four call sites and the IDE tests (`web_settings.rs:407-409`) use the plain value; one new caller instead of five changed ones.
  - Do: the single-memory module is dropped only when `single_memory_fallback == false` **and** the matrix has no WebKit; with WebKit it
    is never dropped silently — Build and Check refuse. Consequence of D5, stated in the CHANGELOG: a Chromium-only project now carries
    both modules by default, so the test at `project_kind.rs:517` changes to "fallback off, no WebKit: multi-memory only".
  - Verify: `cargo test -p cobolt-compiler -p cobolt-project-tools --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
    tests: iOS → single only by default; default Web → both; fallback off + Safari → refused naming Safari (and the iOS web view);
    fallback off without WebKit → multi only; old `cobolt.toml` reads `true`.

### The boundary (R3, R10–R12, R14, R15)

- [ ] **T8 — Optional probes and the conversion counter in the generated loader** (R3, R6, R12, R18, AC9; plan §1.4, D2) — after
  WASM-002 T11
  - Files: `crates/cobolt-web-bundle/src/hostpage.rs` (WASM-002's Rust-generated loader; **no JavaScript file is added**),
    `crates/cobolt-web-bundle/tests/generated_host_page.rs`.
  - Do: probe JS String Builtins (`WebAssembly.validate` of a module importing `wasm:js-string` with `{builtins: ['js-string']}`) and
    ES-module source imports (detected without module syntax); push both into `choice.reasons`; **use neither** (D2). Debug builds only:
    `stats()` with `decodes`/`encodes` counted at the one `TextDecoder`/`TextEncoder` site each.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast`; the generated text contains both probes and, in a release bundle, no
    `stats`; the **AC10** source search: no `.buffer` view stored outside the call that made it (prints views found / stored).

- [ ] **T9 — The exposure list and a test-only panic hook** (R14, R15, AC11; plan §1.5, §5)
  - Files: `crates/cobolt-web-player/src/api.rs` (`pub const EXPOSED: &[&str]`; `force_panic_for_test` behind
    `#[cfg(feature = "test-hooks")]`), `crates/cobolt-web-player/Cargo.toml` (feature `test-hooks = []`), `examples/build_player.rs`
    (enables it for the node and browser test bundles only, never for `assets/web-player/`; the record names the feature set), new
    `crates/cobolt-web-player/tests/exposed.rs`; `crates/cobolt-web-bundle/tests/support/harness.rs` (the forced-panic scenario uses the
    test bundle).
  - Do: `exposed.rs` scans `src/**` and `parts/*/src/lib.rs` for `#[wasm_bindgen]` on a `struct`/`impl`; fails on a type missing from
    `EXPOSED` or a listed type with no `free()` path from `unmount()`. WASM-018 adds its handles to the same list.
  - Verify: `cargo test PLAYER --test exposed` (prints types found / listed) and fails on a planted unlisted type; freshness green; the
    shipped `assets/web-player/*/player.js` has no `force_panic_for_test` (search); node scenario "unmount" asserts every listed object's
    pointer is 0 after `unmount()`; forced panic (test bundle) shows the reload action and raises the error event.

- [ ] **T10 — `INITIAL` and `CANCEL` on both engines** (R4, AC4; plan §1.6) — the `INITIAL` half after WASM-010 T13
  - Files: `crates/cobolt-wasm/tests/calls.rs` (`initial_and_cancel`).
  - Do: a plain called program counts up across calls and after `CANCEL` counts from the start (works today: `Ins::InitUnit`,
    `lower_call.rs:257-266`); a program declared `INITIAL` returns its first value on every call (needs `Program.is_initial`, WASM-010 T13).
  - Verify: `cargo test -p cobolt-wasm --test calls` — `same_output` (interpreter = module, both variants), prints calls / equal.

### The R20 matrix

- [ ] **T11 — The scenario project in COBOL, and the core cells on a generated module** (R1, R2, R5, R11, R12, R20; AC1, AC2, AC3,
  AC6, AC8, AC9, AC10; plan §1.6) — after WASM-010 T9, T10, T16, T17 and WASM-002 T13
  - Files: new `crates/cobolt-web-bundle/tests/projects/core/` (three forms and Common Code: buttons GO, ECHO, QUERY, RAW, BURST,
    REMOTE, YIELD, TRAP, ERR, a child form, a chart on the third form; the form file kind is whatever WASM-002 T13's `WasmBuild` reads;
    English identifiers; no credentials — grep `APIKey|Password|Token` before staging), `tests/support/harness.rs`, `tests/e2e_node.rs`.
  - Do: the four core cells run on the fixture **and** the generated module from one script. New scenario for AC3: a form kept by
    `PreservePreviousForm` keeps its region (`pr_dbg_region_used` unchanged), destroying it releases it, `unmount()` terminates the
    program Worker and drops the module. The Rayon pool is **not** claimed terminated (Q-17c open: it lives with the page); the
    scenario prints that limit.
  - Verify: `cargo test -p cobolt-web-bundle --test e2e_node -- --nocapture`: **AC1** one `program*.wasm` fetched, at start, streamed;
    the chart part only when its form is first shown (fetch list printed); **AC2** 100 open/close cycles → `pr_dbg_live` and
    `pr_dbg_region_used` at baseline, instance `Instances + 1` → `-3` and the form's name; **AC6** `bundle_problems` empty on both
    variants (2 memories / 1) and identical results; **AC8** 1000 writes → 1 flush, read after write; **AC9** 1–4-byte round trip,
    decodes = answers, encodes = arguments (T8's counter); **AC10** reads correct after the player's memory grows. The `passed` floor
    rises to the new count.

- [ ] **T12 — The player axis and the long loop** (R19, R20; AC13; plan §1.6; Q2 first half and Q5 open)
  - Files: `crates/cobolt-web-bundle/tests/support/harness.rs`, `tests/e2e_node.rs`, `specs/WASM-PROGRESS.md`.
  - Do: `player ∈ {plain, threaded}` → 8 cells per module. Try the threaded player under node first; if `wasm-bindgen-rayon` cannot run
    there, the threaded cells run in Chrome through WASM-015's runner and the ledger records them as browser cells with the command —
    never claimed from node. BTN-YIELD runs **10 000 000** iterations. Deadline = 16 ms + `YIELD_BUDGET_MS`, a named constant whose
    working value is 6.9 ms, the largest slice measured with `budget_blocks` 20 000 (WASM-002 T8), until the operator answers Q2's first
    half. Workers = `max(1, min(8, hardwareConcurrency − 1))` (Q2, decided). The fixture's `maxGap < 50` stays.
  - Verify: isolated → threaded + Worker and a Rayon DataGrid sort reports > 1 worker; not isolated → plain + main thread; results
    identical across the 8 cells; every frame gap under the deadline (max gap printed); the ledger lists each cell with where it ran.

- [ ] **T13 — Browser cells: embedding, isolation headers, cross-origin resources** (R19c/d, AC14, AC15) — Chrome only here
  - Files: `crates/cobolt-web-bundle/src/csp.rs` tests (headers per documented host from `security_headers`), the browser runner of
    WASM-015, `specs/WASM-PROGRESS.md`.
  - Do: **AC14** the application embedded (WASM-002 R11) in a non-isolated host page → plain player, works; in an isolated host page →
    threads. **AC15** headers for each documented host; with the shim (`service_worker_shim`, Q10: offered, off by default) on a host
    without headers; a cross-origin image, font and tile load under `credentialless` and with CORS/CORP, and fail with a readable
    diagnostic otherwise. Mobile: plain player, main thread (Q5 working assumption).
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast` (headers); the Chrome runs recorded with their commands. Firefox, Safari
    (macOS, iOS) and the Android WebView are **not installed**: their cells are listed with the command to run them, none claimed.

### Build, records, knowledge

- [ ] **T14 — The Build report test** (R16, R17, AC12; plan D4) — after WASM-016 T18
  - Files: `crates/cobolt-web-player/tests/freshness.rs` (extend `the_build_record_names_the_tools_and_the_pinned_nightly`), the
    bundle's size report in `crates/cobolt-web-bundle/src/lib.rs` if it does not yet print the player's record.
  - Do: WASM-016 T18 makes the edits (`opt-level = "s"`, `wasm-opt -Os` per D4, `nightly-2026-10-22`); this task adds only the
    assertions — not a second edit (D1). What the record lists for `cargo-leptos` follows WASM-016 Q-P1 (open; working assumption:
    `build_player` keeps `cargo` + `wasm-bindgen` CLI + `wasm-opt`).
  - Verify: `cargo test PLAYER --test freshness`: both variants' sizes before and after `wasm-opt`, `opt_level` `s`, `wasm_opt` `-Os`, pinned
    tool versions, `nightly-2026-10-22`, `wasm_pack` "not used"; the Build report prints them.

- [ ] **T15 — The revisit record** (R6, Q7)
  - Files: `specs/WASM-PROGRESS.md` (section "Revisit: Component Model and ES-module integration").
  - Do: the trigger (native support in Chrome, Firefox and Safari), the owner (the developer, Q7), the date of the last check and T1's
    and T8's probe results. Nothing in code: R6 is met by plain imports and exports (ABI v1 §3/§4).
  - Verify: the section exists and cites T1's table.

- [ ] **T16 — System KB** (steering: System KB rule; spec §6)
  - Files: `crates/cobolt-compiler/src/lib.rs` (form property table near :8837: `Instances`, Integer 1–64, default 1 main / 4 others,
    Web and Mobile only; the `[web]`/`[mobile]` settings table near :6202: `max_payload_bytes`, `single_memory_fallback`; a short
    paragraph: in a browser a called program keeps its storage until `CANCEL` or its form instance closes, `INITIAL` starts fresh),
    `assets/knowledge/chunked.data` (regenerated).
  - Verify: `cargo run -p cobolt-ide --example build_chunked_kb`; `cargo test -p cobolt-compiler system_documentation_is_what_publishing_writes`
    and `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green.

- [ ] **T17 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs` (`VERSION`, line 22: bump `z` **once** — 2.0.29 at the time of writing, bump whatever it
    is then), `CHANGELOG.md` (**one** entry above the previous `## [PowerRustCOBOL …]` heading, dated, marked *Written on features-2x*;
    it states the open assumptions Q2-ms, Q5, Q-17a/b/c and the T7 default change).
  - Do: run the tests of every crate touched, with `--no-fail-fast`, reading every `test result` line: `cargo test -p cobolt-forms -p
    cobolt-wasm -p cobolt-wasm-abi -p cobolt-compiler -p cobolt-project-tools -p cobolt-web-bundle --no-fail-fast`;
    `cargo test -p cobolt-forms --features render --no-fail-fast`; `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
    `cargo test PLAYER --no-fail-fast`. `cargo fmt --check`; no warnings; credential scan of the staged diff. `df -h` first.
  - Verify: all green; **no documentation task** (`git diff --stat -- README.md docs/` empty); manual check of plan §6 (Project
    Settings shows the payload limit and the fallback checkbox in the IDE's language; fallback off with Safari → translated refusal)
    is **left to the operator** through the PowerRustCOBOL MCP tools; no browser run is claimed that was not done.

## Coverage of the acceptance criteria

| AC | Tasks whose verification covers it |
|---|---|
| AC1 | T11 |
| AC2 | T2, T3, T11 (instance exhaustion also WASM-010 T9) |
| AC3 | T11 (Rayon pool limit stated, Q-17c) |
| AC4 | T10 |
| AC5 | T5 |
| AC6 | T6, T11 |
| AC7 | T7 (loader choice: existing "choose" scenario, T12) |
| AC8 | T11 |
| AC9 | T8, T11 |
| AC10 | T8, T11 |
| AC11 | T9 |
| AC12 | T14 |
| AC13 | T12 |
| AC14 | T13 |
| AC15 | T13 |

## Still open (not decided by this list)

- **Q2, first half — the yield budget in ms.** Working value 6.9 ms in T12.
- **Q5 — isolation on the mobile shell.** Not answered; Mobile runs the plain player on the main thread.
- **Q-17a — the default `Instances`.** 1 for the main form, 4 for the others (T2).
- **Q-17b — a product cap on data memory.** None below the 32-bit limit (T5).
- **Q-17c — the Rayon pool and R5/AC3.** The program Worker is terminated; the pool lives with the page. AC3 is reported with that
  limit, not as fully met.
- **WASM-016 Q-P1 — `cargo-leptos` for the player.** Decides what T14's record lists.

## Done criteria

Every acceptance criterion above is checked by its task's verification (AC13–AC15 for Chrome only, the other engines listed as
owed); tests pass; the System KB is regenerated; `README.md` and `docs/` untouched; the open questions above are answered or stand
visibly in the CHANGELOG entry. No commit or push unless the operator asks.
