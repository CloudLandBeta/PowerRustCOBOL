<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The Backend project and the secure frontend–backend channel

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-003-backend-project-and-secure-channel/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** operator item **3** (besides the frontend, a project may have a backend that talks to the frontend securely; authentication
  happens in the backend and the frontend sends a bearer token, a JWT, to the backend) and the first sentence of item **5** (the first
  backend is compiled and packaged as a **Desktop binary**; only the interface changes, to a WebSocket API).
- **Depends on:** WASM-001. **Needed by:** WASM-005 (the remote call), WASM-009 (the identity flows).

## 1. Overview

A browser or phone cannot, and must not, open the developer's data files, talk to their database, hold their API keys or run their server
logic. The **Backend** is the project that does: COBOL programs, copybooks, indexed-file definitions and SQL connections that run on the
**existing native RustCOBOL runtime**, packaged as one self-contained **desktop binary** (macOS, Windows, Linux) that listens for a
**WebSocket** connection. The frontend (WASM-002) calls the backend's *exported* programs (WASM-005); every call carries the user's
**JWT**, and the backend checks it before running anything.

This spec defines the Backend project, its packaging, the channel and its security. What a call looks like in COBOL is WASM-005; how a
user obtains the JWT is WASM-009.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **No backend or server concept exists.** A search for `axum`, `actix`, `jwt` and `tiny_http` in every `Cargo.toml` finds nothing. The only
  server code is `cobolt-project-tools/src/http.rs`, a hand-rolled MCP endpoint on `std::net` that requires a Bearer token
  (`:224-229`). "backend" in the compiler (`cobolt-compiler/src/lib.rs:1177`, `:5309`) means an SQL connection backend.
- **No WebSocket and no JWT code.** `tungstenite` / `tokio-tungstenite` are in `Cargo.lock` only through `rig-core` (used by the agents
  crate). A RestClient control sends a Bearer header (`AuthType` None | Bearer | Basic | APIKey, `cobolt-forms/src/connections.rs:78`), which
  is the client side only.
- **What the backend would run on is already there.** Indexed engines (`indexed_disk.rs` is the default; `redb` takes an exclusive file lock,
  so exactly one process may open a container, while PRCIDXD1 admits concurrent readers — CLAUDE.md), `STORAGE IS MEMORY | DISK`, SQL
  (`sql` feature; SQLite, PostgreSQL, MySQL; embedded `EXEC SQL`, spec 087), HTTP, files. **Cross-process record locking is a known gap**
  (it works only inside one run unit) and matters the moment two browsers use one backend.
- **Wire precedents** the project already has: bincode over stdio between the IDE and `rcrun run-form` (`FormIpcMessage`,
  `cobolt-runtime/src/channels.rs:144`) and JSON lines for remote debugging (`RemoteDebugCmd`, `debugger.rs:151`).

## 2. Goals / Non-goals

- **Goals:**
  - A **Backend** — the `backend/` part of every Web or Mobile project, not a project kind of its own (WASM-001) — with exported programs, built into a desktop binary.
  - A **WebSocket server** in that binary, over TLS, serving only the programs the project exported.
  - **Per-call authentication** with a bearer JWT validated against the configured identity provider; per-program authorisation by role.
  - A secure-by-default channel: origin check, size and rate limits, input validation, no secret ever leaves the backend.
  - A development loop: start, stop and watch the backend from the IDE.
- **Non-goals:**
  - Cloud deployment, containers, scaling and operations (proposed WASM-INDEX specs; v1 is a desktop binary).
  - Obtaining tokens and the identity-provider flows (WASM-009).
  - The COBOL syntax and the JSON mapping of a call (WASM-005).
  - A public REST API, GraphQL or server-push subscriptions (listed as proposed specs).
  - Multi-tenant administration.

## 3. User stories

- As a developer, I want my COBOL business programs and my data files to stay on my server, and the browser to call them by name, so that
  nothing sensitive reaches the client.
- As a developer, I want the backend to refuse any caller who has not signed in, so that I never write that check myself.
- As an operator, I want to run the backend as a single program on my own machine, so that I can start small without a cloud account.
- As a developer, I want to start the backend from the IDE and see its log, so that I can test a form end to end.

## 4. Requirements (EARS)

**The project and the build**
- **R1 (ubiquitous):** The Backend — the `backend/` part of a Web or Mobile project — shall hold COBOL programs, copybooks, indexed-file definitions and SQL connections, and a list of its
  **exported programs** — the only programs a remote caller can reach. A program not in the list shall be unreachable from the channel,
  whatever its name.
