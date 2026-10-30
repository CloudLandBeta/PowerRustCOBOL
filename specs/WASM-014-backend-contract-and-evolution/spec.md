<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — The backend contract and how it evolves

- **Status:** draft (needs `/clarify`)
- **Folder:** specs/WASM-014-backend-contract-and-evolution/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-19
- **Branch:** `features-2x` (version line 2.x.x). Part of the WASM series, see `specs/WASM-INDEX.md`.
- **Origin:** the first batch's list of missing specs (WASM-INDEX §5, "the backend contract"): versioning, compatibility handshake, errors, retry, streaming, files — accepted by the operator on 2026-10-19.
- **Depends on:** WASM-003 (the backend and the channel), WASM-005 (the call and the JSON mapping). **Related:** WASM-009 (tokens), WASM-012 (store-delivered clients that cannot be upgraded at once), WASM-013 (limits and hardening).

## 1. Overview

WASM-003 gives the frontend a channel and WASM-005 gives it a call. Neither says what happens **a year later**, when the backend has changed and some browsers still run last month's bundle, or a phone still runs a version the store has not updated.
A frontend and a backend are two programs released at different times; the **contract** between them is the exported programs, their group layouts, their errors and their rules for repeating, streaming and moving files. This spec defines that contract
as something with a version, a compatibility check at connection time, a stable error model and explicit behaviour for the cases the first specs left open.

### What exists today (checked 2026-10-19, `main` at 1.91.217)

- **There is no contract yet.** No backend, no exported programs, no interface description exist (WASM-003 R1, R4 define their first form).
- **What the project has to lean on:**
  - Layouts are COBOL groups shared through `COPY` copybooks (the `COPYBOOKS` folder of every project, the preprocessor in `cobolt-lexer/src/copybook.rs`), which is how a layout is shared in COBOL anyway.
  - Two precedents for versioned wire formats: the IDE ⇄ `rcrun run-form` bincode messages (`FormIpcMessage`, `cobolt-runtime/src/channels.rs:144`) and JSON lines for remote debugging (`RemoteDebugCmd`, `cobolt-runtime/src/debugger.rs:151`);
    and a manifest version (`CURRENT_STRUCTURE`, `project_upgrade.rs:51`) with an upgrade path.
  - **Indexed and SQL concurrency semantics** the backend inherits, including the known gap that record locks do not cross processes (WASM-003 R19).

## 2. Goals / Non-goals

- **Goals:**
  - A **contract version** on every backend and every frontend build, and a **handshake** that decides at connection time whether they can talk.
  - A rule for **compatible and incompatible changes** to an exported program's layout, enforced by tooling, not by memory.
  - One **error model** with classes the program can test and a developer can act on.
  - Defined behaviour for **retry and repeated calls**, **large and long-running calls**, **streams** the server starts, and **files**.
- **Non-goals:**
  - A second protocol (REST, GraphQL, gRPC): the WebSocket call remains the contract.
  - Transport and security hardening (WASM-003, WASM-013), the JSON mapping itself (WASM-005 R5, R6).
  - Multi-version servers (running two backend versions side by side) beyond what the compatibility rules allow.

## 3. User stories

- As a developer, I want to add a field to an exported program's group without breaking the applications already out, so that I can ship a backend change on its own.
- As a developer, I want to be told **at build time** that a change breaks old clients, so that I find out before a user does.
- As a user with an old application, I want to be told to update, rather than see a broken screen.
- As a developer, I want a failed call to say what kind of failure it was, so that I can decide whether to retry.

## 4. Requirements (EARS)

**Versions and handshake**
- **R1 (ubiquitous):** Every backend build shall carry a **contract version** (and a version of the wire protocol itself), and every frontend build the contract version it was compiled against, each recorded in the interface description (WASM-003 R4).
- **R2 (event):** When a connection opens, the two sides shall exchange their versions and the backend shall answer **accepted**, **accepted with a warning** (the client is older but compatible) or **refused** with the reason and, where it can,
  the version the client must update to; the frontend shall raise a defined event and, for a refusal, the application shall be able to tell the user to update (a store link on mobile).
- **R3 (ubiquitous):** The wire protocol's own version shall allow the backend to serve more than one protocol version at a time within a stated window, and a client older than the window shall be refused with an update message, never a protocol error.

