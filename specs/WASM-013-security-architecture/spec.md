<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Security architecture and threat model for the WebAssembly line

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-013-security-architecture/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the first batch's list of missing specs (WASM-INDEX §5, "security architecture"); the operator, 2026-10-19, on its scope: *yes — all of them* (threat model, CSP, CORS, XSS, CSRF, supply chain).
- **Depends on:** WASM-002, WASM-003, WASM-005, WASM-009, WASM-010, WASM-012. **Owns the rules those specs cite:** WASM-002 R7, R14; WASM-003 R14–R17; WASM-005 R14, R17; WASM-006 R7–R8; WASM-007 R13; WASM-008 R8.

## 1. Overview

Until now the product's applications ran on the developer's own machine. A Web or Mobile application is reachable by anyone, and it carries data, accounts and a server. The other specs each state the rules for their own corner (a token
is never in a URL, a stylesheet cannot load a remote resource, a browser cannot send SQL). This spec owns **the whole**: the threat model, the trust boundaries, the policies the generated host page ships, the rules for the supply
chain, the "unsafe" switches, and the checks that prove them — so that no corner is forgotten and no rule exists twice with two meanings.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **No web-security code exists.** The workspace has no Content-Security-Policy, CORS or HTML-sanitising code of its own, and no WebSocket, JWT or identity code (WASM-003, WASM-009).
- **Secret handling today.** The IDE stores secrets in the operating system's store with a plaintext fallback by design (`cobolt-ide/src/secrets/mod.rs`: *"if security doesn't matter to you here, you get plaintext"*); that is right for a
  desktop IDE and wrong for a backend or a signing key (R6). The forms have properties where a key can be typed (for example `AgentAPIKey`, `ConnectionString`) and nothing in a form stops a developer from typing one; the project's
  working notes record live keys having been found in demo forms before, which is why a credential scan gates commits. The committed examples checked on 2026-10-19 hold the key properties empty
  (three `ConnectionString` properties are not — their content was not inspected).
- **Dependency discipline exists.** `cobolt-compiler/tests/dependency_guards_063.rs` guards which packages may enter the build (spec 063 AC8, amended by spec 103); keeping `aws-lc-rs` and `ring` out of the build is a settled rule.
- **The one sandbox:** the agents crate runs validation commands under a time limit (`cobolt-agents/src/sandbox.rs`); there is no sandbox for user code beyond the operating system's.
- **The surfaces that interpret text as markup** are few: the Viewer's HTML subset and its CSS cascade (`cobolt-forms/src/viewer.rs`, `css.rs`), which the web version will also have to render.

## 2. Goals / Non-goals

- **Goals:**
  - A written **threat model** with named trust boundaries and the control for each threat, kept current as the series is built.
  - A **strict Content-Security-Policy** and a set of response headers the build generates and tests.
  - Rules for **XSS**, **CSRF**, **CORS/Origin**, tokens and secrets, **supply chain** and logging, each testable.
  - **Secure defaults**, with every relaxation explicit, named and refused in a release build.
  - Automated checks in the test suite: dependency audit, secret scan, policy tests, sink search, fuzzing of the parsers that face the network.
- **Non-goals:**
  - Operating-system hardening, anti-tampering or root/jailbreak detection on mobile.
  - A web application firewall or a hosted security service.
  - Identity-provider internals (WASM-009 owns the flows; this spec owns how their output is protected).
  - Compliance certification (the licensing and compliance spec proposed in WASM-INDEX covers data-protection law).

## 3. User stories

- As a developer, I want the default build to be safe, so that I do not need to be a security specialist to avoid the common mistakes.
- As a security reviewer, I want one document listing what can attack what and what stops it, so that I can review the product once.
- As an operator, I want a release build that refuses an unsafe setting, so that a debugging shortcut cannot reach production.

## 4. Requirements (EARS)

**The model**
- **R1 (ubiquitous):** A **threat model** shall be part of the series, naming the assets (accounts, tokens, application data, secrets, the developer's source), the actors (an anonymous user, a signed-in user, a malicious page that embeds
  or opens the application, a malicious backend client, a malicious dependency, a malicious project file opened in the IDE) and the **trust boundaries** — browser ⇄ backend, host page ⇄ module, module ⇄ player, IDE ⇄ preview web view, mobile
  shell ⇄ frontend, backend ⇄ identity provider and data — with, for each threat, the control that answers it and the test that proves it. It shall be reviewed at each phase's `/analyze`.