- **R2 (event):** When the developer builds the Backend, the IDE shall produce **one self-contained executable per host platform**
  (macOS, Windows, Linux) that includes the runtime, the programs and the server, extending the existing single-binary build. The server
  is built on Leptos (`cargo-leptos`, WASM-002 Q2) and carries the WebSocket channel and the HTTPS endpoints; it runs the developer's
  COBOL programs, and its Leptos plumbing is invisible to the developer, who writes and sees only COBOL.
- **R3 (ubiquitous):** The backend shall run its programs on **the current native runtime** — the tree-walking interpreter, with `std::fs`, threads and
  every indexed engine including `redb` — with the file, indexed, SQL and HTTP features available to them, plus whatever the new functions need (the WebSocket
  server, JWT validation, the group ⇄ JSON mapping of WASM-005). This is the operator's ruling of 2026-10-19: the backend keeps the runtime; only the browser is
  generated WebAssembly (WASM-010), so the backend is **not** compiled by the generator.
- **R4 (event):** When the backend is built, it shall also write an **interface description** of each exported program (name, parameter
  groups, the `RETURNING` group, required roles) that the frontend project imports (WASM-005 R7).

**The channel**
- **R5 (ubiquitous):** The channel shall be a **WebSocket over TLS** (`wss`). Plain `ws` shall be accepted only for a loopback address in a
  development run.
- **R6 (ubiquitous):** Each request shall carry an id, the exported program's name and the payload; each answer shall carry the same id and
  either the result or an error with a class (WASM-005 R10). Requests may be in flight together; answers may arrive in any order.
- **R7 (event):** When the connection drops, the caller shall learn of it (the call fails, WASM-005 R10); no call shall be lost silently and no
  call shall be run twice because of a reconnection unless the program is declared safe to repeat (Q4).
- **R8 (ubiquitous):** The server shall send periodic keep-alives and drop dead connections, and the client shall reconnect as R25 says (operator, 2026-10-24).

**Authentication and authorisation**
- **R9 (event):** When a request arrives, the backend shall require a valid **bearer JWT** before running any exported program except those the
  project declares **public** (a small, explicit set — the sign-in programs of WASM-009 and a health check).
- **R10 (ubiquitous):** The backend shall validate each token's signature against the issuer's published keys, accept only an allow-list of
  signing algorithms (never `none`), and check issuer, audience, expiry and not-before with a bounded clock skew; it shall distinguish an
  access token from an identity token and accept only the former.
- **R11 (ubiquitous):** The token shall never appear in a URL, query string or log line, and the transport used to deliver it over a WebSocket
  (a first message, a sub-protocol or another) is a `/plan` decision that shall respect this.
- **R12 (ubiquitous):** The validated claims (subject, roles or groups, tenant) shall be readable by the called COBOL program through a built-in
  in the inline `COBOL::"…"` form, read-only; an exported program may declare the role(s) required to call it, and the backend shall refuse
  other callers before the program starts.
- **R13 (event):** When a token expires while the connection is open, the next request shall be refused with an *authentication expired* error
  that the frontend answers by refreshing (WASM-009 R5) and repeating the call.

**Hardening**
- **R14 (ubiquitous):** The backend shall check the WebSocket **Origin** against a configured allow-list and refuse others (protection against
  cross-site WebSocket hijacking).
- **R15 (ubiquitous):** The backend shall validate every payload against the target program's declared layout — names, types, lengths, table
  bounds — before running it, and refuse a payload with an unknown field, a wrong type or an over-long value (it shall not truncate silently).
- **R16 (ubiquitous):** The backend shall enforce a maximum frame size, a per-call timeout, a per-connection and per-user rate limit and a
  concurrent-call limit; all configurable, all with safe defaults.
- **R17 (ubiquitous):** Every secret — database passwords, API keys, identity-provider client secrets, signing keys — shall live in the
  backend's configuration or the operating system's secret store, never in a form, a copybook that is shipped to the frontend or any frontend
  file (see WASM-002 R7).

**Concurrency and state**
- **R18 (ubiquitous):** Concurrent clients shall not see each other's working data: the runtime state of one call shall be isolated from another
  caller's (Q2 decides whether `WORKING-STORAGE` is fresh per call or kept per connection).
