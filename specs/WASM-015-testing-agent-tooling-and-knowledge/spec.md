<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Testing, agent tooling and the knowledge base for the web kinds

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-015-testing-agent-tooling-and-knowledge/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the first batch's list of missing specs (WASM-INDEX §5, "testing, agent tooling and the knowledge base"): conformance in a headless browser, the MCP tools and the System KB for the web kinds — accepted by the operator on 2026-10-19.
- **Depends on:** WASM-001, WASM-004, WASM-008, WASM-010. **Gathers** the test requirements the other specs state, so that the harness is built once.

## 1. Overview

The product is built and checked by three kinds of reader besides the developer: the **test suite** (what proves the product), **coding agents** through the MCP tools (what builds and verifies a project from outside the IDE), and **Grace and the specialists** with the System
KB (what generates forms and code inside the IDE). Each already works for desktop projects. For the web kinds each needs new equipment: a way to run a generated module and a page in a headless browser, tools that can check, build, render and run a web project, and knowledge that says what
exists on the web and what does not. This spec specifies that equipment; without it, every other spec's acceptance criteria are unverifiable.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **The MCP tool set** (`cobolt-project-tools/src/tools/mod.rs`) has fifteen tools: `open_project`, `check`, `regenerate`, `validate`, `build`, `render_form`, `run_form`, `list_files`, `kb_lookup`, `kb_search`, `create_project`, `add_crate`, `remove_crate`, `add_to_project`, `add_powerchat`. Two servers
  carry them: the one the IDE serves on `127.0.0.1` with an access token, and `rcrun mcp` over stdio when the IDE is closed. `render_form` returns a picture of a form as Run Form draws it; `run_form` runs a form off screen through a script (type, click, wait, read a property) and returns what it displayed,
  any run-time error and a picture. Both rely on the egui headless rasteriser (`--headless`, `cobolt-form-host/src/headless.rs`).
- **The plugin's rules for agents** are `APPLICATION_RULES`, **12** entries (`cobolt-project-tools/src/content.rs:254`) — and the plugin carries skills for creating forms, adding controls, shells and navigation, SQL, common procedures, assets, layout and themes, and gap reports.
- **The System KB** is built from Rust constants in the compiler (the property, method and event tables), published as chunked records and shipped pre-embedded as `assets/knowledge/chunked.data`; a test, `prebuilt_chunked_kb_matches_the_published_documentation`, fails when they drift. Grace and the specialists (`FormsDesigner`, `CodeGenerator`,
  `EventBinder`) read it.
- **Tests that exist and will need web counterparts:** `tests/controls/` (35 property round-trip files against 62 control types); the parity guard `engine_reference_form_parity_static_vs_faces` (`cobolt-forms/src/render.rs:22796`, geometry only); the codegen golden files (`crates/cobolt-codegen/tests/golden`);
  the NIST CCVS85 harness (`crates/cobolt-semantic/examples/nist_conformance.rs` with the ledger `NIST/progress.json`); the `cobolt-bench` baseline.
- **No browser, WebAssembly engine or WebDriver is used by any test today** (WASM-008, WASM-010).

## 2. Goals / Non-goals

- **Goals:**
  - A **headless harness** that runs a generated module (no page) and a built bundle (in a real browser engine), reproducibly, in the test suite and in CI.
  - The web counterparts of the test families above, with the conformance numbers WASM-010 asks for.
  - The MCP tools and the plugin **understanding web projects**: creating, checking, building, rendering and running them.
  - The System KB, Grace's specialists and the plugin's rules **knowing what exists on the web** and what does not.
  - Examples that double as tests.
- **Non-goals:**
  - Automating the IDE's own window (never; operator ruling, 2026-07-15 — the tools act through the product).
  - Testing the identity providers' internals or the browsers.
  - Load and soak testing of deployed backends (the operations spec proposed in WASM-INDEX).

## 3. User stories

- As a maintainer, I want one command that runs the NIST subset on the generated module and reports two numbers, so that I see the conformance at once.
- As a coding agent, I want `render_form` and `run_form` to work on a web form, so that I can verify my own work as I do for a desktop one.
- As Grace, I want to know that a `SqlDatabase` cannot sit on a web form, so that I never generate one.
- As a developer, I want the shipped example web applications to run in the test suite, so that they are never stale.

## 4. Requirements (EARS)

**The harness**
- **R1 (ubiquitous):** The test suite shall contain a **module runner**: a way to instantiate a generated module on a headless WebAssembly engine with a **stub host** (the host API of WASM-010 R12 implemented by the test), feed it events and read back its property writes, output, errors and memory.
  It shall be deterministic and fast enough to run the NIST subset (WASM-010 R16).
