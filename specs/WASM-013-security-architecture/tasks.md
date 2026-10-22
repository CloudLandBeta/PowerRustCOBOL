<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Security architecture and threat model for the WebAssembly line (WASM-013)

- **Status:** draft. Written 2026-10-22 from `plan.md` (draft of 2026-10-22). Much of the policy code already exists (plan §0.1); these tasks close
  the gaps G1–G11 and add the assurance layer. Two tasks are **gated** on work other specs own (T7 on WASM-002's linked Web Build, parts of T18 on
  WASM-009's sign-in), and one is gated on the operator (T1's tool install and licence list).
- **Plan:** ./plan.md   **Date:** 2026-10-22

Ordered, small, independently-verifiable tasks. Each names the files it touches, the requirement(s) it satisfies, and how to verify it. Check off
as completed.

**2.x.x overrides for this list.**
- There is **no documentation task**. `README.md` and everything under `docs/` are never touched on 2.x.x.
- What 2.x.x says about itself goes in `specs/`: the threat model and the release-review rule (T23), and the `WASM-PROGRESS.md` owed rows (T24).
- The user-facing description of the new manifest tables lives in the System KB text (T22).
- The finalize task (T25) bumps `z` once and writes one CHANGELOG entry.
- No task commits, pushes, checks out or switches branches.

**Grounding read for this list (2026-10-22, `features-2x` at 2.0.30, `cdad25cd`).** Every path below was opened for this list.
- **Bundle — `crates/cobolt-web-bundle/`.**
  - `src/csp.rs`: `CspOptions` `:26`, `CspOptions::strict` `:42`, `Policy` `:49`, `build` `:68`, `meta_content` `:108`, `header_value` `:113`,
    `header_only_directives` `:128`, `security_headers` `:138`, `host_files` `:165`.
  - `src/lib.rs`: `BundleOptions` `:139` with `release` `:159`, `unsafe_plain_ws` `:160`, `skip_secret_scan` `:161`; `BuildReport` `:231` and
    `BuildReport::text` `:246`; the release refusals `:465-477`; the SBOM written at `:769`; the secret-scan loop `:772-786`.
  - `src/secrets.rs`: `Severity` `:24`, `Finding` `:31`, `secret_property` `:168` (its name list `:175` has no `connectionstring`), `scan_text`
    `:197`, `is_text_path` `:257`. `is_text_path` lists `.cfrm` but **not** `.wcfrm`/`.mcfrm`. No `Key=value` connection-string rule exists:
    `has_credentialed_url` covers URLs only.
  - `src/devserver.rs`: `ServeOptions` `:25` has `extra_headers`; `serve` `:210`; `handle` `:154`.
  - The tests are `csp_origins.rs`, `e2e_node.rs`, `no_string_execution.rs` and `support/`.
- **Project tools — `crates/cobolt-project-tools/src/web/`.**
  - `mod.rs`: `WebBuildRequest` `:46` (no profile field), `from_manifest` `:71`, its own `BuildReport` `:164`, `WebBuildError` (`Refused`,
    `Failed`, `NotYetLinked`), the `WebBuildBackend` trait `:225`, `NotYetLinked` `:260`, `link` `:279`, `backend` `:289`.
  - `devserver.rs` is a second dev server (`start` `:168`).
  - **No `build.rs` exists yet.** Nothing outside `cobolt-web-bundle` constructs `BundleOptions`, and `cobolt-project-tools/Cargo.toml` does not
    depend on `cobolt-web-bundle`. The Web Build is therefore still `NotYetLinked`: mapping a profile or an `unsafe` setting onto `BundleOptions`
    can only happen in WASM-002's linked backend (T7).
- **Manifest — `crates/cobolt-compiler/src/project_kind.rs`.** `WebSettings` `:273` and `MobileSettings` `:320` carry `#[serde(default)]` fields
  and have no `unsafe`, `pinning` or `security` table. `crates/cobolt-compiler/src/lib.rs` holds the KB section "Project kinds: Desktop, Web and
  Mobile" at `:6172`. `crates/cobolt-compiler/src/backend_build.rs` exists. **`mobile_build.rs` does not exist yet** (it is WASM-012's).
- **Server — `crates/cobolt-server/`.**
  - `src/http.rs`: `SECURITY_HEADERS` `:128`.
  - `src/serve.rs`: the download route writes `Access-Control-Allow-Origin` at `:390`; `origin_allowed` `:423`.
  - `src/config.rs`: `Secret` `:31`, `RunMode` `:52`, the `unsafe_*` fields `:229-233`, `resolve_secret` `:329`, `ServerConfig::resolve` `:358`,
    the release refusals `:427-442`, the `"*"` refusal `:449`.
  - `src/dev.rs`: listens on `127.0.0.1:0` `:70`, sets `unsafe_allow_any_origin = true` `:73`, resolves `RunMode::Development` `:74`.
    `src/standalone.rs:84` resolves `RunMode::Release`.
  - `src/tlsutil.rs`: `acceptor` `:23` and `connector` `:36`, both with a TLS 1.2 minimum. `src/jwt.rs`: `TokenValidator` `:70`, `validate`
    `:88`. `src/log.rs`: `LogRecord.subject` `:34`, `MemorySink` `:77`.
  - The tests are `client_contract.rs`, `config_rules.rs` (`a_release_start_refuses_every_unsafe_setting_and_a_development_start_only_warns`
    `:47`), `dependency_guards.rs`, `loopback.rs`, `token_rules.rs`.