- **R19 (state):** While more than one client uses the same indexed or SQL data, the backend shall give them the data engines' locking
  semantics across **connections**, not only within one run unit (the cross-process locking gap above is a prerequisite).

**Operation**
- **R20 (event):** When the developer starts the backend from the IDE, the IDE shall run it, show its log in the Output panel and stop it on
  request; the same binary run outside the IDE shall behave the same.
- **R21 (ubiquitous):** The backend shall write structured logs (call id, program, caller subject, duration, outcome) that never contain a token,
  a password or a payload field the program marks sensitive, and shall answer a health probe.
- **R22 (event):** When a Web or Mobile project is created, a Backend scaffold with the authentication programs of WASM-009 shall be created
  with it (WASM-001 R6).
- **R23 (constraint):** The frontend shall connect only to the backend address(es) configured for it, and the build shall print the
  `connect-src` that implies (WASM-002 R14).

**The operator's ruling of 2026-10-24 (WASM-INDEX D21)**

- **R24 (ubiquitous):** The channel shall be **asymmetrically encrypted**, on top of the TLS of R5: a key pair shall be **generated when the application starts** — never written into the build or the bundle (R23, WASM-002 R7) — and the frontend shall encrypt what it sends to the public key the backend presents at connection time (WASM-014 R18 carries the exchange in the handshake), so that what travels is unreadable to anything that holds only the TLS endpoint, a terminating proxy included. The construction (an asymmetric handshake that protects a per-connection session key is the usual form) and the algorithms are the plan's, recorded in WASM-013 R17; the dependency rules of WASM-013 — pure Rust, no C-based cryptography — hold.
  - **Decided (operator, 2026-10-30):** **both sides** generate a fresh key pair at start — the backend when its process starts, each frontend when the application opens — and the handshake agrees a per-connection session key from the two (ephemeral–ephemeral key agreement), so a restart of either side yields new keys and no private key is ever stored. The construction is **X25519 key agreement, HKDF-SHA256 and ChaCha20-Poly1305** (RustCrypto crates `x25519-dalek`, `hkdf`, `sha2`, `chacha20poly1305`: pure Rust, build for WebAssembly). The plan records it in WASM-013 R17.
