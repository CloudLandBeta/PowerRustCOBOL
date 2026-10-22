<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Module architecture, memory model, threads and the JavaScript boundary (WASM-017)

- **Status:** draft (for the operator's review, then `/tasks`)
- **Spec:** ./spec.md   **Date:** 2026-10-22
- **Branch / line:** `features-2x`, version line 2.x.x. Companions: `specs/WASM-INDEX.md` (D15–D18), `specs/WASM-PLAN.md`,
  `specs/WASM-ABI-v1.md` (§2, §4, A3), and the plans of WASM-002, WASM-010 and WASM-016, which own most of the files this spec rules on.
- **Binding decisions (operator):** Q1 one program module per project (2026-10-20); R9 multi-memory by default, single-memory for iOS
  and any matrix with Safari (2026-10-20); R19 threads and Rayon in (2026-10-20); and on 2026-10-22: Q2 Workers = `hardwareConcurrency − 1`,
  capped at 8; Q4 browsers released from 2024 onwards; Q6 `wasm-opt` only in the player's build; Q7 revisit the Component Model and
  ES-module integration when Chrome, Firefox and Safari all support them natively, and the developer checks; Q8 `opt-level = "s"`;
  Q9 the nightly of 2026-10-22, with `build-std`; Q10 offer the isolation shim.
- **Still open (I state working assumptions and decide none of them):** Q2's first half (the yield budget in milliseconds), Q5
  (isolation on the mobile shell), and the points in §8.

## 0. Where the code stands (read 2026-10-22, `features-2x` at `79400bd9`, 2.0.29)

WASM-017 is an architecture spec. Most of what it requires was built with the player, the loader and the generator, and **another
spec's plan owns the file** in most cases. This plan maps each requirement to what exists, who owns the remaining work, and the part
this spec has to deliver itself. I read every item in the file named.

| R | What exists | Evidence | Remaining, and who does it |
|---|---|---|---|
| R1 | One module for all forms: `compile_project(&[FormSpec], &Options)` lowers every form into one module. Streaming is in the loader: `compileStreaming` when the type is `application/wasm` | `crates/cobolt-wasm/src/module.rs:161`; `crates/cobolt-web-bundle/assets/loader.js:162-163` | The project exports (`pr_open_form` …) are **not emitted yet** (`module.rs:817-823` exports `pr_run` only; manifest `"forms": []` at `:889`). **WASM-010 WP2.** The Build that calls it is **WASM-002 C6** |
| R2 | `FormSpec.instances`; the regions are `align(layout.total,16) × instances` per form, sized at compile time | `module.rs:141-150`, `:308-328` | **No `Instances` property exists** on `cobolt_forms::model::Form` (`model.rs:8804-8868`). No caller passes a count. Exhausting the instances (`-3`, ABI §4) is WASM-010 WP2's. **The property is this plan's (§1.1)** |
| R3 | Streaming instantiation; parts on first use through `loadPart`/`partModule`; scenarios "loading: nothing is fetched twice" and "parts: … fetched on first use, once" | `loader.js:756`, `:788`; `tests/support/harness.rs:368`, `:379` | ES-module integration is **not probed** (grep finds nothing in `loader.js`). This plan, §1.4 |
| R4 | `CANCEL` reinitialises a unit (`Ins::InitUnit`) | `crates/cobolt-wasm/src/lower_call.rs:257-266` | `INITIAL` is not generated because the AST drops it (`specs/WASM-PARITY.md` row 10). **WASM-010 WP4** (`Program.is_initial`). This plan adds the AC4 test only |
| R5 | `unmount()` closes each instance (`pr_close_form`), cancels pending calls, terminates the program Worker and frees `Player`. The core scenarios check it in all four variant × mode cells | `loader.js:792-801`, `:423-428`; `harness.rs:275-282` | The release and reinitialisation of the region on `pr_close_form` is WASM-010 WP2's. **The Rayon pool cannot be terminated** (WASM-016 plan §4). That conflicts with R5/AC3 (§5, §8) |
| R6 | Plain imports and exports (ABI v1 §3/§4); no Component Model anywhere | `WASM-ABI-v1.md`; `module.rs:766-770` | Nothing to build. Q7's trigger becomes a recorded check (§1.4) |
| R7 | Fixed data memory (`minimum == maximum`, `module.rs:788-795`). `scan_module` detects `memory.grow` and `bundle_problems` refuses it | `crates/cobolt-wasm-abi/src/scan.rs:60`, `:127-131`, `:215-216` | **No upper bound:** `data_pages` (`module.rs:333`) is never compared with the 32-bit limit, so a huge table gives an invalid module instead of an error that names the item. This plan, §1.2 |
| R8 | The arena is memory 1, or a region of memory 0 at `pr_arena_base`. The host API is the same in both variants | `module.rs:184-190`, `:796-818`; ABI §2 | The arena size is `Arena::default()` (`module.rs:44-47`: 64 KiB per region). It is **not taken from the project's payload limit** (WASM-005 R11), and no project setting exists for it. The player's `net::Config.max_frame` is 384 KiB (`crates/cobolt-web-player/src/net.rs:83`) and the server's default is 1 MiB (`crates/cobolt-server/src/config.rs:494`). Three numbers. This plan, §1.3 |
| R9 | `WebSettings::module_variants` adds the single-memory variant whenever the matrix includes WebKit. `MobileSettings::new` turns multi-memory off for iOS, and an iOS build always carries single-memory. The loader probes `WebAssembly.validate(MULTI_MEMORY_PROBE)` | `crates/cobolt-compiler/src/project_kind.rs:305-371`; `loader.js:34`, `:91` | **No way to say "multi-memory only"**, and therefore no refusal (AC7). This plan, §1.3 |
| R10 | The loader makes a view at each use (`const view = (off,len) => new Uint8Array(ex.arena.buffer, …)`). The player copies once and keeps nothing (doc of `api.rs:15-17`). A scenario grows the player's memory by 40 MB and reads after it | `loader.js:218`; `worker.js:25`; `harness.rs:258-273` | The "no cached view" source search over the loader (AC10) does not exist. When WASM-002 C1 lands, it runs over the **generated** loader. This plan, §6 |
| R11 | Batching: 1000 writes, 1 flush; read-after-write | `harness.rs:184-190`, `:177-182` | Proven on the **fixture** module (`cobolt-wasm-abi::fixture`), not on a generated one. The flush points of the generator are WASM-010 WP2's |
| R12 | UTF-8 in the arena; `TextEncoder`/`TextDecoder`; a 1–4-byte round trip | `loader.js:97`, `:195`; `harness.rs:166-175` | No conversion counter (AC9 "a trace counts conversions"). JS String Builtins are not probed. This plan, §1.4 |
| R13 | The player moves files, images and pixel buffers (WASM-014 R15, WASM-018) | WASM-016/018 plans | Nothing here |
| R14 | `Player` is the only exposed object. A test greps for `Closure::forget` | `api.rs:19-21` | The "listed objects" exist only as a doc comment. The parts (`parts/{mermaid,pdf,canvas}`) and WASM-018's handles are not on a list a test checks. This plan, §1.5 |
| R15 | `panic = "abort"`; `install_panic_hook`; `force_panic_for_test` | `crates/cobolt-web-player/Cargo.toml:107-112`; `api.rs:1128-1145` | `force_panic_for_test` is exported in the **shipped** player. §5 |
| R16 | Profile `lto`, `codegen-units = 1`, `panic = "abort"`, `strip`; two variants; the threaded build uses `-Zbuild-std`, atomics and `wasm-bindgen-rayon` | `Cargo.toml:107-112`, `:49`, `:78`; `examples/build_player.rs:35-39`, `:121-138` | `opt-level = "z"` (Q8 says `"s"`), `wasm-opt -Oz` (`build_player.rs:154`, `:202`), `NIGHTLY = "nightly-2026-10-08"` (Q9 says 2026-10-22). **WASM-016 plan C5/C6** changes them. This plan sets the `wasm-opt` level that goes with `"s"` (§4 D4) |
| R17 | Pinned `wasm-bindgen` CLI (`=0.2.129`), `wasm-opt`; `wasm_pack: "not used"`; the nightly is recorded in `player-build.json` | `Cargo.toml:75`; `assets/web-player/player-build.json` `tools` | `cargo-leptos` is not in the record. Whether it builds the player is **WASM-016 Q-P1**. The record follows that answer |
| R18 | Baseline probe (`BASELINE_PROBE`), the variant, player and mode chosen independently with reasons; below the baseline, a readable message | `loader.js:27`, `:81-135`; `harness.rs:294-341` | The matrix "from 2024" (Q4) is not written into the loader or the settings. The optional accelerations are not probed. The MDN / Can I Use confirmation is owed (§8). This plan, §1.4 and §1.6 |
| R19 | The threaded player when isolated; the Worker mode with `Atomics.wait` reads; the main-thread mode with yield; Workers `max(1, min(8, hc − 1))`; headers for `_headers`/nginx/Apache; the shim off by default | `loader.js:98-135`, `:333-349`, `:615-616`; `worker.js:37`; WASM-002 plan R20 | Rayon is used for the DataGrid sort and filter (`crates/cobolt-web-player/src/par.rs:24`, `:38`). The **graphics** work under (a) is WASM-018's. AC13–AC15 browser runs are owed. §6 |
| R20 | 4 core cells (program variant × mode) on the fixture, and 7 more scenarios; `passed >= 13` | `crates/cobolt-web-bundle/tests/e2e_node.rs:58-64`; `harness.rs:286-290` | The **player-variant axis** is absent (the node harness runs one player). The cells are not run on a **generated** module. The loop runs 3 000 000 iterations (`harness.rs:222`), but AC13 asks for 10 000 000 and frames under a deadline. This plan, §1.6 |

**What follows.** WASM-017 owns six pieces of its own work (§1.1–§1.6). The rest is done by WASM-002, WASM-010 and WASM-016, and this
plan only checks their acceptance against R1–R20 (§6). I'm not planning any of their files a second time.

## 1. Approach

### 1.1 The form's `Instances` property (R2, AC2)

- **Model:** `Form` gains `instances: Option<u32>` (`crates/cobolt-forms/src/model.rs:8804`). `None` means the default. It is read and
  written by `xml.rs` as an optional form attribute. A form without the attribute writes nothing new, so old `.cfrm`/`.wcfrm`/`.mcfrm`
  files keep their bytes on a round trip.
- **Default (working assumption, §8 Q-17a):** R2 says "4 for a form that `OpenFormSync`/`OpenFormAsync` can open, 1 otherwise". A
  form name passed to those verbs can be a data item, so the Build cannot always know which forms are opened. I read the rule as
  **1 for the project's main form, 4 for every other form**. The value is in a single function, `Form::effective_instances(is_main)`,
  so a different reading changes one line.
- **Range:** 1 to 64, refused out of range by Check with the form's name. The upper bound is mine (the instance table is fixed
  memory, `INST_ROW` = 32 bytes, `module.rs:117`). The operator may change it.