- **Wire — `crates/cobolt-wire/`.** `proto::parse_client` `:340`, `proto::parse_server` `:394`, `json::parse(text, max_bytes)` `:94`. The tests
  are `random_round_trip.rs` (its doc names WASM-013 R15) and `vectors.rs`.
- **Player — `crates/cobolt-web-player/`.** It is a **separate workspace**: root `Cargo.toml:45` `exclude`. No workspace crate's test can link it.
  - `tests/sinks.rs` `FORBIDDEN` `:23`; `tests/abi_host.rs`.
  - `src/view/viewer.rs`: `safe_color` `:109`, `link_allowed` `:156`, `blocks_to_nodes` `:290`.
- **IDE — `crates/cobolt-ide/`.**
  - `src/secrets/mod.rs`: `NATIVE_STORE_DISABLED = true` `:46`.
  - `src/i18n.rs`: `Tr` is a struct of `&'static str` fields (`:196`); the Web/Mobile project-settings keys `pk_set_*` start at `:376`; the
    completeness test is `no_empty_ui_translations` `:15273`.
  - `src/panels/web_settings.rs` draws the Web/Mobile settings (`:210`).
  - `src/panels/web_preview.rs`: `Note` `:65` (`BuildWarning`, `BuildRefused`), `Note::lines` `:140`.
  - `src/app.rs`: `start_web` `:2710`, which hands the Preview's notes to the Output panel at `:2744`.
  - The KB freshness test is `grace_host.rs` `prebuilt_chunked_kb_matches_the_published_documentation` `:4780`. Regenerate with
    `cargo run -p cobolt-ide --example build_chunked_kb`.
  - `src/version.rs:22` is `2.0.30`.
- **Elsewhere.**
  - `crates/cobolt-graphics/tests/display_list_robustness.rs` is the `GRAPHICS_FUZZ_ITERS` pattern (`:12`, `:61`).
  - `crates/cobolt-web-preview/src/main.rs` `may_navigate` `:95`.
  - `crates/cobolt-compiler/tests/dependency_guards_063.rs` exists.
- **Not present.** There is **no `deny.toml`**, and **`cargo-deny` is not installed** (no `cargo-deny` in `~/.cargo/bin`). WASM-012's
  `store_required` is not in the code yet; it is in its plan only.