- **R25 (event):** When the connection breaks, the frontend shall **retry**: a first attempt with a timeout of **10 s**, a second with **20 s**, a third with **30 s** — the timeout raised by 10 s on each retry, for **at most 3 cycles**. When the third attempt fails, it shall **give up and raise an exception**: every call waiting on the connection fails with the class *transport* (WASM-014 R7), the application learns that the backend is unreachable (the player's event, WASM-016 R21), and no further attempt starts on its own. When an attempt succeeds, the connection is re-established and the calls that were in flight follow R7 and WASM-014 R9: repeated only when their program is declared safe to repeat, otherwise reported with *transport* and whether they may have been received.
- **R26 (state):** While the frontend retries, the application shall be **blocked**: all mouse and keyboard input ignored, and the "connecting" modal shown — WASM-016 R20 on the web, WASM-012 R15 on a device — from the moment the connection breaks until it is re-established or R25 gives up.

## 5. Acceptance criteria

- [ ] **AC1** — A backend with one exported and one non-exported program builds to a single executable; a WebSocket client can call the first and
  gets a *not found* error for the second, whatever the spelling it tries.
- [ ] **AC2** — A call without a token is refused; with a token signed by a different key, with `alg: none`, expired, for another audience or
  issuer, or an identity token instead of an access token, is refused; a good token is accepted (a local test issuer supplies the keys).
- [ ] **AC3** — A program declared for role `admin` refuses a caller without it before the program starts, and runs for one with it; the program
  can read the caller's subject through the built-in.
- [ ] **AC4** — A connection from a non-allowed Origin is refused; `ws://` to a non-loopback address is refused.
- [ ] **AC5** — Payloads with an unknown field, a numeric field holding text, a string longer than its `PIC`, or a table longer than `OCCURS` are
  refused with a validation error and the program does not run.
- [ ] **AC6** — Oversized frames, a call that exceeds the timeout and a client over the rate limit are each refused or cut off with a distinct error.
- [ ] **AC7** — Two clients calling the same program at once get their own results (a test with interleaved calls and distinct data).
- [ ] **AC8** — The log of a run containing a token and a password-marked field contains neither.
- [ ] **AC9** — The IDE starts and stops the backend and shows its log; the standalone binary behaves identically.
- [ ] **AC10** — A secret planted in a frontend form stops the frontend build (WASM-002 AC5) while the same value in the backend configuration does not.

- [ ] **AC11** — A capture of the channel between a frontend and its backend, taken at the TLS endpoint (a terminating proxy), holds no readable frame: every call and answer is ciphertext under the key pair generated at this start of the application, and a restart of the application yields a different key pair, so the capture does not decrypt the next session either.
- [ ] **AC12** — With the backend stopped, the frontend attempts to reconnect three times with timeouts of 10 s, 20 s and 30 s (measured), then raises the exception: the waiting call reaches `ON EXCEPTION` with the class *transport*, and no fourth attempt is made. With the backend back before the third timeout, the connection is re-established, the modal of WASM-016 AC13 disappears, a call whose program is declared safe to repeat completes, and another is reported with *transport*.

## 6. Constraints & steering check

- **WASM-INDEX D21 (operator, 2026-10-24):** this spec owns the channel's asymmetric encryption and the retry policy (R24–R26); the blocking modal is WASM-016 R20 and WASM-012 R15, the key exchange in the handshake WASM-014 R18, a call in flight WASM-005 R18, the threat model WASM-013 R17.
- **i18n:** yes — Output-panel and settings strings for the backend are IDE strings (six languages). The wire errors are codes plus English
  text; the frontend translates what it shows (WASM-005).
- **Generated-code contract:** the interface description (R4) and the generated authentication programs (R22) are generated artefacts, banner and
  regenerate-on-Build rules apply, never hand-edited.
- **Docs:** `docs/` untouched on 2.x.x; chapter planned in WASM-INDEX.
- **System KB:** the new built-in of R12 (and any other `COBOL::"…"` built-in this series adds) must be added to
  `cobolt-runtime/src/builtins.rs` `BUILTINS`, the compiler's documentation tables and the regenerated `assets/knowledge/chunked.data` in the
  same change.
- **Fix vs feature:** **feature**. The cross-process locking prerequisite (R19) is a **fix** by the 2026-09-14 ruling and belongs on 1.x.x and,
  by the forward-port rule, on 2.x.x.
- **Tech steering:** pure-Rust drivers and the dependency-guard tests (spec 063 AC8, amended by spec 103) apply to any crate added for
  TLS, WebSocket or JWT; keeping `aws-lc-rs` and `ring` out of the build was a settled constraint (the agents transport uses `native-tls`; spec 103 R10),
  so a TLS choice for the server has to meet it.

## 7. Open questions

- **Q1 — Where the backend is declared — decided** (operator, 2026-10-20): a `backend/` folder **inside** the Web or Mobile project, not a separate project and not a project kind — one repository, one Build and one set of copybooks shared with the frontend
  (WASM-001 Q8 for the folder names).
- **Q2 — Statefulness:** each call starts with fresh `WORKING-STORAGE` (a function), or a connection keeps one instance (a session)? Recommended:
  fresh per call, with explicit session data in a file or database; it is the model that scales and cannot leak between users.
  **Decided (operator, 2026-10-22):** a project property, defaulting to the recommendation (fresh `WORKING-STORAGE` per call).
- **Q3 — Alongside the WebSocket,** does the backend also need ordinary HTTPS endpoints (the refresh cookie of WASM-009 R5, health, an identity
  provider's redirect)? Probably yes; the spec allows them but the contract of this series is the WebSocket.
  **Decided (operator, 2026-10-22):** yes, the backend also serves ordinary HTTPS endpoints.
- **Q4 — Idempotency:** which programs are safe to repeat after a reconnection, and how is that declared? Recommended: none are by default.
  **Decided (operator, 2026-10-22):** the recommendation: no program is safe to repeat by default.
- **Q5 — A development token** (a locally signed JWT so a developer can test without an identity provider): allowed in a development run only,
  never in a release build? Needs the operator's ruling; it is the easiest way to ship an unauthenticated backend by mistake.
  **Decided (operator, 2026-10-22):** the development token is allowed only in a development run, never in a release build.
- **Q6 — Packaging per platform:** one binary per host (as spec 041 requires, host triple only), or cross-built binaries? Recommended: host only
  in 2.0.
  **Decided (operator, 2026-10-22):** one binary per host, as spec 041 requires.
- **Q7 — TLS:** terminated by the binary (certificate files) or by a reverse proxy in front of it? Recommended: both supported; the binary refuses a
  non-loopback `ws` either way.
  **Decided (operator, 2026-10-22):** support both: TLS terminated by the binary and behind a reverse proxy.
