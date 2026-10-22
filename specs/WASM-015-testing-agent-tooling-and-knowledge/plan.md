<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Testing, agent tooling and the knowledge base for the web kinds

- **Status:** draft (awaiting the operator's review, then `/tasks`)
- **Spec:** ./spec.md   **Date:** 2026-10-22   **Line:** 2.x.x (`features-2x`), a **feature**.
- **Companions:** `specs/WASM-PLAN.md` (P1–P8), `specs/WASM-ABI-v1.md` (A1–A9), `specs/WASM-PROGRESS.md` (the ledger), `specs/WASM-PARITY.md`, and the sibling plans this one serves: WASM-004, WASM-008, WASM-010, WASM-017 (each cited where it matters).
- **Binding decisions (operator, 2026-10-22, spec §7):** Q1 the browser runner is **a harness on the web view of WASM-008**; Q2 the module runner's engine is **`wasmtime`** (JIT); Q3 the ledger is **`NIST/progress.json`, extended**; Q4 the parity guard is **geometry only**; Q5 a **`WebFormsDesigner`** specialist **replaces** `FormsDesigner` for Web and Mobile.
- **Still open:** none of Q1–Q5. New questions found while planning are in §8, each with a working assumption.

## 0. Where the code stands (read 2026-10-22, `features-2x` at `cdad25cd`, 2.0.30)

Part of this spec already shipped in 2.0.20 (commits `513a8060` and `64059d31`, CHANGELOG "Merged: the IDE side of the line"). This plan covers **what is left**. Every row below was read in the file named.

| Area | What exists | Evidence | What is missing |
|---|---|---|---|
| `create_project` with a kind (R10) | `kind` (`desktop`/`web`/`mobile`), `platform`, `device`, `template`, `options` | `cobolt-project-tools/src/tools/mod.rs` (`create_project` schema; `create_request`) | nothing for R10's `create_project` |
| `build` on a web kind (R10, R12) | `build::run_web`: regenerate, `check`, then `web::backend().build`; answers `status`, `engine`, `report`, `variants`; a Mobile project says which package step it cannot do and why (iOS off macOS) | `src/tools/build.rs:69-136` | no real backend is linked (`web/mod.rs` `NotYetLinked`; WASM-002/010 WP6 link it); no **secret-scan result** in the answer; the Mobile package step waits for WASM-012 |
| `list_files` on a web kind (R10) | `kind`, `forms_folder`, the backend entry, `web_assets` | `src/tools/list.rs:21-97` | nothing for R10 |
| `check` on a web kind (R10, R18) | checks forms, `.cidx`, sources **and `backend/`** sources | `src/tools/check.rs::check_project` (`view.backend` loop, line 117) | no portable-subset error, no class D error, no layer-name error: a web form is checked exactly as a desktop one |
| `render_form` / `run_form` (R11) | desktop only: `render::run` through an injected `Renderer`; `run::run` through an injected `FormRunner` | `src/tools/render.rs`, `src/tools/run.rs`, `tools/mod.rs` `run_form_in`, `render` | no web route, no `engine` field. The rule `web-kinds` tells agents they "show the desktop engine's picture" (`content.rs`, rule `web-kinds`) |
| Two servers (R12) | the IDE's (`cobolt-ide/src/agent_kit/ide_host.rs:123`) and `rcrun mcp` (`cobolt-cli/src/main.rs:453`) both build `Shared::new().with_renderer(..).with_runner(..)`; every tool body is in `cobolt-project-tools` | files read | nothing proves they answer the same on a web project (AC9) |
| Plugin rules (R13, R18) | `APPLICATION_RULES` has **14** entries, among them `web-kinds` and `backend-secrets` | `content.rs:254` | `web-kinds` says "no file organisation (indexed files included), no SQL", which contradicts WASM-010 Q8 (in-memory INDEXED is in) and WASM-004/005 class C (SqlDatabase through the backend); nothing on layers of web forms |
| Plugin skills and reviewer (R13) | skills `powerrustcobol-web-application` and `powerrustcobol-backend`; the reviewer has a Web/Mobile check | `content.rs:1172`, `:1211`; `reviewer()` | the reviewer's check is prose about "SQL"; it names no control class |
| System KB (R14) | `system_documentation()`; section "Project kinds: Desktop, Web and Mobile"; freshness gate `prebuilt_chunked_kb_matches_the_published_documentation` | `cobolt-compiler/src/lib.rs:4983`, `:6172`; `cobolt-ide/src/grace_host.rs:4780` | the class per control (WASM-004 R12), the browser subset (WASM-010 WP3), the `WEBSOCKET` phrase (WASM-005), the styles (WASM-011), Authentication and Hyperlink (WASM-009/006) — each owned by its spec; no test that the **gathered** set is complete |
| Grace (R15) | mesh specialists `FormsDesigner`, `CodeGenerator`, `EventBinder` (`cobolt-agents/src/specialist.rs:109`), chosen by `route_specialist` (keywords); fixed agents `Form Designer Agent` + its pedantic companion (`cobolt-ide/src/agents_db.rs:100`, `:77`); Grace's planning prompt lists the registry (`grace_host.rs:2449-2498`) | files read | nothing reads the project kind: no planner line, no reviewer line, no web designer |
| Module runner (R1) | `cobolt-wasm/src/host.rs` over **`wasmi`** (feature `host`), `run_program`, `run_conformance`; NC 95/95, IF 45/45, IC 25/25 on it | `cobolt-wasm/Cargo.toml`; `WASM-PROGRESS.md` | `wasmtime` (WASM-010 WP1); form driving (`open_form`/`event`/`resume`, WASM-010 WP2); no memory read-back API |
| Node end-to-end (R8b) | the real loader + real player + fixture module under `node`, 2 variants × 2 modes, script as a Rust constant written to a temp file | `cobolt-web-bundle/tests/e2e_node.rs`; `tests/support/harness.rs:286-290` | **panics when `node` is absent** (`e2e_node.rs:22-24`), which breaks the spec's "not a prerequisite of `cargo test`"; no player axis; no generated module (WASM-017 plan §1.6) |
| Browser runner (R2) | `prc-web-preview` (wry/tao), commands `open`, `size`, `reload`, `scheme`, `devtools`, `emulate`, `close` | `cobolt-web-preview/src/main.rs:14-19`, `:260-342` | no headless mode, no `render`/`script` (WASM-008 plan §2.3 adds them); no DOM, computed-style or geometry read; no Linux build (`Cargo.toml` target gate is macOS and Windows) |
| Player hooks a runner can use | `dispatch`, `type_into`, `invoke`, `settled`, `snapshot` (properties as JSON), `errors`, `stats`; each control's root element carries `data-name` and `id` | `cobolt-web-player/src/api.rs:915-1003`; `view/controls.rs:64` | nothing — a runner can drive and read through these |
| Backend runner (R3) | in-process rig: a TLS loopback server, an issuer of one (key, JWKS, token mint), a raw WebSocket client, version-skew tests | `cobolt-server/tests/support/mod.rs` (`Issuer`, `rig`), `tests/client_contract.rs:241` | lives in one crate's `tests/`, so no other crate can use it; never starts the **built** binary (`cobolt-server/src/standalone.rs::main`, built by `cobolt-compiler/src/backend_build.rs`) |
| Control round-trips (R4) | `tests/controls/` — a standalone package, **35** files, `assert_control` prints PASS/FAIL per property | `tests/controls/Cargo.toml`, `src/lib.rs:109` | no web counterpart. `ControlType::ALL` has **62** entries (`cobolt-forms/src/model.rs:2926`), so 27 are uncovered: SideMenu, Switch, Snackbar (A); Knob, Gauge, Maps, Viewer (B); IndexedFile, KnowledgeBase, FileDropZone, WebSearch and the 16 AWS controls (C) — classes from `cobolt-web-player/src/kinds.rs::TABLE` |
| Parity guard (R5, R17) | `engine_reference_form_parity_static_vs_faces`, geometry only, desktop surfaces only | `cobolt-forms/src/render.rs:22796` | no browser side |
| Goldens (R7) | `UPDATE_GOLDEN=1` in `cobolt-codegen/tests/generated_bytes_golden.rs:122` and `example_corpus_codegen.rs:46` | files read | no web-form artefacts |
| NIST ledger (R6) | `NIST/progress.json` → `modules.NC = {compile:[95,95], execution:[95,95], assertions:{…}}`; nothing in `crates/` reads it | `jq` of the file; grep | no column for the generated module; the module's numbers are only in `WASM-PROGRESS.md`; a regression takes two commands (`nist_wasm census`, then `RCRUN=…/nist_wasm_run nist_conformance run`) |
| Bench (R8) | `cobolt-bench` (dispatch, paragraph calls, decimal math, record batch, object churn, indexed redb) on the interpreter | `cobolt-bench/src/main.rs:140-399` | no module workload, no size or start-up |
| Examples (R16) | `examples/` holds PowerAnalytics, PowerChat, PowerCloud, PowerDemo3, PowerSpatial — all desktop | `ls examples` | no Web application, no Backend |

### 0.1 Who owns what (so nothing is planned twice)

R2, R4, R5, R7, R8b and R16 **gather** checks that other specs state. This plan builds the **runners and the families**; the feature each check proves stays with its spec.

| Item | Owned here | Owned elsewhere |
|---|---|---|
| Module runner | the **contract** of R1 (stub host, scripted events, read-back of writes, output, errors, memory), the shared script format (§3.1), AC1 | `wasmtime` switch and form driving in `host.rs` — WASM-010 WP1/WP2 |
| Browser runner | the **test-side library** (build → serve → drive → read DOM, style, geometry, picture), the closed step vocabulary added to the helper (§3.2), the Linux/CI wiring, AC2 | the helper's `--headless`, `render`, `script` and the web route of `render_form`/`run_form` — WASM-008 plan §2.3 (its C5 says R9/AC8 are met there) |
| Backend runner | the shared test kit (§1.1), binary mode, AC3 | the skew matrix itself — WASM-014 AC10; the identity adapters — WASM-009 |
| R8b matrix | running WASM-017 R20's scenarios on the browser runner where `node` cannot (threaded player) | the scenarios and the generated scenario project — WASM-017 plan §1.6 |
| KB (R14) | the **completeness census** across every spec's table, and the order of regeneration | each table's content — WASM-004 R12, WASM-010 WP3, WASM-005, WASM-011, WASM-006, WASM-009 |
| Check (R10, R18) | wiring the web gates into the `check` **tool** in the same `Finding` shape | the class D rule (WASM-004 plan §1.5) and the subset table (WASM-010 WP3) |

## 1. Approach

One idea runs through the plan: **one script, three engines.** `run_form`'s steps (`set`, `event`, `wait_ms`, `read`, `open_form`; `tools/mod.rs` schema) are already what an agent writes to drive a desktop form. The module runner and the browser runner accept **the same steps**, so a test written once runs on the desktop host, on the generated module and in the web view. R5, R11, R17 and AC13 are that comparison.

### 1.1 The harness (R1, R2, R3, R9)

**Module runner (R1, AC1).** WASM-010 WP1/WP2 rewrite `cobolt-wasm/src/host.rs` over `wasmtime` and give it `open_form`/`event`/`resume` and a scripted queue. This plan adds the three things R1 asks for that WP2 does not name:
- a **stub property store** that answers `query` (`GET`/`CALLR`/`COUNT`, ABI §6) from the `SET`s the module made and from seeded values. A read then reflects the handler's own writes (ABI §6), with no player;
- **read-back** of each batch as typed commands (`cobolt_wasm_abi::batch`), of `DISPLAY` output, of `ERROR`s with unit/line/form/event (ABI §5 A4), and of **memory** (a byte range of memory 0 or the arena, and `pr_dbg_live`/`pr_dbg_region_used`, ABI A3);
- `run_script(&[Step])` over §3.1's steps, with fixed `now`/`random`, so two runs give byte-identical batches (WASM-010 R13).

AC1 is one command (§1.2, R6).

**Browser runner (R2, AC2).** Built on `prc-web-preview --headless` (Q1, decided; WASM-008 plan §2.3). The test-side library is `cobolt_project_tools::web::runner`, next to `web::preview`, which already spawns the helper and pins the protocol (`src/web/preview.rs`). That way `render_form`/`run_form` (WASM-008) and the tests use one implementation. The flow is: build a bundle with `web::backend()` (or assemble a fixture bundle with `cobolt_web_bundle::assemble_bundle`), serve it on loopback with the existing dev server (`web/devserver.rs`), open the helper headless at a preset size, run steps, collect values, page errors, COBOL errors (`cobol-error`, WASM-008 §3) and a PNG.
- **Steps (closed vocabulary, §3.2).** The run_form steps, plus four read-only steps a test needs: `text` (an element's text), `style` (one computed property), `rect` (an element's border box in CSS pixels) and `snapshot` (the player's `snapshot()` JSON). Elements are addressed by **control name** (`[data-name=…]`, `view/controls.rs:64`) or by a fixed list of player roles. There is **no `eval`**: the helper ships with the IDE, and a free-form script channel would be a code-execution path in a shipped binary.
- **No browser to install** (R2, AC2). The engine is the system web view: WKWebView (macOS), WebView2 (Windows 10/11), WebKitGTK 4.1 (Linux, a system package, WASM-008 C6). When the helper or a display is missing, each browser test **prints why and returns** (`skipped: no web view — <reason>`). A developer who never touches the web kinds runs `cargo test` with no new prerequisite. CI sets `PRC_REQUIRE_WEBVIEW=1`, which turns a skip into a failure there, so the gate cannot silently go green.
- **Which engine produced it** is part of every answer (`engine`: `webkit` / `webview2` / `webkitgtk`, plus the engine's version string read from `navigator.userAgent`), for R11 and for the ledger.

**Backend runner (R3, AC3).** The in-process rig in `cobolt-server/tests/support/mod.rs` already does most of R3 (TLS loopback server, issuer of one, client, skew tests). It moves, unchanged in behaviour, to `cobolt_server::testkit` behind a non-default cargo feature `testkit`. `cobolt-server`'s own tests then use it from there, and so can any other crate as a dev-dependency. Its crypto stays RustCrypto (`p256`, `rsa`, `rcgen` without `ring`), as the file's header requires, so `tests/dependency_guards.rs` keeps holding. Two modes:
- **in-process** (default, fast): `Rig`, as today;
- **binary**: the backend built by `cobolt_compiler::backend_build` from a project's `backend/`, started on `127.0.0.1:0` with a test `backend.toml` that trusts the testkit's issuer through a JWKS file. It is a release-mode run, so no `unsafe_*` (`standalone.rs` doc). It needs a `cargo build`, so it runs only when `PRC_WEB_E2E=1` (§5, disk and time), and AC3/AC12 name the command.

**Quantified reports (R9).** Every new test prints one summary line in the shape `tests/controls` uses (`→ N passed, M failed`), plus what it measured: steps run, ms per step, host calls, bytes, pixels compared, the engine. A skip prints its reason. Nothing asserts a number the run did not produce.

### 1.2 The test families (R4–R8b)

**R4, AC4 — web property round-trips.** A new crate, `crates/cobolt-web-tests` (test-only, `publish = false`), holds `tests/controls_web.rs`. For every control whose class is A or B in the shared class table (`cobolt_forms::web`, moved there by WASM-004 plan C1; read today from `kinds.rs::TABLE`), it builds a one-control web form. It sets each property `assert_control`'s list covers to a distinct value with a `set` step, and reads it back from **what the browser shows**: `text`, `style` or `rect` for a visual property, `snapshot` for one with no visual. The output format matches `tests/controls` (a PASS/FAIL line per property and a total per control).
- **The 27 uncovered controls** get a Rust table in the same crate, `COVERAGE: &[(&str, Decision)]` with `Decision::{CoveredNow, KnownGap(reason)}`. A test fails when a `ControlType::ALL` entry has no row, in the WASM-010 `builtin_support` pattern. Working decisions (operator to confirm in review): **CoveredNow** for the seven of class A/B (SideMenu, Switch, Snackbar, Knob, Gauge, Maps, Viewer); **KnownGap** for the twenty of class C, with the reason "runs through the backend; its round-trip is WASM-005's backend-call test". The desktop `tests/controls/` files for the seven are added alongside, so the desktop family stops lagging too.

**R5, R17, AC5, AC13 — the parity guard, extended (geometry only, Q4).** Two reference forms live in `crates/cobolt-web-tests/fixtures/`: `parity-all-classes.wcfrm` (one control of every class A/B type) and `parity-layers.wcfrm` (R17's list: a control in a layer above a base control, a hidden layer, an opaque and a transparent layer background, a DataGrid at a corner, a tab order crossing layers).
- **Desktop side:** the per-control rectangles the desktop engine lays out at a given window size. This comes from `cobolt-forms` with `render` (the same layout `engine_reference_form_parity_static_vs_faces` compares), exposed as a small `layout_rects(form, window) -> Vec<(name, Rect)>` if no such function is public yet. `/tasks` checks first.
- **Browser side:** a `rect` step per control.
- **Compared:** x, y, w, h per control, within **1 CSS pixel**. The guard is geometry-only by the operator's decision, so no colour is compared. The tolerance is the plan's to set (R5). It starts at 1 px, the ledger records what is measured, and the number is not loosened to pass.
- **Allowed differences:** each is a row in a Rust table, `DOCUMENTED: &[(control, property, spec_ref)]`, which must cite WASM-004 (R17/R20 for layers). Any other difference fails and is printed control by control.
- **For the layered form (AC13):** the same script runs on the desktop host (`run_form`'s runner) and in the browser. It reads each layer's visibility, a hit test (an `event` at a point under a layer: which control received it), and the **tab order** (repeated `key Tab` steps, reading `document.activeElement`'s `data-name`, which is a fifth read-only step, `focused`). The results are compared in order.

**R6, AC1, AC6 — the ledger, extended (Q3).** Each module in `NIST/progress.json` gains a sibling object `wasm` = `{compile:[n,N], execution:[n,N], assertions:{pass,fail,deleted}, engine:"wasmtime <version>", excluded:[{program, construct}], measured_at_version}`. The interpreter's fields are untouched and the `protected_baselines` rules unchanged. NC, IF and IC are the first rows, from the measured numbers in `WASM-PROGRESS.md` once WASM-010 WP1 re-measures them on `wasmtime`.
- **One command:** `cargo run -p cobolt-wasm --example nist_wasm -- regress NC IF IC` runs the census and the execution pass for each module on the module runner. It prints, per module, **two numbers** plus the assertion tally and the excluded programs with their reasons. Then it runs the interpreter's execution pass and prints its two numbers beside them. It reuses the code of `census` and of `nist_wasm_run`, not a third copy.
- **Writing the ledger:** `--record` writes only the `wasm` objects and is never the default. The ledger is read by the `nist-grind` skill (`.claude/skills/nist-grind`), which is updated in the same change to read the new column.

**R7, AC7 — goldens for web output.** `crates/cobolt-web-tests/tests/web_golden.rs` snapshots, for a fixed web project in `fixtures/`:
- the generated `.cbl` of a web form (through `cobolt-codegen`, like the desktop goldens);
- the backend's interface description (`cobolt_server::dev::write_interface`);
- the form stylesheet (WASM-007);
- the host page, `config.json` and the generated CSP (`cobolt-web-bundle`, `csp.rs`);
- the manifest (`prmanifest`).

The switch is the existing `UPDATE_GOLDEN=1`. Each golden that is missing fails **on its own**: every file is compared before the test fails, so the problem the desktop snapshot test has (it stops at the first missing golden) does not repeat.

**R8 — bench.** `cobolt-bench` gains three rows under `--features wasm`, off by default so the bench's normal build does not pull `wasmtime`:
- `module_size` (bytes of `program.wasm` and `program.single.wasm` for a fixed project);
- `module_startup` (`pr_init` + `pr_open_form(main)` on `wasmtime`, reported as **native, not a browser figure**, as WASM-010 D5 states);
- the existing `decimal_math` and a string workload run **on the module and on the interpreter** side by side.

No budget is set; the performance spec (WASM-029) is not written yet.

**R8b — lifecycle, memory and boundary matrix.** The scenarios are WASM-017's (plan §1.6), run by its node harness. This plan makes three changes there:
- `e2e_node.rs` **skips with a printed reason** when `node` is absent, instead of panicking (`e2e_node.rs:22-24`). CI requires it with `PRC_REQUIRE_NODE=1`.
- The cells `node` cannot run (the **threaded** player, if its Worker shim cannot load `wasm-bindgen-rayon`) run on the browser runner, under the cross-origin-isolated headers the dev server already sends.
- The WASM-016 R13 / WASM-017 R16 **size report** becomes a test that prints the size per part and fails when a part grows past the previous recorded size by more than a threshold kept in the ledger. The threshold is a working number (§8).

The ledger records each cell as node, webkit or webview2. A cell is never claimed from an engine that did not run it.

### 1.3 Agent tooling (R10–R13)

- **`check` (R10, R18).** `check::check_project` and `check_one`, for a form under `webforms/`/`mobileforms/`, add three gates with the same `Finding` shape (file, line, form, site, severity, message):
  1. the class D gate (WASM-004 §1.5);
  2. the subset refusals of the generator (WASM-010 WP3's `support`/`builtin_support`, called without emitting a module);
  3. **a name used as `NAME::Property` that is neither a control nor a layer of the form** (R18), and a write to `Layer::Name`.

  The gates come from the owning crates. This plan only calls them from the tool and maps their kinds to `Finding`, so Check in the IDE and `check` through MCP say the same thing.
- **`build` (R10).** The answer gains `secret_scan: {scanned_files, findings}` from `cobolt-web-bundle/src/secrets.rs`, so an agent sees the scan ran even when it found nothing. It also gains the bundle sizes by part (`report` already has them once the real backend is linked). Mobile keeps today's honest "cannot do X here because Y" (`build.rs:126-136`) until WASM-012 replaces it.
- **`regenerate` and `validate` (R10).** `regenerate` on a web form regenerates the same `.cbl` it does today. `validate` on a `.cfrm` under `webforms/` also runs the class D and layer gates.
- **`render_form` / `run_form` (R11).** The route is WASM-008's. This plan holds it to R11's shape:
  - the desktop fields **plus `engine`** (and its version);
  - a web form never falls back to a desktop picture. With no web view, the answer is an error that names what is missing;
  - the rule `web-kinds` is rewritten to say so, replacing "they show the desktop engine's picture".
- **Same answers from both servers (R12, AC9).** Every web route lives in `cobolt-project-tools`, so both servers get it. The only per-server input is where the helper is, `web::preview::helper_path`. A test in `cobolt-project-tools/tests/kinds.rs` builds two `ProjectTools` the way `agent_kit/ide_host.rs:123` and `cobolt-cli/src/main.rs:453` do, with the same stub `WebBuildBackend`. It runs the AC8 call sequence on both and compares the JSON answers field by field, ignoring `elapsed_ms`. A kind a tool cannot serve answers `{status:"unsupported", reason}` (iOS off macOS is the existing example).
- **Plugin rules, skills and reviewer (R13, R18, AC10).**
  - `web-kinds` is regenerated from the tables instead of prose:
    - what a web form may not contain: class D, by name, from `cobolt_forms::web`;
    - what runs only through the Backend: class C;
    - the browser subset from WASM-010's table, which now includes in-memory INDEXED files (Q8 of WASM-010);
    - that `render_form`/`run_form` use the web view.
  - `backend-secrets` keeps its text.
  - A new rule, `web-layers` (R18), says:
    - layers on a web form work as on the desktop;
    - `Non-Visuals` is a design-time tab;
    - a layer is addressed as `LAYER-NAME::Property` and is never a control;
    - `check` reports a layer name that does not exist.
  - The two skills exist. They gain the layer paragraph and the `check` gates.
  - The reviewer's Web/Mobile check names the controls by class: a class D control on a web form is rejected; a class C control (SqlDatabase among them) is accepted **only** when it holds a backend configuration name and no connection string, key, token or password (WASM-005 R14). See §8 Q-new-1 for the spec's wording.
  - `APPLICATION_RULES` grows from 14 to 15. Three places pin rule text and need re-reading: `tests/tools.rs:552`, `tests/layout_rule.rs`, and `cobolt-ide/src/panels/beautify.rs:2440`.

### 1.4 Knowledge (R14–R16)

- **R14 — gathered, then proved complete.** Each table is written by its own spec into `cobolt-compiler/src/lib.rs` (see §0.1). This plan adds one test, `web_knowledge_is_complete` (in `cobolt-compiler`), which builds the expected set **from the code**:
  - every `ControlType::ALL` name with its class;
  - every `BUILTINS` name with its browser support;
  - the `WEBSOCKET LINKAGE` phrase;
  - the two application styles (`ApplicationStyle`, `project_kind.rs:142`);
  - Authentication and Hyperlink.

  It then asserts each item appears in `system_documentation()`, and prints `N of M present` with the missing names. Once it passes, `cargo run -p cobolt-ide --example build_chunked_kb` regenerates `assets/knowledge/chunked.data` in the **same** commit, and `prebuilt_chunked_kb_matches_the_published_documentation` must be green (AC11).
- **R15 — Grace reads the kind; `WebFormsDesigner` replaces `FormsDesigner` (Q5).**
  - **Mesh specialist.** `cobolt-agents/src/specialist.rs`: `BUILTIN_SPECIALISTS` gains `"WebFormsDesigner"`, with a preamble of policy rules (what a web form may hold, from the same generated text as `web-kinds`; layers; responsive; Spatial), not keyword heuristics. The swap happens by **kind, not by words**: a new `route_specialist_for(kind, prompt)` maps the `FormsDesigner` outcome to `WebFormsDesigner` when the kind is Web or Mobile. The call site in `cobolt-ide/src/llm.rs:1771` passes the open project's kind. `route_specialist` keeps its contract for Desktop.
  - **Fixed agent.** `cobolt-ide/src/agents_db.rs`:
    - a fixed `Web Form Designer Agent` with its default prompt, `DEFAULT_WEB_FORM_DESIGNER_AGENT_PROMPT`, in `llm.rs` (edit DEFAULT_* constants only; the LEGACY prompts are load-bearing);
    - its own pedantic companion, because companions are one-to-one (`grace_host.rs:2498`, "PEDANTIC COMPANION CONTRACT");
    - membership in `produces_form_change_set` (`agents_db.rs:114`). That set must stay in lockstep with the apply path, or approved change sets are discarded.
  - **Grace's planning prompt** gains a `PROJECT KIND:` block next to `SURFACE CONTEXT`, which says what each kind may use. The registry it lists (`grace_host.rs:2449`) shows `Web Form Designer Agent` instead of `Form Designer Agent` on a Web/Mobile project, and the reverse on Desktop. The pedantic Grace reviewer's default prompt gains the same kind rule.
  - **Context.** The `CONTROLS:` header and its slicing (`agent.rs:2848`) are not renamed. A new header goes **above** the layers block, so the existing slicing is unaffected.
- **R16 — examples that are tests (AC12).**
  - `examples/PowerWebHello/` is a Web project: one form, a button that calls a Backend program through `CALL … WITH WEBSOCKET LINKAGE`, a label showing the answer, a layer shown by the program.
  - Its `backend/` exports that program and reads an in-memory or `redb` indexed file on the Backend.
  - `crates/cobolt-web-tests/tests/examples.rs` builds it with the real Build, runs the backend runner (in-process by default, binary with `PRC_WEB_E2E=1`), drives the page with the browser runner (click, read the label), and runs `cobolt_web_bundle::secrets` over the example's tree. AC12 requires zero findings.
  - The example is written against the rustcobol-code-style skill (properties as data items, inline method calls) and holds no key, password or token.
  - The memory note "example .cfrm files carry credentials" makes the credential grep a commit gate for the example too.

## 2. Affected crates / files

- `crates/cobolt-web-tests/` (**new**, workspace member, test-only, `publish = false`):
  - `tests/controls_web.rs` (R4, with `COVERAGE`);
  - `tests/parity_web.rs` (R5, R17, with `DOCUMENTED`);
  - `tests/web_golden.rs` (R7);
  - `tests/examples.rs` (R16);
  - `tests/servers_agree.rs` (AC9, if it does not fit `kinds.rs`);
  - `fixtures/` (the reference forms, the golden project, goldens).
- `crates/cobolt-project-tools/src/web/runner.rs` (**new**): the browser-runner library. `src/script.rs` (**new**): the shared `Step` type of §3.1, which `tools/run.rs` then parses with. Also changed:
  - `web/mod.rs` (`mod runner`);
  - `web/preview.rs` (the §3.2 steps on the IDE side of the protocol);
  - `tools/check.rs` (web gates, R10/R18);
  - `tools/build.rs` (`secret_scan`);
  - `tools/validate.rs` (web gates);
  - `content.rs`: `web-kinds` generated from the tables, the new `web-layers` rule, the reviewer's class-based check, the two skills' layer and Check paragraphs;
  - `tests/kinds.rs` (AC8, AC9).
- `crates/cobolt-web-preview/src/main.rs`: the read-only steps `text`, `style`, `rect`, `snapshot`, `focused`, `key` inside `script` (§3.2), next to WASM-008's `render`/`script`.
- `crates/cobolt-wasm/src/host.rs`: stub property store, memory read-back, `run_script` (on top of WASM-010 WP1/WP2). Also `examples/nist_wasm.rs`, which gets `regress` and `--record`.
- `crates/cobolt-server/src/testkit.rs` (**new**, feature `testkit`), moved from `tests/support/mod.rs`, plus binary mode. `Cargo.toml` adds the `testkit` feature, and the RustCrypto dev-dependencies become optional dependencies behind it. `tests/*.rs` import from `cobolt_server::testkit`.
- `crates/cobolt-web-bundle/tests/e2e_node.rs`: skip with a reason instead of panicking; the `PRC_REQUIRE_NODE` gate; the size-report test.
- `crates/cobolt-bench/src/main.rs`, `Cargo.toml`: the `wasm` feature and its three rows.
- `crates/cobolt-compiler/src/lib.rs`: the `web_knowledge_is_complete` test only. The tables themselves come from the owning specs.
- `assets/knowledge/chunked.data`: regenerated in the same commit as any doc-table change.
- `crates/cobolt-agents/src/specialist.rs`: `WebFormsDesigner`, `route_specialist_for`.
- `crates/cobolt-ide/src/llm.rs`:
  - `DEFAULT_WEB_FORM_DESIGNER_AGENT_PROMPT` and its pedantic prompt;
  - the kind line in `DEFAULT_GRACE_PROMPT` and `DEFAULT_PEDANTIC_GRACE_PROMPT`;
  - the routing call at `:1771`.
- `crates/cobolt-ide/src/agents_db.rs`: the fixed agent, its companion, `produces_form_change_set`, `is_fixed_agent_name`, `ensure_*`.
- `crates/cobolt-ide/src/grace_host.rs`: the `PROJECT KIND:` block and the registry filtered by kind.
- `crates/cobolt-ide/src/agent.rs`: the kind header above the layers block.
- `crates/cobolt-ide/src/i18n.rs`: **no new key expected** (agent names, tool answers and prompts are English, like the existing fixed agents). If `/tasks` finds an IDE-visible string, such as the agent's description in the Agents dialog, it becomes a `Tr` key in all six languages.
- `NIST/progress.json`: the `wasm` objects (by `--record`, not by hand).
- `.claude/skills/nist-grind/SKILL.md`: reads the `wasm` column. *One rule lives in three files*: if the skill's rule is also in `specs/steering` or CLAUDE.md, all three change together.
- `examples/PowerWebHello/` (**new**): a Web project with `backend/`.
- `.github/workflows/build-all-platforms.yml`: the web-test job.
  - Linux: Xvfb and `libwebkit2gtk-4.1`, with `PRC_REQUIRE_WEBVIEW=1`.
  - macOS and Windows: the same variable.
  - `node` with `PRC_REQUIRE_NODE=1`.
  - The binary-mode job with `PRC_WEB_E2E=1`.
- `specs/WASM-PROGRESS.md`: measured numbers, engine per cell.
- **Not touched:** `README.md` and anything under `docs/` (2.x.x rule). `tests/controls/` gets only the seven new desktop files of R4.

## 3. Data / model changes

### 3.1 The script (shared by the three engines)

The steps are the existing `run_form` objects (`{"set":{control,property,value}}`, `{"event":{control,name}}`, `{"wait_ms":n}`, `{"read":{control,property}}`, `{"open_form":name}`), with no change to their JSON. Rust gets one type for them, `cobolt_project_tools::script::Step`, parsed once. `run::run` switches to it in place of its own parsing. The module runner takes `Step` through a `cobolt-wasm` dev-dependency, so `cobolt-wasm` does not depend on project-tools at run time. The answer shape is the same for all three engines: `values`, `displayed`, `errors`, `png?`, `engine`.

### 3.2 The helper protocol (additive to WASM-008 §3)

The read-only steps go inside WASM-008's `{"cmd":"script","steps":[…]}`:

- `{"type":"text","control":N}`
- `{"type":"style","control":N,"property":"color"}`
- `{"type":"rect","control":N}` → `{x,y,w,h}` in CSS px
- `{"type":"snapshot"}`
- `{"type":"focused"}`
- `{"type":"key","key":"Tab"}`

`control` is a control name, or one of the fixed roles `form`, `layer:<NAME>`, `snackbar`. An old helper answers an unknown step with an error the runner reports as "helper too old"; it never ignores the step silently. Both sides' tests pin the lines, as WASM-008 §3 does.

### 3.3 Other changes

- **`NIST/progress.json`:** the additive `wasm` object per module (§1.2). No existing field changes. A reader that does not know `wasm` is unaffected.
- **MCP answers:** additive fields `engine` (render/run on a web form) and `secret_scan` (build). No field is removed or renamed.
- **`.cfrm`, `cobolt.toml`, generated `.cbl`, AST, ABI:** unchanged by this plan.

## 4. Key decisions & alternatives

- **D1 — One script, three engines (§3.1).**
  - *Why:* R5, R11, R17 and AC13 are comparisons, and a comparison needs the same input. Agents already write this format.
  - *Rejected:* a separate test DSL per runner, which would mean three scripts that drift apart.
- **D2 — The browser runner is the WASM-008 helper, driven by a closed step vocabulary, never `eval` (Q1 decided).**
  - *Why:* the operator's decision, and the helper ships with the IDE.
  - *Rejected:* a WebDriver/DevTools Chromium (the operator's Q1); an `eval` step (a script-injection path in a shipped binary).
  - *Cost stated:* the engine is the OS's. On this Mac the browser runner measures **WebKit only**. Chromium is measured by WebView2 on the Windows CI runner, and nothing measures Firefox (§8 Q-new-3).
- **D3 — Browser and binary-backend tests skip with a printed reason locally and are required in CI.**
  - *Why:* the spec forbids making a browser a prerequisite of `cargo test`, and a skipped gate must not look green where it matters.
  - *Rejected:* `#[ignore]`, which hides the test from the default sweep and its summary line.
- **D4 — A new workspace member `cobolt-web-tests`, not a standalone package like `tests/controls`.**
  - *Why:* the web tests need `cobolt-project-tools`, `cobolt-forms` (render), `cobolt-wasm`, `cobolt-server` and `cobolt-web-bundle`. A standalone package would build all of them into a second `target/`, which costs several GB of disk (memory: a worktree sweep already links ~190 binaries).
  - *Rejected:* spreading the tests over the five crates, because then no crate owns the cross-crate fixture.
- **D5 — The backend test kit becomes a feature-gated module of `cobolt-server`.**
  - *Why:* R3 is reused by WASM-014 AC10, WASM-009 and R16, and today it is visible only to one crate's `tests/`.
  - *Rejected:* copying `support/mod.rs`, because two issuers of one would drift.
- **D6 — The ledger gets a `wasm` sibling object per module, written only by `--record` (Q3 decided: extend the file).**
  - *Why:* no existing reader breaks, and the `nist-grind` skill's protected baselines keep their meaning.
  - *Rejected:* a second file (the operator's Q3); writing on every run (a test sweep would rewrite the ledger).
- **D7 — Geometry tolerance 1 CSS px, colour not compared (Q4 decided).** The number is the plan's; the measured spread goes into the ledger, and if it is larger the difference is investigated, not absorbed.
- **D8 — The swap to `WebFormsDesigner` is by project kind in code (Q5 decided), not by prompt words.**
  - *Why:* the policy-first practice (model quirks and rules in policy, never in word heuristics), and the spec's own R15 wording.
  - *Rejected:* a third keyword list in `route_specialist`.
- **D9 — The web-kinds rule text is generated from the class and subset tables.**
  - *Why:* the rule today contradicts two decided specs (§0). Generated text cannot drift, and the KB census (R14) and the reviewer read the same source.
  - *Rejected:* hand-editing the prose.

## 5. Risks & mitigations

- **The runners depend on unlanded work.** This plan needs `wasmtime` and form driving (WASM-010 WP1/WP2), the helper's headless mode (WASM-008 §2.3), the real Build link (WASM-002/WASM-010 WP6) and the class table in `cobolt-forms` (WASM-004 C1). → `/tasks` orders each AC after its dependency and runs each family on a **fixture** bundle first, the way `cobolt-web-bundle/examples/fixture_bundle.rs` does today. Nothing is claimed end to end before the link exists.
- **A hidden window may not paint** (WebView2 suspends invisible windows). → WASM-008's mitigation: a window placed off every screen. If an engine still refuses, the runner reports an error naming the engine, never an empty picture.
- **The Linux CI image** needs a display and WebKitGTK. → Xvfb plus `libwebkit2gtk-4.1` in the job; `PRC_REQUIRE_WEBVIEW=1` makes a missing piece fail there.
- **The binary-mode backend** needs a `cargo build` of the backend (minutes, GBs). It also needs the issuer's JWKS trusted without `unsafe_*`, because a built binary refuses those. → The testkit writes the JWKS to a file the test `backend.toml` names as a local key set; if the config does not allow a file key set, `/tasks` reports that and asks before adding a setting. The job is gated by `PRC_WEB_E2E=1` and runs once per CI job, not per test.
- **Engine coverage.** WebKit only on this Mac, Chromium only on Windows CI (D2). → The ledger records the engine of each cell; a Chromium claim made from a Mac is forbidden by the same rule as WASM-017's node cells.
- **Flaky geometry from fonts.** Text metrics differ between engines and font stacks. → Geometry compares **control rectangles**, which the form sets, not text runs. WASM-004 §1.6 bundles the built-in face, so the font is the same one. A text-dependent auto-size is listed in `DOCUMENTED` with its spec reference, or fixed.
- **Grace regressions.** The fixed-agent machinery repairs stale prompts (`agents_db.rs:3985-4005`). A new fixed agent must not look "stale" to the detector, or it is rewritten on every open. → The existing loop "no shipped default may look stale" gains the two new prompts. `produces_form_change_set` gets a test with the new agent on both sides.
- **The rule count changes (14 → 15).** Tests and the beautify panel read rules by id. → Lookups go by id, not position (`tests/tools.rs:552` iterates); the CHANGELOG states the count, which an earlier entry miscounted.
- **Disk.** 53 GB free on 2026-10-22 (`df`). `wasmtime` and its Cranelift crates, the web-view crates on three OSes, and the new test crate enlarge `target/`. WASM-010 WP1's probe records `wasmtime`'s clean-build size. → Run targeted tests (`-p cobolt-web-tests`, `-p cobolt-project-tools`), run `df` before a sweep, clone `target/` with `cp -cRp` for a second worktree, and never `cargo clean` a shared one. `/tasks` records the measured growth of `target/` in the ledger, as the spec's §6 asks.
- **Concurrent sessions on `features-2x`.** → Hunk-level staging; the CHANGELOG gap check from the ledger's operating rules.

## 6. Test strategy

Every test prints what it exercised and measured (R9); browser and binary tests print their skip reason. Sweeps use `--no-fail-fast` and every `test result:` line is read; `cobolt-forms` needs `--features render`, `cobolt-ide` needs `--bin cobolt-ide`.

| AC | Test (crate) | Asserts and prints |
|---|---|---|
| AC1 | `cargo run -p cobolt-wasm --example nist_wasm -- regress NC` | two numbers per module on `wasmtime`, the assertion tally, the excluded programs with reasons, the interpreter's numbers beside them; equal to the ledger's last record or a printed difference |
| AC2 | `cobolt-web-tests/tests/runner.rs::hello_world_bundle` | build (or fixture) → serve → open headless → `event` click → `text` of a label, `style` `color`, a PNG of non-zero size; prints engine and version, ms per step; skips with a reason without a web view |
| AC3 | `cobolt-server` (feature `testkit`) `tests/runner.rs` | in-process: start, sign a test user in against the issuer, one call, one skew call (client one minor ahead, then behind), clean shutdown (no task left, port released); with `PRC_WEB_E2E=1` the same against the built binary |
| AC4 | `cobolt-web-tests/tests/controls_web.rs` | per class A/B control: PASS/FAIL per property and a total; `COVERAGE` has a row for each of the 62 types, and the test fails on a missing row |
| AC5 | `tests/parity_web.rs::all_classes` | per control, desktop rect vs browser rect within 1 px; prints the max difference; fails on anything not in `DOCUMENTED` |
| AC6 | `nist_wasm regress NC IF IC --record`, then `jq '.modules.NC.wasm'` | `NIST/progress.json` has the three `wasm` rows; the interpreter fields are byte-identical to before (`git diff` shows only `wasm` objects) |
| AC7 | `tests/web_golden.rs` | five artefacts compared, all reported before failing; `UPDATE_GOLDEN=1` rewrites them; a second run is green |
| AC8 | `cobolt-project-tools/tests/kinds.rs::web_tool_sequence` | `create_project` (kind web) → a form with a class D control and a file verb → `check` gives two errors with line and form → fix → `build` gives `built` with `secret_scan` → `render_form` gives a PNG and `engine` → `run_form` with a click and a read gives the value and `engine` (the browser parts skip with a reason without a web view) |
| AC9 | `kinds.rs::both_servers_agree` | the AC8 sequence on two `ProjectTools` wired as the IDE and `rcrun` wire them; JSON equal field by field except `elapsed_ms` |
| AC10 | `cobolt-project-tools/tests/tools.rs` + `content.rs` tests | the rules include `web-kinds` (generated: every class D name present), `backend-secrets` and `web-layers`; the reviewer's checks name class D and the class C condition; both skills exist in `kit()`; `bundle.rs` writes them into the plugin |
| AC11 | `cobolt-compiler::web_knowledge_is_complete` + `grace_host.rs::prebuilt_chunked_kb_matches_the_published_documentation` | `N of M` web facts present (and the missing ones by name); the store is fresh. **Grace part:** a unit test that the planning prompt for a Web project carries the `PROJECT KIND:` block and lists `Web Form Designer Agent` rather than `Form Designer Agent`; `route_specialist_for(Web, "add a SqlDatabase")` gives `WebFormsDesigner`. The model's actual refusal or reroute is a **manual run** (below), never claimed from a unit test |
| AC12 | `cobolt-web-tests/tests/examples.rs` | `PowerWebHello` builds; the backend runner starts; a click reads the backend's answer in the label; the secret scan reports 0 findings over the example's tree |
| AC13 | `tests/parity_web.rs::layers` | per step, the desktop and browser results (visibility, hit target, focused control after each Tab) equal, except the `DOCUMENTED` rows citing WASM-004 R17/R20; prints the sequence side by side |
| R8 | `cargo run --release -p cobolt-bench --features wasm` | the three rows printed with units; no assertion on a budget |
| R8b | `cobolt-web-bundle/tests/e2e_node.rs` and the browser cells | the scenario count per cell and the engine; the size report per part against the recorded size |

**Manual checks (the operator's, never an agent's).**
- In the IDE, open a Web project and ask Grace for a SqlDatabase on a web form. The expected answer follows §8 Q-new-1's working assumption: either a refusal, or a class C control holding only a backend configuration name, with the Backend program planned.
- The agent tooling part of AC8 can be repeated by a coding agent through the MCP tools, without driving the IDE's window.

## 7. Steering compliance

- [ ] **i18n:** no new IDE string is expected (agent names, prompts and tool answers are English, as the existing fixed agents and `NOT_YET_LINKED_MESSAGE` are). Any IDE-visible string `/tasks` finds becomes a `Tr` key in all six languages.
- [ ] **Generated-code contract:** the goldens and the tools go through `cobolt-codegen` and the regenerate-on-action path; nothing hand-edits `generated/`.
- [ ] **Docs:** `README.md` and `docs/` are **not touched** (2.x.x). What 2.x.x says about itself goes in `specs/` (ledger, this plan) and in the **System KB**.
- [ ] **System KB:** the census test (R14) plus the regenerated `assets/knowledge/chunked.data` in the same commit as any doc-table change; the freshness test is a real gate.
- [ ] **Interpreter/binary parity:** no interpreter change. The module and the interpreter are compared by `regress`, the parity guard and the corpus. A divergence goes to `WASM-PARITY.md`, and an interpreter bug is a 1.x.x fix in its own commits.
- [ ] **NIST (GOLDEN RULE #9):** two numbers per module, never one. The interpreter's protected baselines are untouched by `--record`.
- [ ] **PRIME DIRECTIVE:** the node harness and the helper's page-side code stay Rust string constants written to temp files (as `e2e_node.rs` does). No `.js`, `.mjs` or `.sh` file enters the repository. CI YAML is configuration.
- [ ] **User code is sacred:** the tools add gates and fields. They never rewrite a developer's form, and `check` reports without fixing.
- [ ] **Credentials:** the example and the fixtures hold no key, password, token or account id. The commit gate is a command that aborts on a hit.
- [ ] **Fix vs feature:** a **feature** on `features-2x`. Each commit bumps `z` in `crates/cobolt-ide/src/version.rs` with a top `CHANGELOG.md` entry, inserted above the previous heading. The `e2e_node.rs` panic-to-skip change is part of this feature, because it serves R2's constraint.
- [ ] **No "cobolt" in user-facing text:** the helper is `prc-web-preview`; agent names and rule texts say PowerRustCOBOL; COBOL in the example and the fixtures is English.
- [ ] **Never drive the IDE's window:** every verification here is headless or through the MCP tools.

## 8. Open questions (none decided here)

- **Q-new-1 — a SqlDatabase on a web form.** The spec's story and AC10/AC11 say the reviewer "flags a web form containing a `SqlDatabase`" and Grace "refuses or reroutes it through the backend". WASM-004's class table (spec line 65; `kinds.rs` row `SqlDatabase`, class C) and WASM-005 R14 allow it **as class C**: it holds only the name of a backend configuration, and the Build fails on a connection string. *Working assumption:* the later specs win. The reviewer flags a SqlDatabase that carries a connection string, key, token or password, or that sits on a web project with no Backend. Grace's expected behaviour is the **reroute** half of AC11. *To confirm by the operator.*
- **Q-new-2 — the decisions for the 27 uncovered controls (R4).** *Working assumption:* the 7 of class A/B are covered now, on the web and on the desktop; the 20 of class C are a known gap here, covered by WASM-005's backend-call tests. *To confirm in review.*
- **Q-new-3 — engine coverage under the web-view decision.** The runner measures WebKit on macOS and Linux and Chromium (WebView2) on Windows only; nothing measures Firefox. WASM-004, WASM-009 and WASM-013's plans still name a "Chrome harness"; with Q1 decided, their browser checks run on this runner. *Working assumption:* the Windows CI job is where Chromium cells are measured, and `node` (V8) stays for WASM-017's module/mode cells, recorded as node cells and never as browser cells. *To confirm.* Those three plans are not edited by this run; the alignment is listed for whoever updates them.
- **Q-new-4 — the new fixed agents' names.** *Working assumption:* `Web Form Designer Agent` and `Web Form Designer Agent Pedantic Reviewer` (a companion of its own, because companions are one-to-one), and `WebFormsDesigner` for the mesh specialist as the operator named it. *To confirm.*
- **Q-new-5 — the size-growth threshold of R8b.** *Working assumption:* fail when a part grows by more than 5 % over the size last recorded in the ledger; the operator may set another number or a budget (WASM-029).
- **Inherited, unchanged:** WASM-010 Q-new-2 (the player's `abi_host.rs` stays on `wasmi`); WASM-008 D7/D8 (Preview vs Run, a real local backend); WASM-009's Keycloak container (AC3 here uses the testkit's issuer, which is the "local test identity issuer" of R3, not Keycloak).
