<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Security architecture and threat model for the WebAssembly line (WASM-013)

- **Status:** draft, 2026-10-22. First plan for this spec, written against the spec as it stands after the operator's answers of
  2026-10-22 (Q1–Q5 decided). Every claim about existing code below was read in the tree on `features-2x` at 2.0.30 (`cdad25cd`).
- **Spec:** ./spec.md   **Date:** 2026-10-22
- **Companions:** `specs/WASM-INDEX.md` (D14: markup never inserted as a string), `specs/WASM-PLAN.md` (W6: "security gates and tests
  matrix", after W3–W5), `specs/WASM-ABI-v1.md` (A6 error frame, A8 connection envelope: the token is the first message, never in the
  address or a header).

## 0. What this spec is, in code terms

WASM-013 **owns rules that other specs' code already implements in part**. Much of it exists, written by the WASM-002, 003, 005, 016
and 017 work and citing this spec by requirement number. This plan therefore has three kinds of work:

1. **Write down the whole** — the threat model (R1) as a maintained document whose every row points to a control and to a test that a
   test checks exists (AC1).
2. **Close the gaps** between what is built and what R2–R15b say (table §1.2).
3. **Add the assurance layer** nobody owns yet — supply-chain audit and licences (R9), the fuzz targets with budgets (R15), the
   cross-cutting log/secret tests (R13), the release-review record (R16, Q1), the security contact (Q4) and certificate pinning (Q5).

### 0.1 What exists today (read for this plan)

