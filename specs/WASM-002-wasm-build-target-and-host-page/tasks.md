<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — WebAssembly build target and host page (WASM-002)

- **Status:** in progress — T1–T15, T17, T18, T21, T22 done (2.0.37, 2026-10-23); **stopped at T19**: its Chrome half could not be run
  in this session (see T19); T16's manual MCP check is owed; T20 and T25's remaining sweeps not run. T1–T9 done and measured on 2026-10-20 (the assembler, the loader and the page, the CSP, the secret scan,
  the size report, the dev server). This revision (2026-10-22) follows the plan revised the same day: the host page's code becomes
  Build-generated Rust output (plan C1, Q6), the 5 MB budget line (C4), the real `WebBuildBackend` with Check and Build (C6). Mobile
  (R2, AC7) stays WASM-012's.
- **Plan:** ./plan.md   **Date:** 2026-10-22 (first list 2026-10-20)

Renumbering against the 2026-10-20 list: old T10 (Check, portable subset) → **T14**; old T11 (Mobile) → **T23**; old T12 (missing-tool
messages) → **T18** (Web half here, Mobile half WASM-012); old T13 (Desktop unchanged) → **T20**; old T14 (other browsers) → **T24**.
This list also takes over WASM-016's T14 (*IDE Build step writes the bundle*, `specs/WASM-016-prebuilt-player/tasks.md:41`), which handed
it to WASM-001/002: it is T13–T16 here.

Grounding read for this revision: `crates/cobolt-web-bundle/src/lib.rs` (`include_str!` constants `:36-42`, `BundleOptions` `:139`,
`assemble_bundle` `:415`), `src/secrets.rs` (`is_text_path` `:257`, no `.wcfrm`/`.mcfrm` yet), `crates/cobolt-project-tools/src/web/mod.rs`
(`WebBuildRequest` `:46`, `from_manifest` `:71` refusing a Desktop project, `BuildReport` `:164`, `WebBuildError` `:194`, the trait `:225`
with only `build`, `NotYetLinked` `:258`, `link` `:279`, `backend()` `:289`), `src/tools/build.rs:98` and `src/tools/check.rs:104`
(`check_project`, no portability step), `crates/cobolt-wasm/src/module.rs` (`Options` `:51`, `FormSpec` `:141`, `compile_project` `:161`),
`src/error.rs` (`GenError::all` `:26`), `crates/cobolt-wasm/Cargo.toml` (`default = ["host"]` pulls `wasmi`), `crates/cobolt-codegen/src/lib.rs:356`
(`generate_with_map`), `crates/cobolt-compiler/src/project_kind.rs` (`ModuleVariants` `:257`, `WebSettings::module_variants` `:310`),
`crates/cobolt-ide/src/app.rs` (`do_check` `:4488` — checks the active source only; `themes_dir` `:2442` and `images_dir` `:20415` — the
bundled-asset pattern: `<exe dir>/assets/<x>` if it is a directory, else `assets/<x>` relative), `crates/cobolt-cli/src/main.rs`
(`cmd_mcp` `:402`, the same pattern for `assets/themes`), `crates/cobolt-ide/src/panels/web_preview.rs:653` (shows `Refused` lines).

## Done (kept with their evidence, 2026-10-20)

- [x] **T1 — The bundle assembler** (R1, R8, R16, R17)
  - Files: `crates/cobolt-web-bundle/src/lib.rs`, `hash.rs`
  - Verify: `cargo test -p cobolt-web-bundle` green (unit 18, `csp_origins` 2, `no_string_execution` 3, `e2e_node` 14 scenarios).
- [x] **T2 — Secret scan** (R7, AC5): `src/secrets.rs`; a deliberate test secret stops the bundle naming the file and the pattern, never the value.
- [x] **T3 — The size report** (R8): every part, module, loader, stylesheet and asset, and which parts load on first use.
- [x] **T4 — The loader and the page** (R10-R13, R15, R19): `assets/{loader,app,page}`; mount into an element or fill the viewport; shadow root when embedded; progress and a readable failure.
  *(The behaviour stays; where the code comes from changes in T10–T11.)*
- [x] **T5 — The Content-Security-Policy** (R14): `src/csp.rs`, `tests/csp_origins.rs`; no `unsafe-eval`, no `unsafe-inline`, `connect-src` the configured backends only; the player runs under it in Chrome.
- [x] **T6 — Feature baseline, variants, isolation** (R18, R20): probe on Chrome 152 (baseline true, nothing missing, multi-memory true); variant choice end to end (see WASM-016).
- [x] **T7 — Development server** (R1): `examples/serve.rs` serves a bundle on 127.0.0.1 with or without isolation headers (`--isolated`, `--credentialless`), refuses to leave its folder.
- [x] **T8 — Measured matrix cells** (R20, WASM-015): yield p50 4.8 / p95 5.2 / p99 6.1 / max 6.9 ms over 384 timer samples; fixed memory unchanged after 100 cycles plus a burst (Chrome 152, multi-memory, main mode).
  *(Re-measured on the generated loader in T11 before being claimed for this revision.)*