**Policies the build generates**
- **R2 (ubiquitous):** The generated host page shall ship a **Content-Security-Policy** of default-deny: scripts from the application's own origin plus `'wasm-unsafe-eval'` and no `'unsafe-eval'`, no `'unsafe-inline'` for scripts (a nonce or
  hash where an inline script is unavoidable), styles from the origin and the hashes of generated ones, images and fonts from the origin, **`connect-src` limited to the configured backend addresses**, `base-uri` and `form-action`
  restricted, and `frame-ancestors` set by the embedding mode (WASM-002 R11). Where the browser supports it, Trusted Types shall be enforced.
- **R3 (ubiquitous):** Because a static host may not let the application set headers, the Build shall deliver the policy both as a `<meta>` element (what a `<meta>` can express) and as a ready-to-use header set for the common hosts (what it
  cannot: `frame-ancestors`, report-only, HSTS), and shall print which is which.
- **R4 (ubiquitous):** The backend shall send the security headers its HTTPS surface needs (`Strict-Transport-Security`, `X-Content-Type-Options`, a restrictive `Referrer-Policy`, a `Permissions-Policy`), and its TLS settings shall refuse
  versions and ciphers below a stated floor.

**XSS**
- **R5 (ubiquitous):** Every value that reaches the page — a caption, an item, a cell, a tooltip, a message — shall be inserted as **text**, never parsed as markup, by construction; the player shall contain **no API that inserts an HTML string** (no `innerHTML`, `outerHTML`, `insertAdjacentHTML`, `document.write` or the like) and no parsing of markup from data into the live page,
  and a test shall search its source for those sinks. The only control that renders markup, the **Viewer's HTML subset**, shall do it by **building DOM nodes** (`createElement`, `setAttribute`, `textContent`) from a tree that has been parsed and filtered through an **allow-list** of elements, attributes and URL schemes —
  never by handing a string to the browser's HTML parser — so the rule above holds without an exception. WASM-006 R7 and WASM-007 R13 shall be enforced where URLs and stylesheets enter.

**CSRF, CORS and origin**
- **R6 (ubiquitous):** State-changing backend calls shall be authorised by a **bearer token the page adds**, never by a cookie alone; a request that relies on a cookie (the refresh of WASM-009 R5) shall use `SameSite`, a CSRF token
  or a custom header the browser will not add cross-site. The WebSocket **Origin** shall be checked (WASM-003 R14); the HTTPS surface shall use an explicit origin allow-list, **never a wildcard together with credentials**.

**Secrets**
- **R7 (ubiquitous):** The **secret scan** that gates a web or mobile Build (WASM-002 R7) shall belong to the product, ship with a pattern list this spec owns (cloud access keys and tokens, private-key blocks, JSON Web Tokens, connection
  strings, API-key shapes, high-entropy strings above a configurable length), run over the bundle, the forms, the copybooks and the stylesheets, and fail with the file and the pattern, never printing the value.
- **R8 (ubiquitous):** A backend or a signing key shall never fall back to plaintext storage: where the operating system's store is unavailable the product shall stop and say so (WASM-012 R4), unlike the IDE's own best-effort store.

**Supply chain**
- **R9 (ubiquitous):** Every crate in the frontend, the player, the backend and the shell shall come through an **allow-list** guarded by the dependency-guard tests (extended from spec 063 AC8 / 103), with the lock file committed, a
  vulnerability audit and a licence check in the test run, and **no build script that fetches from the network** at the developer's build except what is declared. The host page's JavaScript shall be hand-written and small (WASM-002 Q6), so
  there is no package-manager dependency tree in a Web project's build.
- **R10 (ubiquitous):** Each Build shall write a **software bill of materials** (a standard format) of what it contains, and the host page's assets shall carry integrity hashes.

**Secure by default**
- **R11 (ubiquitous):** Every relaxation — a development token (WASM-003 Q5), plain `ws://` to a non-loopback address, a wildcard origin, a weakened CSP, skipping the secret scan — shall be an explicit setting whose name says **unsafe**, shall show a
  warning in the IDE, and shall make a **release Build fail**.
- **R12 (ubiquitous):** The backend started for development shall bind to loopback unless told otherwise, and the preview web view shall load only its own development origin (WASM-008 R8).

**Logging and privacy**
- **R13 (ubiquitous):** Logs, crash reports and telemetry of the backend, the frontend and the IDE shall never contain a token, a password, a secret or a field the program marks sensitive; the plan shall list the personal data the product handles
  and where it goes.