- **Designer:** shown in the Properties panel only for Web and Mobile forms. A Desktop form has no fixed memory, so the property
  means nothing there. `/tasks` reads how the panel labels form-level properties: if it uses `Tr` keys, the label and hint are added
  in six languages.
- **Build:** WASM-002's `WasmBuild` (its step 3) fills `FormSpec.instances` from `effective_instances`.
- **System KB:** the property goes into the compiler's form-property documentation table in `crates/cobolt-compiler/src/lib.rs`
  (`system_documentation()`), and `assets/knowledge/chunked.data` is regenerated in the same change
  (`cargo run -p cobolt-ide --example build_chunked_kb`).

### 1.2 Fixed memory has a ceiling (R7, AC5)

In `compile_forms`, after `end` is known (`module.rs:316-333`), the generator checks
`data_pages + (single-memory ? arena_pages : 0) ≤ 65 536` (the 32-bit limit), and also checks a product limit
`MAX_DATA_PAGES` that is not decided yet (§8 Q-17b). If the check fails, it returns a `GenError` that names **the form and its
largest item**, with `region_size × instances`, so the developer knows to lower `Instances` or the table. `layout.rs` already refuses
an item it cannot size: "needs a PICTURE" (`:766`), "more than 38 digits" (`:773`), an unsupported `USAGE` (`:809`). AC5's "fails to
compile with the item named" is the test of those refusals plus the new one. The "no `memory.grow`" half is already enforced by
`bundle_problems`. A generator test asserts `scan_module(..).has_memory_grow == false` for every module the corpus produces
(WASM-010 `tests/corpus.rs`).