- [x] **T9 — Reference bundles for the tests** (AC1, AC2): the control gallery, the shell, the standalone pair, the backend bundle (`tests/support/`, `examples/fixture_bundle.rs`).

## The host page's code becomes generated (plan C1, §4.1 — Q6 decided)

- [x] **T10 — `hostpage.rs` emits today's text, byte for byte** (R10–R15, R19, R20; Q6)
  - *Done 2026-10-22:* `generated_host_page` — 7/7 texts identical (loader.js 36,622 B, worker.js 5,370, app.js 2,835,
    coi-register.js 972, coi-serviceworker.js 1,922, page.html 696, app.css 999); crate green: unit 19, csp_origins 2, e2e_node 1
    (14 scenarios, 0 failed), generated_host_page 1, no_string_execution 3.
  - Files: `crates/cobolt-web-bundle/src/hostpage.rs` (new), `src/lib.rs` (`mod hostpage;`), `tests/generated_host_page.rs` (new).
  - Do: one Rust function per file — `loader.js`, `worker.js`, `app.js`, `coi-register.js`, `coi-serviceworker.js`, the page, `app.css` —
    returning the same text the `include_str!` constants (`lib.rs:36-42`) hold today, the `{{…}}` placeholders included. The assembler is not
    switched yet. No banner yet (it would break equality).
  - Verify: `cargo test -p cobolt-web-bundle --test generated_host_page` — test (d) of plan §6: each generated text equals the tracked file;
    the whole crate still green (`cargo test -p cobolt-web-bundle --no-fail-fast`, every `test result` line read).