| Rule | What the code does today | Where |
|---|---|---|
| R2 CSP | Default-deny policy: `default-src 'none'`, `script-src 'self' 'wasm-unsafe-eval'`, no `'unsafe-eval'`/`'unsafe-inline'`, `style-src 'self'`, `connect-src 'self'` + backends, `object-src`/`base-uri`/`form-action 'none'`, `frame-src 'none'` unless widened, `require-trusted-types-for 'script'` + `trusted-types default` | `crates/cobolt-web-bundle/src/csp.rs` `build` (`:68`), `CspOptions::strict` (`:41`); unit tests in the same file |
| R3 meta + headers | `Policy::meta_content`, `header_value`, `header_only_directives` (`frame-ancestors`, report-only); `host_files` writes `_headers`, `hosting/nginx.conf`, `hosting/.htaccess`; the report carries `csp_meta`, `csp_header`, `header_only` | `csp.rs:104-135`, `:165`; `cobolt-web-bundle/src/lib.rs` `BuildReport` (`:231`) |
| R4 headers (bundle) | `security_headers`: CSP, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Permissions-Policy`, COOP/COEP/CORP per `Isolation` | `csp.rs:138` |
| R4 headers + TLS (backend) | `http::SECURITY_HEADERS` (HSTS two years, nosniff, no-referrer, Permissions-Policy, `no-store`) on every HTTPS response; TLS through `native-tls` with **minimum TLS 1.2** on both ends; no cipher configuration | `crates/cobolt-server/src/http.rs:128`; `tlsutil.rs` `acceptor` / `connector` |
| R5 XSS | A source search of the player for markup/string-execution sinks; the Viewer builds nodes from the typed `cobolt_forms::viewer` tree, links through `weblink::check`, colours through `safe_color`; a ban list on the bundle's JavaScript | `crates/cobolt-web-player/tests/sinks.rs` (`FORBIDDEN`), `src/view/viewer.rs` (module doc, `blocks_to_nodes`, `link_allowed`, `safe_color`); `cobolt-web-bundle/tests/no_string_execution.rs` |
| R6 origin | WebSocket `Origin` checked against `allowed_origins` (`origin_allowed`, `serve.rs:423`), refused with 403 and counted; `"*"` refused as an origin (`config.rs:449`); the download route echoes an allowed origin in `Access-Control-Allow-Origin` **without** `Allow-Credentials` (`serve.rs:389`). No cookie code in the server yet: the `/auth/*` routes and the `__Host-pr_refresh` cookie are WASM-009's plan (`plan.md` D-rows at `:102`, `:182`), not built | `crates/cobolt-server/src/serve.rs`, `config.rs` |
| R7 secret scan | `secrets::scan_text`: known shapes are **errors** (cloud key ids, PEM private keys, JWTs, credentialed URLs, prefixed vendor tokens, a property named like a secret holding a value, `client_secret`/`aws_secret_access_key` lines), high entropy is a **warning** (Q3 as decided); a `Finding` has no field that can hold the value. Run over every text file the bundle holds **except** `player/*` and the SBOM | `crates/cobolt-web-bundle/src/secrets.rs`; `lib.rs:771-786` |
| R8 secret storage | IDE store is best-effort with plaintext fallback **and currently disabled** (`NATIVE_STORE_DISABLED = true`, `cobolt-ide/src/secrets/mod.rs:46`). WASM-012's plan adds `store_required`/`retrieve_required` (D-Store) for signing passwords. The backend reads secrets as `env:NAME`, `file:PATH` **or a literal** (`config.rs` `resolve_secret`, `:329`) and wraps them in `Secret`, whose `Debug` prints `Secret(***)` | as cited |
| R9 supply chain | Per-artefact guards from `cargo tree` / `Cargo.lock`: backend (`cobolt-server/tests/dependency_guards.rs`: headless, no `ring`/`aws-lc-*`/`rustls`/`openssl`), player (`cobolt-web-player/tests/dependency_guard.rs` `NEVER`, wasm-bindgen pin), delivered application (`cobolt-compiler/tests/dependency_guards_063.rs`); `Cargo.lock` is tracked. **No vulnerability audit and no licence check exist** in the tree | as cited |
| R10 SBOM + integrity | A CycloneDX 1.5 `sbom.cdx.json` per Web bundle (every file with SHA-256, plus `PlayerInput.components`); SRI digests on every file the loader fetches | `cobolt-web-bundle/src/lib.rs:750-769`, `Builder::sri_of` uses |
| R11 unsafe switches | Bundle: `BundleOptions.release` refuses `unsafe_plain_ws` and `skip_secret_scan`, warns in a non-release build (`lib.rs:464-477`). Backend: `RunMode::Release` refuses `unsafe_dev_token_secret`, `unsafe_allow_any_origin`, `unsafe_plain_ws` (`config.rs:426-442`); `standalone.rs:84` runs Release, `dev.rs:74` Development. **No project-manifest setting reaches either**: `WebSettings` (`cobolt-compiler/src/project_kind.rs:273`) has no `unsafe_*` field and `WebBuildRequest` (`cobolt-project-tools/src/web/mod.rs:45`) has no release/development profile; the IDE shows none of these warnings | as cited |
| R12 loopback + preview | The dev backend listens on `127.0.0.1:0` (`cobolt-server/src/dev.rs:70`) and turns on `unsafe_allow_any_origin` (`:73`); the preview web view navigates only to its own origin or a blank page (`cobolt-web-preview/src/main.rs` `may_navigate`, `:95`) | as cited |
| R13 logs | Backend `LogRecord` has **no field that can hold** a token, password, payload or claim (`cobolt-server/src/log.rs`); it does hold `subject` (the token's `sub`). Player: token not in storage, `Debug` redacted (`cobolt-web-player/src/auth.rs` module doc) | as cited |
| R15 fuzz | The JSON-to-group target exists as a fixed-seed property test (`cobolt-wire/tests/random_round_trip.rs`, which names WASM-013 R15); the same xorshift pattern guards the display list (`cobolt-graphics/tests/display_list_robustness.rs`, `GRAPHICS_FUZZ_ITERS`). **No** target for WebSocket frame handling or token validation; no stated budgets | as cited |
| R14, R15b | Owned in code by WASM-012 (capability file, `plan.md:57`) and WASM-017 (fixed memory, disposal test `pr_dbg_live`, ABI A3) | as cited |

### 0.2 The gaps this plan closes

| # | Gap | Rule |
|---|---|---|
| G1 | No threat-model document; nothing proves each row has a test | R1, AC1 |
| G2 | Unsafe switches cannot be set from the project, so the IDE cannot warn and the release Build cannot refuse; there is no Development/Release profile on a Web build | R11, AC9 |
| G3 | The secret scan does not see copybooks or Common Code (they are compiled, not shipped, so they are not in the bundle), misses `Key=value` connection strings (`Password=`, `Pwd=`, `AccountKey=`) — the shape a `ConnectionString` property holds — and does not see `.wcfrm`/`.mcfrm` (WASM-001's fix) | R7, AC6 |
| G4 | A release backend accepts a **literal** secret in `backend.toml`, a file that lives in the project and is committed | R7, R8 |
| G5 | No vulnerability audit, no licence check, no proof that a disallowed dependency fails the run per artefact for the mobile shell | R9, AC8 |
| G6 | SBOM only for the Web bundle; none for the backend binary or the mobile shell; `PlayerInput.components` has no producer yet (it is filled by WASM-002's Build) | R10, AC8 |
| G7 | CORS with credentials (WASM-009's routes) has no single function that makes "wildcard with credentials" impossible, including under `unsafe_allow_any_origin` | R6, AC5 |
| G8 | Fuzz targets for frames and tokens missing; budgets unstated | R15, AC11 |
| G9 | No headless-browser proof that the CSP blocks inline script, `eval` and a foreign `connect` | R2, AC2 |
| G10 | No end-to-end grep of backend log, browser console and crash report for planted secrets | R13, AC10 |
| G11 | No release-review record (Q1), no security contact parameters (Q4), no pinning option (Q5) | R16, Q1, Q4, Q5 |

## 1. Approach

### 1.1 The threat model is a spec file with a machine-checked table (R1, AC1)

`specs/WASM-013-security-architecture/threat-model.md` (written at `/implement`, not by this plan) holds:

- **Assets** (R1 list), **actors** (R1 list) and the **seven boundaries**: browser ⇄ backend, host page ⇄ module, module ⇄ player,
  IDE ⇄ preview web view, mobile shell ⇄ frontend, backend ⇄ identity provider, backend ⇄ data.
- **One table**, one row per threat: `id | boundary | actor | threat | control (rule + code path) | test (crate::test_name)`. STRIDE
  is the checklist used to make sure each boundary is walked for spoofing, tampering, repudiation, disclosure, denial of service and
  elevation; it is not a column the reader must learn.
- **Personal data** (R13's "the plan shall list"): the table of §3.3 below, copied there and kept current.

A test, `cobolt-web-bundle/tests/threat_model.rs`, reads the file, parses the table, and asserts: every R1 boundary has at least one
row; every row names a control and a test; **every named test function exists** in the named crate's sources (a text search for
`fn <name>`). A row whose test is owed by a later spec is allowed only with the marker `owed: WASM-NNN`, and the test prints how many
rows are owed so the number is visible and falls over time. The `/analyze` of each later WASM spec updates the file (R1's last
sentence); the `analyze` skill is not changed by this plan — the rule is in the spec and the test makes forgetting visible.

### 1.2 Policies the build generates (R2, R3, R4)

As built (§0.1), with three changes:

- **R2 headless proof (G9).** A test serves a Release bundle through the project dev server
  (`cobolt-project-tools/src/web/devserver.rs`) with the CSP **header** (so `report-uri` works) pointing at a development-only
  `POST /__csp-report` route that records reports in memory. A temporary copy of the bundle — written by the test, never tracked — adds
  an inline `<script>`, a same-origin script that calls `eval`, and a `fetch` to an address outside `connect-src`. Headless Chrome loads
  it; the test asserts three violation reports (`script-src-elem` inline, `script-src` eval, `connect-src`) and that the untouched
  bundle produces **zero** reports while its main form renders (AC2). Chrome is located the way WASM-015's harness will locate it; when
  absent the test prints `SKIPPED: no Chrome` and fails under `PR_SECURITY_GATE=1` (the release run). `report-uri` is deprecated in
  favour of `report-to` but is what Chrome still honours for this; the route is never part of a release bundle.
- **R3** unchanged; the Build's printed report already separates `in the page (<meta>)` from header-only directives — a test asserts the
  printed text names `frame-ancestors` and report-only mode as header-only (AC3).
- **R4 TLS floor stated.** The floor is **TLS 1.2 minimum, the operating system's default cipher suites** (`native-tls` exposes no
  cipher selection; spec 103 R10 keeps the TLS stack C-free and the OS's). A loopback test connects with a client capped at TLS 1.1
  (`max_protocol_version(Some(Protocol::Tlsv11))`) and expects refusal; where the platform's client cannot even offer 1.1 the test says
  so and passes on the server's configured minimum. The threat model records "cipher choice = OS policy; terminate TLS in front for a
  stricter suite list" as an accepted residual risk.

### 1.3 XSS (R5)

Built as the rule requires; this plan adds only proof for AC4:

- a player host test (`cobolt-web-player/tests/abi_host.rs` style) that sets a Caption, a ListBox item, a DataGrid cell and a ToolTip
  to `<img src=x onerror=…>` through `SET` commands and asserts the resulting node tree holds **text nodes** with that literal and **no
  `img` element**;
- a Viewer test feeding HTML with `<script>`, an `onclick=` attribute and a `javascript:` link, asserting the node tree has no `script`
  element, no `on*` attribute and the link as plain text;
- `sinks.rs` keeps watching the sources (it already covers the Viewer). WASM-006 R7 and WASM-007 R13 enforcement points are rows of the
  threat model, tested in their own crates.

### 1.4 CSRF, CORS and origin (R6)

- **One CORS function** in `cobolt-server/src/http.rs`: `cors_headers(cfg, head, Credentials::{No, Yes}) -> String`. With
  `Credentials::Yes` it answers only an origin **listed** in `allowed_origins` — never under `allow_any_origin`, not even in
  development, so wildcard-with-credentials cannot be produced by any setting (G7). The download route (`serve.rs:389`) and WASM-009's
  `/auth/*` routes call it; a test asserts no code path writes `Access-Control-Allow-Credentials` except through it (source search).
- **State-changing calls** travel over the WebSocket with the bearer token as the first message (ABI A8), which a cross-site page
  cannot forge: it has no token. The only cookie is WASM-009's `__Host-pr_refresh` (`HttpOnly; Secure; SameSite=Strict; Path=/`);
  `POST /auth/refresh` additionally requires a custom request header (`X-PR-Refresh: 1`) the browser will not send cross-site without a
  preflight the CORS function refuses. These routes are WASM-009's to build; this plan fixes the rule and the AC5 test:
  a request to `/auth/refresh` carrying the cookie but from a foreign `Origin`, or without the header, is refused; a WebSocket from a
  foreign `Origin` is refused (exists: `loopback.rs`); a configuration with `"*"` is refused (exists: `config_rules.rs`).

### 1.5 Secrets (R7, R8)

- **The pattern list is this spec's**: it stays in `cobolt-web-bundle/src/secrets.rs` (one implementation), gains a public
  `PATTERNS: &[(&str, Severity)]` naming every pattern, and the threat model lists the same names (the `threat_model.rs` test compares
  them). Added shapes (G3): `Key=value` connection strings (`Password=`, `Pwd=`, `AccountKey=`, `SharedAccessKey=` with a non-empty,
  non-placeholder value) — **error**; `connectionstring` added to `secret_property`'s name list with the same placeholder rules.
- **Scope (G3):** the Web/Mobile Build (WASM-002's `WasmBuild`, `cobolt-project-tools/src/web/build.rs`) scans the **project inputs
  before generating** — every form, every Common Code file and copybook in `request.sources`, every stylesheet and text asset, and
  `backend/backend.toml` — then the bundle as today. One function, `secrets::scan_project(files) -> Vec<Finding>`, is used for both. The
  backend's COBOL is **not** scanned for errors (the backend is where secrets live, WASM-INDEX §5 point 4) except for literal secrets in
  `backend.toml` (next point).
- **G4 — no literal secret in a release backend.** `ServerConfig::resolve` with `RunMode::Release` refuses a `Secret` field whose text is
  neither `env:` nor `file:` (`tls.pkcs12_password`, `identity` client secret, `recaptcha.secret`), naming the key and never the value;
  Development keeps accepting it with a warning. `file:` is the deployment's secret mount (container or orchestrator); the product
  never writes it.
- **R8.** No product path writes a signing or backend secret to disk. Signing passwords use WASM-012's `store_required` (D-Store, which
  ignores the kill switch for `mobile-signing/` accounts only and returns `Err` when the store is unavailable). The IDE holds **no**
  backend secret today (the dev backend's HS256 secret is made fresh in memory, `dev.rs:50`); any future one uses the same strict call
  under a `backend/` account prefix. AC7 is WASM-012's test on the signing path plus this plan's test that a `backend.toml` literal is
  refused in Release.

### 1.6 Supply chain (R9, R10)

- **Allow-lists per artefact** stay where they are (backend, player, delivered application); **mobile shell** gets the same shape of
  guard in WASM-012's crate (a row owed by WASM-012). R9's "no build script that fetches from the network except what is declared" is
  checked by running the guarded builds with `CARGO_NET_OFFLINE=true` in the guard tests (they already use `cargo tree --offline`).
- **Vulnerability audit and licences: `cargo-deny`**, configured by a root `deny.toml` (configuration, like `Cargo.toml`; no new source
  language). Sections: `advisories` (RustSec DB), `licenses` (allow-list: Apache-2.0, MIT, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0,
  Unicode-DFS-2016, MPL-2.0 — the exact list is fixed at `/tasks` from `cargo deny list` over today's lock, and any licence not on it
  is the operator's call, not silently added), `bans` (mirrors the per-artefact `NEVER` lists for the whole workspace), `sources`
  (crates.io only). A Rust test, `crates/cobolt-compiler/tests/supply_chain.rs`, runs `cargo deny --offline check` and reports each
  section's result; when the tool is missing it prints `SKIPPED: cargo-deny not installed` and **fails under `PR_SECURITY_GATE=1`**.
  Negative proofs (AC8): the same test runs `cargo deny` against two fixture configurations in a temporary directory — one with a
  local advisory database holding a **synthetic advisory** against a crate in our lock, one with that crate's licence removed from the
  allow-list — and asserts each fails. *[verify at `/tasks`: that `cargo deny` reads a local advisory-db path with fetching disabled; if
  not, the advisory half falls back to an in-repo matcher over a local RustSec checkout, written in Rust.]* The advisory database is
  refreshed by the continuous run (`cargo deny fetch`), never by the test itself, so the test stays deterministic.
- **R9 host page:** the JavaScript is emitted by Rust at every Build (WASM-002 plan §4.1, C1); there is no package-manager tree.
- **R10 SBOM:** the bundle's CycloneDX writer moves to a small shared module, `cobolt-web-bundle/src/sbom.rs` (`fn cyclonedx(name,
  files, components) -> Value`), reused by the backend build (`cobolt-compiler/src/backend_build.rs` writes `sbom.cdx.json` beside the
  binary, components from `cargo metadata --offline` of the generated backend project) and by WASM-012's mobile build. The player's
  `components` are produced by `build_player` from its lock closure and committed with `player-build.json` (WASM-016's file; a row owed
  by WASM-016 until it is). Integrity: SRI as built.

### 1.7 Secure by default (R11, R12)

- **Settings (G2).** `WebSettings` and `MobileSettings` gain an optional `[web.unsafe]` / `[mobile.unsafe]` table with
  `plain_ws`, `skip_secret_scan`, `csp_report_only`, each `false` by default; the TOML key path says **unsafe**. The backend's existing
  `unsafe_*` keys in `backend.toml` stay as they are. *(R11's "a weakened CSP" is `csp_report_only`; extra origins needed by a feature —
  tiles, reCAPTCHA — are not relaxations, they are computed from the project.)*
- **Profile.** `WebBuildRequest` gains `profile: BuildProfile { Development, Release }`. **Working assumption (not stated by the spec):**
  the IDE's **Build** command and `rcrun build` are Release; **Run** and the **Preview** (WASM-008) are Development. Release maps to
  `BundleOptions.release = true` and to `RunMode::Release` for the backend binary — both refusals already exist and now become reachable.
- **IDE warning.** While any `unsafe` setting is on, Check and every Build/Run print a warning line per setting in the Output panel, and
  the project-settings view marks the setting; a Release Build fails with the setting's name. New `Tr` keys in six languages (§2).
- **R12** as built. The dev backend's `unsafe_allow_any_origin = true` (`dev.rs:73`) is replaced by the dev bundle's actual origin
  (`http://127.0.0.1:<port>` of the dev server) so the development path exercises the same origin check as production; it stays on
  loopback.

### 1.8 Logging and privacy (R13)

- Backend: as built (no field can hold a secret). The `subject` field stays (needed for audit); it is listed as personal data (§3.3).
- Player: the console receives `DISPLAY` and `LOG` commands (ABI §5) — the developer's own output — and the player's diagnostics. The
  player's diagnostics never print the token, a claim value, or the text of a **password-mode** control; a source test asserts the
  token type has no `Display` and its `Debug` is redacted (exists in `auth.rs`), extended to the run-time error message path (`ERROR`,
  ABI A4) so a value of a password-mode control is never quoted in it.
- **AC10** end-to-end test (`cobolt-server/tests/no_secret_in_logs.rs` with the player host): a full sign-in (mock provider) and a
  remote call with a planted password and token; the test greps the backend `MemorySink` lines, the player's captured console lines and
  the crash/`ERROR` report for both values and asserts zero hits.
- **Working assumption (spec silent):** "a field the program marks sensitive" = (a) a control in password mode and (b) a field marked
  `sensitive` in the backend contract (WASM-014). A COBOL `DISPLAY` of a value is the developer's explicit output and is **not**
  filtered (user code is sacred). Listed as open point O1.

### 1.9 Mobile (R14)

The capability file is WASM-012's generated artefact (`mobile_build.rs`, its plan `:57`). This plan adds the rule's test as a
threat-model row owed by WASM-012: the generated capability file lists only the main window, has no `remote` section, and its
permissions are exactly the union of the declared features' permissions.

**Certificate pinning (Q5, decided: an option).** `[mobile.pinning]` with `pins = ["sha256/<base64 SPKI>", …]` (at least two: current
and backup) and `expires = "YYYY-MM-DD"`. The mobile build writes the platform's **declarative** pinning — Android
`network_security_config.xml` `<pin-set expiration=…>` for the backend host, iOS `NSAppTransportSecurity` → `NSPinnedDomains` →
`NSPinnedLeafIdentities`/`NSPinnedCAIdentities` *[verify both keys and minimum OS versions at `/tasks`]* — so the web view's own
WebSocket is pinned without a native transport. The build refuses a pin set with fewer than two pins or an expiry in the past, and warns
within 30 days of expiry. Off by default. Implemented in WASM-012's `mobile_build.rs`; the setting, the rules and the tests are this
plan's.

### 1.10 Assurance (R15, R15b, R16)

- **Fuzz targets (G8)** follow the in-tree pattern (fixed-seed xorshift, deterministic, stable Rust, no C toolchain), each with an
  iteration multiplier `PR_FUZZ_ITERS` and a stated budget:

  | Target | Entry point | File | Default run | Continuous run |
  |---|---|---|---|---|
  | WebSocket frame handling | `cobolt_wire::proto::parse_client`, `parse_server`; the server's message dispatch on a loopback socket with random and mutated frames, oversize frames, binary frames, ping floods | `cobolt-wire/tests/fuzz_frames.rs`, `cobolt-server/tests/fuzz_dispatch.rs` | ≤ 5 s each | `PR_FUZZ_ITERS=200`, ≤ 10 min |
  | JSON ⇄ group | exists (`random_round_trip.rs`); gains `cobolt_wire::json::parse` on mutated bytes with the limit | `cobolt-wire/tests/random_round_trip.rs` | as today | ≤ 10 min |
  | Token validation | `jwt::TokenValidator::validate` (`jwt.rs:88`) on mutated headers, `alg` confusion (`none`, `HS256` with the public key), truncated/oversize segments, wrong `kid` | `cobolt-server/tests/fuzz_tokens.rs` | ≤ 5 s | ≤ 10 min |

  Each asserts *no panic, a named refusal, and no acceptance of a token not signed by the configured key*, and prints iterations and
  elapsed time. *Rejected:* `cargo-fuzz` — needs nightly and libFuzzer (C++), against the stable toolchain and the C-free rule.
- **R15b** is WASM-017's code (fixed memory, disposal test via `pr_dbg_live`, the plain path tested as a full path); threat-model rows
  point at those tests.
- **R16 + Q1 (decided: allow choosing).** `specs/WASM-013-security-architecture/release-review.md` is written at the release candidate:
  `kind = review | penetration-test`, reviewer (not the author), scope, date, findings and their dispositions. A test fails when
  `WASM-PROGRESS.md` marks the line as a release candidate and that file is missing or has no reviewer distinct from the author line.
- **Q4 (decided: developer-provided project parameters).** `[security]` in the manifest: `contact` (a `mailto:` or `https:` URI,
  required to enable), `policy` (URL, optional), `expires` (date, required by RFC 9116), `preferred_languages` (optional). When set, the
  Web Build writes `/.well-known/security.txt` into the bundle and the backend's HTTPS surface serves the same text at the same path.
  When unset, nothing is written and the Build prints a note (not a warning). Applies to the **developer's application**; the product
  repository's own `SECURITY` file is not covered by the answer (open point O3).

## 2. Affected crates / files

- `specs/WASM-013-security-architecture/threat-model.md` — **new** (R1).
- `specs/WASM-013-security-architecture/release-review.md` — **new at RC only** (R16).
- `crates/cobolt-web-bundle/src/secrets.rs` — `PATTERNS`, `Key=value` connection strings, `connectionstring` property name,
  `scan_project`.
- `crates/cobolt-web-bundle/src/sbom.rs` — **new**: the CycloneDX writer moved out of `lib.rs:750-769`.
- `crates/cobolt-web-bundle/src/lib.rs` — use `sbom::cyclonedx`; `security.txt` from `BundleOptions.security` (new optional field).
- `crates/cobolt-web-bundle/tests/threat_model.rs`, `tests/csp_browser.rs` — **new** (§6).
- `crates/cobolt-project-tools/src/web/mod.rs` — `WebBuildRequest.profile`; reads `[web.unsafe]`, `[mobile.unsafe]`, `[security]`.
- `crates/cobolt-project-tools/src/web/build.rs` (WASM-002's new file) — project-input scan before generation; maps unsafe settings and
  profile onto `BundleOptions`.
- `crates/cobolt-project-tools/src/web/devserver.rs` — development-only `POST /__csp-report` sink; never in a Release bundle.
- `crates/cobolt-compiler/src/project_kind.rs` — `UnsafeSettings { plain_ws, skip_secret_scan, csp_report_only }` on `WebSettings` and
  `MobileSettings`; `SecuritySettings`; `PinningSettings` on `MobileSettings` (all `#[serde(default)]`, old manifests load unchanged).
- `crates/cobolt-compiler/src/backend_build.rs` — SBOM beside the backend binary.
- `crates/cobolt-compiler/src/lib.rs` — the manifest documentation text ("Project kinds: Desktop, Web and Mobile", `:6172`) gains the
  `[web.unsafe]`, `[mobile.unsafe]`, `[mobile.pinning]` and `[security]` tables (System KB).
- `crates/cobolt-compiler/tests/supply_chain.rs` — **new**: `cargo deny` sections and negative fixtures.
- `deny.toml` — **new**, root.
- `crates/cobolt-server/src/http.rs` — `cors_headers`; `security.txt` route.
- `crates/cobolt-server/src/serve.rs` — download route through `cors_headers`.
- `crates/cobolt-server/src/config.rs` — Release refuses literal secrets (G4).
- `crates/cobolt-server/src/dev.rs` — the dev origin instead of `unsafe_allow_any_origin`.
- `crates/cobolt-server/tests/fuzz_dispatch.rs`, `fuzz_tokens.rs`, `no_secret_in_logs.rs`, `tls_floor.rs` — **new**.
- `crates/cobolt-wire/tests/fuzz_frames.rs` — **new**; `random_round_trip.rs` gains the parser-on-bytes property.
- `crates/cobolt-web-player/tests/` — the AC4 text-not-markup test and the Viewer test (new file `xss.rs`); the redaction test on the
  `ERROR` path.
- `crates/cobolt-ide/src/i18n.rs` — `Tr` keys ×6: unsafe setting on (with its name), unsafe setting refused in a release Build, the
  secret-scan failure heading, the possible-secret warning heading, the pin set near expiry, `security.txt` not configured (note). Key
  names fixed at `/tasks`.
- `crates/cobolt-ide/src/…` (Output panel / project settings) — display of the warnings; files fixed at `/tasks` after reading the
  Build-output path WASM-002 links.
- Owed by other specs' files, listed so nothing falls between them: WASM-009 (`/auth/*` routes through `cors_headers`, refresh header),
  WASM-012 (capability-file test, mobile dependency guard, pinning output, mobile SBOM, `store_required`), WASM-016 (player
  `components`), WASM-001 (`.wcfrm`/`.mcfrm` in `is_text_path`).
- **Not touched:** `README.md`, anything under `docs/` (2.x.x rule). The analyze skill text is not changed.

## 3. Data / model changes

### 3.1 Manifest (`cobolt.toml`) — all optional, defaults keep today's behaviour

```toml
[web.unsafe]                 # every key says what it weakens; a release Build refuses any that is true
plain_ws         = false
skip_secret_scan = false
csp_report_only  = false

[mobile.pinning]             # optional (Q5)
pins    = ["sha256/…", "sha256/…"]   # at least two: the current key and a backup
expires = "2027-10-01"

[security]                   # optional (Q4); the developer's own contact
contact             = "mailto:security@example.com"
policy              = "https://example.com/security-policy"
expires             = "2027-10-01"
preferred_languages = "en, pt"
```

`[mobile.unsafe]` has the same keys as `[web.unsafe]`. Old manifests load unchanged (every field `#[serde(default)]`); no `.cfrm`,
generated-COBOL or serialized-AST change.

### 3.2 `backend.toml`

No new key. Behaviour change in Release: a literal value in a secret field is refused (G4) — a backend that started yesterday with a
literal secret and `RunMode::Release` will now stop with a message naming the key and saying "use env:NAME or file:PATH". Stated as a
deliberate, documented break; the Development run is unaffected.

### 3.3 Personal data the product handles (R13)

| Data | Where it comes from | Where it goes | Kept |
|---|---|---|---|
| User name / e-mail, password | Authentication control | backend → identity provider (WASM-009); the password never reaches the program module or a log | not by the product |
| Token subject (`sub`) | identity provider | backend log `subject` field | as long as the deployment keeps logs |
| Claims | token | `GetClaim` answers (ABI §12.1); never logged | in the page's memory only |
| Source IP address | connection | attempt limiter (`attempts_per_minute`) in memory | the limiter's window |
| Refresh token | identity provider | `__Host-pr_refresh` cookie (web), OS secure store (mobile) | the cookie's lifetime |
| reCAPTCHA token, browser signals | Google, when configured | backend → Google verification | Google's terms (WASM-030 licensing) |
| Map-tile requests (IP, viewed area) | the browser | the tile server named in the CSP | the tile server's policy |
| Uploaded files | the user | backend `files` directory (`cobolt-server/src/files.rs`) | the developer's retention |
| Application data | the developer's program | the developer's backend and storage | the developer's responsibility |

## 4. Key decisions & alternatives

- **Threat model as a spec file checked by a test.** — Why: R1 + AC1 need it current and traceable; a test makes a missing test visible.
  — Rejected: a model inside `docs/` (forbidden on 2.x.x); a model in code comments (no single reading for the reviewer, user story 2).
- **One secret scanner, two scopes (project inputs, bundle).** — Why: one pattern list (R7), Check and Build cannot disagree. —
  Rejected: a second scanner in the IDE; scanning only the bundle (copybooks never reach it).
- **Literal secrets refused only in Release.** — Why: R8/R11 — development must stay easy, production must not hold secrets in a
  committed file. — Rejected: refusing everywhere (breaks every quick local run); allowing everywhere (G4).
- **`cargo-deny` for advisories and licences, in-house guards for per-artefact allow-lists.** — Why: the RustSec database and its
  version-range semantics are maintained upstream; the per-artefact closures (feature-resolved) are already ours and finer than a
  workspace-wide ban list. — Rejected: an in-house advisory matcher by default (second implementation of a maintained tool; kept only as
  the fallback if the offline local-DB mode fails verification); `cargo-audit` (advisories only, a second tool for licences).
- **Fuzzing as deterministic property tests.** — Why: stable toolchain, C-free, reproducible, already the pattern in two crates. —
  Rejected: `cargo-fuzz`/libFuzzer (nightly, C++), AFL (C).
- **Pinning by the platform's declarative configuration.** — Why: the web view's own WebSocket is pinned with no native transport and no
  code path of ours in the TLS handshake. — Rejected: a native WebSocket plugin in the shell (a second transport, against WASM-005's
  single client in the player); pinning on the desktop client (Q5 is about the mobile shell).
- **CSP proof by violation reports to the dev server.** — Why: the browser itself reports what it blocked; no test JavaScript is
  tracked. — Rejected: asserting only the policy text (proves the text, not the enforcement AC2 asks for).
- **Development/Release as a request profile, Build = Release.** — Why: makes R11's refusals reachable from the IDE with no new
  command. — Rejected: a separate "release build" command (another place to forget). This is a working assumption (§7).

## 5. Risks & mitigations

- **`cargo deny` offline negative fixtures may not work as assumed.** → `[verify]` at `/tasks`; the fallback is named (in-repo matcher
  over a local RustSec checkout); the test never silently passes — skipped is printed and fails under `PR_SECURITY_GATE=1`.
- **A licence in today's lock outside the proposed allow-list.** → The list is fixed from `cargo deny list` at `/tasks`; a surprise is
  reported to the operator, not added silently.
- **`native-tls` cannot set ciphers; macOS/Windows clients may refuse to offer TLS 1.1 at all.** → The floor is stated as "1.2 + OS
  policy"; the test reports which side refused; the residual risk is a threat-model row.
- **Literal-secret refusal breaks an existing Release backend.** → Message names the key and the fix; Development unchanged; recorded in
  the CHANGELOG entry at `/implement`.
- **New scan shapes produce false errors on real projects** (`Password=` in a sample string). → Placeholder rules as for properties
  (`${…}`, empty, asterisks); the planted-secret tests and the "innocent lines" test grow together; the committed examples are scanned in
  a test and must stay clean — never by inspecting real values (credentials rule).
- **Headless Chrome not available in a run.** → Loud skip, gate failure under `PR_SECURITY_GATE=1`; Chrome 152 is installed on this
  machine (WASM-002 plan).
- **Pinning bricks an app when keys rotate.** → Two pins required, expiry required, warning 30 days ahead, off by default.
- **Owed rows never get closed.** → `threat_model.rs` prints the owed count; each owing spec's `/analyze` must lower it.
- **Changing the dev backend from any-origin to the dev origin breaks the Preview** (it may load from a different port). → The dev server
  passes its own origin to the dev backend at start; a loopback test covers Preview → dev backend.

## 6. Test strategy

Every new test prints what it asserted and its counts (files, patterns, iterations, reports). Sweeps use `--no-fail-fast` and every
`test result` line is read. `cobolt-ide` tests run with `--bin`.

| AC | Test | Asserts / reports |
|---|---|---|
| AC1 | `cobolt-web-bundle/tests/threat_model.rs` | every boundary has rows; every row has a control and an existing test or an `owed:` marker; secret pattern names equal `secrets::PATTERNS`; prints rows, boundaries, owed count |
| AC2 | `cobolt-web-bundle/tests/csp_browser.rs` (headless Chrome via the dev server) | three violation reports for inline script, `eval`, foreign `connect`; zero for the clean bundle with its form rendered |
| AC3 | `csp.rs` unit tests (exist) + a `BuildReport::text` test | `<meta>` lacks `frame-ancestors`; printed report lists header-only directives |
| AC4 | `cobolt-web-player/tests/xss.rs`; `sinks.rs` (exists) | payload rendered as text in Caption/item/cell/tooltip; Viewer drops `script`, `on*`, `javascript:`; zero sinks |
| AC5 | `cobolt-server/tests/loopback.rs` (origin, exists) + new CORS cases; WASM-009's route tests for the refresh cookie | foreign Origin refused; `"*"` refused; no `Allow-Credentials` under any-origin; cookie-only refresh refused cross-site |
| AC6 | `secrets.rs` unit tests + `cobolt-project-tools/tests/web_build.rs` | each pattern planted (from fragments, never a real value) in the bundle, a `.wcfrm`, a `.cpy` and a `.css` fails the Build naming file + pattern; printed text contains no planted value |
| AC7 | WASM-012's `store_required` test + `cobolt-server/tests/config_rules.rs` (literal secret in Release) | refusal, nothing written |
| AC8 | `cobolt-compiler/tests/supply_chain.rs`; existing guards | advisories, licences, bans, sources pass on the tree; synthetic advisory and removed licence each fail; `sbom.cdx.json` present for bundle and backend; SRI on every fetched file |
| AC9 | `cobolt-project-tools/tests/web_build.rs` + backend `config_rules.rs` (exists) + IDE i18n completeness test | each `unsafe` key → warning in Development, failure in Release naming the key; the six-language keys exist |
| AC10 | `cobolt-server/tests/no_secret_in_logs.rs` | zero hits of the planted password/token in backend lines, player console lines and the `ERROR` report |
| AC11 | `fuzz_frames.rs`, `fuzz_dispatch.rs`, `fuzz_tokens.rs`, `random_round_trip.rs` | no panic; named refusals; no forged token accepted; iterations and seconds printed, within budget |
| R4 | `cobolt-server/tests/tls_floor.rs` | TLS ≤ 1.1 refused; HTTPS responses carry HSTS, nosniff, Referrer-Policy, Permissions-Policy |
| Q4 | `web_build.rs` | `security.txt` written with `Contact` and `Expires` when configured, absent otherwise |
| Q5 | WASM-012's `mobile_build` tests | pin set < 2 or expired refused; the generated declarations hold the pins |
| R16 | `threat_model.rs` | an RC without `release-review.md` (or without a distinct reviewer) fails |

**Manual (through the PowerRustCOBOL MCP tools, never by driving the window):** open a Web project with `[web.unsafe] plain_ws = true`;
Check shows the warning in the Output; Build (Release) fails naming `plain_ws`. The operator judges the warning's look.

**Not measurable here:** Firefox/Safari CSP enforcement and Trusted Types coverage (Q2 decided: best effort), the iOS and Android
pinning on devices — listed in `WASM-PROGRESS.md` as owed, with the command, never claimed.

## 7. Steering compliance and open points

- [ ] **i18n:** the new IDE strings (§2) in all six languages; scan findings and backend messages stay English like compiler diagnostics.
- [x] **Generated-code contract:** CSP, headers, SBOM, `security.txt`, capability file and pinning declarations are generated at every
  Build, never hand-edited; generated COBOL and its banner untouched.
- [x] **`README.md` and `docs/` untouched**; what 2.x.x says about itself is in `specs/` (threat model, release review).
- [ ] **System KB:** the manifest documentation text in `cobolt-compiler/src/lib.rs` (§2) gains the new tables, and
  `assets/knowledge/chunked.data` is regenerated (`cargo run -p cobolt-ide --example build_chunked_kb`) in the same change; no control,
  property, method or event is added.
- [x] **Interpreter/binary parity:** no interpreter change; the backend binary and `rcrun backend run` share `ServerConfig::resolve`.
- [x] **PRIME DIRECTIVE:** no tracked non-Rust source; `deny.toml` is configuration; the CSP test's injected script is written by the
  test into a temporary directory.
- [x] **Credentials rule:** planted secrets are built from fragments in tests (the existing `secrets.rs` practice); no real value is
  read, printed or committed.
- [x] **User code is sacred:** the scan reads the developer's files and never rewrites them; `DISPLAY` output is not filtered.
- [x] **Fix vs feature:** **feature** on `features-2x`; each change bumps `z` (2.0.x) with a CHANGELOG entry.
- [x] No "cobolt" in user-facing text.

**Operator decisions applied:** Q1 → `release-review.md` with `kind = review | penetration-test` (§1.10); Q2 → Trusted Types in the CSP,
enforced where supported (as built); Q3 → known shapes error, entropy warning (as built, extended); Q4 → `[security]` project
parameters → `security.txt` (§1.10); Q5 → `[mobile.pinning]` option (§1.9).

**Open — not decided by this plan (working assumption in brackets):**

- **O1 — How a program marks a field sensitive** (R13). [A password-mode control and a `sensitive` contract field (WASM-014); `DISPLAY`
  unfiltered.]
- **O2 — Which commands are Release.** [IDE Build and `rcrun build` = Release; Run and Preview = Development.]
- **O3 — The product repository's own disclosure policy** (a `SECURITY` file for PowerRustCOBOL itself). Q4's answer covers the
  developer's applications; nothing is added to the repository root by this plan.
- **O4 — The licence allow-list** beyond the permissive set of §1.6 (for example MPL-2.0, already proposed, or any copyleft licence a
  dependency brings) — the operator's call when `/tasks` lists today's licences.
- **O5 — TLS cipher floor.** [TLS 1.2 + the OS's defaults, accepted residual risk; a stricter list means terminating TLS in front.]
- **Dependencies on decisions still open elsewhere:** D5 (WASM-INDEX: a small HTTPS surface, recommended) — R4's headers and §1.4's CORS
  rule assume it; WASM-017 Q5 (isolation in the mobile web views, not answered) — affects only R15b's mobile cells.
