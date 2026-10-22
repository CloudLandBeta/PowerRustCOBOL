<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Testing, agent tooling and the knowledge base for the web kinds (WASM-015)

- **Status:** draft (awaiting the operator's review, then `/implement`)
- **Plan:** ./plan.md   **Date:** 2026-10-22
- **Branch / line:** `features-2x`, 2.x.x, a **feature**. **No documentation task:** `README.md` and `docs/` are never touched on 2.x.x. What 2.x.x says about itself goes in `specs/` (`WASM-PROGRESS.md`, `WASM-PARITY.md`, this folder) and in the System KB.
- **Binding (operator, 2026-10-22, spec §7):** Q1 the browser runner is a harness on the web view of WASM-008 (`prc-web-preview`); Q2 the module runner runs on `wasmtime`; Q3 the ledger is `NIST/progress.json`, extended; Q4 the parity guard is geometry only; Q5 a `WebFormsDesigner` specialist replaces `FormsDesigner` for Web and Mobile.
- **Open, with the working assumption each task uses (not decided here):**
  - Q-new-1 (SqlDatabase on a web form): the later specs win. The reviewer flags a SqlDatabase that carries a connection string, key, token or password, or that sits on a web project with no Backend. Grace's expected behaviour is the reroute half of AC11 (T16, T19, T21).
  - Q-new-2 (the 27 uncovered controls): CoveredNow for the 7 of class A/B, KnownGap for the 20 of class C (T8).
  - Q-new-3 (engine coverage): Chromium is measured only by WebView2 on the Windows CI job; `node` cells are recorded as node, never as browser cells (T6, T13, T23).
  - Q-new-4 (names): `Web Form Designer Agent`, `Web Form Designer Agent Pedantic Reviewer`, `WebFormsDesigner` (T19, T20).
  - Q-new-5 (size growth): fail when a part grows by more than 5 % over the size last recorded (T13).

Each task leaves the workspace green. Every new test prints a quantified summary line (R9): what ran, what it measured, and the engine; a skip prints its reason. `df -h .` before any multi-crate run (53 GB free on 2026-10-22; record the growth of `target/` in `WASM-PROGRESS.md` after T2, T6 and T12).

## Grounding (read for this task list, 2026-10-22, 2.0.30)

- Workspace: `crates/cobolt-web-player` is **excluded** from the root workspace (root `Cargo.toml`, `exclude = ["crates/cobolt-web-player"]`), so no workspace crate can read `cobolt-web-player/src/kinds.rs::TABLE`. The class table must first move to `cobolt-forms` (WASM-004 plan C1); `crates/cobolt-forms/src/` has no `web` module today. Every task that reads classes (T8, T15, T16, T18) is **gated on WASM-004 C1**.
- `ControlType::ALL` is at `crates/cobolt-forms/src/model.rs:2926`. `tests/controls/` holds 35 control files (`agentobject.rs` … `treeview.rs`) and its own `Cargo.lock` (a standalone package).
- The parity guard `engine_reference_form_parity_static_vs_faces` is a test at `cobolt-forms/src/render.rs:22796`; **no public `layout_rects`** (or any per-control rectangle function) exists in `render.rs`. T10 adds one.
- `run_form` steps are `&[serde_json::Value]` (`cobolt-project-tools/src/tools/run.rs:181`, `pub fn run(…, steps: &[Value], …)`); there is no `Step` type and no `src/script.rs`. T1 adds it.
- `cobolt-web-bundle/tests/e2e_node.rs:22-24` panics when `node` is absent. `cobolt-web-bundle/src/secrets.rs` has `scan_text(file, text, entropy_len) -> Vec<Finding>` and `is_text_path`.
- The backend rig is `cobolt-server/tests/support/mod.rs`; `cobolt-server/Cargo.toml` has `[features] default = ["server", "client"]`, the RustCrypto crates (`rcgen`, `p256`, `rsa`, `rand_core`, `jsonwebtoken` with `use_pem`) are dev-dependencies (lines 93–99), and `tests/dependency_guards.rs` exists. `cobolt_server::dev::write_interface` is at `src/dev.rs:103`.
- `cobolt-project-tools`: `content.rs:254` `APPLICATION_RULES: [(&str, &str); 14]` (rules `web-kinds` :445, `backend-secrets` :464); skills `powerrustcobol-web-application` :1172 and `powerrustcobol-backend` :1211; `fn reviewer()` :1262. `web/` holds `devserver.rs`, `mod.rs`, `presets.rs`, `preview.rs` (`helper_path` :160). `tests/` holds `kinds.rs`, `tools.rs`, `layout_rule.rs` among others.
- `cobolt-wasm/examples/` holds `build_rt.rs`, `nist_wasm.rs`, `nist_wasm_run.rs`.
- Grace: `cobolt-agents/src/specialist.rs:109` `BUILTIN_SPECIALISTS: [&str; 3]`, `route_specialist` :145; the call at `cobolt-ide/src/llm.rs:1771`; `agents_db.rs` `PEDANTIC_FORM_DESIGNER_REVIEWER` :77, `FORM_DESIGNER` :100, `produces_form_change_set` :114, `is_fixed_agent_name` :137; the planning prompt with `SURFACE CONTEXT` and `PEDANTIC COMPANION CONTRACT` at `grace_host.rs:2498`; the freshness test at `grace_host.rs:4780`; `cobolt-ide/examples/build_chunked_kb.rs`.
- i18n: `cobolt-ide/src/i18n.rs` has a key `agent_form_designer` (six languages, :1169 and :3505/5554/7603/9651/11705/13755) that **no other file in `cobolt-ide/src` uses**. So agent names are shown untranslated today; T20 checks whether the Agents dialog shows the new agent through a `Tr` key.
- `ApplicationStyle` is in `cobolt-compiler/src/project_kind.rs:142`. `examples/` holds PowerAnalytics, PowerChat, PowerCloud, PowerDemo3, PowerSpatial (all desktop). `crates/cobolt-ide/src/version.rs:22` `VERSION = "2.0.30"`.

## Dependencies on other specs (each task that needs one says so)

| Gate | What it gives | Tasks waiting on it |
|---|---|---|
| WASM-004 C1 | the class table in `cobolt_forms::web`, the class D rule | T8, T9, T15, T16, T18 |
| WASM-010 WP1/WP2 | `wasmtime` host, `open_form`/`event`/`resume` | T11, T12, T14 |
| WASM-010 WP3 | `builtin_support` / typed refusals | T15, T16, T18 |
| WASM-008 §2.3 | `prc-web-preview --headless`, `render`, `script`; the web route of `render_form`/`run_form` | T5, T6, T9, T10, T17 |
| WASM-002/010 WP6 | the real Build link (`web::backend()` not `NotYetLinked`) | T7 (real-bundle half), T22 |
| WASM-005/011/006/009 | their own KB tables | T18 (the census turns green only as they land) |

Until a gate opens, the task runs on a **fixture** bundle (as `cobolt-web-bundle/examples/fixture_bundle.rs` does) or is left unchecked with an owed row in `WASM-PROGRESS.md`. Nothing is claimed end to end before its gate.

## Tasks

### Harness foundations (no gate)

- [ ] **T1 — The shared script type** (R11, D1, plan §3.1)
  - Files: new `crates/cobolt-project-tools/src/script.rs` (`pub enum Step { Set, Event, WaitMs, Read, OpenForm, Text, Style, Rect, Snapshot, Focused, Key }`, `Step::parse(&Value)`), `src/lib.rs` (`pub mod script`), `src/tools/run.rs` (parse through `Step` instead of reading `Value`s directly).
  - Do: the JSON of the five existing steps is unchanged; the six read-only steps (§3.2) parse but the desktop runner answers "not supported on the desktop host" for `text`/`style`/`rect`/`snapshot`; `focused` and `key` are implemented on the desktop host only if `FormRunner` already exposes focus, otherwise reported unsupported and recorded for T10.
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` green (every existing `run_form` test unchanged); a unit test round-trips each step's JSON and rejects an unknown step by name.

- [ ] **T2 — Node end-to-end skips instead of panicking** (R2 constraint, R8b, D3)
  - Files: `crates/cobolt-web-bundle/tests/e2e_node.rs`, `tests/support/` as needed.
  - Do: when `node` is absent print `skipped: node not on PATH` and return; when `PRC_REQUIRE_NODE=1` the absence fails. Same rule for every other `node` test in the file.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast` green with `node` present (cells counted and printed); with `PATH` stripped of `node` the test passes and prints the skip; with `PRC_REQUIRE_NODE=1` and no `node` it fails.

- [ ] **T3 — The backend test kit as `cobolt_server::testkit`** (R3, D5)
  - Files: new `crates/cobolt-server/src/testkit.rs` (moved from `tests/support/mod.rs`, behaviour unchanged), `src/lib.rs` (`#[cfg(feature = "testkit")] pub mod testkit`), `Cargo.toml` (feature `testkit`; `rcgen`, `p256`, `rsa`, `rand_core`, `jsonwebtoken/use_pem` become optional dependencies enabled by it and stay dev-dependencies), every `tests/*.rs` importing from `cobolt_server::testkit`; `tests/support/mod.rs` removed or reduced to a re-export.
  - Do: keep the RustCrypto-only rule in the file's header; `tests/dependency_guards.rs` must keep holding with and without the feature.
  - Verify: `cargo test -p cobolt-server --no-fail-fast` and `cargo test -p cobolt-server --features testkit --no-fail-fast` green, same test counts as before the move (record both); `cargo tree -p cobolt-server -e normal` without the feature shows none of the RustCrypto crates added.

- [ ] **T4 — The test crate `cobolt-web-tests`** (D4)
  - Files: new `crates/cobolt-web-tests/Cargo.toml` (`publish = false`, workspace deps, dev-dependencies on `cobolt-project-tools`, `cobolt-forms` with `render`, `cobolt-wasm` with `host`, `cobolt-server` with `testkit`, `cobolt-web-bundle`, `cobolt-codegen`), `src/lib.rs` (shared helpers: summary line `→ N passed, M failed`, skip helper honouring `PRC_REQUIRE_WEBVIEW`/`PRC_WEB_E2E`), `fixtures/` (empty project skeleton), root `Cargo.toml` `members` (one line, with the usual comment).
  - Verify: `cargo build -p cobolt-web-tests` and `cargo test -p cobolt-web-tests` green (a smoke test of the skip helper); `df -h .` before and after, growth noted.

### Runners

- [ ] **T5 — The read-only steps in the helper protocol** (R2, plan §3.2) — **gated on WASM-008 §2.3**
  - Files: `crates/cobolt-web-preview/src/main.rs` (inside `script`: `text`, `style`, `rect`, `snapshot`, `focused`, `key`; addressing by `[data-name=…]` or the roles `form`, `layer:<NAME>`, `snackbar`; no `eval`), `crates/cobolt-project-tools/src/web/preview.rs` (the IDE side), the page-side code as a Rust string constant.
  - Do: an unknown step answers an error the runner reports as "helper too old"; never ignored.
  - Verify: `cargo test -p cobolt-web-preview -p cobolt-project-tools --no-fail-fast` green; both sides pin each protocol line in a test (as WASM-008 §3 does); a test asserts there is no step that takes free script text.

- [ ] **T6 — The browser runner library** (R2, R9, AC2) — **gated on T5**
  - Files: new `crates/cobolt-project-tools/src/web/runner.rs` (`mod runner` in `web/mod.rs`): bundle (via `web::backend()` or `cobolt_web_bundle::assemble_bundle` on a fixture) → serve on loopback (`web/devserver.rs`) → open the helper headless at a preset size → run `script::Step`s → `values`, `displayed`, `errors` (page and `cobol-error`), `png`, `engine` (`webkit`/`webview2`/`webkitgtk` + `navigator.userAgent`); new `crates/cobolt-web-tests/tests/runner.rs::hello_world_bundle`.
  - Do: without a helper or a display, return a typed "no web view — <reason>" error; the test prints it and returns, and fails under `PRC_REQUIRE_WEBVIEW=1`.
  - Verify (**AC2**): `cargo test -p cobolt-web-tests --test runner --no-fail-fast` on this Mac: click a button, read a label's `text`, a `style` `color`, a PNG of non-zero size; prints engine, version and ms per step. Run once with the helper path pointed nowhere → prints the skip; with `PRC_REQUIRE_WEBVIEW=1` → fails.

- [ ] **T7 — The backend runner: start, sign in, call, skew, teardown** (R3, AC3)
  - Files: `crates/cobolt-server/src/testkit.rs` (binary mode: build with `cobolt_compiler::backend_build` from a fixture `backend/`, start on `127.0.0.1:0` with a test `backend.toml` trusting the testkit's JWKS written to a file), new `crates/cobolt-server/tests/runner.rs`.
  - Do: **if `backend.toml` cannot name a local key-set file without an `unsafe_*` setting, stop and ask the operator** before adding a setting (plan §5). Binary mode runs only with `PRC_WEB_E2E=1`.
  - Verify (**AC3**): `cargo test -p cobolt-server --features testkit --test runner --no-fail-fast`: in-process — sign a test user in, one call, one skew call a minor ahead and one behind, shutdown with no task left and the port released (re-bind succeeds); prints each step's ms. `PRC_WEB_E2E=1 cargo test … --test runner` does the same against the built binary.

### Test families

- [ ] **T8 — Coverage table for the 62 control types** (R4, AC4 second half) — **gated on WASM-004 C1**
  - Files: new `crates/cobolt-web-tests/src/coverage.rs` (`COVERAGE: &[(&str, Decision)]`, `Decision::{CoveredNow, KnownGap(&str)}`), new `tests/coverage.rs`.
  - Do: one row per `ControlType::ALL` entry; Q-new-2's working assumption for the 27 not in `tests/controls/`.
  - Verify: `cargo test -p cobolt-web-tests --test coverage` prints `62 of 62 rows, A/B covered N, known gaps M`; a planted missing row fails it.

- [ ] **T9 — Web property round-trips for class A/B, plus the seven desktop files** (R4, AC4) — **gated on T6, T8**
  - Files: new `crates/cobolt-web-tests/tests/controls_web.rs`; new `tests/controls/{sidemenu,switch,snackbar,knob,gauge,maps,viewer}.rs` (desktop, same shape as `button.rs`), `tests/controls/Cargo.toml` if it lists targets.
  - Do: per class A/B control, a one-control web form; set each property `assert_control` covers to a distinct value, read it back with `text`/`style`/`rect` (visual) or `snapshot` (no visual).
  - Verify (**AC4**): `cargo test -p cobolt-web-tests --test controls_web --no-fail-fast` prints PASS/FAIL per property and a total per control, all PASS on WebKit; `cargo test --manifest-path tests/controls/Cargo.toml --no-fail-fast` green with the seven new files.

- [ ] **T10 — Parity guard on the web, geometry only** (R5, R17, AC5, AC13) — **gated on T6, WASM-004 C1**
  - Files: `crates/cobolt-forms/src/render.rs` (a public `layout_rects(form, window) -> Vec<(String, Rect)>` built on the same layout the existing guard uses; the guard itself unchanged), new `crates/cobolt-web-tests/fixtures/parity-all-classes.wcfrm`, `fixtures/parity-layers.wcfrm` (R17's list), new `tests/parity_web.rs` (`all_classes`, `layers`, `DOCUMENTED: &[(control, property, spec_ref)]`).
  - Do: tolerance 1 CSS px, no colour (Q4); each `DOCUMENTED` row must cite WASM-004 (R17/R20 for layers); for `layers` the same script runs on the desktop host and in the browser: visibility of each layer, a hit test under a layer, the focused control after each `key Tab`.
  - Verify: `cargo test -p cobolt-forms --features render --no-fail-fast` green (the existing guard unchanged); `cargo test -p cobolt-web-tests --test parity_web --no-fail-fast` — **AC5**: per control desktop vs browser rect within 1 px, prints the max difference; **AC13**: the sequences printed side by side and equal except `DOCUMENTED` rows. The measured spread goes into `WASM-PROGRESS.md`; the tolerance is not loosened to pass.

- [ ] **T11 — The module runner contract** (R1, R9) — **gated on WASM-010 WP1/WP2**
  - Files: `crates/cobolt-wasm/src/host.rs` (stub property store answering `query` `GET`/`CALLR`/`COUNT` from the module's `SET`s and seeded values; read-back of batches as `cobolt_wasm_abi::batch` commands, `DISPLAY` output, `ERROR`s with unit/line/form/event, memory ranges and `pr_dbg_live`/`pr_dbg_region_used`; `run_script(&[Step])` with fixed `now`/`random`), `crates/cobolt-wasm/Cargo.toml` (`cobolt-project-tools` as a **dev-dependency only**, for `Step`), new `crates/cobolt-wasm/tests/runner.rs`.
  - Verify: `cargo test -p cobolt-wasm --features host --no-fail-fast` green; `runner.rs` runs one form script twice and asserts byte-identical batches; a read after a handler's own `SET` returns that value; prints steps, host calls, bytes of memory read.

- [ ] **T12 — The ledger column and the one-command regression** (R6, AC1, AC6) — **gated on T11**
  - Files: `crates/cobolt-wasm/examples/nist_wasm.rs` (`regress <MODULES…>` reusing `census` and `nist_wasm_run`'s code, not a third copy; `--record` writes only the `wasm` objects), `NIST/progress.json` (by `--record` only), `.claude/skills/nist-grind/SKILL.md` (reads the `wasm` column; *one rule lives in three files* — check `specs/steering/*.md` and the local CLAUDE.md for the same rule and change them together, CLAUDE.md being an untracked symlink absent in a worktree), `specs/WASM-PROGRESS.md`.
  - Verify: **AC1** — `cargo run -p cobolt-wasm --example nist_wasm -- regress NC` prints two numbers on `wasmtime`, the assertion tally, the excluded programs with reasons, and the interpreter's numbers beside them. **AC6** — `… regress NC IF IC --record`, then `jq '.modules.NC.wasm, .modules.IF.wasm, .modules.IC.wasm' NIST/progress.json` shows three rows; `git diff NIST/progress.json` shows only `wasm` objects (interpreter fields and `protected_baselines` byte-identical).

- [ ] **T13 — Lifecycle/memory matrix cells and the size report** (R8b) — **gated on T2; browser cells on T6**
  - Files: `crates/cobolt-web-bundle/tests/e2e_node.rs` (the size-report test: size per part against the last recorded size, fail above +5 %, Q-new-5), `crates/cobolt-web-tests/tests/matrix.rs` (the cells `node` cannot run — the threaded player — on the browser runner under the dev server's cross-origin-isolation headers), `specs/WASM-PROGRESS.md` (each cell with its engine: node, webkit, webview2).
  - Do: the scenarios themselves are WASM-017's (plan §1.6); nothing here re-writes them. No cell is claimed from an engine that did not run it.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast` and `cargo test -p cobolt-web-tests --test matrix --no-fail-fast` print the scenario count per cell and the engine; a planted +10 % part fails the size report.

- [ ] **T14 — Bench rows for the module** (R8) — **gated on T11**
  - Files: `crates/cobolt-bench/Cargo.toml` (feature `wasm`, off by default, pulls `cobolt-wasm/host`), `crates/cobolt-bench/src/main.rs` (`module_size`, `module_startup` labelled "native, not a browser figure", `decimal_math` and a string workload on the module and the interpreter side by side).
  - Verify: `cargo build -p cobolt-bench` (default) does not build `wasmtime` (`cargo tree -p cobolt-bench | grep -c wasmtime` = 0); `cargo run --release -p cobolt-bench --features wasm` prints the three rows with units. No budget asserted.

- [ ] **T15 — Goldens for web output** (R7, AC7) — **gated on WASM-004 C1 for the form; WASM-007 for the stylesheet**
  - Files: new `crates/cobolt-web-tests/tests/web_golden.rs`, `fixtures/golden-project/`, `fixtures/golden/` (the generated `.cbl` through `cobolt-codegen`, the interface description via `cobolt_server::dev::write_interface`, the stylesheet, the host page + `config.json` + CSP from `cobolt-web-bundle`, the manifest).
  - Do: `UPDATE_GOLDEN=1` rewrites them; every artefact is compared **before** the test fails (unlike the desktop snapshot test that stops at the first missing golden). An artefact whose producer has not landed is reported "owed: <spec>" and does not fail.
  - Verify (**AC7**): `UPDATE_GOLDEN=1 cargo test -p cobolt-web-tests --test web_golden`, then a second run without it is green; deleting two goldens reports both missing in one run.

### Agent tooling

- [ ] **T16 — `check` and `validate` on web forms; `build`'s secret scan** (R10, R18, AC8 check half) — **gated on WASM-004 C1, WASM-010 WP3**
  - Files: `crates/cobolt-project-tools/src/tools/check.rs` (`check_project`/`check_one` for forms under `webforms/`/`mobileforms/`: class D gate, subset refusals, `NAME::Property` that is neither a control nor a layer, a write to `Layer::Name`), `tools/validate.rs` (class D and layer gates on a `.cfrm` under `webforms/`), `tools/build.rs` (`secret_scan: {scanned_files, findings}` from `cobolt_web_bundle::secrets`), `tests/kinds.rs`.
  - Do: the gates come from the owning crates; this task only calls them and maps their kinds to the existing `Finding` shape. Check reports, never fixes (user code is sacred).
  - Verify: `cargo test -p cobolt-project-tools --no-fail-fast` green; a `kinds.rs` test: a web form with a class D control and a file verb gives two errors with file, line, form and site; a bad layer name gives one; `build` (stub backend) answers `secret_scan` with `scanned_files > 0`.

- [ ] **T17 — `render_form`/`run_form` on a web form; both servers agree** (R11, R12, AC8, AC9) — **gated on T6 and WASM-008's web route**
  - Files: `crates/cobolt-project-tools/src/tools/{render,run}.rs` (hold WASM-008's route to R11: the desktop fields plus `engine`; never a desktop picture for a web form; no web view → an error naming what is missing), `tests/kinds.rs` (`web_tool_sequence`, `both_servers_agree`, or `crates/cobolt-web-tests/tests/servers_agree.rs` if it does not fit).
  - Do: `both_servers_agree` builds two `ProjectTools` as `cobolt-ide/src/agent_kit/ide_host.rs:123` and `cobolt-cli/src/main.rs:453` do, with the same stub `WebBuildBackend`; a kind a tool cannot serve answers `{status:"unsupported", reason}`.
  - Verify: **AC8** — `cargo test -p cobolt-project-tools --test kinds web_tool_sequence --no-fail-fast`: `create_project` (kind web) → `check` two errors → fix → `build` `built` with `secret_scan` → `render_form` PNG + `engine` → `run_form` click + read + `engine` (browser parts print a skip without a web view). **AC9** — `both_servers_agree`: JSON equal field by field except `elapsed_ms`.

- [ ] **T18 — Plugin rules, skills and reviewer** (R13, R18, AC10) — **gated on WASM-004 C1, WASM-010 WP3**
  - Files: `crates/cobolt-project-tools/src/content.rs` (`web-kinds` generated from the class and subset tables — class D by name, class C through the Backend, the browser subset including in-memory INDEXED, `render_form`/`run_form` use the web view; new rule `web-layers`; `APPLICATION_RULES` 14 → 15; the two skills gain the layer paragraph and the `check` gates; `reviewer()` names class D as rejected and class C accepted only with a backend configuration name and no connection string, key, token or password — Q-new-1), `tests/tools.rs` (:552), `tests/layout_rule.rs`, `crates/cobolt-ide/src/panels/beautify.rs` (:2440, re-read; lookups by id).
  - Verify (**AC10**): `cargo test -p cobolt-project-tools --no-fail-fast` green: every class D name appears in `web-kinds`; `web-layers` and `backend-secrets` present; the reviewer's checks flag a web form whose SqlDatabase holds a connection string and one on a project with no Backend; both skills in `kit()` and written by `bundle.rs`. `cargo test -p cobolt-ide --bin cobolt-ide beautify --no-fail-fast` green.

### Knowledge and Grace

- [ ] **T19 — Grace reads the kind; `WebFormsDesigner` replaces `FormsDesigner` on Web/Mobile** (R15, AC11 Grace half, Q5)
  - Files: `crates/cobolt-agents/src/specialist.rs` (`BUILTIN_SPECIALISTS` gains `"WebFormsDesigner"` with a policy-rule preamble generated from the same text as `web-kinds`; `route_specialist_for(kind, prompt)` maps the `FormsDesigner` outcome to `WebFormsDesigner` for Web/Mobile; `route_specialist` unchanged for Desktop), `crates/cobolt-ide/src/llm.rs` (:1771 passes the open project's kind; `DEFAULT_WEB_FORM_DESIGNER_AGENT_PROMPT` and its pedantic prompt; the kind rule in `DEFAULT_GRACE_PROMPT` and `DEFAULT_PEDANTIC_GRACE_PROMPT` — DEFAULT_* only, LEGACY prompts untouched), `crates/cobolt-ide/src/agents_db.rs` (`WEB_FORM_DESIGNER`, its own pedantic companion, `produces_form_change_set`, `is_fixed_agent_name`, `ensure_*`; the "no shipped default may look stale" loop gains both prompts), `crates/cobolt-ide/src/grace_host.rs` (a `PROJECT KIND:` block next to `SURFACE CONTEXT` at :2498; the registry lists `Web Form Designer Agent` instead of `Form Designer Agent` on Web/Mobile and the reverse on Desktop), `crates/cobolt-ide/src/agent.rs` (a kind header **above** the layers block; `CONTROLS:` and its slicing not renamed).
  - Do: dispatch by kind in code, never by prompt words (policy-first). If an agent golden changes, regenerate with `COBOLT_WRITE_GOLDEN=1` and read the diff.
  - Verify: `cargo test -p cobolt-agents --no-fail-fast` (`route_specialist_for(Web, "add a SqlDatabase") == "WebFormsDesigner"`, Desktop unchanged); `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` green, with new tests: the Web planning prompt carries `PROJECT KIND:` and lists `Web Form Designer Agent` and not `Form Designer Agent`; `produces_form_change_set` is true for the new agent and the apply path accepts its change set; neither new prompt is detected as stale.

- [ ] **T20 — i18n check for the new agent** (spec §6 i18n)
  - Files: `crates/cobolt-ide/src/i18n.rs` only if needed.
  - Do: find where the Agents dialog draws agent names and descriptions. Today `agent_form_designer` exists in six languages but is unused, so names are shown in English. If the dialog shows the new agent through any IDE-visible string that is not an agent name or prompt, add a `Tr` key in EN/ES/PT (pt-BR)/JA/ZH/FR; otherwise record "no new key" here with the file read.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n --no-fail-fast` green (no empty translation).

- [ ] **T21 — The web knowledge census and the KB store** (R14, AC11 KB half)
  - Files: `crates/cobolt-compiler/src/lib.rs` (test `web_knowledge_is_complete` only: the expected set built from `ControlType::ALL` with classes, `BUILTINS` with browser support, the `WEBSOCKET LINKAGE` phrase, `ApplicationStyle` (`project_kind.rs:142`), Authentication and Hyperlink; asserts each in `system_documentation()`; prints `N of M present` and the missing names), `assets/knowledge/chunked.data` (regenerated in the same task as any doc-table change).
  - Do: the tables are written by their own specs; this task never writes them. So that the workspace stays green while they land, the test asserts the facts whose owning spec is marked landed in a Rust list next to it, and prints the rest as `owed: <spec>` (never `#[ignore]`); each owed spec is a row in `WASM-PROGRESS.md`. AC11 is checked only when the owed list is empty.
  - Verify (**AC11** KB): `cargo test -p cobolt-compiler web_knowledge_is_complete -- --nocapture`; `cargo run -p cobolt-ide --example build_chunked_kb`; `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green; `git status` shows `chunked.data` changed together with `lib.rs`.

### Examples

- [ ] **T22 — `examples/PowerWebHello` built and run by the suite** (R16, AC12) — **gated on T6, T7, WASM-005 (`WEBSOCKET LINKAGE`), WASM-002/010 WP6**
  - Files: new `examples/PowerWebHello/` (a Web project: one form, a button calling a Backend program through `CALL … WITH WEBSOCKET LINKAGE`, a label showing the answer, a layer shown by the program; `backend/` exporting that program and reading an in-memory or `redb` indexed file), new `crates/cobolt-web-tests/tests/examples.rs`.
  - Do: COBOL written against the rustcobol-code-style skill (properties as data items, inline method calls, English identifiers); no key, password, token or account id; regenerate `generated/` through the regenerate path, never by hand.
  - Verify (**AC12**): `cargo test -p cobolt-web-tests --test examples --no-fail-fast`: the real Build succeeds, the backend runner starts (in-process; binary with `PRC_WEB_E2E=1`), a click reads the backend's answer in the label, `secrets` over the example's tree reports 0 findings. Commit gate: `! grep -rEq 'AKIA|ASIA|Password=|APIKey|Token=|BEGIN .*PRIVATE KEY' examples/PowerWebHello`.

### CI

- [ ] **T23 — The web-test jobs** (R2, R3, R8b, D3)
  - Files: `.github/workflows/build-all-platforms.yml`.
  - Do: Linux with Xvfb and `libwebkit2gtk-4.1`, macOS and Windows, all with `PRC_REQUIRE_WEBVIEW=1`; `node` with `PRC_REQUIRE_NODE=1`; a binary-mode job with `PRC_WEB_E2E=1`, once per job. Note in `WASM-PROGRESS.md` that `cobolt-web-preview`'s target gate excludes Linux today (WASM-008 C6); until it builds there, the Linux job is an owed row, never a silent skip.
  - Verify: the YAML parses (`cargo` is not needed; read back the job list); the next CI run shows the three jobs, with engine per cell recorded in `WASM-PROGRESS.md`.

### Finalize

- [ ] **T24 — Finalize: version, CHANGELOG, the tests of the crates touched**
  - Files:
    - `crates/cobolt-ide/src/version.rs`: `z` + 1, **once** — from `2.0.30` (or whatever `z` is current then) to the next free `2.0.z`; never `x` or `y`.
    - `CHANGELOG.md`: **one** entry above the previous `## [PowerRustCOBOL 2.0.z]` heading, dated, "*(Written on `features-2x`, after 2.0.<prev>.)*". It lists the runners, the families and their measured numbers, the tool changes (`engine`, `secret_scan`, web gates), the rule count 14 → 15, `WebFormsDesigner`, the example, and what stays open (Q-new-1 … Q-new-5, every gated task still unchecked).
  - Do: `df -h .` first. Run the tests of the crates touched, `--no-fail-fast`, reading every `test result:` line:
    - `cargo test -p cobolt-project-tools -p cobolt-web-tests -p cobolt-web-bundle -p cobolt-web-preview -p cobolt-wasm -p cobolt-compiler -p cobolt-agents -p cobolt-bench --no-fail-fast`;
    - `cargo test -p cobolt-server --features testkit --no-fail-fast`;
    - `cargo test -p cobolt-forms --features render --no-fail-fast`;
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
    - `cargo test --manifest-path tests/controls/Cargo.toml --no-fail-fast`.
  - Verify: everything above green, counts recorded here; the staged diff has no credential (`AKIA`/`ASIA`, PEM blocks, tokens outside test fixtures, the operator's account id); `git status` shows nothing under `docs/` and no `README.md` change. Manual check (the operator's, never an agent's): launch the IDE, confirm the version string, open a Web project and ask Grace for a SqlDatabase on a web form — expect the reroute through the Backend (Q-new-1 working assumption).

## Done criteria

- Every acceptance criterion in spec.md is covered by at least one task's verification:
  - **AC1** (NC subset, two numbers, exclusions with reasons): T11, T12;
  - **AC2** (browser runner on a hello-world bundle, DOM, style, picture, no separate browser): T5, T6;
  - **AC3** (backend runner: sign-in, call, skew call, clean teardown): T3, T7;
  - **AC4** (web round-trips for class A/B; uncovered list with decisions): T8, T9;
  - **AC5** (reference form renderings agree within tolerance): T10;
  - **AC6** (ledger rows NC/IF/IC, one-command regression): T12;
  - **AC7** (web goldens regenerate with the switch): T15;
  - **AC8** (MCP sequence on a web form, engine reported): T1, T16, T17;
  - **AC9** (both servers answer the same): T17;
  - **AC10** (web rules, reviewer flags SqlDatabase, new skills): T18;
  - **AC11** (KB freshness with web knowledge; Grace reroutes a SqlDatabase): T19, T21, plus T24's manual run;
  - **AC12** (example Web application and Backend build, run, no secret): T22;
  - **AC13** (layers: stacking, visibility, hit testing, tab order): T10.
  - R8 (bench): T14; R8b (matrix, size report): T2, T13; R9 (quantified reports): every test task; R2/R3 CI requirement: T23.
- The tests pass, the System KB is regenerated with its freshness test green, and the i18n test is green in six languages.
- No `README.md` or `docs/` change. The change is a **feature** on `features-2x`. Nothing is committed or pushed by these tasks.