**Compatibility**
- **R4 (ubiquitous):** The rules for **backward-compatible** changes to an exported program shall be written down and checked by the build: adding an optional trailing field, adding a new exported program, widening a table's `OCCURS`, or adding an
  error class are compatible; removing or renaming a field, changing a `PICTURE` or `USAGE` incompatibly, narrowing an `OCCURS`, changing a program's required roles or removing a program are not.
- **R5 (event):** When the backend is built, the build shall **compare the new interface description with the last released one** (kept in the project) and report each incompatible change with the program, the item and the rule, failing a release Build unless
  the developer raises the contract's major version deliberately.
- **R6 (ubiquitous):** A program may be marked **deprecated** with a message and a removal version; a call to it shall succeed and raise a warning event on the frontend.

**Errors**
- **R7 (ubiquitous):** Every failure of a call shall carry a **class** from a closed list — *transport* (no connection, closed, timeout), *authentication* (missing, invalid, expired), *authorisation* (role refused), *validation* (a payload that does not fit its layout), *not found* (no such
  exported program), *version* (R2), *rate or size limit*, *application* (the program itself reported a failure), *server* (an unexpected fault) — a stable numeric code, an English message for logs and a field the program can set for its own detail. Classes shall
  not be added in a way that breaks a client that does not know them: an unknown class is read as *server*.
- **R8 (ubiquitous):** The **application** class shall let a backend program report a business failure (a rejected order, a duplicate key) with a code and data the frontend reads in its `ON EXCEPTION` branch, distinct from the platform's own classes; the mechanism (a status group, a
  built-in the program calls) is for `/plan` (Q1).

**Repeating and ordering**
- **R9 (ubiquitous):** A program shall declare whether it is **safe to repeat** (idempotent) or not. After a lost connection the runtime shall repeat a call automatically **only** when the program is declared safe, and otherwise report the failure with the class *transport* and whether the call **may have been received** (Q2).
- **R10 (ubiquitous):** A non-idempotent call may carry an **idempotency key** the runtime generates, so that a repeat the developer chooses to make can be recognised and answered from the stored result, for a stated time, rather than run twice.
- **R11 (ubiquitous):** Calls on one connection shall run in the order they were made unless the program is declared independent; the frontend shall be able to have several calls in flight (WASM-003 R6) without the program order changing.

**Size, duration and streams**
- **R12 (ubiquitous):** Payloads above a configured size shall be refused with the *size limit* class; a program that legitimately returns more shall use a **chunked** result the runtime reassembles invisibly (R13) or a **file transfer** (R15).
- **R13 (optional):** Where a call is declared **long-running**, the backend shall be able to send **progress** messages and a final answer; the frontend shall raise a progress event the form can show, and the user or the program shall be able to **cancel** it, which the backend honours at its next safe point.
- **R14 (optional):** Where a program is declared a **stream** (the backend sends a series of results over time: a feed, a counter, a log), the frontend shall receive them as events, subscribe and unsubscribe explicitly, and be re-subscribed after a reconnection; an unsubscribed or disconnected client shall cost the server nothing (Q3).

**Files**
- **R15 (ubiquitous):** File upload and download shall be a part of the contract: transferred **in pieces** with a declared size, a type allow-list and a checksum, resumable after a lost connection, subject to the size and rate limits, with a progress event; the backend shall refuse a path or name chosen by
  the browser (WASM-005 R15) and shall store and name files itself; a download address shall be issued per request, short-lived and tied to the caller (Q4).

**Evolution of the contract itself**
- **R16 (ubiquitous):** The contract's own rules (this spec) shall carry a version, so that a change to the handshake, the error list or the file protocol is a protocol-version change with the window of R3.
- **R17 (constraint):** A Desktop project is unaffected.

**The operator's ruling of 2026-10-24 (WASM-INDEX D21)**

- **R18 (event):** When a connection opens, the handshake of R2 shall also carry the **key exchange** of WASM-003 R24: the public key of the key pair the backend generated when the application started, and what the frontend needs to encrypt to it. The key exchange shall be part of the contract's own version (R16), so that a change to it is a contract change; a connection that cannot complete the exchange shall be refused with the class *transport*.
  - **Decided (operator, 2026-10-30):** both sides generate their key pair at start, so the handshake carries **two** X25519 public keys — the backend's in its hello, the frontend's in its reply — and both derive the session key (HKDF-SHA256, ChaCha20-Poly1305); see WASM-003 R24 and WASM-013 R17.