**Mobile**
- **R14 (ubiquitous):** The shell's capability list shall give each window the least native access it needs, shall give no remote origin any, and shall be generated from the declared device features (WASM-012 R7, R12).

**Assurance**
- **R15 (ubiquitous):** The test suite shall include: the dependency and licence audit; the secret scan against planted secrets; a test that the generated CSP and headers equal the expected text and that a page under them cannot run an inline
  script or `eval`; the XSS sink search; **fuzzing** of the WebSocket frame handling, the JSON-to-group mapping (WASM-005) and the token validation (WASM-003 R10), each with a stated run budget in the continuous run; and the tests the other
  specs name for their corners.
- **R15b (constraint):** The application shall run **without cross-origin isolation** (no `COOP`/`COEP` requirement), **without threads or shared memory** (the operator's ruling of 2026-10-19, WASM-017 R19), with a program-module data memory that never grows and a fixed transport arena (WASM-017 R7–R8): the page stays embeddable in someone else's layout
  (WASM-002 R11), a static host needs no special headers, and a hostile payload cannot make a module's memory grow without bound. The loader shall release a destroyed form's module completely, so that opening and closing forms cannot exhaust the page's memory (WASM-017 R5).
- **R16 (event):** When the series reaches a release candidate, a **review by someone other than the author** — and, if the operator decides (Q1), an external penetration test — shall be recorded before the line is released.

## 5. Acceptance criteria

- [ ] **AC1** — The threat model exists, names every boundary of R1, and each row points to a control and a test; the `/analyze` of each later spec updates it.
- [ ] **AC2** — A built bundle served under its generated CSP loads and runs, and the browser reports a violation for an injected inline script, an `eval` and a connection to an address outside `connect-src` (a headless test).
- [ ] **AC3** — The `<meta>` and the header set are generated, and the Build prints which directives are only deliverable as headers.
- [ ] **AC4** — A caption, item, cell and tooltip containing `<img src=x onerror=…>` render as text; a source search finds no forbidden sink in the player (the Viewer included); the Viewer's HTML with a script element, an inline event handler and a `javascript:` link renders without them, built from nodes.
- [ ] **AC5** — A cross-site page cannot make the backend run a call using only the refresh cookie; a connection from a foreign Origin is refused; no wildcard origin is accepted together with credentials.
- [ ] **AC6** — The secret scan finds each planted secret kind in the bundle, a form, a copybook and a stylesheet, fails the Build with file and pattern, and prints no value.
- [ ] **AC7** — On a machine with no operating-system secret store, a signing or backend secret is refused rather than written to disk.
- [ ] **AC8** — A dependency outside the allow-list, a crate with a known vulnerability and a disallowed licence each fail the test run; a Build writes an SBOM and integrity hashes.
- [ ] **AC9** — Each unsafe switch of R11 shows a warning in the IDE and fails a release Build.
- [ ] **AC10** — A full sign-in and call flow leaves no token, password or marked field in the backend log, the browser console or a crash report (a test greps all three).
- [ ] **AC11** — The fuzz targets of R15 exist and run within their budget without a crash.

## 6. Constraints & steering check

- **i18n:** yes — the IDE's warnings for unsafe switches and scan failures, six languages.
- **Credentials rule:** the scan's own tests plant fake secrets in fixtures only; nothing real is committed, and no real value is printed (the operator's no-credentials rule and the commit-gate practice).
- **Generated-code contract:** the CSP, headers, SBOM and capability list are generated artefacts, never hand-edited.
- **Tech steering:** the dependency rules above sit on top of MSRV 1.92 and the C-free TLS constraint already settled (spec 103 R10).
- **Docs:** `docs/` untouched on 2.x.x; the guide's security chapter is planned in WASM-INDEX.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — External penetration test.** Required before 2.0, or only a review by a second reader? A budget and a vendor are the operator's call.
- **Q2 — Trusted Types** are enforced only where supported and need the player's code to comply; accept the partial coverage, or require it everywhere (not possible today)?
- **Q3 — Secret-scan strictness.** A high-entropy rule produces false positives (hashes, UUIDs); recommended: on by default as a **warning** for entropy, an **error** for the known shapes.
- **Q4 — A security contact and disclosure policy** (a `SECURITY` file, a mailbox) for the published project: needed before 2.0, and who owns it?
- **Q5 — Certificate pinning** for the mobile shell's backend connection: offer as an option, require, or leave to the developer?