**Deviations from the plan, found while grounding (each is the smallest change that keeps the plan's intent).**
- **Δ1 — CSP test server.** Plan §1.2 puts the `POST /__csp-report` sink in `cobolt-project-tools/src/web/devserver.rs`, and its test in
  `cobolt-web-bundle/tests/csp_browser.rs`. `cobolt-web-bundle` cannot reach `cobolt-project-tools` (no dependency edge). The sink therefore goes
  into `cobolt-web-bundle/src/devserver.rs`, which already serves extra headers and is the server that crate's tests use (T12).
- **Δ2 — the AC10 test.** Plan §1.8 has one `cobolt-server` test drive the player host. The player is an excluded workspace, so the test splits
  into a server half and a player half. Both halves plant the same values and use the same grep helper shape (T18).
- **Δ3 — `.wcfrm`/`.mcfrm` in `is_text_path`.** Plan §2 lists this as owed by WASM-001. AC6 needs a planted secret in a Web form, and the function
  is in this spec's `secrets.rs`, so T2 adds the two extensions. If WASM-001 lands first, nothing more is needed.

**Open, carried and not decided** (plan §7). The working assumption is in brackets.
- **O1 — what "marked sensitive" means.** [A password-mode control, plus a `sensitive` contract field (WASM-014). `DISPLAY` is not filtered.]
- **O2 — which commands are Release.** [IDE Build and `rcrun build` are Release. Run and Preview are Development.]
- **O3 — the product repository's own `SECURITY` file.** [None is added.]
- **O4 — the licence allow-list beyond the permissive set.** [The operator's call at T1.]
- **O5 — the TLS cipher floor.** [TLS 1.2 plus the OS defaults, recorded as an accepted residual risk.]
- **Dependencies on decisions open elsewhere.** WASM-INDEX D5 (a small HTTPS surface) and WASM-017 Q5 (isolation in the mobile web views).

**Decided by the operator (binding).**
- **Q1:** `release-review.md` with `kind = review | penetration-test` (T23).
- **Q2:** Trusted Types are enforced where the browser supports them; this is already built in `csp.rs`.
- **Q3:** known shapes are errors and entropy is a warning (T2 keeps it).
- **Q4:** a `[security]` project parameter becomes `security.txt` (T5, T14).
- **Q5:** `[mobile.pinning]` is an option (T5, T21).

## M0 — verify before building

- [ ] **T1 — Close the plan's `[verify]` points; fix the licence list (operator-gated)** (R9; prepares AC8)
  - Files: none in the tree. Record the findings in this file, under T1, at `/implement`.
  - Do:
    1. **Install the tool, with the operator's approval.** `cargo-deny` is not installed. Installing it changes the machine, not the repository,
       so ask the operator before running `cargo install cargo-deny --locked`. Check that the version builds with the toolchain in use.
    2. **Confirm offline use with a local advisory database.** Confirm that `cargo deny check advisories` reads an advisory database from a
       **local path with fetching disabled**: `[advisories] db-path` plus `--offline` / `--disable-fetch`, whichever the installed version
       documents in `cargo deny check --help`.
       - If it does, T16 uses it.
       - If it does not, T16 uses the plan's named fallback: an in-repo Rust matcher over a local RustSec checkout.
    3. **List today's licences.** Run `cargo deny list` (or `cargo deny list -l crate`) over today's `Cargo.lock`, and over the player's own lock
       (`--manifest-path crates/cobolt-web-player/Cargo.toml`).
       - List every licence found.
       - Licences outside the plan §1.6 set (Apache-2.0, MIT, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, Unicode-DFS-2016, MPL-2.0) go to the
         operator (O4). They are **never added silently**.
    4. **Record the iOS pinning keys.** From Apple's documentation, record the exact keys (`NSPinnedDomains`, `NSPinnedLeafIdentities`,
       `NSPinnedCAIdentities`) and the minimum iOS version that honours them. From Android's documentation, record `<pin-set expiration>` and
       its minimum API level. These are inputs for T21 and WASM-012.
  - Verify: the four findings are written here, with the tool version and the date. The licence list is either approved by the operator or
    explicitly left open as O4. **No task from T16 onward starts on an unverified assumption.**

## M1 — secrets (R7, R8)

- [ ] **T2 — The pattern list is public and complete; one project-scan function** (R7, Q3; AC6 logic)
  - Files: `crates/cobolt-web-bundle/src/secrets.rs`, `crates/cobolt-web-bundle/src/lib.rs` (`:772-786`).
  - Do:
    - Add `pub const PATTERNS: &[(&str, Severity)]`, naming every pattern `scan_text` can push. It is one table, and `scan_text` pushes only
      names taken from it.
    - Add the `Key=value` connection-string rule as an **Error**. The keys are `Password=`, `Pwd=`, `AccountKey=` and `SharedAccessKey=`; the
      match ignores case and the value must be non-empty and not a placeholder (`${…}`, empty, `*`/`•` only, `<…>`), the same rules as
      `secret_property`.
    - Add `connectionstring` to the `secret_property` name list.
    - Add `.wcfrm` and `.mcfrm` to `is_text_path` (Δ3).
    - Add `pub fn scan_project(files: &[(String, String)], entropy_len: usize) -> Vec<Finding>`, which filters with `is_text_path` and calls
      `scan_text`.
    - Rewrite the bundle loop in `lib.rs` to call `scan_project`. It keeps the exclusions for `player/*` and `sbom.cdx.json`, so there is one
      function for both scopes.
  - Verify:
    - `cargo test -p cobolt-web-bundle --no-fail-fast` is green.
    - New unit tests plant each `PATTERNS` entry, **built from fragments** as the existing tests do. Each is found with its severity; entropy
      stays a Warning.
    - Every `Finding`'s `Display`/`Debug` text contains no planted value.
    - The existing "innocent lines" cases still pass.
    - Placeholder `ConnectionString` values are not flagged.

- [ ] **T3 — The committed examples stay clean under the new shapes** (R7)
  - Files: `crates/cobolt-web-bundle/tests/examples_clean.rs` (new).
  - Do: run `scan_project` over the **tracked** text files under `examples/` (`git ls-files examples`; an untracked local file must not
    decide the result) and assert there are **zero Errors**. On a failure, print only file, line and
    pattern, **never the value** (credentials rule).
    - If a real-looking value is found, **stop and tell the operator**. Do not open or print it, and do not "fix" the example: user code is
      sacred, and the example `.cfrm` files may carry live keys.
  - Verify: `cargo test -p cobolt-web-bundle --test examples_clean -- --nocapture` passes and prints the file count, or the operator has been
    told which file and pattern hit.

- [ ] **T4 — A release backend refuses a literal secret** (R7, R8; AC7 backend half)
  - Files: `crates/cobolt-server/src/config.rs` (`ServerConfig::resolve` `:358`), `crates/cobolt-server/tests/config_rules.rs`.
  - Do:
    - In `RunMode::Release`, refuse a `Secret` field whose text is neither `env:…` nor `file:…`. The fields are `tls.pkcs12_password`, the
      identity client secret and `recaptcha.secret`, where those fields exist in `config.rs` today. Fields that do not exist yet are a WASM-009
      row in T23.
    - The message names the key and says "use env:NAME or file:PATH". It never contains the value.
    - `RunMode::Development` accepts the literal and adds a warning.
  - Verify:
    - `cargo test -p cobolt-server --test config_rules --no-fail-fast` is green.
    - New cases: in Release a literal is refused with its key named and the value absent from the error; `env:` and `file:` resolve; in
      Development there is a warning and no error.
    - The existing `a_release_start_refuses_every_unsafe_setting…` still passes.

## M2 — settings, profile, warnings (R11, R12)

- [ ] **T5 — Manifest tables: `[web.unsafe]`, `[mobile.unsafe]`, `[mobile.pinning]`, `[security]`** (R11, Q4, Q5)
  - Files: `crates/cobolt-compiler/src/project_kind.rs`.
  - Do:
    - Add `UnsafeSettings { plain_ws, skip_secret_scan, csp_report_only }` (all `bool`, `#[serde(default)]`) as `unsafe` on `WebSettings` and
      `MobileSettings`, with `fn enabled(&self) -> Vec<&'static str>` returning the TOML key paths that are set.
    - Add `PinningSettings { pins: Vec<String>, expires: Option<String> }` on `MobileSettings`, optional.
    - Add `SecuritySettings { contact, policy, expires, preferred_languages }` at the manifest top level, optional, wherever
      `project_manifest_view` reads the tables.
    - Every field is `#[serde(default)]`.
  - Verify:
    - `cargo test -p cobolt-compiler project_kind --no-fail-fast` is green.
    - New tests: a manifest without the tables loads unchanged; each table round-trips; `enabled()` names `web.unsafe.plain_ws` and so on.
    - The existing `project_kind` tests are unchanged.

- [ ] **T6 — `WebBuildRequest` carries the profile and the settings** (R11; O2 as a working assumption)
  - Files: `crates/cobolt-project-tools/src/web/mod.rs`.
  - Do:
    - Add `pub enum BuildProfile { Development, Release }` and `WebBuildRequest.profile`. `from_manifest` defaults to `Development`; the caller
      sets `Release` (O2: Build and `rcrun build` are Release, Run and Preview are Development).
    - Carry the manifest's `unsafe`, `pinning` and `security` into the request. For a Mobile project, map `[mobile.unsafe]`, as `from_manifest`
      already maps the other Mobile settings (`:83-97`).
    - Add `fn unsafe_problems(&self) -> (Vec<String> /*warnings*/, Vec<String> /*refusals*/)`: each enabled key is a warning in Development and
      a refusal naming the key in Release.
  - Verify:
    - `cargo test -p cobolt-project-tools --no-fail-fast` is green.
    - New tests in `mod.rs` `tests`: each `unsafe` key gives a warning in Development and a refusal in Release, naming the key; a clean
      manifest gives neither.

- [ ] **T7 — The Web Build scans the project inputs and enforces the profile — GATED on WASM-002's linked backend** (R7, R11; AC6, AC9 build half)
  - Files: WASM-002's backend that implements `WebBuildBackend` (plan §2's `cobolt-project-tools/src/web/build.rs`, or wherever WASM-002 puts
    it), and `crates/cobolt-project-tools/tests/web_build.rs` (new).
  - Do:
    - **Do not start while `web::backend()` is still `NotYetLinked`.** Record that it is blocked and move on.
    - Once it is linked, run before generation:
      - `unsafe_problems()`. Refusals become `WebBuildError::Refused`.
      - `secrets::scan_project` over every form, every `request.sources` file (Common Code and copybooks), every stylesheet and text asset, and
        `backend/backend.toml`. Errors become `Refused` lines `file:line: pattern`; warnings go into the report.
      - The backend's COBOL is not scanned (plan §1.5).
    - Then map `profile == Release` → `BundleOptions.release = true`, `plain_ws` → `unsafe_plain_ws`, `skip_secret_scan` → `skip_secret_scan`,
      and `csp_report_only` → the CSP report-only mode.
    - The bundle scan runs as today.
  - Verify:
    - `cargo test -p cobolt-project-tools --test web_build --no-fail-fast` is green.
    - Each `PATTERNS` Error, planted from fragments in a `.wcfrm`, a `.cpy` and a `.css`, and one in the bundle, fails the Build naming file
      and pattern.
    - The printed text contains no planted value.
    - Each `unsafe` key warns in Development and fails in Release naming the key.