- [x] **T11 — The assembler writes the generated code; the tracked JS/HTML/CSS go** (R10–R15, R19, R20; PRIME DIRECTIVE)
  - *Done 2026-10-22:* `hostpage.rs` writes the seven files with `BANNER_JS`/`BANNER_HTML`/`BANNER_CSS`; written in: the hashed
    loader name and `{ loader, abi }` versions (`app.js`), the ABI version, `WORKERS_CAP = 8` and `DECLARED_BROWSERS` in the R15 message
    (`loader.js`), the page from `hostpage::Page`. `assets/*.{js,html,css}` deleted from the working tree (unstaged: `git ls-files` lists
    them until the pipeline records the deletion). `generated_host_page` 3/3 — banner on 9 files, 21 files byte-identical across two
    builds, no `.js/.html/.css` under the crate; `no_string_execution` 3/3, now also over the committed
    `assets/web-player/threaded` (`player.js`, `workerHelpers.js` — it found 0 threaded files before); `e2e_node` 14 scenarios, 0
    failed; unit 19, `csp_origins` 2. **Chrome yield / fixed-memory measurements (T8) were not re-run** on the generated loader in
    this session: no browser was driven here.
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (the seven constants and the `replace` chains `:645-649`, `:717-734` give way to `hostpage`
    calls with the per-build values written in), `src/hostpage.rs`, `assets/{app.css,app.js,coi-register.js,coi-serviceworker.js,loader.js,page.html,worker.js}`
    (**deleted** — product source moving into Rust, not developer code), `tests/generated_host_page.rs`, `tests/no_string_execution.rs`.
  - Do: every emitted file starts with the banner `// Generated by PowerRustCOBOL AI at Build. Do not edit: it is written again at every Build.`
    (`<!-- … -->` for the page, `/* … */` for the stylesheet), no timestamp. Adopted rulings written by the generator: the Worker default
    `hardwareConcurrency − 1`, capped at 8, when `BundleOptions.workers` is `None` (WASM-017 Q2); the declared matrix "browsers released from
    2024 onwards" named in the R15 message (WASM-017 Q4); Trusted Types stay best effort (WASM-013 Q2, `csp.rs` unchanged). `config.json`
    keeps its shape. Test (d) is removed with the files.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast` — `generated_host_page` (a) every `.js`, the page and the stylesheet start with
    the banner, (b) two builds of the same input are byte-identical, (c) the crate holds no `.js`/`.html`/`.css` source
    (`git ls-files crates/cobolt-web-bundle` lists none); `no_string_execution` scans the **generated** output of the reference bundles
    (threaded glue and `workerHelpers.js` included); `e2e_node` still ≥ 14 scenarios, 0 failed. In Chrome on the generated loader: re-run the
    T8 yield and fixed-memory measurements and record the numbers here (or record that they were not re-run).

- [x] **T12 — The 5 MB budget line** (R8; plan C4, Q5 — *answered, to confirm*)
  - *Done 2026-10-22:* `FIRST_DOWNLOAD_BUDGET = 5_000_000`, `BuildReport.budget` (`BudgetLine`), a `first download` line in
    `text()`, a warning starting `OVER_BUDGET_PREFIX` when over. `tests/budget.rs` 2/2: fixture bundle 50,449 B → line, no warning;
    with a 5,000,000-byte player 5,049,449 B → warning, bundle still built. Q5 stays open (scope, unit, first paint).
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (`BuildReport` `:231` gains the line; one constant for the budget).
  - Do: compare the first download — the program module(s) the probe can pick, the plain player, loader, page, stylesheet — with the budget;
    over it → a **warning**, never a failure. **Working assumption, not decided:** first download, 5 × 10⁶ bytes, no first-paint target —
    Q5 is still to confirm (scope, unit, first paint); one constant to change.
  - Verify: `cargo test -p cobolt-web-bundle budget` — a bundle under the budget has a budget line and no warning; one over it carries the
    warning and still builds.

## The Build and the Check (plan C6, §4.2–§4.3)

- [x] **T13 — `WasmBuild`: project → bundle** (R1, R16, R17, R9; AC4b)
  - *Done 2026-10-23 (2.0.37)* on WASM-010's `GeneratorBackend` (`cobolt-project-tools/src/web/generator.rs`), per the owner decision
    below — no `WasmBuild` was written. Unblocked: WASM-010 T8–T10 and T19 are checked. `tests/web_build.rs`:
    `the_bundle_holds_the_page_the_modules_the_player_and_nothing_else` — `index.html`, `config.json`, `_headers`, `player/*.wasm`; a
    Chromium-only matrix gives `program.<hash>.wasm` alone, `multi_memory = false` gives `program.single.<hash>.wasm` alone; each module
    imports `pr.flush`, `pr.abort` only and `cobolt_wasm_abi::scan::bundle_problems` is empty (87,452 / 87,469 B); 24 + 24 files, none an
    interpreter, AST (no gzip stream) or runtime (**AC4b**). `a_standalone_project_has_one_entry_page_per_form` (`main-form.html`,
    `second-form.html`, R17); `a_failed_build_leaves_the_previous_bundle_as_it_was` (25 files byte-identical after a refused build);
    `a_desktop_project_is_refused_before_anything_is_written` (R9). `cobolt-wasm-abi` (feature `scan`) added as a dev-dependency.
    **Build time** (fresh `CARGO_TARGET_DIR`, `cargo build -p cobolt-project-tools --timings`, 702 units): 67.0 s wall; the WebAssembly chain
    (`cobolt-wasm`, `cobolt-wasm-abi`, `cobolt-web-bundle`, wasmtime*, cranelift*, wasmparser, wasm-encoder) 94.7 of 645.9 unit-seconds
    (14.7 %), `cobolt-wasm` 6.1 s, `cranelift-codegen` 23.7 s, `wasmtime` 10.0 s. (`cobolt-wasm` is linked **with** `host`, per WASM-010 T19.)
  - **Owner decided (operator, 2026-10-22): the Web Build backend belongs to WASM-010 T19** (`GeneratorBackend`,
    `compile_web_project`, the two `web::link` calls). This task and T14–T16 do not build a second one (`WasmBuild`):
    they run WASM-010's backend from the Web Build and check what it writes (`dist/web`, AC4b; the portable subset; the
    player copied; the backend linked). Order: WASM-010 (T8–T10, T19) runs next, then this spec resumes at T13.
  - *(Resolved 2026-10-23: WASM-010 T8–T10, T19 done.)* **BLOCKED 2026-10-22 on WASM-010 T8/T9/T10** (all unchecked in `specs/WASM-010-webassembly-code-generation/tasks.md`).
    `compile_project` (`cobolt-wasm/src/module.rs:161`) has no caller anywhere and does not yet produce a bundleable module: its
    exports are `memory`, `arena`, `pr_abi`, `pr_variant`, `pr_arena_base`, `pr_init`, `pr_run` (`module.rs:817-823`) — no
    `pr_open_form`, `pr_close_form`, `pr_event`, `pr_resume`, `pr_cancel`, `pr_tick`, `pr_status` — and its embedded manifest is a
    `json!` with `"forms": []` and no `names`/`events`/`builtins`/`sizes` (`module.rs:878-890`). `scan::bundle_problems`
    (`cobolt-wasm-abi/src/scan.rs:196-213`) refuses a module missing a required export or exporting `pr_run`, and
    `Manifest::parse` needs the missing fields, so `assemble_bundle` would refuse every module this Build could generate; the
    verification (`dist/web` with the modules, AC4b) cannot pass. **Also for the operator:** WASM-010 T19 plans the same backend
    (`web/generator.rs`, `GeneratorBackend`, the same two `web::link` calls) that this list plans as T13–T16 (`web/build.rs`,
    `WasmBuild`); one of the two lists should own it. T14–T22 depend on T13 and were not started.
  - Files: `crates/cobolt-project-tools/Cargo.toml` (adds `cobolt-wasm = { path = "../cobolt-wasm", default-features = false }` — no `wasmi` —
    and `cobolt-web-bundle`), `src/web/build.rs` (new), `src/web/mod.rs` (`pub mod build;`; `NotYetLinked` stays as the fallback),
    `tests/web_build.rs` (new), a hello-world Web fixture under `tests/fixtures/`.
  - Do: plan §1 steps 1–6. Per form in `request.forms`: load it with `cobolt-forms`, `cobolt_codegen::generate_with_map`; lex, parse and
    `cobolt-semantic` with the Common Code of `request.sources`, as the desktop Build does; one `FormSpec` per form; `compile_project` once
    per variant of `request.variants` with `Options { multi_memory, conformance: false, tolerate_undeclared: false, .. }`; the player from
    `WasmBuild`'s `player_dir` (T15); `assemble_bundle`; write to a sibling temporary folder and rename it over `dist/web`; map
    `cobolt_web_bundle::BuildReport` onto `web::BuildReport` (sizes, warnings — the budget line among them, duration). Generated COBOL and
    its banner unchanged; only `dist/web` is written (user code is sacred).
  - Verify: `cargo test -p cobolt-project-tools --test web_build --no-fail-fast` — `dist/web` holds `index.html`, the modules `module_variants`
    asks for, the player, `config.json`, `_headers`; each module's import section lists only ABI v1 §3 imports and no file is an interpreter,
    AST blob or runtime library (**AC4b**); a standalone-style project has one entry page per form (R17); a second build that fails leaves
    the first bundle byte-identical (atomic replace); a Desktop manifest is refused before anything is written (R9). Record the added
    build time of `cobolt-project-tools` with `cobolt-wasm` (plan §5 risk), measured with `cargo build -p cobolt-project-tools --timings`.

- [x] **T14 — The portable subset at Check and at Build** (R3, AC3; plan C5, §4.3)
  - *Done 2026-10-23 (2.0.37):* `WebBuildBackend::check` (default `Err(NotYetLinked)`); `GeneratorBackend::compile` shared by `check` and
    `build` (steps 1–3, nothing written); `check_project` adds each `Refused` line of a Web/Mobile project as an error (`web_finding`: file,
    line, form, site parsed from the location); the IDE's `do_check` calls `check_web_portability` (heading `web_check_not_portable`, then
    each line through `web_preview::diagnostic_text`, translated by kind). `web_build portable`: 3/3 — disk `OPEN INPUT` refused at
    `webforms/main-form.cfrm (main-form ▸ BTN-OK ▸ onClick):2` naming `file IN-FILE` and the backend alternative, Check = Build, no
    `dist/web`, `check_project` returns the same line (file, line 2, site, form); a `STORAGE IS MEMORY` INDEXED file and an
    INPUT/OUTPUT-PROCEDURE `SORT` build; **9 R3 constructs** refused at the handler, each naming an alternative (disk file, disk INDEXED,
    `WITH PERSISTENCE`, `EXEC SQL`, `COBOL-OPEN-DB`, `COBOL-HTTP-GET`, `COBOL-AWS-SET-CREDENTIALS`, `COBOL-KEY-SET`, `EXEC RUST`).
    `cargo test -p cobolt-ide --bin cobolt-ide check`: 22 passed.
    Found, not changed: (1) the language has no verb of its own for a child process, an OS thread or a native socket — in the runtime they
    are behind built-ins (AWS host `aws/process.rs:316` `spawn`; HTTP/SQL async `interpreter.rs:3524`, `:13087` `thread::spawn`), which
    the walk covers by their built-ins; the AWS/HTTP/SQL **controls** reached by `INVOKE` were not walked (WASM-010's to confirm). (2) A
    file declared in the form's COBOL Structure `FILE SECTION` with `FD … IS GLOBAL` is not visible to a handler in the generator
    (`CUST-ID is not defined`); the test declares its files in the handler — a parity question for WASM-010, not decided here.
    `WITH PERSISTENCE` refused stays the **working assumption, to confirm** (the generator already refuses it, `support.rs` `DISK_FILES`).
  - Files: `crates/cobolt-project-tools/src/web/build.rs`, `src/web/mod.rs` (the trait gains `fn check(&self, request) -> Result<(), WebBuildError>`,
    default `Err(NotYetLinked)`, so existing implementors and the test stubs compile), `src/tools/check.rs` (`check_project` `:104`: for a Web
    or Mobile project, the refusals join the report as errors), `crates/cobolt-ide/src/app.rs` (`do_check` `:4488`: when the open project's
    manifest gives a `WebBuildRequest`, each refused line is pushed as an error diagnostic), `tests/web_build.rs`.
  - Do: one refusal source — the generator. `check` runs steps 1–3 of T13 and discards the bytes; `build` and `check` share that code.
    Each `GenError::all()` entry is mapped through the `SourceMap` to `file:line: construct — alternative` (the `.wcfrm` handler line,
    not the generated `.cbl` line). Portable per the amended R3: `INDEXED … STORAGE IS MEMORY` (`StorageMode::Memory`) and in-memory
    `SORT`/`MERGE`. **Working assumption, to confirm (plan §7):** `STORAGE IS MEMORY … WITH PERSISTENCE` needs the local disk and is refused.
    A refusal message without an alternative is a generator gap reported to WASM-010, not papered over here.
  - Verify: `cargo test -p cobolt-project-tools --test web_build portable --no-fail-fast` — **AC3**: a handler with `OPEN INPUT` of a disk
    file → `Refused` naming the `.wcfrm` handler line, the construct and an alternative; `dist/web` not written; `check_project` returns the
    same lines; a `STORAGE IS MEMORY` INDEXED file and an in-memory `SORT` build; a test walks the refusal for each R3 construct (disk file,
    disk indexed / `redb`, child process, OS thread, socket, SQL, `EXEC RUST`, embedded secret) and asserts it names an alternative;
    `cargo test -p cobolt-ide --bin cobolt-ide check` green.

- [x] **T15 — Which player the Build copies** (R1 step 4; WASM-016 R11/R12, Q6 decided)
  - Files: `crates/cobolt-project-tools/src/web/build.rs`.
  - Do: `WasmBuild::new(player_dir)` — the caller resolves `assets/web-player` with the bundled-asset pattern the IDE and `rcrun` already use
    (`app.rs:2442` `themes_dir`, `app.rs:20415` `images_dir`, `cobolt-cli/src/main.rs` `cmd_mcp`): `<exe dir>/assets/web-player` if it is a
    directory, else `assets/web-player`. A missing folder or `player-build.json` → `WebBuildError::Failed` naming the path. The **pin**:
    the manifest field is WASM-016 R11's and does not exist (`project_kind.rs` has none); until it does, the current player is used; once it
    does, a pin the compiler can no longer emit for fails naming the version.
  - Note: no packaging script copies `assets/web-player` into an installed IDE today (no match outside the specs, `Cargo.toml` and the bundle
    tests); shipping it is WASM-016 R12's. Until then an installed IDE fails this step with the readable message.
  - Verify: `cargo test -p cobolt-project-tools --test web_build player` — a missing player folder fails naming it; the committed one builds.
  - *Done 2026-10-23 (2.0.37)* in `generator.rs`: `player_dir()` / `player_dir_from(env, exe_dir)` — `PRC_WEB_PLAYER_DIR` when set (that
    folder only), else `<exe dir>/assets/web-player`, `assets/web-player`, the repository's copy; a missing folder or `player-build.json`
    → `Failed` starting `PLAYER_UNAVAILABLE_PREFIX` and naming the path. Pin: no manifest field yet, current player used. `web_build player`
    2/2.

- [ ] **T16 — Link the backend where the program starts** (R1; plan §1 layer 2)
  - *2026-10-23, the manual MCP check run outside the pipeline:* `rcrun mcp` (2.0.37) — `create_project` kind web → `check` 0 errors, 0
    warnings → `build` writes `dist/web` in 93 ms (page, `config.json`, `_headers`, both players, `program.wasm` + `program.single.wasm`, SBOM).
  - *2026-10-23:* the links exist (WASM-010 T19: `cobolt-ide/src/main.rs:102`, `cobolt-cli/src/main.rs:83`, `GeneratorBackend`).
    `cargo test -p cobolt-ide --bin cobolt-ide web_preview` 15 passed; `--test tools build` matches no test (0 run) and `--test build`
    0 passed, 1 ignored (pre-existing); `--lib build` 4 passed. **Not checked:** the manual MCP check — the PowerRustCOBOL MCP server
    failed to connect in this session and the stdio substitute (`rcrun mcp` driven by a pipe) was not permitted. Note for it: the tools'
    `BuildReport` carries the budget only as a warning when over (plan §3, no new field), so an under-budget build shows no budget line.
  - Files: `crates/cobolt-ide/src/main.rs` (`main` `:99`), `crates/cobolt-cli/src/main.rs` (`cmd_mcp` `:402`).
  - Do: one `cobolt_project_tools::web::link(Arc::new(WasmBuild::new(player_dir)))` each, as `web::link`'s doc comment says
    (`web/mod.rs:276-278`). The IDE's Build button, its Preview (`app.rs:2701`) and the agents' `build` tool (`tools/build.rs:98`) then get it
    with no screen change.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide web_preview` and `cargo test -p cobolt-project-tools --test tools build` green;
    through the PowerRustCOBOL MCP tools (not by driving the window): `open_project` on a Web template project, `check`, then `build`
    answers `"status": "built"` with `"engine": "cobolt-wasm"` and the size report including the budget line.

## Acceptance evidence

- [x] **T17 — A secret in a `.wcfrm` stops the Build** (R7, AC5)
  - *Done 2026-10-23 (2.0.37):* `web_build secret` 1/1 — AWS's documented example key (built from fragments) in a Label `Caption` of
    `webforms/main-form.wcfrm` → `Failed`: `forms/main-form.<hash>.cfrm: line 10: secret (cloud access key id); the value is not shown`;
    `dist/web` unchanged. The message names the bundle's copy of the form, not its project path (a possible follow-up). Credential scan of
    the diff: no hit.
  - Files: `crates/cobolt-project-tools/tests/web_build.rs`.
  - Depends on: WASM-001 T15.5 (`.wcfrm`/`.mcfrm` in `secrets::is_text_path`, `secrets.rs:259`).
  - Verify: `cargo test -p cobolt-project-tools --test web_build secret` — a deliberate test key in a `.wcfrm` property stops `WasmBuild`
    naming the file and the pattern, never the value; `dist/web` unchanged. Run the credential scan on the staged diff before the commit
    (the fixture key is a recognisable fake, never a real one).

- [x] **T18 — A Web build needs no tool** (R6, AC6 — Web half)
  - *Done 2026-10-23 (2.0.37):* `web_build no_tool` 2/2 — the test binary re-runs itself with `PATH=""`; the hello-world builds (25 files).
  - Files: `crates/cobolt-project-tools/tests/web_build.rs`.
  - Verify: `cargo test -p cobolt-project-tools --test web_build no_tool` — the hello-world build succeeds with `PATH` emptied for the
    process (no `cargo`, `rustc`, `wasm-bindgen`, `node`). The Mobile half (Tauri 2 CLI, Android SDK/NDK, Xcode, Rust; `rustup target add`
    offered after consent, the SDKs not) is WASM-012's.

- [ ] **T19 — The Build's own bundle in node and Chrome** (AC1, AC2)
  - *2026-10-23, Chrome half run outside the pipeline* (Chromium 152.0.7977.167 in the desktop app's browser pane; the 2.0.37 Web template
    built through `rcrun mcp`, served by `cobolt-web-bundle --example serve --isolated`, `crossOriginIsolated` true):
    **AC2 passes** — mounted in a 420×300 `<div>` the app fills only that div, follows a resize to 640×420, nothing paints past it; a host
    `button {background:red !important; font-size:40px}` does not reach the app's button (rgb(58,62,71), 14px) and the app's `.pr-window`
    rules do not reach a host `<p class="pr-window">` (Georgia, rgb(10,20,30)). **AC1 fails:** a click with the box empty shows "Type your
    name first."; after typing "Ada" the click leaves `"\u0001"` in `LBL-RESULT` instead of "Hello, Ada" — the template's
    `STRING "Hello, " TXT-NAME::Text::Trim() DELIMITED BY SIZE INTO LBL-RESULT::Caption` (a generator defect, fixes-2x). Also seen: mounted in
    a `<div>` the app does not paint its window background (it comes from `body.pr-window`), so its white labels sit on the host page.
  - *2026-10-23, node half done:* `node_the_builds_own_hello_world_answers_a_click` (in `cobolt-project-tools/tests/web_build.rs`, the
    harness reached by `#[path]`, no dependency cycle) — the Build's bundle, a click on `BTN-OK` writes `Hi`, multi- and single-memory, under
    node v23.3.0 (**AC1, node**). `PRC_WEB_E2E_KEEP=1` keeps the bundle for a browser run. **Stopped here:** the Chrome half (AC1 in Chrome,
    AC2 — mounted in a `<div>`, resize, computed styles) needs a host test page and script served beside the bundle; writing them was not
    permitted in this session, so Chrome was not driven and **AC2 is not measured**. T8's yield/memory numbers were not re-run either.
  - Files: `crates/cobolt-web-bundle/tests/support/` or `crates/cobolt-project-tools/tests/web_build.rs` (whichever reaches the harness
    without a dependency cycle; decided when written).
  - Verify: the hello-world bundle **from `WasmBuild`** (not the fixture module), served by `cargo run -p cobolt-web-bundle --example serve`,
    answers a button click under node and in Chrome (**AC1**); mounted into one `<div>` beside other content, it fills only that `<div>`,
    follows its resize, and a computed style asserted on each side shows no leak either way (**AC2**). Record the browser version used.

- [ ] **T20 — Desktop builds as before** (R9, AC8; plan C3)
  - *2026-10-23, run outside the pipeline on `ddaa2d12` (2.0.37):* `cobolt-compiler` + `cobolt-cli` + `cobolt-web-bundle` — 297 passed,
    4 failed: the known `cobolt-compiler` SDK failures (`sdk_crate_set_is_closed_under_path_dependencies` and three `resolve_main_tests`;
    the SDK crate set lacks `cobolt-numedit`/`cobolt-decimal`, added at 2.0.13/2.0.16) — a fixes-2x item. Box stays open until it is fixed.
  - Files: none changed — this is the check that none of the desktop path was touched.
  - Verify: `git diff --stat` shows no change to `crates/cobolt-compiler/src/lib.rs` (`build_project`), `bin/` handling, eframe or the
    host-triple rule; `cargo test -p cobolt-compiler --no-fail-fast` and `cargo test -p cobolt-cli --no-fail-fast` green, every `test result`
    line read.

## i18n and the System KB

- [x] **T21 — IDE strings in six languages** (spec §6 i18n; plan §2, §7)
  - Files: `crates/cobolt-ide/src/i18n.rs`, `crates/cobolt-ide/src/panels/web_preview.rs`, `crates/cobolt-ide/src/app.rs` (`do_check`).
  - Do: three `Tr` keys, each in English, Spanish, Portuguese (pt-BR), French, Japanese and Chinese — `Tr::WebCheckNotPortable` (heading
    above the refused lines in Check), `Tr::WebBuildOverBudget` (the budget warning), `Tr::WebBuildPlayerUnavailable` (the missing or pinned
    player). The refused lines themselves stay the generator's English diagnostics, like every compiler diagnostic; the loader's messages are
    the application's texts (`BundleOptions.texts`), not IDE strings.
  - **Plan gap, working approach (to confirm at `/analyze`):** plan §3 carries the budget warning in `web::BuildReport.warnings` as a
    `String` (no new field) and the player error as `WebBuildError::Failed(String)`, while §7 asks for them translated. Working approach:
    `cobolt_project_tools::web` exports the two English texts' fixed prefixes as `pub const`, and the IDE shows the translated heading followed
    by the English detail when a line starts with one — tools and agents keep their English answer; no format change.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` — `no_empty_ui_translations`, `non_english_is_actually_translated` and a new
    test asserting the three keys in all six languages and their placeholders kept.
  - *Done 2026-10-23 (2.0.37), ahead of order because T14's Check needed the heading:* `web_check_not_portable`, `web_build_over_budget`,
    `web_build_player_unavailable` (fields are snake_case in `Tr`); the working approach above (prefix consts `web::OVER_BUDGET_PREFIX`,
    `web::PLAYER_UNAVAILABLE_PREFIX`; heading + English detail in `Note::lines`). `cargo test -p cobolt-ide --bin cobolt-ide i18n` 19 passed
    (incl. `web_build_headings_in_six_languages`, `no_empty_ui_translations`, `non_english_is_actually_translated`); `web_preview` 15 passed
    (incl. `the_budget_warning_and_the_missing_player_have_a_translated_heading`). The plan gap stays for `/analyze` to confirm.

- [x] **T22 — System KB stays true** (steering: the System KB rule)
  - Files: only if needed — the compiler's doc constants in `crates/cobolt-compiler/src/` and `assets/knowledge/chunked.data`.
  - Do: no control, property, method or event is added. If T14 changes a refusal message that a compiler doc table quotes, update that table
    and regenerate `chunked.data` in the same change; otherwise record "no KB text affected" here. The portable-subset list for Grace is the
    documentation/KB spec's (spec §6).
  - Verify: `rg` the compiler doc constants for each changed message; if any matched, the KB freshness test is green after regeneration.
  - *2026-10-23:* no refusal message changed (they are the generator's, unchanged); the one changed text, the missing-player error, and the
    budget warning appear nowhere in `crates/cobolt-compiler/src` (`rg "web player is not|PRC_WEB_PLAYER_DIR|first download is"`: no match).
    **No KB text affected**; `chunked.data` not regenerated.

## Not this spec's, or not measurable here

- [ ] **T23 — Mobile build** (R2, AC7): WASM-012. This Build writes the bundle a Mobile shell wraps (`from_manifest`, `Isolation::Off`
  for Mobile, `web/mod.rs:95`); isolation inside the Mobile web views is WASM-017 Q5, **not answered**.
- [ ] **T24 — Firefox, Safari, iOS, Android WebView cells of the matrix** (R18): **cannot be measured here** — none of those browsers is
  installed on this machine. Run them where they are, with the same node and page scenarios; never claimed until run.
- AC4 (same output as the desktop runtime) is WASM-010's parity gate (R16, AC7–AC8); this spec adds nothing of its own.
- The threaded player's nightly bump (WASM-017 Q9: `build_player.rs:35`, `tests/freshness.rs:104`) and the player's Leptos alignment
  (plan §4.4) are WASM-016/017's files.

## Finalize

- [ ] **T25 — Version, CHANGELOG, tests of the touched crates**
  - Files: `crates/cobolt-ide/src/version.rs` (`VERSION`, `2.0.29` when this list was written — bump `z` **once** from whatever it is then),
    `CHANGELOG.md`.
  - Do: one entry above the previous `## [PowerRustCOBOL 2.0.x]` heading, dated, with *(Written on `features-2x`, after 2.0.N.)*, saying what
    changed (generated host page, budget line, the Web Build and Check). **No documentation task:** `README.md` and `docs/` are not touched on
    2.x.x; what 2.x.x says about itself stays in `specs/`. Update this file's boxes and the spec's AC boxes with evidence.
  - *2026-10-23, partly:* `VERSION` 2.0.36 → **2.0.37** (once) and the CHANGELOG entry written. Run: `cargo test -p cobolt-project-tools
    --no-fail-fast` — lib 101, build 0 (1 ignored), crates 3, exec_sql_check 2, form_design_rule 6, http 2, kinds 7, layout_rule 6, tools 13,
    web_build 15, all passed; `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` — 1,872 passed, **5 failed**, 4 ignored: the five
    recorded before this change in WASM-010 T25 (`every_completion_description_exists_in_six_languages`,
    `every_document_ships_in_every_language`, `live_resolver_verdicts`, `compatible_and_reserved_collisions_still_refuse`,
    `claude_validates_the_written_bundle`); `cargo build -p cobolt-cli -p cobolt-ide` green. rustfmt: the new test code and `generator.rs`
    (clean at HEAD) formatted; `check.rs`, `mod.rs`, `web_preview.rs`, `app.rs` already carried rustfmt differences and were not reformatted
    (the edits add none in `check.rs`/`mod.rs`; `web_preview.rs` follows its wide-line style). **Not run:** `cobolt-web-bundle`, `cobolt-cli`,
    `cobolt-compiler` sweeps (T20/AC8) and the MCP manual check. AC boxes of the spec not ticked: none is fully proved by this run.
  - Verify: `df -h .` first (a worktree sweep needs disk); then `cargo test -p cobolt-web-bundle --no-fail-fast`,
    `cargo test -p cobolt-project-tools --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`,
    `cargo test -p cobolt-cli --no-fail-fast`, `cargo test -p cobolt-compiler --no-fail-fast` (AC8) — every `test result` line read and
    reported; `cargo fmt --check` on the touched crates. Manual check through the MCP tools as in T16.

## Done criteria

| Acceptance criterion | Covered by |
|---|---|
| AC1 — hello-world builds and answers a click | T9 (fixture, done), T13, T19 |
| AC2 — embedded in a `<div>`, no CSS leak | T9 (fixture, done), T11 (generated page), T19 |
| AC3 — a non-portable line fails Check, no bundle | T14 |
| AC4 — same output as the desktop | WASM-010 parity gate (not here) |
| AC4b — no interpreter/AST/runtime; imports = host API | T13 |
| AC5 — a key in a form stops the Build | T2 (done), T17 |
| AC6 — missing-tool messages | T18 (Web half); Mobile half WASM-012 |
| AC7 — Android and iOS packages | T23 → WASM-012 |
| AC8 — Desktop builds as before | T20 |

Open, not decided by this list: Q5 (budget scope, unit, first paint — T12's assumption); `WITH PERSISTENCE` on a MEMORY file (T14's
assumption); WASM-017 Q5 (Mobile isolation); the T21 translation carrier (plan gap). Do not commit or push — the pipeline does the
bookkeeping.