- **R2 (ubiquitous):** The suite shall contain a **browser runner**: a way to build a bundle, serve it and drive it in a real headless browser engine (click, type, wait, read the DOM and computed styles, take a picture), used for the checks the other specs name (WASM-002 AC1–AC2, WASM-004 AC2–AC3, AC6–AC7,
  WASM-006, WASM-007, WASM-009, WASM-011 and WASM-013's policy tests). Which engine, and in which CI image, is a `/plan` decision (Q1); it shall not require the developer to install a browser to run the ordinary tests.
- **R3 (ubiquitous):** The suite shall contain a **backend runner**: start the backend binary on a loopback port with a local test identity issuer (WASM-009 R12), connect a client and exercise calls (WASM-003, WASM-005, WASM-014), including the version-skew runs of WASM-014 AC10.

**The test families**
- **R4 (ubiquitous):** Every class A and B control (WASM-004) shall have a web property round-trip test as `tests/controls/` has for the desktop (set each property, read it back from what the browser shows), and the 27 controls the desktop families do not yet cover shall be listed with a decision for each: covered
  now, or recorded as a known gap.
- **R5 (ubiquitous):** The **parity guard** shall be extended to the web: a reference form that contains a control of every class is rendered by the desktop engine and by a browser, and geometry and, where the plan sets a tolerance, colour are compared, with the differences WASM-004 documents as the only ones allowed.
- **R6 (ubiquitous):** The **conformance ledger** shall be extended in the NIST style (two numbers per module: compiles, runs with 0 failures) with a second column for the generated module; NC, IF and IC are the first rows (WASM-010 R16). A regression run shall be one command.
- **R7 (ubiquitous):** The **golden files** of the code generator shall cover the web output (the generated COBOL for web forms, the interface description, the stylesheet, the host page and the generated CSP), and `UPDATE_GOLDEN`-style regeneration shall work as it does today.
- **R8 (ubiquitous):** The `cobolt-bench` baseline shall gain the numbers WASM-010 R15 and the performance spec propose (module size, start-up, a fixed arithmetic and string workload on the module and on the interpreter).
- **R8b (ubiquitous):** The suite shall carry the **lifecycle, memory and boundary tests** of WASM-017 R20, **run for every combination** of program-module variant (multi-memory, single), player variant (threaded, plain) and execution mode (Worker, main thread) — loading (a network trace), disposal (open and close a form many times; instances and memory return to baseline), fixed memory, the isolation test, batching (a bound on host calls), strings, views after growth of the player's memory, the baseline and browser-matrix check,
  and the yield test (a very long loop keeps frames within a deadline) — and the size report of WASM-016 R13 and WASM-017 R16, so that a regression in footprint or in leaks is a failing test and not a surprise.
- **R9 (ubiquitous):** Each test shall report **quantified, human-readable results** (what was exercised and how it performed), not a bare pass count, as the project's testing rule requires.

**Agent tooling**
- **R10 (ubiquitous):** The MCP tools shall understand the project kinds: `create_project` accepts a kind (WASM-001 R10); `check` reports the portable-subset errors and the class D controls (WASM-002 R3, WASM-004 R5) in the same form-control-event-line shape; `regenerate` and `validate` cover the new artefacts;
  `build` builds a Web bundle, a Mobile package and a Backend binary and reports sizes and the secret-scan result; `list_files` shows the new tree entries.
- **R11 (ubiquitous):** `render_form` and `run_form` shall work for a web form with the same shape of answer as for a desktop form (WASM-008 R9), backed by the module and browser runners, and shall state which engine produced the picture.
- **R12 (ubiquitous):** The server the IDE serves and `rcrun mcp` shall offer the same web behaviour, and a tool that cannot do something on a kind (building iOS off macOS) shall say so and why.
- **R13 (ubiquitous):** The plugin's `APPLICATION_RULES` shall gain the rules the web kinds need — what a web form may not contain, that secrets live only on the backend, that remote calls use `CALL … WITH WEBSOCKET LINKAGE`, that a web form is responsive and uses the Spatial look as the others — and the plugin's skills
  shall gain a skill for a Web or Mobile application and one for a Backend; the reviewer agent shall check the new rules.

**Knowledge**
- **R14 (ubiquitous):** The System KB shall state, in the compiler's documentation tables, for every control its class on the web (WASM-004 R12), the web-only members, the browser-capable built-ins (WASM-010 R7), the `CALL … WITH WEBSOCKET LINKAGE` phrase and its mapping, the application styles (WASM-011) and the Authentication and Hyperlink
  controls; `chunked.data` shall be regenerated and its freshness test shall pass.
- **R15 (ubiquitous):** Grace's specialists shall not propose a construct the web kinds refuse: the planner and the pedantic reviewer shall read the project kind, and their preambles (the policy rules, not word heuristics) shall say what each kind may use.
- **R16 (ubiquitous):** The shipped **examples** shall include at least one Web application and one Backend, built and run by the test suite with the module, browser and backend runners; the examples shall hold no key or password (WASM-013 R7).

**Layers**
- **R17 (ubiquitous):** The parity guard (R5) shall include a reference form with layers: a control in a layer above a base control, a hidden layer, an opaque and a transparent layer background, a `DataGrid` at a corner of the form, and a tab order that crosses layers. The same script runs on the desktop engine and in the browser, and the only differences found are those WASM-004 R17 and R20 record.
- **R18 (ubiquitous):** The knowledge base and the plugin's rules shall state the layer model for web forms (WASM-004 R13–R24), the `Non-Visuals` tab as a design-time surface, and the rule that a layer is addressed as `LAYER-NAME::Property` and never as a control. The agents' layer operations (spec 091 R71) apply to web forms, and `check` reports a name that is not a layer of the web form.

## 5. Acceptance criteria

- [ ] **AC1** — One command runs the NC subset on the generated module through the module runner and prints two numbers per module with the excluded programs and their reasons.
- [ ] **AC2** — The browser runner builds, serves and drives a hello-world bundle on a headless engine, reads the DOM and a computed style and returns a picture, on a machine with no separately installed browser (or with the requirement stated and checked).
- [ ] **AC3** — The backend runner starts a backend, signs a test user in against the local issuer, makes a call and a version-skew call, and tears down cleanly.
- [ ] **AC4** — The web property round-trip tests pass for every class A and B control; the list of uncovered controls with a decision each exists.
- [ ] **AC5** — The reference form's desktop and browser renderings agree within the plan's tolerance, with only the documented differences.
- [ ] **AC6** — The ledger shows the generated module's NC, IF and IC rows; a regression run is one command.
- [ ] **AC7** — The golden files for a web form's artefacts exist and regenerate with the golden switch.
- [ ] **AC8** — Through the MCP tools on a web form: `create_project` with a kind, `check` (an error for a class D control and for a file verb), `build` (a bundle), `render_form` (a picture) and `run_form` (a click and a property read) all work and report the engine used.
- [ ] **AC9** — The IDE-served and the stdio server give the same answers for the same web project.
- [ ] **AC10** — The plugin's rules include the web rules; the reviewer flags a web form containing a `SqlDatabase`; the new skills exist.
- [ ] **AC11** — `prebuilt_chunked_kb_matches_the_published_documentation` is green with the web knowledge; a Grace run asked for a `SqlDatabase` on a web form refuses or reroutes it through the backend.
- [ ] **AC12** — The example Web application and Backend build and run in the test suite and contain no secret.
- [ ] **AC13** — The reference form with layers gives the same stacking, visibility, hit testing and tab order on the desktop and in the browser, except the differences WASM-004 R17 and R20 record (R17).

## 6. Constraints & steering check

- **i18n:** yes for IDE strings only (the new tool messages shown in the IDE); the MCP tools' own text is English like the rest of the agent kit.
- **Test rules:** user-provided tests are report-or-fix; new tests report quantified results (GOLDEN RULE #7); a sweep uses `--no-fail-fast` and every `test result:` line is read; `cobolt-forms` tests need `--features render` and `cobolt-ide` tests need `--bin`.
- **Never drive the IDE's window:** every verification here goes through the tools and headless runs.
- **System KB:** R14 is itself the KB requirement of every spec in the series, gathered; the freshness test is a real gate.
- **Disk:** a browser engine and the web targets enlarge the build caches; the plan shall state the cost (builds already dominate the working tree).
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — The browser engine for the runners.** A headless Chromium or Firefox driven over WebDriver or the DevTools protocol, or a test harness built on the platform web view of WASM-008? Weigh determinism, CI images and licences at `/plan`; the harness must not make a browser a prerequisite of `cargo test` for people who do not touch the web kinds.
  **Decided (operator, 2026-10-22):** a harness on the web view of WASM-008.
- **Q2 — The WebAssembly engine for the module runner:** a pure-Rust interpreter (small, no JIT, slower) or a JIT engine (fast, larger) — WASM-010 Q2.
  **Decided (operator, 2026-10-22):** a JIT engine (`wasmtime`, WASM-010 Q2).
- **Q3 — Where the golden and ledger data live** for the web (the existing ledger is `NIST/progress.json`, one file per concern): extend it, or a sibling ledger for the module?
  **Decided (operator, 2026-10-22):** extend `NIST/progress.json`.
- **Q4 — Visual tolerance** for R5: a geometry-only guard (as today) is cheap; a pixel comparison is stricter and flaky across engines. Set after the first measurements.
  **Decided (operator, 2026-10-22):** geometry only.
- **Q5 — Should the Grace specialists get a web-aware specialist** (a `WebFormsDesigner`) or stay three and read the kind? Recommended: stay three.
  **Decided (operator, 2026-10-22):** a `WebFormsDesigner` specialist replaces `FormsDesigner` for Web and Mobile.