- [ ] **T8 — The IDE shows the warnings and the refusals, in six languages** (R11, i18n; AC9 IDE half)
  - Files:
    - `crates/cobolt-ide/src/i18n.rs`: new `Tr` fields, filled in all six language tables.
    - `crates/cobolt-ide/src/panels/web_preview.rs` (`Note`, `Note::lines`).
    - `crates/cobolt-ide/src/panels/web_settings.rs`.
    - `crates/cobolt-ide/src/app.rs`: where the Release profile is set for Build (O2).
  - Do:
    - Add the `Tr` keys, named here: `pk_unsafe_on` ("Unsafe setting on: {0}"), `pk_unsafe_refused` ("A release build refuses the unsafe
      setting {0}"), `pk_scan_failed` (the secret-scan failure heading), `pk_scan_possible` (the possible-secret warning heading),
      `pk_pin_expiring` (the pin set expires on {0}), and `pk_security_txt_unset` (a note that `security.txt` is not configured).
    - Add `Note::UnsafeSetting(String)` and `Note::SecretFindings { errors, warnings }`. The Build refusal lines stay in English, like compiler
      diagnostics.
    - In `web_settings.rs`, mark each enabled `unsafe` key.
    - Locate the Check path that reaches the Output panel for a Web project; the plan does not name it. Make Check print one `pk_unsafe_on`
      line per enabled key. Record the file and symbol here.
    - No "cobolt" in any new string.
  - Verify:
    - `cargo test -p cobolt-ide --bin cobolt-ide no_empty_ui_translations` is green, and so is `cargo test -p cobolt-ide --bin cobolt-ide
      web_` (the Note lines tests).
    - Each new string fits the settings panel in all six languages, judged by the operator. Do not drive the window.
    - Manual check (through the MCP tools): open a Web project with `[web.unsafe] plain_ws = true`. Check shows the warning, and once T7 has
      landed, Build fails naming `plain_ws`.

- [ ] **T9 — The dev backend uses the dev origin instead of any origin** (R12)
  - Files: `crates/cobolt-server/src/dev.rs` (`:73`), the call site that starts the dev backend for the Preview, and
    `crates/cobolt-server/tests/loopback.rs`.
  - Do:
    - Replace `cfg.unsafe_allow_any_origin = true` with `allowed_origins = [<dev server origin>]`, passed in through the dev options, for
      example `DevOptions.page_origin: Option<String>`.
    - With `None`, keep the current behaviour and warn, so callers that are not migrated yet still run.
    - The listen address stays `127.0.0.1:0`.
  - Verify:
    - `cargo test -p cobolt-server --test loopback --no-fail-fast` is green.
    - New case: a WebSocket from the dev origin is accepted, and one from `http://127.0.0.1:<other port>` gets 403.
    - The Preview still connects; this is a WASM-008 loopback test if one exists, otherwise a T24 owed row.

## M3 — CORS and origin (R6)

- [ ] **T10 — One CORS function; wildcard-with-credentials is impossible** (R6; AC5 server half)
  - Files: `crates/cobolt-server/src/http.rs`, `crates/cobolt-server/src/serve.rs` (`:389-390`), `crates/cobolt-server/tests/loopback.rs`, and
    `crates/cobolt-server/tests/cors_sources.rs` (new).
  - Do:
    - Add `pub(crate) enum Credentials { No, Yes }` and `pub(crate) fn cors_headers(cfg: &Resolved, head: &Head, c: Credentials) -> String`.
    - With `Yes`, it answers only an origin **listed** in `allowed_origins`, never under `allow_any_origin`.
    - With `No`, it keeps today's echo of an allowed origin.
    - It always adds `Vary: Origin`.
    - Route the download response through it.
  - Verify:
    - `cargo test -p cobolt-server --no-fail-fast` is green.
    - `loopback.rs` gains these cases: a foreign Origin gets no `Access-Control-Allow-Origin`; with `unsafe_allow_any_origin` and
      `Credentials::Yes`, no `Allow-Credentials` and no echo; `"*"` is refused (that case exists in `config_rules.rs`).
    - `cors_sources.rs` searches `src/` and asserts that `Access-Control-Allow-Credentials` and `Access-Control-Allow-Origin` appear only inside
      `cors_headers`.
    - The refresh-cookie half of AC5 is a WASM-009 row (T23).

## M4 — policies the build generates (R2, R3, R4, Q4)

- [ ] **T11 — The printed report names the header-only directives** (R3; AC3)
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (`BuildReport::text` `:246`) and its tests.
  - Do: a test, plus a wording fix only if the test shows one is needed. `BuildReport::text` must say which directives are in the `<meta>` and
    must list `frame-ancestors` and report-only mode as header-only.
  - Verify: `cargo test -p cobolt-web-bundle --no-fail-fast` is green. The existing `csp.rs` unit tests show that `meta_content` has no
    `frame-ancestors`.

- [ ] **T12 — The browser enforces the CSP: a headless proof** (R2; AC2)
  - Files: `crates/cobolt-web-bundle/src/devserver.rs` (Δ1) and `crates/cobolt-web-bundle/tests/csp_browser.rs` (new).
  - Do:
    - Add `ServeOptions.csp_report_sink: bool`, default `false`. When it is true, `handle` accepts `POST /__csp-report` and stores the bodies,
      and `Server::csp_reports()` returns them. This is the development server only; a bundle never contains the route.
    - In the test:
      - Assemble a Release bundle with the `support/` harness.
      - Serve it with the CSP as a **header** (`extra_headers`) plus `report-uri /__csp-report`.
      - In a **temporary copy** written by the test, add an inline `<script>`, a same-origin script that calls `eval`, and a `fetch` to an
        address outside `connect-src`. The scripts are written into the temp dir only (PRIME DIRECTIVE: nothing tracked).
      - Run headless Chrome, located the way `e2e_node.rs`/WASM-015 locate a browser.
    - When Chrome is missing, print `SKIPPED: no Chrome` and fail when `PR_SECURITY_GATE=1`.
  - Verify: `cargo test -p cobolt-web-bundle --test csp_browser -- --nocapture` shows three reports (`script-src-elem`, `script-src` eval,
    `connect-src`) for the tampered copy, and **zero** for the clean bundle, whose main form renders. The counts are printed.

- [ ] **T13 — The TLS floor is proven; the HTTPS headers are present** (R4; O5)
  - Files: `crates/cobolt-server/tests/tls_floor.rs` (new).
  - Do:
    - Use a loopback HTTPS server with a test certificate, built the way the existing TLS tests in `loopback.rs` build theirs.
    - Connect with a `native_tls` client capped at `Protocol::Tlsv11` and expect refusal. If the platform's client cannot offer 1.1 at all,
      print which side refused and assert the server's configured minimum instead.
    - A TLS 1.2+ response carries every line of `SECURITY_HEADERS`.
  - Verify: `cargo test -p cobolt-server --test tls_floor -- --nocapture` is green, and its output names the platform's behaviour.

- [ ] **T14 — `security.txt` from the developer's parameters** (Q4)
  - Files:
    - `crates/cobolt-web-bundle/src/lib.rs`: a new optional `BundleOptions.security`, which writes `/.well-known/security.txt` with `Contact`,
      `Expires`, and `Policy` and `Preferred-Languages` when given (RFC 9116).
    - `crates/cobolt-server/src/http.rs` and `serve.rs`: the same text served at the same path, from a `[security]` table in `backend.toml`
      that the backend's own config reads.
    - The tests of both crates.
  - Do:
    - Refuse a `[security]` that has no `contact` or no `expires`, or whose `expires` is in the past. Name the key.
    - When the table is absent, write nothing and add a **note** to the report, not a warning.
    - Wiring the manifest `[security]` into `BundleOptions` is part of T7's mapping.
  - Verify: `cargo test -p cobolt-web-bundle -p cobolt-server --no-fail-fast` is green. New cases: the file is present with `Contact` and
    `Expires` when configured, absent otherwise, and refused when incomplete; the backend answers `GET /.well-known/security.txt` with the same
    text.

## M5 — XSS (R5)

- [ ] **T15 — Text, never markup, in the controls and in the Viewer** (R5; AC4)
  - Files: `crates/cobolt-web-player/tests/xss.rs` (new), in the `abi_host.rs` style.
  - Do:
    - Set a Caption, a ListBox item, a DataGrid cell and a ToolTip to `<img src=x onerror=alert(1)>` through `SET` commands. Assert that the
      node tree holds **text nodes** with that literal and **no `img` element**.
    - Feed the Viewer HTML containing `<script>`, an `onclick=` attribute and a `javascript:` link. Assert that the `blocks_to_nodes` output
      has no `script` element and no `on*` attribute, and that the link is plain text (`link_allowed` refuses it).
    - `sinks.rs` already covers the sources and is not changed.
  - Verify: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --test xss --test sinks --no-fail-fast` is green, and the counts are
    printed.

## M6 — supply chain (R9, R10)

- [ ] **T16 — `deny.toml` and the supply-chain test** (R9; AC8 audit half). Depends on T1.
  - Files: `deny.toml` (new, at the root; it is configuration) and `crates/cobolt-compiler/tests/supply_chain.rs` (new).
  - Do:
    - `deny.toml` sections:
      - `advisories`: the RustSec database at the local path T1 verified.
      - `licenses`: the T1 list as approved; O4 items are left out until the operator decides.
      - `bans`: mirrors the `NEVER` lists of `cobolt-server/tests/dependency_guards.rs` and `cobolt-web-player/tests/dependency_guard.rs`.
      - `sources`: crates.io only, plus the in-tree path crates.
    - The test runs `cargo deny --offline check` (or the flags T1 verified), section by section, and prints each section's result.
    - Negative proofs, in a temp directory: a local advisory database with one **synthetic** advisory against a crate in our lock must fail; a
      config with that crate's licence removed must fail.
    - If T1 found the offline local-DB mode unusable, write the plan's fallback matcher in Rust inside the test instead.
    - When the tool is missing, print `SKIPPED: cargo-deny not installed` and fail when `PR_SECURITY_GATE=1`.
    - The guard tests run their `cargo tree` with `--offline`; the network-fetching build-script rule is checked with `CARGO_NET_OFFLINE=true`
      on the guarded builds.
  - Verify: `cargo test -p cobolt-compiler --test supply_chain -- --nocapture` shows the four sections passing on the tree and both negative
    fixtures failing. `cargo test -p cobolt-compiler --test dependency_guards_063`, `-p cobolt-server --test dependency_guards` and the player's
    `--test dependency_guard` stay green.

- [ ] **T17 — One SBOM writer; an SBOM beside the backend binary** (R10; AC8 SBOM half)
  - Files: `crates/cobolt-web-bundle/src/sbom.rs` (new; it moves the writer out of `lib.rs` around `:750-769`), `crates/cobolt-web-bundle/src/lib.rs`,
    `crates/cobolt-compiler/src/backend_build.rs`, and `crates/cobolt-compiler/Cargo.toml`, which gains the `cobolt-web-bundle` dependency only
    if that edge is acceptable.
  - Do:
    - Add `pub fn cyclonedx(name, files, components) -> serde_json::Value`, keeping CycloneDX 1.5 with a SHA-256 per file. The bundle output
      must stay byte-identical.
    - `build_backend` writes `sbom.cdx.json` beside the binary, with components from `cargo metadata --offline` of the generated project.
    - **Check the dependency edge first.** If adding `cobolt-web-bundle` to `cobolt-compiler` creates a cycle or pulls in what the
      `dependency_guards_063` rules forbid, put `sbom.rs` in the smaller crate that both can reach, and record the choice here. Do not widen
      the guards.
  - Verify:
    - `cargo test -p cobolt-web-bundle -p cobolt-compiler --no-fail-fast` is green.
    - A bundle test asserts the SBOM bytes are unchanged by the move.
    - A backend-build test asserts that `sbom.cdx.json` exists, parses, and lists the binary with its hash.
    - SRI on every fetched file is covered by existing tests; name them here.

## M7 — logs and privacy (R13)

- [ ] **T18 — No token, password or marked field in a log, the console or an error report** (R13; AC10; O1 as a working assumption)
  - Files:
    - Player: `crates/cobolt-web-player/src/` on the `ERROR` path (ABI A4), and `crates/cobolt-web-player/tests/no_secret_in_console.rs` (new).
    - Server: `crates/cobolt-server/tests/no_secret_in_logs.rs` (new). This is Δ2.
  - Do:
    - **Player.** A password-mode control's value is never quoted in an `ERROR` message or a player diagnostic. The token type has no `Display`
      and its `Debug` is redacted; `auth.rs` asserts this already, so extend the existing assertion.
      - The test plants a password in a password-mode control and a token.
      - It triggers an error on that control and a remote-call failure.
      - It greps the captured console lines and the `ERROR` report.
    - **Server.** Use a dev-token sign-in (`unsafe_dev_token_secret`, Development) and a remote call that carries a planted password field.
      Grep every `MemorySink` line for the token and the password.
    - Both halves assert zero hits and print the line counts.
    - The full sign-in with a provider is WASM-009's. It is a T23 row `owed: WASM-009` until that spec's mock provider exists.
  - Verify:
    - `cargo test -p cobolt-server --test no_secret_in_logs -- --nocapture` and `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml
      --test no_secret_in_console -- --nocapture` are green, with zero hits.
    - The `subject` field remains, and is listed as personal data (T23).

## M8 — fuzzing (R15)

- [ ] **T19 — Wire fuzz targets** (R15; AC11 frames and JSON)
  - Files: `crates/cobolt-wire/tests/fuzz_frames.rs` (new) and `crates/cobolt-wire/tests/random_round_trip.rs`.
  - Do:
    - Follow the `display_list_robustness.rs` pattern: a fixed-seed xorshift and the iteration multiplier `PR_FUZZ_ITERS` (default 1).
    - `fuzz_frames.rs` feeds `proto::parse_client` and `parse_server` with random and mutated frames from `vectors.rs`, oversize text and
      invalid UTF-8 boundaries.
    - `random_round_trip.rs` gains `json::parse(mutated bytes, limit)`.
    - Both assert no panic and a named error, then print the iterations and seconds.
  - Verify: `cargo test -p cobolt-wire --release --no-fail-fast -- --nocapture` takes 5 s or less per target by default. One recorded run with
    `PR_FUZZ_ITERS=200` takes 10 min or less; write its time here.

- [ ] **T20 — Server fuzz targets: dispatch and tokens** (R15; AC11 dispatch and tokens)
  - Files: `crates/cobolt-server/tests/fuzz_dispatch.rs` and `crates/cobolt-server/tests/fuzz_tokens.rs` (new).
  - Do:
    - **Dispatch:** on a loopback socket, send random and mutated frames, oversize frames, binary frames and a ping flood. The connection is
      closed with a named code and the server keeps serving the next connection.
    - **Tokens:** pass mutated headers to `TokenValidator::validate`, plus `alg: none`, `HS256` signed with the public key, truncated and
      oversize segments, and a wrong `kid`. Each is a refusal; **no token not signed by the configured key is accepted**.
    - Use the same `PR_FUZZ_ITERS` multiplier.
  - Verify: `cargo test -p cobolt-server --release --test fuzz_dispatch --test fuzz_tokens -- --nocapture` takes 5 s or less each by default,
    with the budgets printed. One recorded `PR_FUZZ_ITERS=200` run takes 10 min or less.

## M9 — mobile (R14, Q5)

- [ ] **T21 — Pinning rules (the option); the capability rule as an owed row** (R14, Q5)
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`PinningSettings::check(today) -> (warnings, errors)`) and its tests.
  - Do:
    - `check` refuses fewer than two pins, a pin that is not `sha256/<base64 of 32 bytes>`, a missing `expires` and a past `expires`. It warns
      within 30 days of the expiry (`pk_pin_expiring`, from T8).
    - Pinning is off when the table is absent.
    - Writing Android's `network_security_config.xml` and iOS's `NSPinnedDomains` (using the keys T1 recorded) is WASM-012's
      `mobile_build.rs`, which does not exist yet. It is a T23 row `owed: WASM-012`.
    - So is the capability-file test: main window only, no `remote` section, and permissions equal to the union of the declared features.
  - Verify: `cargo test -p cobolt-compiler project_kind --no-fail-fast` is green, with each refusal and the 30-day warning as a case.

## M10 — System KB

- [ ] **T22 — The manifest documentation gains the new tables; the KB is regenerated** (R11, Q4, Q5; System KB rule)
  - Files: `crates/cobolt-compiler/src/lib.rs` (the section at `:6172`) and `assets/knowledge/chunked.data` (regenerated).
  - Do:
    - Add `[web.unsafe]` / `[mobile.unsafe]`: each key, what it weakens, and that a release Build refuses it.
    - Add `[mobile.pinning]`: two pins, expiry, and the 30-day warning.
    - Add `[security]`: becomes `security.txt`.
    - State that a release backend refuses a literal secret (T4).
    - No control, property, method or event is added.
    - Then run `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify:
    - `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` is green.
    - `git diff --stat assets/knowledge/chunked.data` shows a change.

## M11 — the whole, written down (R1, R16)

- [ ] **T23 — The threat model, its test, and the release-review rule** (R1, R16, Q1; AC1)
  - Files: `specs/WASM-013-security-architecture/threat-model.md` (new) and `crates/cobolt-web-bundle/tests/threat_model.rs` (new).
  - Do:
    - Write the model:
      - the assets and actors of R1, and the **seven boundaries**;
      - one table `id | boundary | actor | threat | control (rule + code path) | test (crate::test_name)`, with STRIDE walked per boundary;
      - the plan §3.3 personal-data table;
      - the pattern names of `secrets::PATTERNS`;
      - the accepted residual risk O5.
    - Rows whose test belongs to another spec carry `owed: WASM-NNN`:
      - WASM-009: `/auth/refresh` through `cors_headers`, the `X-PR-Refresh` header, the cookie-only refresh refused cross-site, the full
        sign-in log test, the identity secret fields of T4;
      - WASM-012: the capability file, the mobile dependency guard, the mobile SBOM, the pinning output, `store_required` for AC7's signing
        half;
      - WASM-016: the player `components`;
      - WASM-002: T7's wiring, while it is gated;
      - WASM-017: the R15b memory and disposal tests, if not already named.
    - The test parses the table and asserts:
      - every boundary has at least one row;
      - every row has a control and either an existing test (a text search for `fn <name>` in the named crate) or an `owed:` marker;
      - the pattern names equal `secrets::PATTERNS`.
    - It prints the rows, the boundaries and the **owed count**.
    - **R16:** when `specs/WASM-PROGRESS.md` marks the line as a release candidate, `release-review.md` must exist with `kind = review |
      penetration-test` and a reviewer different from its author line. Until then the check prints "not a release candidate".
  - Verify: `cargo test -p cobolt-web-bundle --test threat_model -- --nocapture` is green and prints the row count and the owed count. A
    temporary edit that removes a row's test name makes it fail; revert the edit.

- [ ] **T24 — Owed and unmeasured items recorded in `WASM-PROGRESS.md`** (R1, R15)
  - Files: `specs/WASM-PROGRESS.md`.
  - Do:
    - Add one WASM-013 section listing what is owed and by whom: the T23 owed rows, T7 if it is still gated, and T9's Preview case if it is
      owed.
    - List what cannot be measured here, with the command and without claiming it: Firefox and Safari CSP and Trusted Types enforcement (Q2,
      best effort), and iOS and Android pinning on devices.
    - Record the fuzz budgets from T19 and T20.
  - Verify: the section exists, its owed count equals the one `threat_model.rs` prints, and nothing under `docs/` changed.

## M12 — finalize

- [ ] **T25 — Finalize: version, CHANGELOG, the tests of the touched crates**
  - Files:
    - `crates/cobolt-ide/src/version.rs`: `z` + 1, **once**. Go from `2.0.30` to the next free `2.0.z` at that time; never change `x` or `y`.
    - `CHANGELOG.md`: **one** entry above the previous `## [PowerRustCOBOL 2.0.z]` heading, dated, "Written on features-2x". It lists what
      landed, the deliberate break of T4 (a literal secret in a release `backend.toml` now stops the start, and the message names the fix), and
      what stays open: O1–O5, the gated T7, the owed rows and count, and T1's licence decisions.
  - Do:
    - Run `df -h .` first.
    - Run the tests of the crates touched, with `--no-fail-fast`, and read every `test result` line:
      - `cargo test -p cobolt-web-bundle -p cobolt-server -p cobolt-wire -p cobolt-compiler -p cobolt-project-tools --no-fail-fast`;
      - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
      - `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast`;
      - with `PR_SECURITY_GATE=1`, `--test csp_browser` and `--test supply_chain`, to show they do not skip on this machine.
  - Verify:
    - Everything above is green, and the counts are recorded here.
    - The staged diff has no credential: `AKIA`/`ASIA`, PEM blocks, or the fragment-built test secrets assembled anywhere outside tests.
    - `git status` shows nothing under `docs/` and no `README.md` change.
    - Manual check (the operator's): launch the IDE and confirm the version string; look at the T8 warnings in six languages.

## Done criteria
- Every acceptance criterion in spec.md is covered by at least one task's verification:
  - AC1: T23, T24;
  - AC2: T12;
  - AC3: T11, plus the existing `csp.rs` tests;
  - AC4: T15, plus the existing `sinks.rs`;
  - AC5: T10, plus the existing `loopback.rs` and `config_rules.rs`; the refresh-cookie half is an owed WASM-009 row in T23;
  - AC6: T2, T3 and T7 (the Build half is gated on WASM-002);
  - AC7: T4 (backend); the signing half is an owed WASM-012 row (`store_required`) in T23;
  - AC8: T1, T16, T17, plus the existing guards;
  - AC9: T5, T6, T7, T8, plus the existing `config_rules.rs`;
  - AC10: T18; the full provider sign-in is an owed WASM-009 row;
  - AC11: T19, T20.
- Requirement coverage beyond the AC:
  - R4: T13;
  - R12: T9;
  - R14: T21 plus an owed WASM-012 row;
  - R15b: owed WASM-017 rows in T23;
  - R16 and Q1: T23;
  - Q4: T5, T14;
  - Q5: T5, T21.
- The touched crates' tests pass.
- Nothing under `docs/` or `README.md` changes.
- O1–O5 are carried, not decided.
- Do **not** commit or push; the pipeline does the bookkeeping.