### 1.3 One payload limit, and "multi-memory only" (R8, R9, AC6, AC7)

- **Payload limit:** `WebSettings` and `MobileSettings` gain `max_payload_bytes` (serde default **384 KiB**, the player's value
  today, so behaviour doesn't change). From it:
  - the generator's `Arena { frame }`. `Arena::batch` and `answer` stay 64 KiB, as in ABI §2;
  - the player's `net::Config.max_frame`, through `config.json`, which the assembler writes;
  - the value the backend is told to accept. The backend still enforces its own limit (WASM-005 R11, WASM-003 R16).

  The manifest's `limits` already carry the sizes, and the loader and player read them from there. *Ownership:* the setting and its
  plumbing are this plan's. The frame codec stays WASM-010's.
- **"Multi-memory only":** `WebSettings` gains `single_memory_fallback: bool` (serde default **true**). `module_variants()` drops the
  single-memory module only when it is `false` **and** the matrix has no WebKit. If it is `false` while Safari or the iOS web view is
  in the matrix, Check and Build **refuse** with a typed error naming the browsers (AC7). It never drops the module silently.
  - iOS (`MobileSettings::module_variants`, `project_kind.rs:366-371`) already always carries single-memory, so R9's iOS half holds.
  - The IDE's Project Settings gain the checkbox, and the refusal gets a message. Both are `Tr` keys in six languages, beside
    `pk_set_multi_memory` (`crates/cobolt-ide/src/i18n.rs:391-392`).
- AC6's section scan and "same host API" are covered by `bundle_problems` (`scan.rs:174-253`) and by the four core cells. This plan
  points those cells at a generated module (§1.6).

### 1.4 Optional accelerations and the revisit record (R3, R6, R12, R18, Q7)

- **The probe:** the loader probes, and reports in `choice.reasons`, two optional features:
  - **JS String Builtins:** `WebAssembly.validate` of a module importing `wasm:js-string` with `{builtins: ['js-string']}`;
  - **ES-module source imports:** detected without module syntax, so a browser without them doesn't fail to parse.

  The probes go into the loader that **WASM-002 C1 generates from Rust** (`crates/cobolt-web-bundle/src/hostpage.rs`, new there), so
  this plan adds no JavaScript file.
- **Use (decision D2, §4):** neither feature is *used* in v1, and the reason is technical, not deferral:
  - Text crosses as UTF-8 bytes in the arena and is decoded **once** by `TextDecoder` and encoded **once** by `TextEncoder`. That is
    already R12's "once in each direction". Builtins remove a copy only when Wasm code builds JS strings itself, and neither the
    generated module nor the player's boundary does.
  - ES-module integration would replace `compileStreaming`, which already streams.

  R12 says "may", and R3 says "never required". The probe results are logged, so whoever checks Q7 sees the support in real browsers.
- **Q7 record:** a section "Revisit" in `specs/WASM-PROGRESS.md` (written at implementation, not by this run). It holds the trigger
  (native Component Model and ES-module integration in Chrome, Firefox and Safari) and the developer who checks it, and it is filled in
  each time the matrix is re-confirmed.

### 1.5 The exposure list (R14, AC11)

A constant `EXPOSED: &[&str]` in `crates/cobolt-web-player/src/api.rs` lists every `#[wasm_bindgen]` type the player and its parts
export. A test in `crates/cobolt-web-player/tests/sinks.rs` (or a new `exposed.rs`) parses the crate's sources and the parts'
`src/lib.rs` for `#[wasm_bindgen]` on a `struct`/`impl`. It fails when one is missing from `EXPOSED`, or when a listed type has no
`free()` call path from `unmount()`. When WASM-018 adds graphics handles, it adds them to the same list. The node scenario
"unmount" asserts that, after `unmount()`, every listed object reports freed (the `wasm-bindgen` pointer is 0).

### 1.6 The R20 matrix on generated modules, with the player axis (R20, AC1–AC14)

- **Program module:** the four core cells (`harness.rs:286-290`) run against a **generated** project module as well as the fixture.
  This happens once WASM-010 WP2 exports the form functions and WASM-002's `WasmBuild` exists. The generated project is the same
  scenario project written in COBOL: buttons GO, ECHO, QUERY, RAW, BURST, REMOTE, YIELD, TRAP, ERR and a child form. It lives under
  `crates/cobolt-web-bundle/tests/projects/core/` as `.wcfrm` + Common Code, so both modules are held to one script.
- **Player axis:** the harness gains `player ∈ {plain, threaded}`, giving **8 cells per module**. Node can load the threaded player
  only if its `SharedArrayBuffer` + Worker shim runs `wasm-bindgen-rayon`'s helpers, and this session didn't verify that. *Working
  plan:* try it in node first. If it can't run there, the threaded cells run in **Chrome** through the existing browser runner
  (WASM-015), and the ledger records them as browser cells with the command, never claimed from node.
- **AC13 loop:** BTN-YIELD runs **10 000 000** iterations in the generated project. The deadline is "every gap under the yield
  budget + 16 ms", with the yield budget from Q2's open half (§8). The existing check `maxGap < 50` stays the fixture's.
- **AC9 conversions:** the loader gets a debug counter (`stats().decodes`/`encodes`, debug builds only). The test asserts
  one decode per answer text and one encode per argument.
- **The INITIAL/CANCEL test (AC4):** this one is a `cobolt-wasm` test (`tests/calls.rs`), not a browser cell. A called program
  declared `INITIAL` returns its first value on every call. A plain program counts up, and after `CANCEL` it counts from the
  start. It runs on both engines.

## 2. Affected crates / files

- `crates/cobolt-forms/src/model.rs`: `Form.instances`, `Form::effective_instances`. `src/xml.rs`: the optional attribute,
  read and written.
- `crates/cobolt-semantic` or the Check path that WASM-002 builds (`crates/cobolt-project-tools/src/web/build.rs`): the 1–64 range
  check, with the form named.
- `crates/cobolt-wasm/src/module.rs`: the memory ceiling (§1.2); `Arena` taken from the settings.
- `crates/cobolt-compiler/src/project_kind.rs`: `max_payload_bytes`, `single_memory_fallback`, and the refusal in
  `module_variants` (returning `Result`, or a separate `variant_problem()` that Check calls; decided at `/tasks` by reading the
  callers).
- `crates/cobolt-web-bundle/src/lib.rs` (`config.json`: `max_frame`) and `src/hostpage.rs` (WASM-002's new file: the two optional
  probes, the debug conversion counter).
- `crates/cobolt-web-bundle/tests/support/harness.rs`, `tests/e2e_node.rs`: the player axis and the generated project; the `passed`
  floor raised to the new count. `tests/projects/core/` (new): the scenario project in COBOL.
- `crates/cobolt-web-bundle/tests/generated_host_page.rs` (WASM-002's new test): the no-cached-view search (AC10).
- `crates/cobolt-web-player/src/api.rs`: `EXPOSED`; `force_panic_for_test` gated (§5). `src/net.rs`: `max_frame` from the
  configuration (already a field). `tests/exposed.rs` (new).
- `crates/cobolt-web-player/examples/build_player.rs`: the `wasm-opt` level follows `opt-level` (`-Os` for `"s"`); the nightly
  constant changes per WASM-016 C6. One edit, owned there and listed here only so that it isn't made twice.
- `crates/cobolt-wasm/tests/calls.rs`: the AC4 test. `tests/memory_limits.rs` (new): AC5.
- `crates/cobolt-ide/src/i18n.rs`: `Tr` keys ×6 (EN/ES/PT/JA/ZH/FR) for the "single-memory fallback" checkbox and hint, the
  multi-memory-only refusal, the `Instances` label and hint (if the panel translates labels), and the out-of-range message.
  `src/panels/…` Project Settings (where `pk_set_multi_memory` is drawn) and the Properties panel.
- `crates/cobolt-compiler/src/lib.rs` (System KB): `Instances`; "in a browser, a called program keeps its storage until `CANCEL`
  or until its form instance closes; `INITIAL` starts fresh"; the payload limit. Then regenerate `assets/knowledge/chunked.data`.
- `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`: `z` bump per commit (feature).
- **Not touched:** `README.md`, anything under `docs/` (2.x.x rule). Generated `.cbl` and its banner are unchanged.

## 3. Data / model changes

- **`.cfrm` / `.wcfrm` / `.mcfrm`:** one optional form attribute, `Instances`. It is absent in every existing file and absent after a
  round trip when unset, so it is backward compatible. A desktop IDE of the 1.x.x line reading a 2.x.x form ignores an unknown
  attribute; `/tasks` checks this in `xml.rs` before relying on it.
- **`cobolt.toml`:** `[web]` / `[mobile]` gain `max_payload_bytes` (default 384 KiB) and `[web]` gains `single_memory_fallback`
  (default true). Both have serde defaults, so old manifests read unchanged.
- **ABI v1:** no change. `limits.frame_size` already exists (ABI §10). If a rule proves incomplete, it is amended in §12 of
  `WASM-ABI-v1.md` in the same commit as the code.
- **`config.json`:** `max_frame` added (an additive key; the loader treats it as optional and falls back to the player's default).
- **AST:** none from this plan (`is_initial` is WASM-010's).

## 4. Key decisions & alternatives

- **D1 — This plan builds only what no other plan owns.** *Why:* the code of R1, R4, R11, R16 and R17 sits in files that WASM-010,
  WASM-002 and WASM-016 already plan to change. Planning them twice would produce competing tasks. *Rejected:* restating their work
  packages here.
- **D2 — JS String Builtins and ES-module integration are probed and reported, not used.** *Why:* §1.4. No copy is removed, and
  streaming already exists. *Rejected:* a second text path behind a feature check, which means two code paths to test for no
  measured gain. If a measurement later shows a copy (for example, the player building strings in Wasm), D2 is revisited with that
  number.
- **D3 — One payload setting feeds the arena, the player and the backend.** *Why:* three numbers exist today (64 KiB, 384 KiB,
  1 MiB) and R8 says one. *Rejected:* keeping the generator's own default, which would refuse a frame the player accepts.
- **D4 — `wasm-opt` level follows `opt-level`** (`-Os` with `"s"`). *Why:* Q8 chose size-with-speed; `-Oz` after `"s"` would undo
  half the ruling. *Rejected:* keeping `-Oz`.
- **D5 — "Multi-memory only" is an explicit opt-out, refused when WebKit is in the matrix.** *Why:* AC7 asks for a refusal, and an
  implicit drop would ship a bundle that fails on Safari. *Rejected:* inferring it from the matrix alone, which is today's behaviour
  and cannot express the refusal.
- **D6 — The default `Instances` is 1 for the main form and 4 for the others** (working assumption, §8). *Rejected:* scanning
  handlers for `OpenForm*` literals, because a name in a data item defeats it, and a wrong 1 traps a real application at run time.

## 5. Risks & mitigations

- **R5/AC3 against Rayon.** `wasm-bindgen-rayon` has no way to stop its pool (WASM-016 plan §4), so "an unmount terminates its
  Workers" cannot hold for the threaded player's pool. → The program Worker is terminated (already tested, `harness.rs:281`). The
  pool is created once per page and reused by a later mount. This is stated as a limit and put to the operator (§8 Q-17c). It is not
  claimed as met.
- **`force_panic_for_test` ships in production.** A page script can end the application. → Export it only when the player is built
  with a `test-hooks` feature, which `build_player` enables for the node and browser test bundles and never for `assets/web-player/`.
  The freshness record names the feature set.
- **The threaded player may not run under node.** → Chrome cells, recorded as such (§1.6).
- **The browser data is unconfirmed.** Multi-memory, JS String Builtins and threads per engine for "released from 2024": this run
  had no web access (WebSearch/WebFetch were refused), so the spec's snapshot (`caniuse.com/wf-wasm-multi-memory`: no Safari) is not
  re-confirmed. → The first `/tasks` step reads MDN and Can I Use and writes the baseline and matrix table into
  `WASM-PROGRESS.md`. If Safari of 2024+ turns out to have multi-memory, R9's WebKit rule is put to the operator; it is not changed
  silently.
- **The memory ceiling refuses a real project.** → The error names the form, the item and the arithmetic. `Instances` is the lever.
- **The fixture and the generated module drift.** → Both run the same harness script (§1.6), so a difference is a failing cell.
- **Disk and concurrent sessions.** → Targeted `cargo test -p`, `df` before sweeps, hunk-level staging, the CHANGELOG gap check from
  the ledger.

## 6. Test strategy

Every test prints a quantified line: cells run, calls counted, bytes, names.

| AC | Test | Asserts and reports |
|---|---|---|
| AC1 | `e2e_node` core cells on the generated 3-form project | one `program*.wasm` fetched, at start, streamed; the chart part fetched only when its form is first shown; fetch list printed |
| AC2 | core cells (`disposal`), plus `cobolt-wasm/tests/forms.rs` (WASM-010) | 100 open/close cycles leave `pr_dbg_live` and `pr_dbg_region_used` at baseline; instance `Instances + 1` gives `-3` and the form's name |
| AC3 | new scenario | a `PreservePreviousForm` form keeps its region (`pr_dbg_region_used` unchanged); destroying it releases it; unmount terminates the program Worker and drops the module (Rayon pool: see §5) |
| AC4 | `cobolt-wasm/tests/calls.rs::initial_and_cancel` | `INITIAL` fresh every call; plain keeps its count until `CANCEL`; both engines |
| AC5 | `cobolt-wasm/tests/memory_limits.rs` + corpus | no `memory.grow` in any module produced; an oversized table refused naming form and item |
| AC6 | `bundle_problems` on both variants of the generated project + core cells | 2 memories / 1 memory and baseline-valid; identical scenario results |
| AC7 | `project_kind` unit tests + Build test | iOS → single only; default Web → both; fallback off with Safari → refused naming Safari; fallback off without WebKit → multi only; loader picks each by probe (existing "choose" scenario) |
| AC8 | core cell "batching" on the generated module | 1000 writes → ≤ 1 flush (stated number); read after write |
| AC9 | core cell "strings" + debug counter | 1–4-byte round trip both ways; decodes = answers, encodes = arguments |
| AC10 | `generated_host_page.rs` search + core "player memory" | no `.buffer` view stored outside the call that made it; reads correct after growth |
| AC11 | `cobolt-web-player/tests/exposed.rs` + node scenario | every `#[wasm_bindgen]` type on `EXPOSED` and freed by `unmount()`; forced panic (test bundle) shows the reload action and raises the error event |
| AC12 | `cobolt-web-bundle` build report test | both variants' sizes before/after `wasm-opt`, profile (`"s"`, `-Os`), pinned tools, nightly `nightly-2026-10-22` read from `player-build.json` and printed |
| AC13 | 8 cells (player × program × mode); threaded cells in Chrome if node cannot | isolated → threaded + Worker, Rayon sort uses > 1 worker; not isolated → plain + main; identical results; 10 M-iteration loop, every gap under the deadline |
| AC14 | Chrome, the embedding page (WASM-002 R11) | non-isolated host → plain, works; isolated host → threads |
| AC15 | `csp.rs`/headers tests (exist) + Chrome | headers per documented host; a cross-origin image under `credentialless` and with CORP loads; without, a readable diagnostic |

**Owed, not measurable here:** Firefox, Safari (macOS and iOS) and the Android WebView are not installed (WASM-002 plan §6). Their
cells are listed in the ledger with the command to run them, and none is claimed.

**Manual:** the operator opens the Web template's Project Settings and sees the payload limit and the fallback checkbox in the IDE's
language. Turning the fallback off with Safari in the matrix, then pressing Build, gives the translated refusal. This goes through
the PowerRustCOBOL MCP tools, never by driving the window.

## 7. Steering compliance

- [ ] **i18n:** new IDE strings (fallback checkbox and hint, refusal, `Instances` texts, range error) in all six languages; the
  loader's own messages remain the application's texts (WASM-002 R15).
- [x] **Generated-code contract:** no change to generated COBOL or its banner. The loader probes go into WASM-002's
  Rust-generated host page, and the generated code is regenerated at every Build.
- [x] **Docs:** `README.md` and `docs/` untouched (2.x.x). What 2.x.x says about itself goes in `specs/` (ledger, ABI amendments).
- [ ] **System KB:** `Instances`, the browser `INITIAL`/`CANCEL` behaviour and the payload limit in the compiler's doc tables;
  `chunked.data` regenerated in the same change.
- [x] **Interpreter/binary parity:** no interpreter change. AC4 runs on both engines, and the generated module is held equal by
  WASM-010 R16.
- [x] **PRIME DIRECTIVE:** Rust only. The scenario project is COBOL + forms. The probes are Rust-emitted text (WASM-002 C1), and no
  `.js` file is added to the tree.
- [x] **User code is sacred:** no developer file is rewritten. `Instances` is written only when the developer sets it.
- [ ] **Fix vs feature:** **feature** on `features-2x`; each commit bumps `z` (2.0.z) with a top `CHANGELOG.md` entry.
- [x] **No "cobolt" in user-facing text;** COBOL identifiers in the scenario project are English.

## 8. Open questions (none decided here)

- **Q2, first half: the yield budget in milliseconds. Not answered** (the 2026-10-22 answer covers the Worker count only). *Working
  assumption:* keep the generator's block budget (`budget_blocks` 20 000, `module.rs:70`), which measured p95 5.2 ms per slice in
  Chrome (WASM-002 plan §6), and use **"yield budget + 16 ms"** as AC13's frame deadline. A millisecond budget would need a clock read
  in the dispatcher, which is a WASM-010 change.
- **Q5: isolation on the mobile shell. Not answered.** *Working assumption,* as in WASM-016's plan: the Mobile kind runs the plain
  player on the main thread, and its bundles are written with `Isolation::Off` (`crates/cobolt-project-tools/src/web/mod.rs:95`).
- **Q-17a: the default `Instances`** (§1.1). Working assumption: 1 for the main form, 4 for the others.
- **Q-17b: the product limit on data memory** (§1.2). The 32-bit limit is enforced regardless. Is a lower product cap wanted, and
  what value?
- **Q-17c: the Rayon pool and R5/AC3** (§5). Accept "the program Worker is terminated; the pool lives with the page", or require a
  pool that can be stopped (which `wasm-bindgen-rayon` doesn't offer; it would mean a custom pool).
- **Q-17d: the browser baseline and matrix data** (R18, Q4) are to be confirmed from MDN and Can I Use at the start of `/tasks`. This
  run had no web access.
- **Dependencies on other plans' open points:** WASM-016 Q-P1 (`cargo-leptos` for the player) decides what R17's record lists.
  WASM-010 Q-new-3 decides whether start-up is measured in the Build.