## 5. Acceptance criteria

- [ ] **AC1** — A frontend built against contract 1.0 connecting to a backend at 1.2 is accepted; to a backend whose major version differs is refused with an update message; one older than the protocol window is refused with an update message, not a decoding error.
- [ ] **AC2** — Each compatible change of R4 builds and an old frontend still works against it (a test runs the old client against the new backend); each incompatible change fails the release Build naming program, item and rule.
- [ ] **AC3** — A deprecated program works and the frontend raises the warning event.
- [ ] **AC4** — Each failure class is produced by a test (a dropped connection, a bad token, a refused role, a bad payload, an unknown program, a version refusal, an oversized frame, an error raised by the program, a crash) and the program reads the class and code; an unknown class from a newer backend is read as *server*.
- [ ] **AC5** — A business failure raised in a backend program arrives in `ON EXCEPTION` with its code and data.
- [ ] **AC6** — A dropped connection during a call to a program declared safe is repeated once and succeeds; during one not declared safe it fails and says whether the call may have been received; a repeat with the idempotency key returns the stored result without running twice.
- [ ] **AC7** — Interleaved calls on one connection keep their order for dependent programs and may overlap for independent ones.
- [ ] **AC8** — A long-running call reports progress and cancels on request; a stream delivers events, survives a reconnection and costs nothing after unsubscribing.
- [ ] **AC9** — A file larger than a single frame uploads in pieces, resumes after a forced disconnection, and arrives with a matching checksum; a name with `../` is refused; an expired download address is refused.
- [ ] **AC10** — All the above run against a backend and a frontend in a test, in both directions of version skew the windows allow.

- [ ] **AC11** — The handshake records the key exchange; a frontend built against a contract version without it is refused with the reason; a captured handshake from one start of the application does not decrypt a session of the next start.

## 6. Constraints & steering check

- **WASM-INDEX D21 (operator, 2026-10-24):** this spec owns the key exchange of the asymmetric channel as part of the handshake and the contract's version (R18); the encryption itself is WASM-003 R24, the threat model WASM-013 R17.
- **i18n:** only IDE strings (the build's compatibility report), six languages; wire messages are codes plus English text.
- **Generated-code contract:** the interface description, the compatibility report and the generated clients' handling of versions and errors are generated artefacts, never hand-edited.
- **System KB:** the error classes, the declarations (safe to repeat, long-running, stream, deprecated) and the events are language and control behaviour: update the compiler's documentation tables and `chunked.data`.
- **COBOL written for this** follows the extensions ruling and the call-comment ruling.
- **Fix vs feature:** **feature** (`features-2x`).

## 7. Open questions

- **Q1 — How a program reports an *application* failure:** a status group in the `RETURNING` item by convention, a built-in (`COBOL::"…"`) the program calls, or both? Recommended: a built-in, so the platform can tell a deliberate failure from a fault.
  **Decided (operator, 2026-10-22):** both; agents (Codex) use only the built-in.
- **Q2 — "May have been received."** The runtime cannot know whether a dropped call ran. State the guarantee plainly (at-most-once by default, at-least-once for declared-safe programs) and decide whether the application class can ask the backend afterwards.
  **Answered (operator, 2026-10-22), to confirm:** at-least-once. *Whether for every call or only for programs declared safe to repeat is to confirm.*
- **Q3 — Streams** need server-side state per subscriber, which sits badly with WASM-003 Q2's stateless calls. Is server push in 2.0 at all, or a later spec (WASM-INDEX lists it as a gap)?
  **Decided (operator, 2026-10-22):** streams are in 2.0.
- **Q4 — Where a downloaded file is fetched from:** over the WebSocket in pieces (one channel, no second origin), or through a short-lived HTTPS address (cacheable, streamable by the browser, needs the HTTPS surface of WASM-003 Q3)? Recommended: HTTPS address for downloads, pieces for uploads.
  **Decided (operator, 2026-10-22):** HTTPS for downloads.
- **Q5 — Retention** of idempotency results and of the previous interface description (where they are kept, for how long) — decide with the operations spec.
  **Decided (operator, 2026-10-22):** per session; nothing stays in the frontend; kept in memory or on the server, configurable in the project properties.
